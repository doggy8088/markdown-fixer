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
  assert.equal(VERSION, '1.0.0');
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

test('fixes list marker spacing and bullet style without touching flags or versions', () => {
  assert.equal(fixed('- a\n* b\n+ c\n'), '- a\n- b\n- c\n');
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
  assert.equal(fixed('* spaced* and *spaced * and * spaced *\n'), '*spaced* and *spaced* and * spaced *\n');
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
  assert.ok(once.text.includes('- other'));
  assert.ok(once.text.includes('**bold**'));
  assert.ok(once.text.includes('| --- | --- |'));
});

test('honors rule selection and ignore lists', () => {
  assert.equal(fixed('- a\n* b\n', { ignoreRules: ['MDX031'] }), '- a\n* b\n');
  assert.equal(fixed('- a\n* b\n', { rules: ['MDX031'] }), '- a\n- b\n');
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
  assert.match(version.stdout, /markdown-fixer 1\.0\.0/);

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
