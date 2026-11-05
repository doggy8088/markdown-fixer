#!/usr/bin/env node
// 依 crates/markdown-fixer-cli/tests/cli-golden/manifest.json，在暫存目錄裡執行
// oracle CLI（reference/js/markdown-fixer.mjs），把 stdout、stderr、結束碼與執行後的
// 檔案內容寫成 crates/markdown-fixer-cli/tests/cli-golden/cases/<name>.json。
//
// Golden 檔自帶輸入內容（不再引用 fixture），Rust 端的
// tests/cli_golden.rs 只讀 golden 檔重播，不需要 Node。例外是 `copyDir`：
// 整個目錄（例如 fixtures/corpus）太大，不內嵌，改在重播時從 repo 複製，
// golden 只記錄指紋（fingerprint）；目錄內容改變時 Rust 測試會要求重新產生。
// copyDir 底下的檔案在執行後只記錄 FNV-1a 雜湊，不記錄內容。
//
// 用法：node scripts/gen-cli-golden.mjs [--check] [案例名稱...]
//   --check  不寫檔，只比對現有 golden 是否與 oracle 一致（給 CI 或升版時用）。

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const oracle = path.join(repo, 'reference/js/markdown-fixer.mjs');
const cliDir = path.join(repo, 'crates/markdown-fixer-cli/tests');
const fixturesDir = path.join(cliDir, 'fixtures');
const goldenDir = path.join(cliDir, 'cli-golden');
const casesDir = path.join(goldenDir, 'cases');
const manifest = JSON.parse(fs.readFileSync(path.join(goldenDir, 'manifest.json'), 'utf8'));

const argv = process.argv.slice(2);
const checkOnly = argv.includes('--check');
const only = new Set(argv.filter((arg) => arg !== '--check'));

const ORACLE_VERSION = (() => {
  const result = spawnSync(process.execPath, [oracle, '--version'], { encoding: 'utf8' });
  const match = /^markdown-fixer (\S+)\n$/.exec(result.stdout);
  if (!match) throw new Error(`Cannot read oracle version: ${result.stdout}`);
  return match[1];
})();

function expandSetup(setup = []) {
  const entries = [];
  for (const item of setup) {
    if (typeof item === 'string' && item.startsWith('@')) {
      const preset = manifest.presets[item.slice(1)];
      if (!preset) throw new Error(`Unknown preset ${item}`);
      entries.push(...preset);
    } else {
      entries.push(item);
    }
  }
  // 把 fixture 換成內嵌內容，讓 golden 檔自給自足。
  return entries.map((entry) => {
    const out = { path: entry.path };
    if (entry.copyDir !== undefined) {
      out.copyDir = entry.copyDir;
      out.fingerprint = fingerprintDir(path.join(repo, entry.copyDir));
    } else if (entry.dir) out.dir = true;
    else if (entry.symlink !== undefined) out.symlink = entry.symlink;
    else {
      const bytes = entry.fixture !== undefined
        ? fs.readFileSync(path.join(fixturesDir, entry.fixture))
        : entry.hex !== undefined
          ? Buffer.from(entry.hex, 'hex')
          : Buffer.from(entry.text, 'utf8');
      Object.assign(out, encodeBytes(bytes));
    }
    if (entry.mode) out.mode = entry.mode;
    return out;
  });
}

// FNV-1a 64：Rust 端以同樣的演算法計算，不需要額外相依。
function fnv1a(bytes, hash = 0xcbf29ce484222325n) {
  let h = hash;
  for (const byte of bytes) {
    h ^= BigInt(byte);
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return h;
}

function hex64(value) {
  return value.toString(16).padStart(16, '0');
}

function fileHash(bytes) {
  return hex64(fnv1a(bytes));
}

// 目錄指紋：每個檔案以「相對路徑 + NUL + 內容」計算 FNV-1a，再相加（mod 2^64），與順序無關。
function fingerprintDir(dir) {
  let sum = 0n;
  const walk = (current, rel) => {
    for (const name of fs.readdirSync(current)) {
      const full = path.join(current, name);
      const relPath = rel ? `${rel}/${name}` : name;
      const stat = fs.lstatSync(full);
      if (stat.isDirectory()) walk(full, relPath);
      else if (stat.isFile()) {
        const h = fnv1a(fs.readFileSync(full), fnv1a(Buffer.from(`${relPath}\0`, 'utf8')));
        sum = (sum + h) & 0xffffffffffffffffn;
      }
    }
  };
  walk(dir, '');
  return hex64(sum);
}

function encodeBytes(bytes) {
  try {
    const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
    return { text };
  } catch {
    return { hex: bytes.toString('hex') };
  }
}

function applySetup(root, entries) {
  for (const entry of entries) {
    const target = path.join(root, entry.path);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    if (entry.copyDir !== undefined) fs.cpSync(path.join(repo, entry.copyDir), target, { recursive: true });
    else if (entry.dir) fs.mkdirSync(target, { recursive: true });
    else if (entry.symlink !== undefined) fs.symlinkSync(entry.symlink, target);
    else fs.writeFileSync(target, entry.text !== undefined ? entry.text : Buffer.from(entry.hex, 'hex'));
  }
  // 權限最後才設，避免 000 的目錄擋住後續建立。
  for (const entry of entries) {
    if (entry.mode) fs.chmodSync(path.join(root, entry.path), parseInt(entry.mode, 8));
  }
}

function collectFiles(root, entries) {
  const copied = entries.filter((entry) => entry.copyDir !== undefined).map((entry) => `${entry.path}/`);
  const modes = new Map();
  for (const entry of entries) {
    if (!entry.mode) continue;
    const target = path.join(root, entry.path);
    modes.set(entry.path, (fs.statSync(target).mode & 0o777).toString(8).padStart(3, '0'));
    fs.chmodSync(target, entry.dir ? 0o755 : 0o644);
  }
  const files = [];
  const walk = (dir, rel) => {
    const names = fs.readdirSync(dir).sort();
    for (const name of names) {
      const full = path.join(dir, name);
      const relPath = rel ? `${rel}/${name}` : name;
      const stat = fs.lstatSync(full);
      if (stat.isDirectory()) walk(full, relPath);
      else if (stat.isFile()) {
        const bytes = fs.readFileSync(full);
        const record = copied.some((prefix) => relPath.startsWith(prefix))
          ? { path: relPath, fnv: fileHash(bytes) }
          : { path: relPath, ...encodeBytes(bytes) };
        if (modes.has(relPath)) record.mode = modes.get(relPath);
        files.push(record);
      }
    }
  };
  walk(root, '');
  return files;
}

function normalizeOutput(text, roots) {
  let out = text;
  for (const root of roots) out = out.split(root).join('<TMP>');
  out = out.split(`markdown-fixer ${ORACLE_VERSION}\n`).join('markdown-fixer <VERSION>\n');
  out = out.split(`"version": "${ORACLE_VERSION}"`).join('"version": "<VERSION>"');
  // PRD〈5.4〉第 3 項：--help 的 Usage 與 Examples 行改用執行檔名稱。
  out = out.split('Usage: node markdown-fixer.mjs [options]').join('Usage: markdown-fixer [options]');
  out = out.split('\n  node markdown-fixer.mjs ').join('\n  markdown-fixer ');
  return out;
}

function runCase(testCase) {
  const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), 'mdfix-cli-golden-'));
  const root = fs.realpathSync(tmpBase);
  try {
    const setup = expandSetup(testCase.setup);
    applySetup(root, setup);
    const cwd = path.join(root, testCase.cwd || '');
    fs.mkdirSync(cwd, { recursive: true });
    const result = spawnSync(process.execPath, [oracle, ...testCase.args], { cwd, encoding: 'utf8' });
    if (result.error) throw result.error;
    const roots = [root, tmpBase];
    const golden = {
      name: testCase.name,
      args: testCase.args,
      cwd: testCase.cwd || '',
      nonRoot: Boolean(testCase.nonRoot),
      setup,
      expected: {
        status: result.status,
        stdout: normalizeOutput(result.stdout, roots),
        stderr: normalizeOutput(result.stderr, roots),
        files: collectFiles(root, setup)
      }
    };
    return golden;
  } finally {
    fs.rmSync(tmpBase, { recursive: true, force: true });
  }
}

if (process.getuid && process.getuid() === 0) {
  console.error('Refusing to generate CLI golden files as root: permission cases would be wrong.');
  process.exit(2);
}

fs.mkdirSync(casesDir, { recursive: true });
const names = new Set();
let mismatches = 0;
let written = 0;
for (const testCase of manifest.cases) {
  if (names.has(testCase.name)) throw new Error(`Duplicate case name ${testCase.name}`);
  names.add(testCase.name);
  if (only.size > 0 && !only.has(testCase.name)) continue;
  const golden = runCase(testCase);
  const file = path.join(casesDir, `${testCase.name}.json`);
  const contents = `${JSON.stringify(golden, null, 2)}\n`;
  if (checkOnly) {
    const existing = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
    if (existing !== contents) {
      console.error(`MISMATCH ${testCase.name}`);
      mismatches++;
    }
  } else {
    fs.writeFileSync(file, contents);
    written++;
  }
}

if (only.size === 0 && !checkOnly) {
  for (const name of fs.readdirSync(casesDir)) {
    if (name.endsWith('.json') && !names.has(name.slice(0, -'.json'.length))) {
      fs.unlinkSync(path.join(casesDir, name));
      console.log(`removed stale ${name}`);
    }
  }
}

if (checkOnly) {
  console.log(mismatches === 0 ? 'CLI golden files are up to date.' : `${mismatches} CLI golden file(s) differ.`);
  process.exit(mismatches === 0 ? 0 : 1);
}
console.log(`Wrote ${written} CLI golden file(s) to ${path.relative(process.cwd(), casesDir) || casesDir}.`);
