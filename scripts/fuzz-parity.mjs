#!/usr/bin/env node
// 差異模糊測試：以 Markdown 片段文法產生隨機文件，批次比對 JS oracle 與 Rust。
//
// 用法：
//   node scripts/fuzz-parity.mjs [--count N] [--seed S] [--batch B] [--options all|default]
//                                [--no-save] [--max-failures K] [--idempotence] [--dump-docs DIR]
//
//   --count N        產生 N 份文件（預設 500）
//   --seed S         亂數種子（預設 1）；同一個種子永遠產生同一批文件
//   --batch B        每批送給 Rust 的案例數（預設 200）
//   --options        all：每份文件跑 gen-golden.mjs 的全部選項組合；default：只跑 {}（預設 all）
//   --no-save        找到差異時不寫入 fixtures/corpus/regressions/
//   --max-failures K 收集到 K 個差異就停止（預設 5）
//   --idempotence    只跑 oracle，統計 fixDocument 不冪等的文件（不需要 Rust）
//   --dump-docs DIR  把產生的文件寫到 DIR（除錯文法用）
//
// Rust 端透過 `cargo build -p markdown-fixer --example dump --features serde` 建置，
// target 目錄取自 CARGO_TARGET_DIR（預設 target/parity）；也可以用 MDFIX_DUMP 指定執行檔。
// 這個模組也匯出 compareResults、RustRunner 等，給 scripts/parity.sh 使用。
'use strict';

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { CORPUS_DIR, OPTION_SETS, REPO_ROOT, canonicalOptions, runCase } from './gen-golden.mjs';
import { fixDocument } from '../reference/js/markdown-fixer.mjs';

// ---------------------------------------------------------------- Rust 端

export function buildDump() {
  if (process.env.MDFIX_DUMP) return process.env.MDFIX_DUMP;
  const targetDir = process.env.CARGO_TARGET_DIR || path.join(REPO_ROOT, 'target', 'parity');
  const result = spawnSync('cargo', ['build', '-q', '-p', 'markdown-fixer', '--example', 'dump', '--features', 'serde'], {
    cwd: REPO_ROOT,
    env: { ...process.env, CARGO_TARGET_DIR: targetDir },
    stdio: ['ignore', 'inherit', 'inherit']
  });
  if (result.status !== 0) throw new Error('cargo build of the dump example failed');
  const exe = path.join(path.resolve(REPO_ROOT, targetDir), 'debug', 'examples', process.platform === 'win32' ? 'dump.exe' : 'dump');
  if (!fs.existsSync(exe)) throw new Error(`dump executable not found at ${exe}`);
  return exe;
}

export class RustRunner {
  constructor(exe, { timeoutMs = 60000 } = {}) {
    this.exe = exe;
    this.timeoutMs = timeoutMs;
  }

  // cases: [{ text, options }] → 每個案例回傳 { diagnose, fix } 或 { panic } / { error } / { timeout }
  runBatch(cases) {
    if (cases.length === 0) return [];
    const input = cases.map((c) => JSON.stringify({ text: c.text, options: c.options })).join('\n') + '\n';
    const result = spawnSync(this.exe, ['--batch'], {
      input,
      encoding: 'utf8',
      maxBuffer: 1 << 30,
      timeout: this.timeoutMs * Math.max(1, Math.ceil(cases.length / 50))
    });
    const lines = (result.stdout || '').split('\n').filter((line) => line !== '');
    if (result.error || result.status !== 0 || lines.length !== cases.length) {
      // 整批失敗（逾時或程序異常）時逐一重跑，找出是哪一份文件。
      if (cases.length === 1) {
        return [{ timeout: true, error: String(result.error || `exit ${result.status}`) }];
      }
      return cases.flatMap((c) => this.runBatch([c]));
    }
    return lines.map((line) => JSON.parse(line));
  }

  run(text, options) {
    return this.runBatch([{ text, options }])[0];
  }
}

// ---------------------------------------------------------------- 比對

const PROBLEM_KEYS = ['rule', 'name', 'severity', 'fixable', 'line', 'column', 'message'];
const FIX_KEYS = ['text', 'changed', 'refused', 'reason'];

function formatProblem(p) {
  if (!p) return '(none)';
  return `${p.rule} ${p.line}:${p.column} ${p.severity}${p.fixable ? ' fixable' : ''} ${JSON.stringify(p.message)}`;
}

function firstDifferentLine(a, b) {
  const la = a.split('\n');
  const lb = b.split('\n');
  for (let i = 0; i < Math.max(la.length, lb.length); i++) {
    if (la[i] !== lb[i]) return { line: i + 1, expected: la[i], actual: lb[i] };
  }
  return null;
}

// 回傳 null 表示一致，否則回傳一段描述第一個差異的文字。
export function compareResults(expected, actual) {
  if (!actual || actual.panic !== undefined) return `Rust panicked: ${actual ? actual.panic : '(no output)'}`;
  if (actual.timeout) return `Rust timed out or crashed: ${actual.error}`;
  if (actual.error !== undefined) return `dump error: ${actual.error}`;
  const ed = expected.diagnose;
  const ad = actual.diagnose || [];
  for (let i = 0; i < Math.max(ed.length, ad.length); i++) {
    const e = ed[i];
    const a = ad[i];
    if (!e || !a || PROBLEM_KEYS.some((key) => e[key] !== a[key])) {
      return [
        `diagnose differs at problem #${i} (oracle ${ed.length} problem(s), Rust ${ad.length})`,
        `  oracle: ${formatProblem(e)}`,
        `  rust:   ${formatProblem(a)}`
      ].join('\n');
    }
  }
  const ef = expected.fix;
  const af = actual.fix || {};
  for (const key of FIX_KEYS) {
    if (ef[key] === af[key]) continue;
    if (key === 'text' && typeof af.text === 'string') {
      const diff = firstDifferentLine(ef.text, af.text);
      return [
        `fix.text differs at line ${diff.line}`,
        `  oracle: ${JSON.stringify(diff.expected)}`,
        `  rust:   ${JSON.stringify(diff.actual)}`
      ].join('\n');
    }
    return `fix.${key} differs: oracle ${JSON.stringify(ef[key])}, Rust ${JSON.stringify(af[key])}`;
  }
  return null;
}

// ---------------------------------------------------------------- 文法

export function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6D2B79F5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function makeGenerator(rand) {
  const int = (n) => Math.floor(rand() * n);
  const pick = (items) => items[int(items.length)];
  const chance = (p) => rand() < p;
  const repeat = (n, fn) => Array.from({ length: n }, fn);

  const WORDS = ['安裝', '設定', '說明', '範例', '注意', 'Rust', 'Markdown', 'API', 'token', '結論', '測試', 'docs', 'v1.2', '使用者', 'のテキスト', '한국어', 'café', 'naïve'];
  const ASTRAL = ['😀', '🎉', '𠮷', '𝒳', '👨‍👩‍👧', '🇹🇼'];
  const PUNCT = ['，', '。', '：', '；', '！', '？', '、', '（', '）', '「', '」', '…', '.', ',', ':', '!', '?', '(', ')'];
  const ODD = [' ', '​', '﻿', '　', '\t', ' ', ' ', '  ', '\\', '|', '#', '>', '-', '+', '`', '~', '$', '<', '&'];
  const LABELS = ['注意', '結論', '審查執行', 'Note', '負責人', '狀態', 'Spec 來源'];
  const URLS = ['https://example.com', 'https://example.com/a**b**', 'http://例子.測試/路徑', 'www.example.org/x_y_z', 'https://example.com/a|b', 'mailto:me@example.com'];

  const word = () => (chance(0.08) ? pick(ASTRAL) : pick(WORDS));
  const label = () => pick(LABELS);

  const inline = function (depth = 0) {
    const atoms = [
      () => word(),
      () => word() + ' ' + word(),
      () => pick(PUNCT),
      () => pick(ODD),
      () => `\`${word()}\``,
      () => `\`\`a\`b\`\``,
      () => `$${word()}*x*$`,
      () => `$$${word()}$$`,
      () => `$${int(100)}`,
      () => `**${word()}**`,
      () => `** ${word()} **`,
      () => `**${word()} **`,
      () => `*${word()}*`,
      () => `* ${word()}*`,
      () => `_${word()} _`,
      () => `__ ${word()} __`,
      () => `~~${word()}~~`,
      () => `~~${word()}`,
      () => `**${label()}：**${word()}`,
      () => `**${label()}**：${word()}`,
      () => `**「${word()}」**`,
      () => `*${label()}：*${word()}`,
      () => `***${word()}：***`,
      () => `[${word()}](${pick(URLS)})`,
      () => `[${word()}] (${pick(URLS)})`,
      () => `[${word()}][${pick(['ref', 'Ref', 'missing', ''])}]`,
      () => `![${pick(['', ' ', word()])}](img.png)`,
      () => `[](${pick(URLS)})`,
      () => `[${word()}]()`,
      () => pick(URLS),
      () => `<${pick(URLS)}>`,
      () => `<span>${word()}</span>`,
      () => `＃${word()}`,
      () => `［${word()}］（${pick(URLS)}）`,
      () => '\\*' + word() + '\\*',
      () => (depth < 2 ? `**${inline(depth + 1)}**` : word())
    ];
    return repeat(1 + int(6), () => pick(atoms)()).join(pick(['', ' ', ' ', ' ']));
  };

  const trailing = () => pick(['', '', '', '', ' ', '  ', '   ', '\t', ' \t ', ' ']);

  const blocks = {
    heading: () => {
      const hashes = '#'.repeat(1 + int(7));
      const indent = pick(['', '', '', ' ', '   ', '    ']);
      const gap = pick([' ', ' ', '', '  ', '\t', '　']);
      const close = pick(['', '', '', ' #', ' ###', '#', ' # ']);
      const text = chance(0.1) ? '' : inline();
      return [`${indent}${hashes}${gap}${text}${close}${trailing()}`];
    },
    paragraph: () => repeat(1 + int(3), () => inline() + trailing()),
    labels: () => repeat(1 + int(4), () => `**${label()}：**${inline()}`),
    list: () => {
      const marker = () => pick(['-', '-', '*', '+', '1.', '2)', '10.']);
      return repeat(1 + int(5), () => {
        const indent = pick(['', '', '', '  ', '    ', '   ']);
        const space = pick([' ', ' ', ' ', '', '  ', '\t']);
        const task = chance(0.15) ? pick(['[ ] ', '[x] ', '[X] ']) : '';
        const body = chance(0.08) ? '' : task + inline();
        return `${indent}${marker()}${space}${body}${trailing()}`;
      });
    },
    quote: () => repeat(1 + int(4), () => {
      const depth = 1 + int(3);
      const marker = repeat(depth, () => pick(['>', '> ', '>  '])).join('');
      return `${pick(['', ' '])}${marker}${pick([inline(), '', '    code', '- ' + word(), '```', '$$', '#' + word()])}`;
    }),
    fence: () => {
      const ch = pick(['`', '`', '~']);
      const len = 3 + int(2);
      const info = pick(['', 'js', 'rust', ' sh ', 'md title="x"', 'a`b']);
      const indent = pick(['', '', '  ', '   ']);
      const body = repeat(int(4), () => pick([inline(), '#' + word(), '** x **', '', '\t' + word(), '  ']));
      const lines = [`${indent}${ch.repeat(len)}${info}`, ...body];
      if (!chance(0.07)) lines.push(`${indent}${ch.repeat(len + int(2))}`);
      return lines;
    },
    quotedFence: () => ['> ```' + pick(['', 'js']), '> #' + word(), '> ** x **', pick(['> ```', '```', '>'])],
    table: () => {
      const cols = 1 + int(4);
      const row = (n) => '| ' + repeat(n, () => pick([word(), '`a|b`', '`a\\|b`', pick(URLS), inline()])).join(' | ') + ' |';
      const delim = '|' + repeat(cols + (chance(0.1) ? 1 : 0), () => pick([' --- ', ' - ', ':--', '--:', ' :-: ', '---'])).join('|') + '|';
      return [row(cols), delim, ...repeat(int(4), () => row(cols + (chance(0.15) ? pick([-1, 1]) : 0)))];
    },
    math: () => {
      const lines = ['$$', ...repeat(int(3), () => pick(['x = 1', '#' + word(), '** y **', 'a\tb', '', ' z'])) ];
      if (!chance(0.07)) lines.push('$$');
      return lines;
    },
    inlineMath: () => [`$$ x^2 $$ ${inline()}`],
    html: () => pick([
      ['<div>', '#' + word(), '** x **', '</div>'],
      ['<details>', '<summary>' + word() + '</summary>', '', '#' + word(), '', '</details>'],
      ['<!-- ' + word() + ' -->'],
      chance(0.3) ? ['<!--', word()] : ['<!--', word(), '-->'],
      ['<script>', 'const a = "#x";', '', '</script>'],
      ['<custom-tag>', word()],
      ['<?php echo 1; ?>']
    ]),
    hr: () => [pick(['---', '***', '___', '- - -', '* * *', '===', '--', '  ---'])],
    definition: () => [`${pick(['', '> ', '   '])}[${pick(['ref', 'Ref', 'other', word()])}]: ${pick(URLS)}`],
    indentedCode: () => repeat(1 + int(3), () => '    ' + pick([inline(), '#x', '** y **'])),
    blank: () => repeat(1 + int(3), () => pick(['', '', ' ', '\t'])),
    jsx: () => [pick(['import X from "y";', '<Tabs>', '</Tabs>', 'export const a = 1;', ':::tip', ':::', '{/* c */}'])],
    fullwidth: () => [pick(['＃ ' + word(), '－ ' + word(), '＊ ' + word(), '＋　' + word()])]
  };
  const weights = [
    ['paragraph', 8], ['heading', 5], ['labels', 2], ['list', 5], ['quote', 3], ['fence', 3], ['quotedFence', 1],
    ['table', 3], ['math', 1], ['inlineMath', 1], ['html', 2], ['hr', 2], ['definition', 2], ['indentedCode', 1],
    ['blank', 4], ['jsx', 1], ['fullwidth', 1]
  ];
  const total = weights.reduce((sum, [, w]) => sum + w, 0);
  const pickBlock = function () {
    let r = rand() * total;
    for (const [name, w] of weights) {
      r -= w;
      if (r < 0) return blocks[name]();
    }
    return blocks.paragraph();
  };

  return function generate() {
    const lines = [];
    if (chance(0.15)) lines.push(...repeat(1 + int(2), () => ''));
    if (chance(0.15)) {
      lines.push('---', ...repeat(int(3), () => pick(['title: x', 'tags: [a]', '#x', ''])));
      if (!chance(0.15)) lines.push(pick(['---', '...']));
    }
    const count = 1 + int(12);
    for (let i = 0; i < count; i++) {
      lines.push(...pickBlock());
      if (chance(0.6)) lines.push('');
    }
    if (chance(0.05) && lines.length > 0) {
      const at = int(lines.length);
      lines[at] = '\uFEFF' + lines[at];
    }
    let eol = pick(['\n', '\n', '\n', '\n', '\r\n', '\r', 'mixed']);
    let text = lines.map((line, i) => line + (eol === 'mixed' ? pick(['\n', '\r\n', '\r']) : eol)).join('');
    if (chance(0.2)) text = text.replace(/(?:\r\n|\r|\n)+$/, '');
    if (chance(0.1)) text += pick(['\n', '\n\n', '\r\n\r\n']);
    if (chance(0.1)) text = pick(['﻿', '﻿﻿']) + text;
    return text;
  };
}

// ---------------------------------------------------------------- 縮小

function minimize(text, stillFails, budget = 1500) {
  let current = text;
  let evaluations = 0;
  const attempt = function (candidate) {
    if (evaluations >= budget || candidate === current) return false;
    evaluations++;
    if (stillFails(candidate)) {
      current = candidate;
      return true;
    }
    return false;
  };
  // 先以行為單位做 delta debugging，再逐字元刪除。
  for (const unit of ['line', 'char']) {
    const split = (t) => (unit === 'line' ? t.split(/(?<=\n)/) : Array.from(t));
    let chunk = Math.max(1, Math.floor(split(current).length / 2));
    while (chunk >= 1 && evaluations < budget) {
      let progress = false;
      for (let start = 0; ; ) {
        const parts = split(current);
        if (start >= parts.length) break;
        const candidate = [...parts.slice(0, start), ...parts.slice(start + chunk)].join('');
        if (attempt(candidate)) progress = true;
        else start += chunk;
      }
      if (!progress) chunk = Math.floor(chunk / 2);
    }
  }
  return current;
}

// ---------------------------------------------------------------- 主程式

function parseCli(argv) {
  const opts = { count: 500, seed: 1, batch: 200, options: 'all', save: true, maxFailures: 5, idempotence: false, dumpDocs: null };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = () => {
      if (i + 1 >= argv.length) throw new Error(`${arg} needs a value`);
      return argv[++i];
    };
    if (arg === '--count') opts.count = Number(next());
    else if (arg === '--seed') opts.seed = Number(next());
    else if (arg === '--batch') opts.batch = Number(next());
    else if (arg === '--options') opts.options = next();
    else if (arg === '--no-save') opts.save = false;
    else if (arg === '--max-failures') opts.maxFailures = Number(next());
    else if (arg === '--idempotence') opts.idempotence = true;
    else if (arg === '--dump-docs') opts.dumpDocs = next();
    else throw new Error(`Unknown argument: ${arg}`);
  }
  if (!Number.isInteger(opts.count) || opts.count < 1) throw new Error('--count must be a positive integer');
  if (!Number.isInteger(opts.seed)) throw new Error('--seed must be an integer');
  if (!['all', 'default'].includes(opts.options)) throw new Error('--options must be all or default');
  return opts;
}

function idempotenceReport(docs) {
  const sets = OPTION_SETS;
  let checked = 0;
  const failures = [];
  for (const [index, text] of docs.entries()) {
    for (const set of sets) {
      const once = fixDocument(text, set.options);
      if (once.refused) continue;
      checked++;
      const twice = fixDocument(once.text, set.options);
      if (twice.text !== once.text) failures.push({ index, set: set.name, text });
    }
  }
  console.log(`oracle idempotence: ${failures.length} of ${checked} non-refused (doc, option set) pair(s) change again on a second fixDocument`);
  const bySet = {};
  for (const f of failures) bySet[f.set] = (bySet[f.set] || 0) + 1;
  if (failures.length > 0) console.log(`by option set: ${JSON.stringify(bySet)}`);
  for (const f of failures.slice(0, 3)) {
    const set = OPTION_SETS.find((s) => s.name === f.set);
    const small = minimize(f.text, (t) => {
      const once = fixDocument(t, set.options);
      return !once.refused && fixDocument(once.text, set.options).text !== once.text;
    }, 3000);
    console.log(`  doc #${f.index} [${f.set}] minimized: ${JSON.stringify(small)}`);
  }
  return failures.length;
}

function main(argv) {
  let opts;
  try {
    opts = parseCli(argv);
  } catch (error) {
    console.error(`Error: ${error.message}`);
    return 2;
  }
  const rand = mulberry32(opts.seed);
  const generate = makeGenerator(rand);
  const docs = Array.from({ length: opts.count }, () => generate());
  if (opts.dumpDocs) {
    fs.mkdirSync(opts.dumpDocs, { recursive: true });
    docs.forEach((text, i) => fs.writeFileSync(path.join(opts.dumpDocs, `doc-${String(i).padStart(5, '0')}.md`), text));
  }
  if (opts.idempotence) return idempotenceReport(docs) > 0 ? 1 : 0;

  const runner = new RustRunner(buildDump());
  const sets = opts.options === 'all' ? OPTION_SETS : OPTION_SETS.slice(0, 1);
  const cases = [];
  docs.forEach((text, index) => {
    for (const set of sets) cases.push({ index, set: set.name, text, options: canonicalOptions(set.options) });
  });

  const failures = [];
  const started = Date.now();
  for (let start = 0; start < cases.length && failures.length < opts.maxFailures; start += opts.batch) {
    const batch = cases.slice(start, start + opts.batch);
    const actual = runner.runBatch(batch);
    batch.forEach((c, k) => {
      if (failures.length >= opts.maxFailures) return;
      const difference = compareResults(runCase(c.text, c.options), actual[k]);
      if (difference && !failures.some((f) => f.index === c.index)) failures.push({ ...c, difference });
    });
  }
  const seconds = ((Date.now() - started) / 1000).toFixed(1);
  console.log(`fuzz-parity: seed ${opts.seed}, ${docs.length} doc(s), ${cases.length} case(s), ${failures.length} divergence(s), ${seconds}s`);

  for (const [n, failure] of failures.entries()) {
    const stillFails = (candidate) => compareResults(runCase(candidate, failure.options), runner.run(candidate, failure.options)) !== null;
    const small = minimize(failure.text, stillFails);
    const difference = compareResults(runCase(small, failure.options), runner.run(small, failure.options));
    console.log(`\n#${n + 1} doc ${failure.index} [${failure.set}] ${JSON.stringify(failure.options)}`);
    console.log(`  minimized input: ${JSON.stringify(small)}`);
    console.log(difference.split('\n').map((line) => `  ${line}`).join('\n'));
    if (opts.save) {
      const dir = path.join(CORPUS_DIR, 'regressions');
      fs.mkdirSync(dir, { recursive: true });
      const file = path.join(dir, `fuzz-s${opts.seed}-d${failure.index}.md`);
      fs.writeFileSync(file, small);
      console.log(`  saved: ${path.relative(REPO_ROOT, file)} (run: node scripts/gen-golden.mjs)`);
    }
  }
  return failures.length > 0 ? 1 : 0;
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) {
  process.exitCode = main(process.argv.slice(2));
}
