#!/usr/bin/env bash
# 即時比對 JS oracle 與 Rust：對每個檔案、每組選項跑 diagnose 與 fixDocument，
# 列出每個檔案的第一個差異。有任何差異時以結束碼 1 結束，用法錯誤為 2。
#
# 用法：
#   scripts/parity.sh <file-or-dir>...
#   PARITY_OPTIONS=default scripts/parity.sh README.md   # 只跑預設選項
#
# 目錄的展開方式與 oracle CLI 相同（.md／.markdown，跳過 .git、node_modules 等）。
# 選項組合定義在 scripts/gen-golden.mjs 的 OPTION_SETS。Rust 端使用
# `--example dump --features serde`，target 目錄為 CARGO_TARGET_DIR（預設 target/parity），
# 或以 MDFIX_DUMP 指定已建好的執行檔。說明見 docs/parity.md。
set -euo pipefail

if [[ $# -eq 0 || "$1" == "-h" || "$1" == "--help" ]]; then
  sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
  [[ $# -eq 0 ]] && exit 2 || exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v node >/dev/null 2>&1 || { echo "Error: node is required" >&2; exit 2; }

exec node --input-type=module - "$repo_root" "$@" <<'JS'
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [repoRoot, ...inputs] = process.argv.slice(2);
const load = (rel) => import(pathToFileURL(path.join(repoRoot, rel)).href);
const { OPTION_SETS, canonicalOptions, readMarkdownFile, runCase } = await load('scripts/gen-golden.mjs');
const { RustRunner, buildDump, compareResults } = await load('scripts/fuzz-parity.mjs');

const EXCLUDES = new Set(['.git', '.hg', '.svn', 'node_modules', 'vendor', 'coverage', 'test-results', 'playwright-report', 'blob-report', '.e2e-tmp']);
const files = [];
let usageError = false;
const visit = (target) => {
  let stat;
  try {
    stat = fs.statSync(target);
  } catch (error) {
    console.error(`Cannot access ${target}: ${error.message}`);
    usageError = true;
    return;
  }
  if (stat.isFile()) return void files.push(path.resolve(target));
  if (!stat.isDirectory()) return;
  for (const entry of fs.readdirSync(target, { withFileTypes: true })) {
    if (EXCLUDES.has(entry.name)) continue;
    const full = path.join(target, entry.name);
    if (entry.isDirectory()) visit(full);
    else if (entry.isFile() && /\.(?:md|markdown)$/i.test(entry.name)) files.push(path.resolve(full));
  }
};
inputs.forEach(visit);
const unique = [...new Set(files)].sort();

const only = process.env.PARITY_OPTIONS || 'all';
const sets = only === 'all' ? OPTION_SETS : OPTION_SETS.filter((s) => s.name === only);
if (sets.length === 0) {
  console.error(`Unknown PARITY_OPTIONS=${only}; use all or one of: ${OPTION_SETS.map((s) => s.name).join(', ')}`);
  process.exit(2);
}

const runner = new RustRunner(buildDump());
const display = (file) => path.relative(process.cwd(), file) || file;
let diverged = 0;
let unreadable = 0;
let compared = 0;
const cases = [];
for (const file of unique) {
  let text;
  try {
    text = readMarkdownFile(file);
  } catch (error) {
    console.log(`SKIP ${display(file)}: ${error.message}`);
    unreadable++;
    continue;
  }
  for (const set of sets) cases.push({ file, set: set.name, text, options: canonicalOptions(set.options) });
}

const reported = new Set();
for (let start = 0; start < cases.length; start += 200) {
  const batch = cases.slice(start, start + 200);
  const actual = runner.runBatch(batch);
  batch.forEach((c, k) => {
    compared++;
    if (reported.has(c.file)) return;
    const difference = compareResults(runCase(c.text, c.options), actual[k]);
    if (!difference) return;
    reported.add(c.file);
    diverged++;
    const opts = JSON.stringify(c.options);
    console.log(`DIFF ${display(c.file)} [${c.set}] ${opts}`);
    console.log(difference.split('\n').map((line) => `  ${line}`).join('\n'));
    console.log(`  repro: cargo run -q -p markdown-fixer --example dump --features serde -- '${opts}' < ${display(c.file)}`);
  });
}

console.log(`parity: ${unique.length} file(s), ${compared} case(s), ${diverged} file(s) diverged${unreadable ? `, ${unreadable} unreadable` : ''}`);
process.exit(usageError ? 2 : diverged > 0 ? 1 : 0);
JS
