#!/usr/bin/env node
// 以 JS oracle 產生 fixtures/golden/ 下的預期輸出。
//
// 用法：
//   node scripts/gen-golden.mjs           重新產生全部 golden 檔
//   node scripts/gen-golden.mjs --check   只比對，golden 過期、缺少或多餘時以結束碼 1 失敗
//   node scripts/gen-golden.mjs --quiet   不印統計
//   node scripts/gen-golden.mjs --no-cli  不連帶執行 scripts/gen-cli-golden.mjs
//
// 預設在最後連帶執行 scripts/gen-cli-golden.mjs（--check 時也帶 --check），因為 CLI 的
// golden 會記錄 fixtures/corpus 的指紋，語料一改就要一起重新產生。
//
// 這個模組也匯出 OPTION_SETS、runCase、runCases、readMarkdownFile 等，
// 讓 scripts/parity.sh 與 scripts/fuzz-parity.mjs 共用同一組選項定義。
// 詳細說明見 docs/parity.md 與 docs/testing.md。
'use strict';

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { RULES, VERSION, diagnose, fixDocument } from '../reference/js/markdown-fixer.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = path.resolve(here, '..');
export const CORPUS_DIR = path.join(REPO_ROOT, 'fixtures', 'corpus');
export const GOLDEN_DIR = path.join(REPO_ROOT, 'fixtures', 'golden');
export const REFERENCE_VERSION = VERSION;

// 選項組合。順序與名稱是 golden 格式的一部分，Rust 測試依 name 對應。
export const OPTION_SETS = [
  { name: 'default', options: {} },
  { name: 'snippet', options: { ignoreRules: ['MDX003'] } },
  { name: 'line-length-80', options: { maxLineLength: 80 } },
  { name: 'keep-encoding', options: { ignoreRules: ['MDX001', 'MDX002'] } },
  { name: 'headings-only', options: { rules: ['MDX010', 'MDX011', 'MDX012', 'MDX013'] } }
];

const BLOCKING_RULES = ['MDX081', 'MDX083', 'MDX053', 'MDX085'];

// 與 oracle CLI 的 readMarkdownFile 相同：拒絕 NUL、嚴格 UTF-8、保留 BOM。
export function readMarkdownFile(file) {
  const buffer = fs.readFileSync(file);
  if (buffer.includes(0)) {
    throw new Error('file contains NUL bytes; refusing to treat it as text');
  }
  const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
  return decoder.decode(buffer);
}

// 固定鍵順序的選項物件（rules、ignoreRules、maxLineLength）。
export function canonicalOptions(options = {}) {
  const result = {};
  if (options.rules !== undefined) result.rules = [...options.rules];
  if (options.ignoreRules !== undefined) result.ignoreRules = [...options.ignoreRules];
  if (options.maxLineLength !== undefined) result.maxLineLength = options.maxLineLength;
  return result;
}

function canonicalProblem(problem) {
  return {
    rule: problem.rule,
    name: problem.name,
    severity: problem.severity,
    fixable: problem.fixable,
    line: problem.line,
    column: problem.column,
    message: problem.message
  };
}

function canonicalFix(result) {
  return {
    text: result.text,
    changed: result.changed,
    refused: result.refused,
    reason: result.reason === undefined ? '' : result.reason
  };
}

// 以 oracle 跑一組選項，回傳 { diagnose, fix }。
export function runCase(text, options = {}) {
  return {
    diagnose: diagnose(text, options).map(canonicalProblem),
    fix: canonicalFix(fixDocument(text, options))
  };
}

// 以 oracle 跑全部選項組合。
export function runCases(text) {
  return OPTION_SETS.map((set) => ({
    name: set.name,
    options: canonicalOptions(set.options),
    ...runCase(text, set.options)
  }));
}

// 列出語料目錄下的 Markdown 檔（相對路徑、以 / 分隔、依 JS 預設排序）。
export function listCorpus(root = CORPUS_DIR) {
  const files = [];
  const visit = function (dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        visit(full);
      } else if (entry.isFile() && /\.(?:md|markdown)$/i.test(entry.name)) {
        files.push(path.relative(root, full).split(path.sep).join('/'));
      }
    }
  };
  visit(root);
  return files.sort();
}

// golden 檔的 JSON 序列化：2 空格縮排、結尾換行。不可見字元改寫成 \uXXXX，
// 讓 git diff 看得出差異；JSON 語意不變。
const INVISIBLE_RE = /[\u00A0\u00AD\u1680\u180E\u2000-\u200F\u2028-\u202F\u205F-\u2064\u3000\uFEFF]/g;
export function stringifyGolden(value) {
  return JSON.stringify(value, null, 2).replace(
    INVISIBLE_RE,
    (ch) => `\\u${ch.charCodeAt(0).toString(16).toUpperCase().padStart(4, '0')}`
  ) + '\n';
}

export function goldenFor(rel) {
  const text = readMarkdownFile(path.join(CORPUS_DIR, rel));
  return {
    source: rel,
    referenceVersion: REFERENCE_VERSION,
    cases: runCases(text)
  };
}

export function rulesGolden() {
  return {
    referenceVersion: REFERENCE_VERSION,
    rules: RULES.map((rule) => ({
      id: rule.id,
      name: rule.name,
      category: rule.category,
      severity: rule.severity,
      fixable: rule.fixable,
      summary: rule.summary
    }))
  };
}

// 語料自我檢查：rules/<ID>/trigger.md 必須觸發該規則，negative.md 不可觸發；
// edge/blocking-*.md 與 edge/frontmatter-trap-*.md 必須被拒絕修正。
function checkCoverage(goldens) {
  const errors = [];
  const triggered = new Set();
  const ruleIds = new Set(RULES.map((rule) => rule.id));
  const caseOf = (golden, name) => golden.cases.find((c) => c.name === name);

  for (const golden of goldens) {
    for (const c of golden.cases) {
      for (const problem of c.diagnose) triggered.add(problem.rule);
    }
    const match = golden.source.match(/^rules\/(MDX\d{3})\/(trigger|negative)[^/]*\.md$/);
    if (match) {
      const [, id, kind] = match;
      if (!ruleIds.has(id)) {
        errors.push(`${golden.source}: unknown rule directory ${id}`);
        continue;
      }
      const c = caseOf(golden, id === 'MDX090' ? 'line-length-80' : 'default');
      const has = c.diagnose.some((problem) => problem.rule === id);
      if (kind === 'trigger' && !has) errors.push(`${golden.source}: expected ${id} but it was not reported`);
      if (kind === 'negative' && has) errors.push(`${golden.source}: ${id} must not be reported`);
      if (BLOCKING_RULES.includes(id) && kind === 'trigger' && !c.fix.refused) {
        errors.push(`${golden.source}: blocking rule ${id} must refuse the fix`);
      }
    }
    if (/^edge\/(?:blocking|frontmatter-trap)-/.test(golden.source)) {
      if (!caseOf(golden, 'default').fix.refused) {
        errors.push(`${golden.source}: expected fixDocument to refuse`);
      }
    }
  }

  for (const id of ruleIds) {
    for (const kind of ['trigger', 'negative']) {
      if (!goldens.some((g) => g.source.startsWith(`rules/${id}/${kind}`))) {
        errors.push(`missing fixtures/corpus/rules/${id}/${kind}.md`);
      }
    }
    if (!triggered.has(id)) errors.push(`rule ${id} is never reported anywhere in the corpus`);
  }
  return errors;
}

function walkGoldenFiles(dir, base = dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkGoldenFiles(full, base, out);
    else if (entry.isFile() && entry.name.endsWith('.json')) {
      out.push(path.relative(base, full).split(path.sep).join('/'));
    }
  }
  return out;
}

function main(argv) {
  const check = argv.includes('--check');
  const quiet = argv.includes('--quiet');
  const withCli = !argv.includes('--no-cli');
  const unknown = argv.filter((arg) => !['--check', '--quiet', '--no-cli'].includes(arg));
  if (unknown.length > 0) {
    console.error(`Unknown argument: ${unknown[0]}`);
    console.error('Usage: node scripts/gen-golden.mjs [--check] [--quiet] [--no-cli]');
    return 2;
  }

  const sources = listCorpus();
  const expected = new Map();
  const goldens = [];
  let totalLines = 0;
  for (const rel of sources) {
    let golden;
    try {
      golden = goldenFor(rel);
    } catch (error) {
      console.error(`Error: fixtures/corpus/${rel}: ${error.message}`);
      return 2;
    }
    goldens.push(golden);
    const text = readMarkdownFile(path.join(CORPUS_DIR, rel));
    totalLines += text.split(/\r\n|\r|\n/).length - (/(?:\r\n|\r|\n)$/.test(text) ? 1 : 0);
    expected.set(rel.replace(/\.(?:md|markdown)$/i, '.json'), stringifyGolden(golden));
  }
  expected.set('rules.json', stringifyGolden(rulesGolden()));

  const coverageErrors = checkCoverage(goldens);

  let stale = 0;
  if (check) {
    const existing = new Set(walkGoldenFiles(GOLDEN_DIR));
    for (const [rel, contents] of expected) {
      const file = path.join(GOLDEN_DIR, rel);
      if (!existing.has(rel)) {
        console.error(`missing: fixtures/golden/${rel}`);
        stale++;
      } else if (fs.readFileSync(file, 'utf8') !== contents) {
        console.error(`stale:   fixtures/golden/${rel}`);
        stale++;
      }
      existing.delete(rel);
    }
    for (const rel of existing) {
      console.error(`orphan:  fixtures/golden/${rel}`);
      stale++;
    }
  } else {
    const existing = new Set(walkGoldenFiles(GOLDEN_DIR));
    for (const [rel, contents] of expected) {
      const file = path.join(GOLDEN_DIR, rel);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      if (!fs.existsSync(file) || fs.readFileSync(file, 'utf8') !== contents) {
        fs.writeFileSync(file, contents);
      }
      existing.delete(rel);
    }
    for (const rel of existing) fs.rmSync(path.join(GOLDEN_DIR, rel));
  }

  if (!quiet) {
    const perRule = new Map(RULES.map((rule) => [rule.id, 0]));
    for (const golden of goldens) {
      for (const problem of golden.cases[0].diagnose) perRule.set(problem.rule, perRule.get(problem.rule) + 1);
    }
    const refused = goldens.filter((g) => g.cases[0].fix.refused).length;
    const changed = goldens.filter((g) => g.cases[0].fix.changed).length;
    console.log(`corpus: ${sources.length} file(s), ${totalLines} line(s); default fix: ${changed} changed, ${refused} refused`);
    console.log(`option sets: ${OPTION_SETS.map((s) => s.name).join(', ')}`);
    console.log(`rules reported (default option set, count): ${[...perRule].map(([id, n]) => `${id}=${n}`).join(' ')}`);
  }

  for (const error of coverageErrors) console.error(`coverage: ${error}`);
  if (check && stale > 0) {
    console.error(`${stale} golden file(s) are out of date. Run: node scripts/gen-golden.mjs`);
  }
  if (coverageErrors.length > 0 || stale > 0) return 1;
  if (!quiet) console.log(check ? 'golden files are up to date.' : `wrote ${expected.size} golden file(s) to fixtures/golden/.`);
  return withCli ? runCliGolden(check) : 0;
}

// CLI 的 golden 由 agent「cli」的 scripts/gen-cli-golden.mjs 產生；存在時連帶執行。
function runCliGolden(check) {
  const script = path.join(REPO_ROOT, 'scripts', 'gen-cli-golden.mjs');
  if (!fs.existsSync(script)) return 0;
  const result = spawnSync(process.execPath, [script, ...(check ? ['--check'] : [])], {
    cwd: REPO_ROOT,
    stdio: 'inherit'
  });
  if (result.error) {
    console.error(`Error: cannot run scripts/gen-cli-golden.mjs: ${result.error.message}`);
    return 2;
  }
  return result.status === 0 ? 0 : 1;
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) {
  process.exitCode = main(process.argv.slice(2));
}
