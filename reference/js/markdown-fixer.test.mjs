import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  RULES,
  VERSION,
  buildMask,
  diagnose,
  fixDocument,
  matchDefinition,
  parseArgs,
  scanDocument,
  splitTableCells
} from './markdown-fixer.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const cli = path.join(here, 'markdown-fixer.mjs');

function rulesOf(text, options) {
  return diagnose(text, options).map((problem) => problem.rule);
}

function fixed(text, options) {
  return fixDocument(text, options).text;
}

function tempFile(t, contents) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'markdown-fixer-test-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'doc.md');
  fs.writeFileSync(file, contents);
  return file;
}

function runCli(args, options = {}) {
  return spawnSync(process.execPath, [cli, ...args], {
    encoding: 'utf8',
    ...options
  });
}

test('rule catalog is well formed and unique', () => {
  assert.equal(VERSION, '1.1.0');
  const ids = RULES.map((rule) => rule.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const rule of RULES) {
    assert.match(rule.id, /^MDX\d{3}$/);
    assert.ok(['error', 'warning', 'info'].includes(rule.severity), rule.id);
    assert.equal(typeof rule.fixable, 'boolean', rule.id);
    assert.ok(rule.summary.length > 0, rule.id);
  }
});

test('fixes heading spacing, gaps, and closing hashes', () => {
  assert.equal(fixed('#Title\n'), '# Title\n');
  assert.equal(fixed('##  Sub  ##\n'), '## Sub\n');
  assert.equal(fixed('#  Title  ##\n'), '# Title\n');
  assert.equal(fixed('#Title #\n'), '# Title\n');
});

test('fixes list marker spacing and reports bullet style without touching flags or versions', () => {
  // A different bullet starts a new list in CommonMark, so MDX031 only reports.
  assert.equal(fixed('- a\n* b\n+ c\n'), '- a\n* b\n+ c\n');
  assert.deepEqual(diagnose('- a\n* b\n+ c\n').map((problem) => [problem.rule, problem.line, problem.fixable]), [['MDX031', 2, false], ['MDX031', 3, false]]);
  assert.equal(fixed('-item text\n'), '- item text\n');
  assert.equal(fixed('+other text\n'), '+ other text\n');
  assert.equal(fixed('1.item text\n'), '1. item text\n');
  assert.equal(fixed('--help\n-1\n+1\n1.a\n1.5\n'), '--help\n-1\n+1\n1.a\n1.5\n');
});

test('does not turn a thematic break into a list item', () => {
  const input = '* item\n\n- - -\n';
  assert.equal(fixed(input), input);
  assert.deepEqual(rulesOf(input), []);
});

test('fixes emphasis inner spacing but keeps symmetric single-delimiter spacing', () => {
  assert.equal(fixed('**  bold  ** and **bold ** and __ x __\n'), '**bold** and **bold** and __x__\n');
  assert.equal(fixed('Text * spaced* and *spaced * and * spaced *\n'), 'Text *spaced* and *spaced* and * spaced *\n');
  // A bullet `*` at the start of a line is a list marker, not a delimiter.
  assert.equal(fixed('* spaced* and *spaced *\n'), '* spaced* and *spaced*\n');
  assert.equal(fixed('> - * x*\n'), '> - * x*\n');
  assert.equal(fixed('* 說明**[](https://example.com)**\n'), '* 說明**[](https://example.com)**\n');
  assert.equal(fixed('_ spaced_ and _spaced _ and _ spaced _\n'), '_spaced_ and _spaced_ and _ spaced _\n');
});

test('fixes table delimiter width but leaves prose pipes alone', () => {
  assert.equal(
    fixed('| a | b |\n| - | -- |\n| 1 | 2 |\n'),
    '| a | b |\n| --- | --- |\n| 1 | 2 |\n'
  );
  assert.equal(fixed('| a | b |\n|:--|--:|\n| 1 | 2 |\n'), '| a | b |\n|:---|---:|\n| 1 | 2 |\n');
  assert.equal(fixed('foo | -- | bar\n'), 'foo | -- | bar\n');
  assert.equal(fixed('Text\n| - | - |\n'), 'Text\n| - | - |\n');
  assert.deepEqual(rulesOf('Text\n| - | - |\n'), []);
  assert.equal(fixed('  | a | b |\n  | - | -- |\n'), '  | a | b |\n  | --- | --- |\n');
  assert.ok(rulesOf('  | a | b |\n  | --- | --- |\n  | 1 | 2 | 3 |\n').includes('MDX061'));
  assert.ok(!rulesOf('    | a | b |\n    | --- | --- |\n').includes('MDX061'));
  assert.deepEqual(rulesOf('- item\n  | a | b |\n  | --- | --- |\n'), []);
  assert.equal(fixed('- item\n  | a | b |\n  | --- | --- |\n'), '- item\n  | a | b |\n  | --- | --- |\n');
  assert.ok(rulesOf('| a | b |\n| --- | --- |\n| https://example.com/a|b | c |\n').includes('MDX063'));
});

test('normalizes blockquote markers and preserves indented code', () => {
  assert.equal(fixed('>quote\n>>nested\n>  text\n>     code\n'), '> quote\n> > nested\n> text\n>     code\n');
  assert.equal(fixed('>\tcode\n'), '>\tcode\n');
});

test('protects indented code inside a blockquote', () => {
  const input = '>     https://example.com\n>     ** bold **\n> \tcode\n';
  assert.equal(fixed(input), input);
  assert.deepEqual(rulesOf(input), []);
  assert.ok(rulesOf('>     [label]: https://example.com\n> [text][label]\n').includes('MDX073'));
  const tabCode = '>  \tcode\n';
  assert.deepEqual(rulesOf(tabCode), []);
  assert.equal(fixed(tabCode), tabCode);
});

test('preserves a hard break when other trailing whitespace is removed', () => {
  assert.equal(fixed('text\t  \nnext\n'), 'text  \nnext\n');
});

test('normalizes whitespace, BOM, CRLF, and final newline', () => {
  assert.equal(fixed('\uFEFF# Title\r\n\r\n\r\ntext  \r\n'), '# Title\n\ntext  \n');
  assert.equal(fixed('text'), 'text\n');
  assert.equal(fixed('text\n\n'), 'text\n');
  assert.equal(fixed('\n\ntext\n'), 'text\n');
  assert.equal(fixed('line with 3   \nnext\n'), 'line with 3  \nnext\n');
  assert.equal(fixed('line with one \nnext\n'), 'line with one\nnext\n');
  assert.equal(fixed('line with break  \nnext\n'), 'line with break  \nnext\n');
  assert.equal(fixed(''), '');
});

test('protects frontmatter, code, math, HTML, and link destinations', () => {
  const input = [
    '---',
    'title: "#Title"',
    '---',
    '',
    '#Title',
    '',
    '`#NoSpace` and $x** y**$ and [link](https://example.com/a**b**).',
    '',
    '```js',
    '#NoSpace',
    '** bold **',
    '```',
    '',
    '<div>',
    '#NoSpace',
    '</div>',
    '',
    '    #NoSpace',
    ''
  ].join('\n');

  const output = fixed(input);
  assert.ok(output.includes('# Title'));
  assert.ok(!output.includes('# NoSpace'));
  assert.ok(output.includes('`#NoSpace`'));
  assert.ok(output.includes('$x** y**$'));
  assert.ok(output.includes('https://example.com/a**b**'));
  assert.ok(output.includes('    #NoSpace'));
});

test('refuses to write structurally ambiguous files', () => {
  const unclosedFrontmatter = '---\ntitle: x\n#Title\n';
  const frontmatterResult = fixDocument(unclosedFrontmatter);
  assert.equal(frontmatterResult.refused, true);
  assert.equal(frontmatterResult.text, unclosedFrontmatter);
  assert.match(frontmatterResult.reason, /frontmatter/i);

  const leadingBlanks = '\n\n---\ntitle: x\n#Title\n';
  const leadingResult = fixDocument(leadingBlanks);
  assert.equal(leadingResult.refused, true);
  assert.equal(leadingResult.text, leadingBlanks);

  const unclosedComment = '# Title\n\n<!--\n#Title\n';
  const commentResult = fixDocument(unclosedComment);
  assert.equal(commentResult.refused, true);
  assert.equal(commentResult.text, unclosedComment);
  assert.match(commentResult.reason, /comment/i);

  const invalidFence = '# Title\n\n```js`bad\ncode\n```\n';
  const fenceResult = fixDocument(invalidFence);
  assert.equal(fenceResult.refused, true);
  assert.equal(fenceResult.text, invalidFence);
  assert.match(fenceResult.reason, /backtick/i);

  const frontmatterTrap = '\n---\ntext\n---\n';
  const trapResult = fixDocument(frontmatterTrap);
  assert.equal(trapResult.refused, true);
  assert.equal(trapResult.text, frontmatterTrap);
  assert.match(trapResult.reason, /frontmatter/i);
});

test('refuses to expose frontmatter hidden behind a leading BOM line', () => {
  const closed = '\n\uFEFF---\ntitle: x\n---\n';
  const closedResult = fixDocument(closed);
  assert.equal(closedResult.refused, true);
  assert.equal(closedResult.text, closed);
  assert.match(closedResult.reason, /frontmatter/i);

  const unclosed = '\n\uFEFF---\ntitle: x\n';
  const unclosedResult = fixDocument(unclosed);
  assert.equal(unclosedResult.refused, true);
  assert.equal(unclosedResult.text, unclosed);
});

test('is idempotent on a mixed document', () => {
  const input = [
    '#Title',
    '',
    'Paragraph',
    '##  Sub  ##',
    '-item',
    '* other',
    '+ third',
    '',
    '**  bold  ** and * italic* and _ italic _',
    '',
    '>quote',
    '>     code',
    '',
    '| a | b |',
    '| - | -- |',
    '| 1 | 2 |',
    '',
    '```js',
    'code',
    '```',
    ''
  ].join('\n');

  const once = fixDocument(input);
  assert.equal(once.refused, false);
  const twice = fixDocument(once.text);
  assert.equal(twice.text, once.text);
  assert.equal(twice.changed, false);
  assert.ok(once.text.includes('# Title'));
  assert.ok(once.text.includes('## Sub'));
  assert.ok(once.text.includes('* other'));
  assert.ok(once.text.includes('**bold**'));
  assert.ok(once.text.includes('| --- | --- |'));
});

test('honors rule selection and ignore lists', () => {
  assert.equal(fixed('- a\n* b\n', { ignoreRules: ['MDX031'] }), '- a\n* b\n');
  assert.equal(fixed('- a\n* b\n', { rules: ['MDX031'] }), '- a\n* b\n');
  assert.equal(fixed('| a | b |\n| - | -- |\n', { rules: ['MDX031'] }), '| a | b |\n| - | -- |\n');
  assert.equal(fixed('| a | b |\n| - | -- |\n', { rules: ['MDX060'] }), '| a | b |\n| --- | --- |\n');
});

test('diagnoses structural, link, table, and CJK problems', () => {
  const input = [
    'Text before',
    '---',
    '',
    '## Section',
    '### Sub',
    '##### Deep',
    '',
    '[link][nope] and [dup][label].',
    '',
    '[label]: https://example.com',
    '[label]: https://example.com',
    '',
    '## Another',
    '',
    '![](img.png)',
    '',
    '| a | b |',
    '| --- | --- |',
    '| 1 | 2 | 3 |',
    '| `x|y` | z |',
    '',
    '＃ Fullwidth',
    '',
    '> quote',
    '>no space',
    '',
    'Bare https://example.com.',
    '',
    '#### Jump',
    ''
  ].join('\n');

  const rules = rulesOf(input);
  for (const rule of [
    'MDX080',
    'MDX018',
    'MDX015',
    'MDX073',
    'MDX074',
    'MDX075',
    'MDX061',
    'MDX063',
    'MDX084',
    'MDX020',
    'MDX070'
  ]) {
    assert.ok(rules.includes(rule), `expected ${rule} in ${rules.join(', ')}`);
  }
});

test('detects line length with CJK characters and skips tables', () => {
  const problems = diagnose('你好世界你好\n\nshort\n\n| long | table |\n| --- | --- |\n| a | b |\n', { maxLineLength: 5 });
  assert.deepEqual(problems.map((problem) => problem.rule), ['MDX090']);
  assert.equal(problems[0].line, 1);
  assert.equal(problems[0].severity, 'info');
});

test('does not report a thematic break after a code fence as Setext', () => {
  assert.ok(!rulesOf('```\ncode\n```\n---\n').includes('MDX080'));
});

test('handles Setext ambiguity and quoted reference definitions', () => {
  assert.ok(rulesOf('Text\n---\n').includes('MDX080'));
  assert.ok(!rulesOf('  Text\n  ---\n').includes('MDX080'));
  assert.ok(!rulesOf('> [label]: https://example.com\n\n> [text][label]\n').includes('MDX073'));
});

test('removes repeated leading BOMs and still fixes the heading', () => {
  assert.equal(fixed('\uFEFF\uFEFF#Title\n'), '# Title\n');
  assert.equal(fixDocument('\uFEFF\uFEFF#Title\n', { ignoreRules: ['MDX001'] }).text, '\uFEFF\uFEFF# Title\n');
});

test('detects a BOM, CRLF, and a missing heading space together', () => {
  const rules = rulesOf('\uFEFF#Title\r\n');
  assert.ok(rules.includes('MDX001'));
  assert.ok(rules.includes('MDX002'));
  assert.ok(rules.includes('MDX010'));
});

test('preserves BOM, CRLF, and missing final newline when their rules are disabled', () => {
  const result = fixDocument('\uFEFF#Title\r\ntext', {
    ignoreRules: ['MDX001', 'MDX002', 'MDX003', 'MDX013']
  });
  assert.equal(result.text, '\uFEFF# Title\r\ntext');
});

test('does not rewrite ignored BOM or mixed line endings when nothing else changes', () => {
  const input = '\uFEFF# Title\r\n\r\nText\n';
  const result = fixDocument(input, { ignoreRules: ['MDX001', 'MDX002'] });
  assert.equal(result.changed, false);
  assert.equal(result.text, input);
});

test('reports NBSP inside a URL without breaking the URL', () => {
  const text = 'text\u00A0here\n\nhttps://example.com/a\u00A0b\n';
  const problems = diagnose(text);
  const nbsp = problems.filter((problem) => problem.rule === 'MDX008');
  assert.equal(nbsp.length, 2);
  assert.equal(nbsp.filter((problem) => problem.fixable).length, 1);
  assert.equal(fixed(text), 'text here\n\nhttps://example.com/a\u00A0b\n');
});

test('buildMask covers code, math, links, and URLs selectively', () => {
  const line = '`code` https://example.com [text](dest) $x$ **bold**';
  const mask = buildMask(line, { maskBareUrls: false });
  assert.equal(mask[line.indexOf('`code`')], true);
  assert.equal(mask[line.indexOf('https')], false);
  assert.equal(mask[line.indexOf('dest')], true);
  assert.equal(mask[line.indexOf('$x$')], true);
  assert.equal(mask[line.indexOf('**bold**')], false);

  const link = '[https://example.com](https://example.com)';
  assert.equal(buildMask(link, { maskBareUrls: false })[link.indexOf('https')], false);
  assert.equal(buildMask(link, { maskBareUrls: false, maskLinkText: true })[link.indexOf('https')], true);
});

test('splitTableCells handles escaped and code-span pipes', () => {
  assert.deepEqual(splitTableCells('| a | b |'), ['a', 'b']);
  assert.deepEqual(splitTableCells('| a \\| b | c |'), ['a \\| b', 'c']);
  assert.deepEqual(splitTableCells('| `a|b` | c |'), ['`a|b`', 'c']);
});

test('treats tab-expanded four-column indentation as code', () => {
  const code = '   \t** bold **\n';
  assert.equal(fixed(code), code);
  assert.deepEqual(rulesOf(code), []);
  const table = '   \t| a | b |\n   \t| --- | --- |\n   \t| 1 | 2 | 3 |\n';
  assert.ok(!rulesOf(table).includes('MDX061'));
  assert.equal(fixed(table), table);
});

test('keeps multi-line indented paragraph continuations out of code', () => {
  const input = 'paragraph\n    ** bold **\n    ** bold **\n';
  assert.equal(fixed(input), 'paragraph\n    **bold**\n    **bold**\n');
  const scan = scanDocument(input);
  assert.equal(scan.info[1].indentedCode, false);
  assert.equal(scan.info[2].indentedCode, false);
});

test('classifies indented code after a heading or fence close', () => {
  assert.equal(fixed('# Title\n    code  \n'), '# Title\n    code  \n');
  const scan = scanDocument('```\ncode\n```\n    # code\n');
  assert.equal(scan.info[3].indentedCode, true);
  assert.equal(fixed('```\ncode\n```\n    code  \n'), '```\ncode\n```\n\n    code  \n');
});

test('protects multi-line $$ math blocks and reports unclosed ones', () => {
  const math = '$$\n#Title\n** bold **\n$$\n';
  assert.deepEqual(rulesOf(math), []);
  const mathResult = fixDocument(math);
  assert.equal(mathResult.changed, false);
  assert.equal(mathResult.text, math);

  const quoted = '> $$\n> #Title\n> ** bold **\n> $$\n';
  assert.deepEqual(rulesOf(quoted), []);
  assert.equal(fixDocument(quoted).text, quoted);

  const mathContent = '$$\nx = 1  \nx\u00A0y\n\n\n\u200B[label]: https://example.com\n$$\n';
  assert.deepEqual(rulesOf(mathContent), []);
  assert.equal(fixDocument(mathContent).text, mathContent);

  const listMath = '- $$\n  x = 1\n  $$\n';
  assert.deepEqual(rulesOf(listMath), []);
  assert.equal(fixDocument(listMath).text, listMath);

  const quoteListMath = '> - $$\n>   x = 1\n>   $$\n';
  assert.deepEqual(rulesOf(quoteListMath), []);
  assert.equal(fixDocument(quoteListMath).text, quoteListMath);

  const taskListMath = '- [ ] $$\n  x = 1\n  $$\n';
  assert.deepEqual(rulesOf(taskListMath), []);
  assert.equal(fixDocument(taskListMath).text, taskListMath);

  const listQuoteMath = '- > $$\n  > x = 1\n  > $$\n';
  assert.deepEqual(rulesOf(listQuoteMath), []);
  assert.equal(fixDocument(listQuoteMath).text, listQuoteMath);

  const leadingMath = '\n\n$$\n#Title\n$$\n';
  assert.equal(fixed(leadingMath), '$$\n#Title\n$$\n');

  const closedMath = '$$\nx\n$$\n#Title\n';
  assert.equal(fixed(closedMath), '$$\nx\n$$\n# Title\n');

  assert.deepEqual(diagnose('$$\na\tb\n$$\n'), []);
  assert.deepEqual(diagnose('$$\n' + 'x'.repeat(50) + '\n$$\n', { maxLineLength: 10 }), []);

  const unclosed = '$$\n#Title\n';
  assert.ok(rulesOf(unclosed).includes('MDX085'));
  const unclosedResult = fixDocument(unclosed);
  assert.equal(unclosedResult.refused, true);
  assert.equal(unclosedResult.text, unclosed);
  assert.match(unclosedResult.reason, /math/i);
  assert.equal(fixDocument(unclosed, { ignoreRules: ['MDX085'] }).refused, true);

  const fenced = '```\n$$\n#Title\n```\n';
  assert.ok(!rulesOf(fenced).includes('MDX010'));
  assert.ok(!rulesOf(fenced).includes('MDX085'));

  const listFence = '- example:\n  ```\n  $$\n  #Title\n  ```\n';
  assert.ok(!rulesOf(listFence).includes('MDX010'));
  assert.ok(!rulesOf(listFence).includes('MDX085'));

  const headingWithMath = '#Title $$x$$';
  assert.equal(fixed(headingWithMath), '# Title $$x$$\n');

  const scan = scanDocument(math);
  assert.equal(scan.info[0].mathOpen, true);
  assert.equal(scan.info[1].mathBlock, true);
  assert.equal(scan.info[3].mathClose, true);
});

test('scanDocument classifies frontmatter, fences, and indented code', () => {
  const scan = scanDocument('---\ntitle: x\n---\n\n```js\ncode\n```\n\n    indented\n');
  assert.equal(scan.frontmatterEnd, 2);
  assert.equal(scan.info[4].fenceOpen, true);
  assert.equal(scan.info[4].fenceInfo, 'js');
  assert.equal(scan.info[5].fenceContent, true);
  assert.equal(scan.info[6].fenceClose, true);
  assert.equal(scan.info[8].indentedCode, true);
  assert.equal(scan.issues.length, 0);
});

test('parseArgs handles modes, rule lists, limits, and errors', () => {
  const parsed = parseArgs([
    '--write',
    '--json',
    '--strict',
    '--rule',
    'MDX010,MDX011',
    '--ignore-rule=MDX031',
    '--max-line-length',
    '80',
    'a.md'
  ]);
  assert.equal(parsed.mode, 'write');
  assert.equal(parsed.json, true);
  assert.equal(parsed.strict, true);
  assert.deepEqual(parsed.rules, ['MDX010', 'MDX011']);
  assert.deepEqual(parsed.ignoreRules, ['MDX031']);
  assert.equal(parsed.maxLineLength, 80);
  assert.deepEqual(parsed.paths, ['a.md']);

  assert.equal(parseArgs([]).mode, 'check');
  assert.equal(parseArgs(['--rule', 'MDX090']).maxLineLength, 120);
  assert.deepEqual(parseArgs(['--', '--not-an-option.md']).paths, ['--not-an-option.md']);
  assert.throws(() => parseArgs(['--rule', 'NOPE']), /Unknown rule/);
  assert.throws(() => parseArgs(['--max-line-length', '0']), /positive integer/);
  assert.throws(() => parseArgs(['--bogus']), /Unknown option/);
});

test('CLI reports version, rules, and check results', (t) => {
  const version = runCli(['--version']);
  assert.equal(version.status, 0);
  assert.match(version.stdout, /markdown-fixer 1\.1\.0/);

  const list = runCli(['--list-rules']);
  assert.equal(list.status, 0);
  assert.ok(list.stdout.includes('MDX001'));
  assert.ok(list.stdout.includes('MDX085'));
  assert.ok(list.stdout.includes('MDX090'));

  const file = tempFile(t, '#Title\n');
  const check = runCli(['--check', file]);
  assert.equal(check.status, 1);
  assert.match(check.stdout, /MDX010/);
});

test('CLI writes fixes, supports JSON, and reports refusals', (t) => {
  const file = tempFile(t, '#Title\n');
  const jsonCheck = runCli(['--check', '--json', file]);
  assert.equal(jsonCheck.status, 1);
  const checkOutput = JSON.parse(jsonCheck.stdout);
  assert.equal(checkOutput.summary.files, 1);
  assert.equal(checkOutput.summary.changed, 0);
  assert.equal(checkOutput.summary.wouldChange, 1);
  assert.equal(checkOutput.files[0].problems[0].rule, 'MDX010');

  const write = runCli(['--write', '--json', file]);
  assert.equal(write.status, 0);
  const writeOutput = JSON.parse(write.stdout);
  assert.equal(writeOutput.summary.changed, 1);
  assert.ok(writeOutput.summary.fixed >= 1);
  assert.equal(fs.readFileSync(file, 'utf8'), '# Title\n');

  const refusedFile = tempFile(t, '---\ntitle: x\n#Title\n');
  const refused = runCli(['--write', refusedFile]);
  assert.equal(refused.status, 1);
  assert.match(refused.stdout, /Not modified/);
  assert.equal(fs.readFileSync(refusedFile, 'utf8'), '---\ntitle: x\n#Title\n');

  const refusedJson = runCli(['--check', '--json', refusedFile]);
  assert.equal(refusedJson.status, 1);
  const refusedOutput = JSON.parse(refusedJson.stdout);
  assert.equal(refusedOutput.files[0].refused, true);
  assert.equal(refusedOutput.files[0].wouldChange, false);
  assert.equal(refusedOutput.files[0].wouldFix, 0);
});

test('CLI requires explicit paths for --write and honors --strict', (t) => {
  const noPaths = runCli(['--write']);
  assert.equal(noPaths.status, 2);
  assert.match(noPaths.stderr, /requires explicit files or directories/);

  const infoFile = tempFile(t, 'Bare https://example.com.\n');
  assert.equal(runCli(['--check', infoFile]).status, 0);
  assert.equal(runCli(['--check', '--strict', infoFile]).status, 1);
});

test('CLI reports invalid UTF-8 and NUL bytes as I/O errors', (t) => {
  const invalid = tempFile(t, Buffer.from([0xff, 0xfe, 0x23, 0x20]));
  const invalidResult = runCli(['--check', invalid]);
  assert.equal(invalidResult.status, 2);
  assert.match(invalidResult.stderr, /utf-8/i);

  const nul = tempFile(t, '# Title\0\n');
  const nulResult = runCli(['--check', nul]);
  assert.equal(nulResult.status, 2);
  assert.match(nulResult.stderr, /NUL/);
});

test('does not claim success when some files cannot be read', (t) => {
  const good = tempFile(t, '# Title\n');
  const bad = tempFile(t, Buffer.from([0xff, 0xfe]));
  const result = runCli(['--check', good, bad]);
  assert.equal(result.status, 2);
  assert.match(result.stdout, /Some files could not be read/);
  assert.doesNotMatch(result.stdout, /No Markdown issues found/);
});

test('writes through a symlink without replacing it', { skip: process.platform === 'win32' }, (t) => {
  const target = tempFile(t, '#Title\n');
  const link = path.join(path.dirname(target), 'link.md');
  fs.symlinkSync(path.basename(target), link);
  const result = runCli(['--write', link]);
  assert.equal(result.status, 0, result.stderr);
  assert.ok(fs.lstatSync(link).isSymbolicLink());
  assert.equal(fs.readFileSync(target, 'utf8'), '# Title\n');
});

test('MDX040 leaves bold that already pairs across CJK punctuation alone', () => {
  // Real line 40 of docs/plans/CODE_REVIEW_REBALANCE_CHART_TUI_zh-TW.md.
  // The first bold closes at the 。** run and the second opens at the next **,
  // so the sentence is already correct Markdown and must not be rewritten.
  const line = '1. **缺漏｜流程動作不在 diff。** E＋F（試發→OK→部署）與 G（進版）屬流程，diff 無法驗證。**（本會話已實證完成：試發 Message ID `1567269943236362380`、部署 `52a62cc`＋容器 `/app` 雜湊回讀一致、進版 `8013246`。）**\n';
  assert.deepEqual(rulesOf(line), []);
  assert.equal(fixed(line), line);
});

test('MDX042 reports unpaired emphasis next to CJK punctuation and fixes the safe closing form', () => {
  const one = '**審查執行：**2026-11-04\n';
  assert.ok(rulesOf(one).includes('MDX042'));
  assert.equal(fixed(one), '**審查執行**：2026-11-04\n');
  assert.equal(fixed('**Spec 來源：**本次工作\n'), '**Spec 來源**：本次工作\n');
  const problems = diagnose(one).filter((problem) => problem.rule === 'MDX042');
  assert.ok(problems.length > 0);
  assert.ok(problems.every((problem) => problem.fixable));

  // An even count of ** can still be broken; the MDX041 parity check misses it.
  const two = '**審查執行：**2026 與 **追蹤：**2027\n';
  assert.ok(!rulesOf(two).includes('MDX041'));
  assert.ok(rulesOf(two).includes('MDX042'));
  assert.equal(fixed(two), '**審查執行**：2026 與 **追蹤**：2027\n');

  const once = fixed(one);
  assert.equal(fixed(once), once);
});

test('MDX042 leaves working emphasis, ASCII cases, code, and escapes alone', () => {
  assert.ok(!rulesOf('**狀態：待修正**（2026-…）\n').includes('MDX042'));
  assert.ok(!rulesOf('標籤：**內容**\n').includes('MDX042'));
  assert.ok(!rulesOf('**Note:**text\n').includes('MDX042'));
  assert.ok(!rulesOf('`**審查執行：**2026`\n').includes('MDX042'));
  assert.ok(!rulesOf('```text\n**審查執行：**2026\n```\n').includes('MDX042'));
  assert.ok(!rulesOf('這是\\*「引號」\\*文字\n').includes('MDX042'));
  // Literal asterisks and money amounts must not look like broken emphasis:
  // only two unpaired runs that were meant to be one span are reported.
  assert.ok(!rulesOf('打錯字（gpt-5.x、claude-*、gemini-2.5-flash）。\n').includes('MDX042'));
  assert.ok(!rulesOf('2 * 三 * 四\n').includes('MDX042'));
  assert.ok(!rulesOf('| **$10.35** | – | $56.97 | **$490.20**（x） |\n').includes('MDX042'));
  assert.ok(!rulesOf('- **重度使用者**：victor0891（0.419B，**100% glm-5.2，花掉 $593.06**，是最貴的）、wei（$183.93）。\n').includes('MDX042'));
  assert.equal(fixed('**狀態：待修正**（2026-…）\n'), '**狀態：待修正**（2026-…）\n');
  assert.equal(fixed('標籤：**內容**\n'), '標籤：**內容**\n');
});

test('MDX042 reports opening-side problems without rewriting them', () => {
  const line = '這是**「引號」**文字\n';
  const problems = diagnose(line).filter((problem) => problem.rule === 'MDX042');
  assert.ok(problems.length > 0);
  assert.ok(problems.every((problem) => problem.fixable === false));
  assert.equal(fixed(line), line);

  // Single-star leftovers are reported but never auto-rewritten.
  const star = '*標籤：*內容\n';
  assert.ok(rulesOf(star).includes('MDX042'));
  assert.equal(fixed(star), star);
});

test('matchDefinition matches the v1.0.0 reference-definition regex', () => {
  // The old regex backtracks exponentially, so keep generated quote depth <= 12.
  const old = /^(?: {0,3}>[ \t]?)* {0,3}\[([^\]\n]+)\]:[ \t]*(?:<[^>\n]*>|\S+)/;
  const check = function (line) {
    const expected = line.match(old);
    const actual = matchDefinition(line);
    if (!expected) {
      assert.equal(actual, null, JSON.stringify(line));
      return;
    }
    assert.ok(actual, JSON.stringify(line));
    assert.equal(actual.label, expected[1], JSON.stringify(line));
    assert.equal(actual.index, expected[0].indexOf('[') + 1, JSON.stringify(line));
  };

  // Every short string over an alphabet of the syntax characters.
  const alphabet = [' ', '\t', '>', '[', ']', ':', 'x', '<', ' ', '\n'];
  const build = function (prefix, length) {
    check(prefix);
    if (length === 0) return;
    for (const ch of alphabet) build(prefix + ch, length - 1);
  };
  build('', 5);

  // Structured lines: a quote prefix, optional indentation, then definition parts.
  let seed = 1101;
  const random = function (n) {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed % n;
  };
  const pick = (items) => items[random(items.length)];
  for (let n = 0; n < 20000; n++) {
    let line = '';
    const depth = random(13);
    for (let d = 0; d < depth; d++) {
      line += ' '.repeat(random(5)) + '>' + pick(['', '', ' ', '\t', '  ']);
    }
    line += ' '.repeat(random(5));
    line += pick(['[', '[', '[', '', '>', '\t[']);
    line += pick(['', 'a', 'label', 'a b', ']', '\n', '　']);
    line += pick([']:', ']:', ']', ']: ', '] :', '']);
    line += pick(['', ' ', '\t', '  ', ' ']);
    line += pick(['https://x', '<>', '<a b>', '<', '', ' ', '　', 'x y']);
    check(line);
  }
});

test('MDX086 reports blockquotes nested deeper than maxQuoteDepth and blocks fixing', () => {
  const deep = '> '.repeat(33) + 'x';
  const text = `# T\n\n${deep} \t\n`;
  const problems = diagnose(text);
  assert.deepEqual(problems.filter((problem) => problem.rule === 'MDX086').map((problem) => [problem.line, problem.severity, problem.fixable]), [[3, 'error', false]]);
  assert.match(problems.find((problem) => problem.rule === 'MDX086').message, /depth 33 exceeds the limit of 32/);
  // The deep line is opaque: no other rule reports it.
  assert.deepEqual(problems.filter((problem) => problem.line === 3).map((problem) => problem.rule), ['MDX086']);

  const result = fixDocument(text);
  assert.equal(result.refused, true);
  assert.equal(result.changed, false);
  assert.match(result.reason, /Blockquote nesting depth 33/);
  // Blocking like MDX081/083/053/085: ignoreRules cannot bypass it.
  assert.equal(fixDocument(text, { ignoreRules: ['MDX086'] }).refused, true);
  assert.ok(!rulesOf(text, { ignoreRules: ['MDX086'] }).includes('MDX086'));

  // 32 levels is within the default limit; 0 disables the limit.
  assert.ok(!rulesOf('> '.repeat(32) + 'x\n').includes('MDX086'));
  assert.ok(!rulesOf(text, { maxQuoteDepth: 0 }).includes('MDX086'));
  assert.equal(fixDocument(text, { maxQuoteDepth: 0 }).text, `# T\n\n${deep}\n`);
  assert.ok(rulesOf('>>>> x\n', { maxQuoteDepth: 3 }).includes('MDX086'));

  // Deep lines inside fenced code are code, not blockquotes.
  assert.ok(!rulesOf('```\n' + deep + '\n```\n').includes('MDX086'));
});

test('onLimit ignore leaves over-deep lines untouched and undiagnosed', () => {
  const deep = '>'.repeat(5) + '#Head##  **  x** y\thttps://x  ';
  const deepDefinition = '>'.repeat(5) + '[a]: https://x';
  const text = `#Title\n\n${deep}\n${deepDefinition}\n\n[t][a]\n`;
  const options = { maxQuoteDepth: 4, onLimit: 'ignore' };
  const problems = diagnose(text, options);
  assert.ok(problems.every((problem) => problem.line !== 3 && problem.line !== 4), JSON.stringify(problems));
  const result = fixDocument(text, options);
  assert.equal(result.refused, false);
  assert.equal(result.text, `# Title\n\n${deep}\n${deepDefinition}\n\n[t][a]\n`);
  // The opaque line is not a definition, so [t][a] has none.
  assert.ok(rulesOf(text, options).includes('MDX073'));
  assert.ok(!rulesOf(text, { maxQuoteDepth: 0 }).includes('MDX073'));
  // Without the limit the same line is an ordinary blockquote.
  assert.notEqual(fixDocument(text, { maxQuoteDepth: 0 }).text.split('\n')[2], deep);
});

test('fixing blockquote spacing never changes the counted quote depth', () => {
  let seed = 86;
  const random = function (n) {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed % n;
  };
  for (let n = 0; n < 3000; n++) {
    let line = ' '.repeat(random(4));
    const depth = 1 + random(8);
    for (let d = 0; d < depth; d++) line += '>' + ' '.repeat(random(6));
    line += random(2) ? 'x' : '';
    for (let limit = 1; limit <= 8; limit++) {
      const before = rulesOf(`${line}\n`, { maxQuoteDepth: limit }).includes('MDX086');
      const after = rulesOf(fixed(`${line}\n`, { maxQuoteDepth: 0 }), { maxQuoteDepth: limit }).includes('MDX086');
      assert.ok(!after || before, JSON.stringify([line, limit]));
    }
  }
});

test('deep blockquotes and very long lines finish without exceptions', () => {
  const started = Date.now();
  fixDocument('>'.repeat(200) + ' q\n', { maxQuoteDepth: 0 });
  diagnose('> '.repeat(200) + 'q\n', { maxQuoteDepth: 0 });
  assert.ok(Date.now() - started < 2000);
  const long = 'a'.repeat(200000) + '\n';
  assert.equal(fixDocument(long).changed, false);
  assert.deepEqual(diagnose(long), []);
  assert.equal(diagnose('a \n'.repeat(150000)).length, 150000);
});

test('MDX052 inside a blockquote inserts a quoted blank line', () => {
  assert.equal(
    fixed('# T\n\n> text\n> ```js\n> x\n> ```\n> more\n'),
    '# T\n\n> text\n>\n> ```js\n> x\n> ```\n>\n> more\n'
  );
  // A lazy continuation line stays in the same blockquote.
  assert.equal(
    fixed('# T\n\n> para\nlazy\n> ```js\n> x\n> ```\n'),
    '# T\n\n> para\nlazy\n>\n> ```js\n> x\n> ```\n'
  );
  assert.equal(
    fixed('# T\n\n> > a\n> > ```\n> > x\n> > ```\n> > b\n'),
    '# T\n\n> > a\n> >\n> > ```\n> > x\n> > ```\n> >\n> > b\n'
  );
  // Already separated by a quoted blank line: nothing to report.
  const separated = '# T\n\n> text\n>\n> ```js\n> x\n> ```\n>\n> more\n';
  assert.ok(!rulesOf(separated).includes('MDX052'));
  assert.equal(fixDocument(separated).changed, false);
  // Top-level fences still get a fully blank line.
  assert.equal(fixed('# T\n\ntext\n```js\nx\n```\nmore\n'), '# T\n\ntext\n\n```js\nx\n```\n\nmore\n');
});

test('CLI accepts --max-quote-depth and --on-limit', (t) => {
  const file = tempFile(t, '# T\n\n>>>> x\n');
  const strict = runCli(['--check', '--max-quote-depth', '3', file]);
  assert.equal(strict.status, 1);
  assert.match(strict.stdout, /MDX086/);
  assert.match(strict.stdout, /Structural error: Blockquote nesting depth 4 exceeds the limit of 3/);
  const ignored = runCli(['--check', '--max-quote-depth=3', '--on-limit=ignore', file]);
  assert.equal(ignored.status, 0, ignored.stdout);
  const unlimited = runCli(['--check', '--max-quote-depth=0', file]);
  assert.match(unlimited.stdout, /MDX020/);

  for (const args of [['--max-quote-depth'], ['--max-quote-depth=-1'], ['--max-quote-depth=1e2'], ['--max-quote-depth=']]) {
    const bad = runCli([...args, file]);
    assert.equal(bad.status, 2);
    assert.match(bad.stderr, /--max-quote-depth requires a non-negative integer\./);
  }
  for (const args of [['--on-limit'], ['--on-limit=IGNORE'], ['--on-limit', 'skip']]) {
    const bad = runCli([...args, file]);
    assert.equal(bad.status, 2);
    assert.match(bad.stderr, /--on-limit requires error or ignore\./);
  }
  assert.deepEqual(
    [parseArgs([]).maxQuoteDepth, parseArgs([]).onLimit],
    [32, 'error']
  );
});

test('MDX020 keeps spaces after > that are part of the content', () => {
  assert.equal(fixed('>quote\n>>nested\n>  > x\n>  text\n'), '> quote\n> > nested\n> > x\n> text\n');
  assert.equal(fixed('> a\n>   \n> b\n'), '> a\n>\n> b\n');
  // A whole `>  >  >` chain is normalized in one pass, so fixing is idempotent.
  const chain = '>  '.repeat(12) + 'x\n';
  assert.equal(fixed(chain), '> '.repeat(12) + 'x\n');
  assert.equal(fixDocument(fixed(chain)).changed, false);
  // Two or more spaces may continue a list item; fences and list markers keep their indentation.
  const kept = '# T\n\n> - item\n>   > nested\n>\n>   continued\n\n>  ```\n>  code\n>  ```\n\n>  - one\n>    two\n';
  assert.equal(fixed(kept), kept);
  assert.ok(!rulesOf(kept).includes('MDX020'));
  // A raw HTML block may open with a tag name at the end of the line.
  const html = '<iframe\n></iframe>\n';
  assert.equal(fixed(html), html);
});

test('MDX060 skips header and delimiter rows that GFM does not treat as a table', () => {
  const notTable = '| a | `x|y` |\n|:--|--:|\n';
  assert.equal(fixed(notTable), notTable);
  assert.ok(!rulesOf(notTable).includes('MDX060'));
  assert.equal(fixed('| a | `x\\|y` |\n|:--|--:|\n'), '| a | `x\\|y` |\n|:---|---:|\n');
});

test('MDX006 does not turn a U+FEFF into a BOM by removing leading blank lines', () => {
  const text = '\n﻿# T\n';
  assert.equal(fixDocument(text).changed, false);
  assert.deepEqual(diagnose(text).filter((problem) => problem.rule === 'MDX006').map((problem) => problem.fixable), [false]);
  assert.equal(fixed('\n\n# T\n'), '# T\n');
});

test('MDX008 keeps an NBSP that is the only content of a line or list item', () => {
  for (const text of ['a\n\u00A0\nb\n', '- \u00A0\n', '> \u00A0\u00A0\n']) {
    assert.equal(fixed(text), text);
    assert.ok(!rulesOf(text).includes('MDX008'));
  }
  assert.equal(fixed('- a\u00A0b\n'), '- a b\n');
});

test('MDX008 reports but keeps an NBSP right after a list marker', () => {
  for (const text of ['*\u00A0x\n', '1.\u00A0x\n', '- *\u00A0x\n', '> +\u00A0\u00A0x\n']) {
    assert.equal(fixed(text), text);
    const problems = diagnose(text).filter((problem) => problem.rule === 'MDX008');
    assert.deepEqual(problems.map((problem) => problem.fixable), [false], JSON.stringify(text));
  }
  // Other NBSPs on the same line are still replaced.
  assert.equal(fixed('*\u00A0x\u00A0y\n'), '*\u00A0x y\n');
  assert.deepEqual(diagnose('*\u00A0x\u00A0y\n').map((problem) => [problem.rule, problem.column, problem.fixable]), [['MDX008', 2, false], ['MDX008', 4, true]]);
});

test('protects an HTML comment or raw block opened after a list or blockquote marker', () => {
  const text = '# T\n\n- <!-- note\n\n\n  still  comment\n-->\n\n> <!-- quoted\n>\n>\n> -->\n';
  assert.equal(fixed(text), text);
  assert.ok(!rulesOf(text).includes('MDX005'));
  // Unclosed, it blocks fixing like a top-level unclosed comment.
  const unclosed = '# T\n\n- <!-- note\n\n\n\t#x\n';
  const result = fixDocument(unclosed);
  assert.equal(result.refused, true);
  assert.match(result.reason, /Unclosed HTML comment/);
  // A comment closed on the same line is masked inline as before.
  assert.equal(fixed('- <!-- x -->  **  y**\n'), '- <!-- x -->  **y**\n');
  for (const block of ['- <pre>\n    \n\n\tx  **  y**\n  </pre>\n', '> <script>\n>\n>\n> </script>\n', '- <?php\n\n\n?>\n', '- <![CDATA[\n\n\n]]>\n']) {
    const text = `# T\n\n${block}`;
    assert.equal(fixed(text), text, JSON.stringify(block));
  }
  // Unclosed raw blocks stay protected to the end, without blocking (as at the top level).
  assert.equal(fixDocument('# T\n\n- <pre>\n\n\nx\n').refused, false);
  assert.equal(fixed('# T\n\n- <pre>\n\n\nx\n'), '# T\n\n- <pre>\n\n\nx\n');
});

test('keeps whitespace-only lines that GitHub treats as content after an empty list item or in a footnote', () => {
  // [input, fixed output, line number of the kept whitespace-only line]
  const cases = [
    ['- \n  \n  x\n', '-\n  \n  x\n', 2],
    ['- a\n- \n  \n  b\n- c\n', '- a\n-\n  \n  b\n- c\n', 3],
    ['1.\n   \n   x\n', '1.\n   \n   x\n', 2],
    ['- \n\t\n  x\n', '-\n\t\n  x\n', 2],
    ['> - \n>   \n>   x\n', '> -\n>   \n>   x\n', 2],
    ['x[^1]\n\n[^1]: a\n  \n    b\n', 'x[^1]\n\n[^1]: a\n  \n    b\n', 4],
    // MDX005 does not collapse across the kept line, but still collapses the empty lines after it.
    ['# T\n\n- \n  \n\n\n  x\n', '# T\n\n-\n  \n\n  x\n', 4]
  ];
  for (const [input, output, kept] of cases) {
    assert.equal(fixed(input), output, JSON.stringify(input));
    const onKept = diagnose(input).filter((problem) => problem.line === kept && ['MDX004', 'MDX005', 'MDX020'].includes(problem.rule));
    assert.deepEqual(onKept, [], JSON.stringify(input));
  }
  // Elsewhere whitespace-only lines are still cleared.
  assert.equal(fixed('text  \n\n  \n\nmore\n'), 'text  \n\nmore\n');
  assert.equal(fixed('- a\n  \n  b\n'), '- a\n\n  b\n');
});
