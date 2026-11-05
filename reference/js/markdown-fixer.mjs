#!/usr/bin/env node
'use strict';

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const VERSION = '1.2.0';

const RULES = [
  // Encoding and whitespace
  { id: 'MDX001', name: 'bom', category: 'encoding', severity: 'warning', fixable: true, summary: 'Remove a UTF-8 BOM from the start of the file.' },
  { id: 'MDX002', name: 'line-endings', category: 'encoding', severity: 'warning', fixable: true, summary: 'Normalize CRLF/CR line endings to LF.' },
  { id: 'MDX003', name: 'final-newline', category: 'whitespace', severity: 'warning', fixable: true, summary: 'End the file with exactly one newline.' },
  { id: 'MDX004', name: 'trailing-whitespace', category: 'whitespace', severity: 'warning', fixable: true, summary: 'Remove trailing whitespace, preserving two-space hard breaks.' },
  { id: 'MDX005', name: 'multiple-blank-lines', category: 'whitespace', severity: 'warning', fixable: true, summary: 'Collapse two or more blank lines to one.' },
  { id: 'MDX006', name: 'leading-blank-lines', category: 'whitespace', severity: 'warning', fixable: true, summary: 'Remove blank lines before the first block.' },
  { id: 'MDX007', name: 'hard-tab', category: 'whitespace', severity: 'warning', fixable: false, summary: 'Avoid tab characters in prose and indentation.' },
  { id: 'MDX008', name: 'non-breaking-space', category: 'whitespace', severity: 'warning', fixable: true, summary: 'Replace NBSP (U+00A0) with a normal space outside code.' },
  { id: 'MDX009', name: 'zero-width-character', category: 'encoding', severity: 'warning', fixable: false, summary: 'Detect stray zero-width spaces and mid-file BOMs.' },

  // Headings
  { id: 'MDX010', name: 'atx-missing-space', category: 'heading', severity: 'warning', fixable: true, summary: 'Add a space after the ATX heading marker (#).' },
  { id: 'MDX011', name: 'atx-multiple-space', category: 'heading', severity: 'warning', fixable: true, summary: 'Use one space after the ATX heading marker.' },
  { id: 'MDX012', name: 'atx-closing-hashes', category: 'heading', severity: 'warning', fixable: true, summary: 'Remove optional closing hashes from an ATX heading.' },
  { id: 'MDX013', name: 'heading-blank-lines', category: 'heading', severity: 'warning', fixable: true, summary: 'Separate top-level headings from surrounding paragraphs with blank lines.' },
  { id: 'MDX014', name: 'heading-indent', category: 'heading', severity: 'info', fixable: false, summary: 'Verify indented headings are intentional (for example, inside a list).' },
  { id: 'MDX015', name: 'heading-increment', category: 'heading', severity: 'warning', fixable: false, summary: 'Do not skip heading levels.' },
  { id: 'MDX016', name: 'empty-heading', category: 'heading', severity: 'warning', fixable: false, summary: 'Remove or fill an empty heading.' },
  { id: 'MDX017', name: 'heading-trailing-punctuation', category: 'heading', severity: 'info', fixable: false, summary: 'Avoid ending headings with sentence punctuation.' },
  { id: 'MDX018', name: 'first-heading-h1', category: 'heading', severity: 'info', fixable: false, summary: 'Start the document outline with an H1.' },
  { id: 'MDX019', name: 'multiple-h1', category: 'heading', severity: 'info', fixable: false, summary: 'Prefer one H1 per document; repeated H1s are sometimes intentional in changelogs and task logs.' },

  // Blockquotes
  { id: 'MDX020', name: 'blockquote-space', category: 'blockquote', severity: 'warning', fixable: true, summary: 'Put one space after each blockquote marker.' },

  // Lists
  { id: 'MDX030', name: 'list-marker-space', category: 'list', severity: 'warning', fixable: true, summary: 'Add a space between a list marker and its content.' },
  { id: 'MDX031', name: 'list-marker-style', category: 'list', severity: 'warning', fixable: false, summary: 'Use one bullet marker per list; a different marker starts a new list, so this is reported, not fixed.' },
  { id: 'MDX032', name: 'list-blank-lines', category: 'list', severity: 'warning', fixable: false, summary: 'Separate lists from surrounding paragraphs with blank lines.' },
  { id: 'MDX035', name: 'empty-list-item', category: 'list', severity: 'warning', fixable: false, summary: 'Remove empty list items or give them content.' },

  // Emphasis
  { id: 'MDX040', name: 'emphasis-inner-space', category: 'emphasis', severity: 'warning', fixable: true, summary: 'Remove spaces just inside emphasis delimiters.' },
  { id: 'MDX041', name: 'unmatched-emphasis', category: 'emphasis', severity: 'warning', fixable: false, summary: 'Check for a possible unmatched strong or strikethrough delimiter.' },
  { id: 'MDX042', name: 'cjk-emphasis-unpaired', category: 'emphasis', severity: 'warning', fixable: true, summary: 'Report ** or * that cannot pair next to CJK or full-width punctuation; move full-width punctuation outside the delimiters when safe.' },

  // Fenced code
  { id: 'MDX050', name: 'fence-language', category: 'fence', severity: 'info', fixable: false, summary: 'Add a language to fenced code blocks when one is known.' },
  { id: 'MDX051', name: 'fence-unclosed', category: 'fence', severity: 'error', fixable: false, summary: 'Close every fenced code block.' },
  { id: 'MDX052', name: 'fence-blank-lines', category: 'fence', severity: 'warning', fixable: true, summary: 'Surround top-level fenced code blocks with blank lines.' },
  { id: 'MDX053', name: 'fence-invalid', category: 'fence', severity: 'error', fixable: false, summary: 'Use a valid fence info string.' },

  // Tables
  { id: 'MDX060', name: 'table-delimiter-width', category: 'table', severity: 'warning', fixable: true, summary: 'Use at least three hyphens in each table delimiter cell.' },
  { id: 'MDX061', name: 'table-column-count', category: 'table', severity: 'error', fixable: false, summary: 'Keep the same number of cells in every table row.' },
  { id: 'MDX062', name: 'table-blank-lines', category: 'table', severity: 'warning', fixable: true, summary: 'Surround top-level tables with blank lines.' },
  { id: 'MDX063', name: 'table-pipe-in-code', category: 'table', severity: 'warning', fixable: false, summary: 'Escape pipes inside code spans or URLs in table cells.' },

  // Links and images
  { id: 'MDX070', name: 'bare-url', category: 'link', severity: 'info', fixable: false, summary: 'Prefer an explicit link or angle-bracket autolink for bare URLs.' },
  { id: 'MDX071', name: 'empty-link', category: 'link', severity: 'warning', fixable: false, summary: 'Give links meaningful text and a destination.' },
  { id: 'MDX072', name: 'link-space-before-destination', category: 'link', severity: 'info', fixable: false, summary: 'A space between ] and ( prevents link parsing.' },
  { id: 'MDX073', name: 'reference-missing-definition', category: 'link', severity: 'warning', fixable: false, summary: 'Define every explicit reference-link label.' },
  { id: 'MDX074', name: 'reference-duplicate-definition', category: 'link', severity: 'warning', fixable: false, summary: 'Define each reference-link label once.' },
  { id: 'MDX075', name: 'image-alt', category: 'link', severity: 'warning', fixable: false, summary: 'Add image alt text or confirm the image is decorative.' },

  // Structure and style
  { id: 'MDX080', name: 'setext-ambiguity', category: 'structure', severity: 'warning', fixable: false, summary: 'A line of --- or === after a paragraph is a Setext heading, not a rule.' },
  { id: 'MDX081', name: 'frontmatter-unclosed', category: 'structure', severity: 'error', fixable: false, summary: 'Close YAML frontmatter with --- or ... .' },
  { id: 'MDX082', name: 'frontmatter-blank-line', category: 'structure', severity: 'warning', fixable: true, summary: 'Put a blank line after frontmatter.' },
  { id: 'MDX083', name: 'html-comment-unclosed', category: 'structure', severity: 'error', fixable: false, summary: 'Close every HTML comment.' },
  { id: 'MDX084', name: 'fullwidth-markdown-syntax', category: 'structure', severity: 'warning', fixable: false, summary: 'Use ASCII punctuation for Markdown syntax.' },
  { id: 'MDX085', name: 'math-unclosed', category: 'structure', severity: 'error', fixable: false, summary: 'Close every $$ math block.' },
  { id: 'MDX086', name: 'resource-limit', category: 'structure', severity: 'error', fixable: false, summary: 'Keep blockquote nesting within the configured depth limit (maxQuoteDepth).' },
  { id: 'MDX090', name: 'line-length', category: 'style', severity: 'info', fixable: false, summary: 'Keep prose lines within the configured limit.' }
];

const RULE_BY_ID = new Map(RULES.map((rule) => [rule.id, rule]));

const DEFAULT_EXCLUDES = new Set([
  '.git',
  '.hg',
  '.svn',
  'node_modules',
  'vendor',
  'coverage',
  'test-results',
  'playwright-report',
  'blob-report',
  '.e2e-tmp'
]);

const RAW_HTML_TAGS = 'script|pre|style|textarea|xmp|iframe|noembed|noframes|noscript|plaintext';
const HTML_BLOCK_TAGS = [
  'address', 'article', 'aside', 'base', 'basefont', 'blockquote', 'body', 'caption',
  'center', 'col', 'colgroup', 'dd', 'details', 'dialog', 'dir', 'div', 'dl', 'dt',
  'fieldset', 'figcaption', 'figure', 'footer', 'form', 'frame', 'frameset', 'h1',
  'h2', 'h3', 'h4', 'h5', 'h6', 'head', 'header', 'hr', 'html', 'legend', 'li',
  'link', 'main', 'menu', 'menuitem', 'meta', 'nav', 'noframes', 'ol', 'optgroup',
  'option', 'p', 'param', 'section', 'source', 'summary', 'table', 'tbody', 'td',
  'tfoot', 'th', 'thead', 'title', 'tr', 'track', 'ul'
].join('|');

function makeProblem(ruleId, line, column, message, overrides = {}) {
  const rule = RULE_BY_ID.get(ruleId);
  if (!rule) {
    throw new Error(`Unknown rule: ${ruleId}`);
  }
  return {
    rule: ruleId,
    name: rule.name,
    severity: overrides.severity || rule.severity,
    fixable: overrides.fixable === undefined ? rule.fixable : overrides.fixable,
    line: Math.max(1, line),
    column: Math.max(1, column || 1),
    message
  };
}

const DEFAULT_MAX_QUOTE_DEPTH = 32;

// Resource limits shared by every scan of one diagnose/fixDocument call.
// maxQuoteDepth 0 means no limit; onLimit is 'error' (MDX086, blocking) or 'ignore'.
function createLimits(options = {}) {
  const depth = options.maxQuoteDepth ?? DEFAULT_MAX_QUOTE_DEPTH;
  return {
    maxQuoteDepth: typeof depth === 'number' && depth >= 0 ? Math.floor(depth) : DEFAULT_MAX_QUOTE_DEPTH,
    onLimit: options.onLimit === 'ignore' ? 'ignore' : 'error'
  };
}

function createEnabled(options = {}) {
  const include = options.rules ? new Set(options.rules) : null;
  const ignore = options.ignoreRules ? new Set(options.ignoreRules) : null;
  const isEnabled = function (ruleId) {
    if (include && !include.has(ruleId)) return false;
    if (ignore && ignore.has(ruleId)) return false;
    if (ruleId === 'MDX090' && !(options.maxLineLength > 0)) return false;
    return true;
  };
  isEnabled.limits = createLimits(options);
  return isEnabled;
}

function normalizeForAnalysis(text) {
  return text.replace(/^\uFEFF+/, '').replace(/\r\n?/g, '\n');
}

function countIndent(value) {
  let columns = 0;
  for (const ch of value) {
    columns += ch === '\t' ? 4 - (columns % 4) : 1;
  }
  return columns;
}

function isIndentedCodeLine(line) {
  return countIndent(line.match(/^[ \t]*/)[0]) >= 4;
}

function isBlank(line) {
  return /^[ \t]*$/.test(line);
}

// A leading blank line is only safe to remove when it cannot expose a `---`
// line at byte 0, where frontmatter parsers would start interpreting YAML.
function startsFrontmatterAfterLeadingBlanks(text) {
  const lines = text.split('\n');
  let i = 0;
  while (i < lines.length && isBlank(lines[i])) i++;
  if (i === 0 || i >= lines.length) return false;
  return /^---[ \t]*$/.test(lines[i].replace(/^\uFEFF+/, ''));
}

function isThematicBreak(line) {
  return /^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$/.test(line);
}

function isAtxHeading(line) {
  if (!/^ {0,3}#{1,6}(?:[ \t]|$)/.test(line)) return false;
  const indent = line.match(/^ */)[0].length;
  const hashes = line.slice(indent).match(/^#+/)[0].length;
  return hashes <= 6 && line[indent + hashes] !== '#';
}

function isListLine(line) {
  return /^(?: {0,3})(?:[-*+]|\d{1,9}[.)])[ \t]+\S/.test(line);
}

function isTopLevelLine(line) {
  return line.length > 0 && line[0] !== ' ' && line[0] !== '\t';
}

function isParagraphLike(line, lineInfo) {
  if (isBlank(line)) return false;
  if (line[0] === ' ' || line[0] === '\t' || line[0] === '>') return false;
  if (isProtectedLine(lineInfo)) return false;
  if (isAtxHeading(line) || isListLine(line) || isThematicBreak(line)) return false;
  if (lineInfo.definition) return false;
  if (/^\|/.test(line)) return false;
  if (line.startsWith('```') || line.startsWith('~~~')) return false;
  return true;
}

function detectHtmlBlockStart(line) {
  let match = line.match(new RegExp(`^ {0,3}<(${RAW_HTML_TAGS})(?=[\\s>]|$)`, 'i'));
  if (match) {
    const closeRe = new RegExp(`</${match[1]}\\s*>`, 'i');
    return {
      kind: 'raw',
      closeRe,
      closedOnSameLine: closeRe.test(line.slice(match[0].length))
    };
  }
  if (/^ {0,3}<!--/.test(line)) {
    const open = line.indexOf('<!--');
    return {
      kind: 'comment',
      closeRe: /-->/,
      closedOnSameLine: line.indexOf('-->', open + 4) !== -1
    };
  }
  if (/^ {0,3}<\?/.test(line)) {
    return { kind: 'pi', closeRe: /\?>/, closedOnSameLine: /\?>/.test(line.slice(line.indexOf('<?') + 2)) };
  }
  if (/^ {0,3}<!\[CDATA\[/.test(line)) {
    return { kind: 'cdata', closeRe: /\]\]>/, closedOnSameLine: /\]\]>/.test(line.slice(line.indexOf('<![CDATA[') + 9)) };
  }
  if (/^ {0,3}<![A-Za-z]/.test(line)) {
    return { kind: 'declaration', closeRe: />/, closedOnSameLine: />/.test(line.slice(line.indexOf('<!') + 2)) };
  }
  match = line.match(new RegExp(`^ {0,3}</?(?:${HTML_BLOCK_TAGS})(?=[\\s/>]|$)`, 'i'));
  if (match) {
    return { kind: 'type6', closeRe: /^[ \t]*$/, closedOnSameLine: false, type6: true };
  }
  if (/^ {0,3}<\/?[A-Za-z][\w-]*(?:\s+[^>\n]*)?\/?>[ \t]*$/.test(line)) {
    return { kind: 'type7', closeRe: /^[ \t]*$/, closedOnSameLine: false, type6: true, type7: true };
  }
  return null;
}

function parseQuotePrefix(line) {
  const leading = (line.match(/^ {0,3}/) || [''])[0];
  let pos = leading.length;
  let depth = 0;
  while (line[pos] === '>') {
    depth++;
    pos++;
    if (line[pos] === ' ') pos++;
    if (line[pos] !== '>') break;
  }
  if (depth === 0) return null;
  return { depth, leading, rest: line.slice(pos), prefixLength: pos };
}

function parseQuoteFence(line) {
  const quote = parseQuotePrefix(line);
  if (!quote) return null;
  const match = quote.rest.match(/^( {0,3})(`{3,}|~{3,})(.*)$/);
  if (!match) return null;
  return {
    quoteDepth: quote.depth,
    char: match[2][0],
    len: match[2].length,
    info: match[3].trim(),
    rawInfo: match[3]
  };
}

// Strip blockquote, list, and task-list prefixes so math nested in containers
// is protected instead of being rewritten by the Markdown rules.
function parseMathOpen(line) {
  let rest = line;
  let quoteDepth = 0;
  let inList = false;
  for (let depth = 0; depth < 16; depth++) {
    const quote = parseQuotePrefix(rest);
    if (quote) {
      rest = quote.rest;
      quoteDepth += quote.depth;
      continue;
    }
    const list = rest.match(/^( {0,3})([-*+]|\d{1,9}[.)])([ \t]+)(.*)$/);
    if (list) {
      rest = list[4];
      inList = true;
      continue;
    }
    if (inList) {
      const task = rest.match(/^\[[ xX]\][ \t]+(.*)$/);
      if (task) {
        rest = task[1];
        continue;
      }
    }
    break;
  }
  const match = rest.match(/^( {0,3})\$\$(.*)$/);
  if (!match) return null;
  return { quoteDepth, closed: match[2].includes('$$') };
}

function isClosingFence(line, fence) {
  let rest = line;
  if (fence.quoteDepth > 0) {
    const quote = parseQuotePrefix(line);
    if (!quote || quote.depth < fence.quoteDepth) return false;
    rest = quote.rest;
  }
  const match = rest.match(/^ {0,3}(`{3,}|~{3,})[ \t]*$/);
  return Boolean(match && match[1][0] === fence.char && match[1].length >= fence.len);
}

// Count blockquote markers the way /^(?: {0,3}>[ \t]?)*/ consumes them greedily:
// up to three spaces, `>`, then one optional space or tab. Greedy consumption
// reaches every marker any backtracking path can reach, so `depth` is the
// maximum and `end` is the furthest position. Linear time.
function scanQuoteMarkers(line) {
  let pos = 0;
  let depth = 0;
  for (;;) {
    let p = pos;
    while (p - pos < 3 && line[p] === ' ') p++;
    if (line[p] !== '>') break;
    p++;
    if (line[p] === ' ' || line[p] === '\t') p++;
    pos = p;
    depth++;
  }
  return { depth, end: pos };
}

function isJsWhitespace(ch) {
  return ch !== undefined && /\s/.test(ch);
}

// Linear equivalent of
// /^(?: {0,3}>[ \t]?)* {0,3}\[([^\]\n]+)\]:[ \t]*(?:<[^>\n]*>|\S+)/,
// which backtracks exponentially on deep quote prefixes in v1.0.0. Returns the
// label (group 1) and its start index, or null.
function matchDefinition(line) {
  let pos = scanQuoteMarkers(line).end;
  const indentStart = pos;
  while (pos - indentStart < 3 && line[pos] === ' ') pos++;
  if (line[pos] !== '[') return null;
  const labelStart = pos + 1;
  let close = labelStart;
  while (close < line.length && line[close] !== ']' && line[close] !== '\n') close++;
  if (close === labelStart || line[close] !== ']' || line[close + 1] !== ':') return null;
  let dest = close + 2;
  while (line[dest] === ' ' || line[dest] === '\t') dest++;
  if (dest >= line.length || isJsWhitespace(line[dest])) return null;
  return { label: line.slice(labelStart, close), index: labelStart };
}

function resourceLimitMessage(depth, limit) {
  return `Blockquote nesting depth ${depth} exceeds the limit of ${limit}. Reduce the nesting, raise --max-quote-depth (0 = no limit), or use --on-limit ignore to leave such lines untouched.`;
}

function scanDocument(text, limits = createLimits()) {
  const lines = text.split('\n');
  const info = lines.map((line, index) => ({
    index,
    line,
    blank: isBlank(line),
    frontmatter: false,
    fence: false,
    fenceOpen: false,
    fenceClose: false,
    fenceContent: false,
    fenceInfo: '',
    fenceChar: '',
    fenceLen: 0,
    htmlBlock: false,
    htmlStart: false,
    htmlKind: '',
    indentedCode: false,
    definition: false,
    mathBlock: false,
    mathOpen: false,
    mathClose: false,
    opaque: false,
    keepWhitespace: false
  }));
  const issues = [];
  let frontmatterEnd = -1;

  const firstLine = lines.length > 0 ? lines[0].replace(/^\uFEFF/, '') : '';
  if (/^---[ \t]*$/.test(firstLine)) {
    let close = -1;
    for (let i = 1; i < lines.length; i++) {
      if (/^(?:---|\.\.\.)[ \t]*$/.test(lines[i])) {
        close = i;
        break;
      }
    }
    if (close !== -1) {
      frontmatterEnd = close;
      for (let i = 0; i <= close; i++) info[i].frontmatter = true;
    } else {
      const firstContent = lines.slice(1, 12).find((line) => line.trim() !== '');
      if (firstContent && /^[A-Za-z0-9_.-]+[ \t]*:/.test(firstContent)) {
        issues.push(makeProblem('MDX081', 1, 1, 'Unclosed YAML frontmatter: no closing --- or ... line was found.'));
        let end = lines.length - 1;
        for (let i = 1; i < lines.length; i++) {
          if (lines[i].trim() === '') {
            end = i - 1;
            break;
          }
        }
        for (let i = 0; i <= end; i++) info[i].frontmatter = true;
      }
    }
  }

  let fence = null;
  let html = null;
  let math = null;
  let inIndented = false;

  const nextNonBlank = function (start) {
    for (let i = start + 1; i < lines.length; i++) {
      if (!isBlank(lines[i])) return i;
    }
    return -1;
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const lineInfo = info[i];
    if (lineInfo.frontmatter) continue;

    if (fence) {
      lineInfo.fence = true;
      lineInfo.fenceContent = true;
      if (isClosingFence(line, fence)) {
        lineInfo.fenceClose = true;
        lineInfo.fenceContent = false;
        fence = null;
      }
      continue;
    }

    if (html) {
      lineInfo.htmlBlock = true;
      if (html.closeRe.test(line)) {
        html = null;
      }
      continue;
    }

    if (math) {
      // A math block ends at a line containing $$ at the same blockquote depth.
      let rest = line;
      if (math.quoteDepth > 0) {
        const quote = parseQuotePrefix(line);
        if (!quote || quote.depth < math.quoteDepth) {
          issues.push(makeProblem('MDX085', math.start + 1, 1, 'Unclosed $$ math block: the blockquote ended before a closing $$ was found.'));
          math = null;
        } else {
          rest = quote.rest;
          lineInfo.mathBlock = true;
          if (rest.includes('$$')) {
            lineInfo.mathClose = true;
            math = null;
          }
          continue;
        }
      } else {
        lineInfo.mathBlock = true;
        if (rest.includes('$$')) {
          lineInfo.mathClose = true;
          math = null;
        }
        continue;
      }
    }

    if (lineInfo.blank) {
      if (inIndented) {
        const next = nextNonBlank(i);
        if (next !== -1 && isIndentedCodeLine(lines[next])) {
          lineInfo.indentedCode = true;
        } else {
          inIndented = false;
        }
      }
      continue;
    }

    if (inIndented) {
      if (isIndentedCodeLine(line)) {
        lineInfo.indentedCode = true;
        continue;
      }
      inIndented = false;
    }

    // Lines nested deeper than maxQuoteDepth are opaque: never a block start or
    // definition, never modified, never diagnosed. In 'error' mode MDX086 also
    // blocks fixDocument.
    if (limits.maxQuoteDepth > 0) {
      const depth = scanQuoteMarkers(line).depth;
      if (depth > limits.maxQuoteDepth) {
        lineInfo.opaque = true;
        if (limits.onLimit !== 'ignore') {
          issues.push(makeProblem('MDX086', i + 1, 1, resourceLimitMessage(depth, limits.maxQuoteDepth)));
        }
        continue;
      }
    }

    const previousInfo = i > 0 ? info[i - 1] : null;
    const previousIsIndentedContinuation = i > 0 &&
      isIndentedCodeLine(lines[i - 1]) &&
      !previousInfo.indentedCode;
    const canStartIndentedCode = i === 0 ||
      previousInfo.blank ||
      previousInfo.indentedCode ||
      (!previousIsIndentedContinuation && !isParagraphLike(lines[i - 1], previousInfo));
    if (isIndentedCodeLine(line) && canStartIndentedCode) {
      inIndented = true;
      lineInfo.indentedCode = true;
      continue;
    }

    const quoteCode = parseQuotePrefix(line);
    if (quoteCode && isIndentedCodeLine(quoteCode.rest)) {
      lineInfo.indentedCode = true;
      continue;
    }

    const mathOpen = parseMathOpen(line);
    if (mathOpen) {
      lineInfo.mathBlock = true;
      if (!mathOpen.closed) {
        lineInfo.mathOpen = true;
        math = { start: i, quoteDepth: mathOpen.quoteDepth };
      }
      continue;
    }

    const fenceMatch = line.match(/^( {0,3})(`{3,}|~{3,})(.*)$/);
    if (fenceMatch) {
      const char = fenceMatch[2][0];
      const rawInfo = fenceMatch[3];
      if (char === '`' && rawInfo.includes('`')) {
        issues.push(makeProblem('MDX053', i + 1, 1, 'Invalid fenced-code opener: a backtick fence info string cannot contain backticks.'));
      } else {
        lineInfo.fence = true;
        lineInfo.fenceOpen = true;
        lineInfo.fenceInfo = rawInfo.trim();
        lineInfo.fenceChar = char;
        lineInfo.fenceLen = fenceMatch[2].length;
        fence = { char, len: fenceMatch[2].length, start: i, quoteDepth: 0 };
        continue;
      }
    }

    const quoteFence = parseQuoteFence(line);
    if (quoteFence) {
      if (quoteFence.char === '`' && quoteFence.rawInfo.includes('`')) {
        issues.push(makeProblem('MDX053', i + 1, 1, 'Invalid fenced-code opener: a backtick fence info string cannot contain backticks.'));
      } else {
        lineInfo.fence = true;
        lineInfo.fenceOpen = true;
        lineInfo.fenceInfo = quoteFence.info;
        lineInfo.fenceChar = quoteFence.char;
        lineInfo.fenceLen = quoteFence.len;
        fence = {
          char: quoteFence.char,
          len: quoteFence.len,
          start: i,
          quoteDepth: quoteFence.quoteDepth
        };
        continue;
      }
    }

    const htmlStart = detectHtmlBlockStart(line);
    if (htmlStart && (!htmlStart.type7 || i === 0 || info[i - 1].blank)) {
      lineInfo.htmlBlock = true;
      lineInfo.htmlStart = true;
      lineInfo.htmlKind = htmlStart.kind;
      if (!htmlStart.closedOnSameLine) {
        html = { ...htmlStart, start: i };
      }
      continue;
    }

    // An HTML block that ends at a marker (raw <pre>/<script>/<style>, comment,
    // <?...?>, <!...>, CDATA) opened after blockquote or list markers keeps its
    // content, blank lines included, verbatim. Like $$ math, protect it until
    // the marker; only an unclosed comment blocks fixing, as at the top level.
    // Blocks that end at a blank line (types 6 and 7) are left to the line rules.
    const nestedStart = containerPrefix(line).end;
    const nestedHtml = nestedStart > 0 ? detectHtmlBlockStart(line.slice(nestedStart)) : null;
    if (nestedHtml && !nestedHtml.type6 && !nestedHtml.closedOnSameLine) {
      lineInfo.htmlBlock = true;
      lineInfo.htmlStart = true;
      lineInfo.htmlKind = nestedHtml.kind;
      html = { ...nestedHtml, start: i };
      continue;
    }

    if (matchDefinition(line)) {
      lineInfo.definition = true;
    }
  }

  markSignificantWhitespace(lines, info);

  if (fence) {
    issues.push(makeProblem('MDX051', fence.start + 1, 1, `Unclosed ${fence.char === '`' ? 'backtick' : 'tilde'} code fence.`));
  }
  if (math) {
    issues.push(makeProblem('MDX085', math.start + 1, 1, 'Unclosed $$ math block: no closing $$ was found.'));
  }
  if (html && html.kind === 'comment') {
    issues.push(makeProblem('MDX083', html.start + 1, 1, 'Unclosed HTML comment: no closing --> was found.'));
  }

  return { lines, info, issues, frontmatterEnd };
}

// GitHub (cmark-gfm) treats an indented whitespace-only line as content after
// an empty list item marker and inside a footnote definition: `- \n  \n  x`
// is one item `x` there, but two blocks once the middle line is cleared. Mark
// such lines (also `>   ` inside a blockquote) so whitespace and blank-line
// rules leave them as they are.
function markSignificantWhitespace(lines, info) {
  let context = null;
  for (let i = 0; i < lines.length; i++) {
    const lineInfo = info[i];
    if (lineInfo.frontmatter || lineInfo.fenceContent || lineInfo.htmlBlock || lineInfo.mathBlock || lineInfo.opaque) {
      context = null;
      continue;
    }
    const line = lines[i];
    const quoteEnd = scanQuoteMarkers(line).end;
    const rest = line.slice(quoteEnd);
    if (isBlank(rest)) {
      if (context !== null && rest.length > 0) lineInfo.keepWhitespace = true;
      continue;
    }
    if (/^ {0,3}\[\^[^\]\n]+\]:/.test(rest)) {
      context = 'footnote';
    } else if (containerPrefix(line).end === line.length && !isThematicBreak(rest)) {
      context = 'item';
    } else if (!(context === 'footnote' && /^[ \t]/.test(rest))) {
      context = null;
    }
  }
}

function isProtectedLine(lineInfo) {
  return Boolean(
    lineInfo.frontmatter ||
    lineInfo.fenceContent ||
    lineInfo.htmlBlock ||
    lineInfo.indentedCode ||
    lineInfo.mathBlock ||
    lineInfo.definition ||
    lineInfo.opaque
  );
}

function buildMask(line, options = {}) {
  const maskBareUrls = options.maskBareUrls !== false;
  const maskLinkDestinations = options.maskLinkDestinations !== false;
  const maskReferenceLabels = options.maskReferenceLabels !== false;
  const maskLinkText = options.maskLinkText === true;
  const mask = new Array(line.length).fill(false);

  const mark = function (start, end) {
    const from = Math.max(0, start);
    const to = Math.min(line.length, end);
    for (let i = from; i < to; i++) mask[i] = true;
  };

  for (let i = 0; i < line.length - 1; i++) {
    if (line[i] === '\\' && !mask[i]) {
      mark(i, i + 2);
      i++;
    }
  }

  let cursor = 0;
  while (cursor < line.length) {
    if (line[cursor] !== '`' || mask[cursor]) {
      cursor++;
      continue;
    }
    let run = 1;
    while (line[cursor + run] === '`') run++;
    let scan = cursor + run;
    let close = -1;
    while (scan < line.length) {
      if (line[scan] === '`') {
        let closeRun = 1;
        while (line[scan + closeRun] === '`') closeRun++;
        if (closeRun === run) {
          close = scan;
          break;
        }
        scan += closeRun;
      } else {
        scan++;
      }
    }
    if (close !== -1) {
      mark(cursor, close + run);
      cursor = close + run;
    } else {
      cursor += run;
    }
  }

  const markRegex = function (regex) {
    regex.lastIndex = 0;
    let match;
    while ((match = regex.exec(line)) !== null) {
      if (!mask[match.index]) {
        mark(match.index, match.index + match[0].length);
      }
      if (match[0].length === 0) regex.lastIndex++;
    }
  };

  markRegex(/\$\$[\s\S]+?\$\$/g);
  markRegex(/\$(?![\s$])[^$\n]*?(?<![\s\\])\$/g);

  if (maskLinkDestinations) {
    for (let i = 0; i < line.length - 1; i++) {
      if (line[i] !== ']' || line[i + 1] !== '(' || mask[i]) continue;
      let depth = 1;
      let scan = i + 2;
      let inAngle = false;
      let quote = '';
      while (scan < line.length) {
        const ch = line[scan];
        if (quote) {
          if (ch === quote && line[scan - 1] !== '\\') quote = '';
        } else if (inAngle) {
          if (ch === '>') inAngle = false;
        } else if (ch === '<') {
          inAngle = true;
        } else if (ch === '"' || ch === "'") {
          quote = ch;
        } else if (ch === '(') {
          depth++;
        } else if (ch === ')') {
          depth--;
          if (depth === 0) break;
        }
        scan++;
      }
      if (depth === 0 && scan < line.length) {
        mark(i + 1, scan + 1);
        i = scan;
      }
    }
  }

  if (maskReferenceLabels) {
    markRegex(/\]\[[^\]\n]*\]/g);
  }
  if (maskLinkText) {
    markRegex(/!?\[[^\]\n]*\]\(/g);
    markRegex(/!?\[[^\]\n]*\]\[[^\]\n]*\]/g);
  }
  markRegex(/<[^>\n]{1,500}>/g);

  if (maskBareUrls) {
    markRegex(/\b(?:https?:\/\/|ftp:\/\/|www\.)(?:[^\s<>"']|\u00A0)+/gi);
    markRegex(/\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi);
  }

  return mask;
}

function splitMasked(line, mask) {
  const segments = [];
  let i = 0;
  while (i < line.length) {
    const prot = mask[i];
    let j = i + 1;
    while (j < line.length && mask[j] === prot) j++;
    segments.push({ text: line.slice(i, j), start: i, protected: prot });
    i = j;
  }
  return segments;
}

function replaceWithOffsets(text, regex, replacer) {
  let output = '';
  let last = 0;
  let match;
  regex.lastIndex = 0;
  while ((match = regex.exec(text)) !== null) {
    const start = match.index;
    output += text.slice(last, start);
    output += replacer(match, start);
    last = start + match[0].length;
    if (match[0].length === 0) regex.lastIndex++;
  }
  output += text.slice(last);
  return output;
}

function fixHeading(line, isEnabled = () => true) {
  if (!/^ {0,3}#/.test(line)) return null;
  const indent = line.match(/^ */)[0];
  const afterIndent = line.slice(indent.length);
  const hashes = afterIndent.match(/^#+/)[0];
  if (hashes.length > 6) return null;
  if (afterIndent[hashes.length] === '#') return null;

  const rest = afterIndent.slice(hashes.length);
  if (rest !== '' && !/^[ \t]/.test(rest)) {
    if (!isEnabled('MDX010')) return null;
    return {
      line: `${indent}${hashes} ${rest}`,
      changes: [{
        rule: 'MDX010',
        column: indent.length + hashes.length + 1,
        message: `Add a space after the heading marker: "${hashes} " rather than "${hashes}".`
      }]
    };
  }

  const gapMatch = rest.match(/^[ \t]*/);
  const originalGap = gapMatch ? gapMatch[0] : '';
  let gap = originalGap;
  let content = rest.slice(originalGap.length);
  const changes = [];

  if (gap.length > 1 && content !== '' && isEnabled('MDX011')) {
    changes.push({
      rule: 'MDX011',
      column: indent.length + hashes.length + 1,
      message: 'Use exactly one space after the ATX heading marker.'
    });
    gap = ' ';
  }

  const closing = content.match(/^(.*?)[ \t]+(#+)[ \t]*$/);
  if (closing && closing[1].trim() !== '' && isEnabled('MDX012')) {
    changes.push({
      rule: 'MDX012',
      column: indent.length + hashes.length + originalGap.length + closing[1].length + 1,
      message: 'Remove optional closing hashes from the heading; they add noise and are easy to miscount.'
    });
    content = closing[1].replace(/[ \t]+$/, '');
  }

  if (content.trim() === '') {
    if (isEnabled('MDX011') && gap.length > 1) gap = '';
    return { line: `${indent}${hashes}${gap}`, changes };
  }

  if (gap === '') gap = ' ';
  return { line: `${indent}${hashes}${gap}${content}`, changes };
}

// Blocks whose meaning depends on their indentation inside a blockquote.
function startsIndentSensitiveBlock(content) {
  return /^(?:`{3,}|~{3,}|(?:[-*+]|\d{1,9}[.)])(?:[ \t]|$))/.test(content);
}

function fixBlockquote(line, isEnabled = () => true) {
  if (!isEnabled('MDX020')) return null;
  const quote = parseQuotePrefix(line);
  if (!quote) return null;

  // parseQuotePrefix stops at `>  >`; one extra space before another marker is
  // still the same nesting, so take the whole chain in one pass. Otherwise each
  // fixDocument pass would merge only one level and the result would not be
  // idempotent.
  let depth = quote.depth;
  let content = quote.rest;
  while (content[0] === ' ' && content[1] === '>') {
    const inner = parseQuotePrefix(content.slice(1));
    depth += inner.depth;
    content = inner.rest;
  }
  const contentGap = content.match(/^[ \t]*/)[0];
  if (contentGap.includes('\t')) {
    return { line, changes: [] };
  }
  // Spaces after the marker's own space are significant when two or more of
  // them could continue a list item, and for fences (they set how much
  // indentation is removed from the code) and list markers (they set the item's
  // content column). Only a single space before other content, or up to three
  // spaces with nothing after them, are removed.
  const blankContent = contentGap.length === content.length;
  if ((blankContent && contentGap.length <= 3) ||
    (contentGap.length === 1 && !startsIndentSensitiveBlock(content.slice(1)))) {
    content = content.slice(contentGap.length);
  }

  const markerPrefix = new Array(depth).fill('>').join(' ');
  const fixed = content === ''
    ? `${quote.leading}${markerPrefix}`
    : `${quote.leading}${markerPrefix} ${content}`;

  if (fixed === line) {
    return { line, changes: [] };
  }

  return {
    line: fixed,
    changes: [{
      rule: 'MDX020',
      column: quote.leading.length + 1,
      message: 'Put exactly one space after each blockquote marker; preserve four or more spaces only when they are content indentation.'
    }]
  };
}

function fixListMarker(line, isEnabled = () => true) {
  if (!isEnabled('MDX030')) return null;
  if (isThematicBreak(line)) return null;

  let match = line.match(/^( {0,3})([-+])(\S.*)$/);
  if (match) {
    if (match[2] === match[3][0] || /[0-9]/.test(match[3][0])) return null;
    return {
      line: `${match[1]}${match[2]} ${match[3]}`,
      changes: [{
        rule: 'MDX030',
        column: match[1].length + 2,
        message: `Add a space after the "${match[2]}" list marker.`
      }]
    };
  }

  match = line.match(/^( {0,3})(\*)(\S.*)$/);
  if (match) {
    const rest = match[3];
    if (/(?<!\\)\*/.test(rest)) {
      return null;
    }
    return {
      line: `${match[1]}* ${rest}`,
      changes: [{
        rule: 'MDX030',
        column: match[1].length + 2,
        message: 'Add a space after the "*" list marker.'
      }]
    };
  }

  match = line.match(/^( {0,3})(\d{1,9})([.)])([^\d\s].*)$/);
  if (match) {
    if (/^[A-Za-z]$/.test(match[4])) return null;
    return {
      line: `${match[1]}${match[2]}${match[3]} ${match[4]}`,
      changes: [{
        rule: 'MDX030',
        column: match[1].length + match[2].length + 2,
        message: `Add a space after the "${match[2]}${match[3]}" list marker.`
      }]
    };
  }

  return null;
}

// CommonMark treats Unicode whitespace and Unicode punctuation (general
// categories P and S) specially when deciding whether a delimiter run may
// open or close emphasis. JS \w is ASCII-only, so these helpers are what the
// emphasis rules must use instead of word-character lookbehinds.
const UNICODE_PUNCT_RE = /[\p{P}\p{S}]/u;
const CJK_ADJACENT_RE = /[\u2E80-\u2FFF\u3000-\u303F\u3040-\u30FF\u31C0-\u31EF\u3200-\u33FF\u3400-\u4DBF\u4E00-\u9FFF\uF900-\uFAFF\uFE30-\uFE4F\uFF00-\uFFEF]/u;
// Trailing sentence punctuation that can safely move outside a closing
// delimiter without changing meaning. Brackets and quotes stay inside.
const MOVABLE_PUNCT_RE = /[、。，．：；！？…‥]/u;
// Matching full-width quote and bracket pairs, keyed by the closing mark.
// When both marks wrap a leftover emphasis span (**「X」**) they can both move
// outside the delimiters (「**X**」) without changing meaning; one-sided marks
// stay inside.
const QUOTE_OPEN_BY_CLOSE = new Map([
  ['」', '「'], ['』', '『'], ['》', '《'], ['〉', '〈'],
  ['】', '【'], ['〕', '〔'], ['）', '（']
]);

function isBoundaryOrWhitespace(ch) {
  return ch === null || ch === undefined || /\s/u.test(ch);
}

function isUnicodePunct(ch) {
  return !isBoundaryOrWhitespace(ch) && UNICODE_PUNCT_RE.test(ch);
}

// CommonMark flanking rules. `before`/`after` are the single characters
// touching the delimiter run; null stands for the start or end of the inline
// context, which counts as whitespace.
function delimiterFlanking(before, after) {
  const beforeWs = isBoundaryOrWhitespace(before);
  const afterWs = isBoundaryOrWhitespace(after);
  const beforePunct = isUnicodePunct(before);
  const afterPunct = isUnicodePunct(after);
  return {
    leftFlanking: !afterWs && (!afterPunct || beforeWs || beforePunct),
    rightFlanking: !beforeWs && (!beforePunct || afterWs || afterPunct)
  };
}

// Blockquote and list markers at the start of a line: `end` is where the
// content starts, `stars` holds the indices of `*` bullet markers.
function containerPrefix(line) {
  const stars = new Set();
  let pos = scanQuoteMarkers(line).end;
  for (;;) {
    const match = line.slice(pos).match(/^[ \t]*([-*+]|\d{1,9}[.)])(?:[ \t]+|$)/);
    if (!match) break;
    const markerStart = pos + match[0].indexOf(match[1]);
    if (match[1] === '*') stars.add(markerStart);
    pos += match[0].length;
  }
  return { end: pos, stars };
}

function fixEmphasis(line, lineNo, isEnabled = () => true) {
  if (!isEnabled('MDX040')) return { line, changes: [] };
  // A bullet `*` is not an emphasis delimiter; trimming the space after it
  // would turn the list item into a paragraph.
  const markerStars = containerPrefix(line).stars;
  const mask = buildMask(line, { maskBareUrls: false });
  const segments = splitMasked(line, mask);
  const changes = [];
  let output = '';

  for (const segment of segments) {
    if (segment.protected) {
      output += segment.text;
      continue;
    }

    let text = segment.text;
    // A trimmed pair must be one emphasis span in the making. The opening
    // delimiter run has to be left-flanking once the inner whitespace is
    // removed, and it must not already look like the closer of a preceding
    // emphasis (mirrored for the closing run). Without this, a run such as
    // the 。** that closes one bold gets reused as the opener of the next
    // match and the rewrite nests the two bold spans into each other.
    const charBefore = function (start) {
      if (start > 0) return text[start - 1];
      if (segment.start > 0) return line[segment.start - 1];
      return null;
    };
    const charAfter = function (end) {
      if (end < text.length) return text[end];
      const segmentEnd = segment.start + segment.text.length;
      if (segmentEnd < line.length) return line[segmentEnd];
      return null;
    };
    const trimPair = function (value, regex, delimiter, wsSide, message) {
      const contentIndex = wsSide === 'after' ? 2 : 1;
      const wsIndex = wsSide === 'after' ? 1 : 2;
      return replaceWithOffsets(value, regex, (match, start) => {
        const size = delimiter.length;
        const closeStart = start + match[0].length - size;
        const content = match[contentIndex].trim();
        const openerBefore = charBefore(start);
        const closerAfter = charAfter(closeStart + size);
        const openerOriginal = delimiterFlanking(openerBefore, match[wsIndex][0]);
        const openerRepaired = delimiterFlanking(openerBefore, content[0]);
        const closerOriginal = delimiterFlanking(match[wsIndex][match[wsIndex].length - 1], closerAfter);
        const closerRepaired = delimiterFlanking(content[content.length - 1], closerAfter);
        const allowed = !markerStars.has(segment.start + start) &&
          openerRepaired.leftFlanking && closerRepaired.rightFlanking &&
          (wsSide === 'after' ? !openerOriginal.rightFlanking : !closerOriginal.leftFlanking);
        if (!allowed) return match[0];
        changes.push({
          rule: 'MDX040',
          column: segment.start + start + 1,
          message
        });
        return `${delimiter}${content}${delimiter}`;
      });
    };
    text = trimPair(
      text,
      /(?<![\w*])\*\*([ \t]+)([^*\n]*?\S)([ \t]*)\*\*(?!\*)/g,
      '**',
      'after',
      'Remove spaces just inside the bold (**) delimiters.'
    );
    text = trimPair(
      text,
      /(?<![\w*])\*\*(\S[^*\n]*?)([ \t]+)\*\*(?!\*)/g,
      '**',
      'before',
      'Remove spaces just inside the bold (**) delimiters.'
    );
    text = trimPair(
      text,
      /(?<![\w_])__([ \t]+)([^_\n]*?\S)([ \t]*)__(?![\w_])/g,
      '__',
      'after',
      'Remove spaces just inside the bold (__) delimiters.'
    );
    text = trimPair(
      text,
      /(?<![\w_])__(\S[^_\n]*?)([ \t]+)__(?![\w_])/g,
      '__',
      'before',
      'Remove spaces just inside the bold (__) delimiters.'
    );
    text = trimPair(
      text,
      /(?<![*\w])\*([ \t]+)([^*\n]*?\S)\*(?![*\w])/g,
      '*',
      'after',
      'Remove the space just inside the italic (*) delimiter. Single-star spacing is fixed only when the other side is attached.'
    );
    text = trimPair(
      text,
      /(?<![*\w])\*(\S[^*\n]*?)([ \t]+)\*(?![*\w])/g,
      '*',
      'before',
      'Remove the space just inside the italic (*) delimiter. Single-star spacing is fixed only when the other side is attached.'
    );
    text = trimPair(
      text,
      /(?<![\w_])_([ \t]+)([^_\n]*?\S)_(?![\w_])/g,
      '_',
      'after',
      'Remove the space just inside the italic (_) delimiter. Single-underscore spacing is fixed only when the other side is attached.'
    );
    text = trimPair(
      text,
      /(?<![\w_])_(\S[^_\n]*?)([ \t]+)_(?![\w_])/g,
      '_',
      'before',
      'Remove the space just inside the italic (_) delimiter. Single-underscore spacing is fixed only when the other side is attached.'
    );
    output += text;
  }

  return { line: output, changes };
}

function fixNbsp(line, isEnabled = () => true) {
  if (!isEnabled('MDX008')) return { line, changes: [] };
  const mask = buildMask(line);
  const segments = splitMasked(line, mask);
  const changes = [];
  // NBSP is not whitespace for a list marker, so `*<NBSP>text` is a paragraph.
  // A normal space there would turn it into a list item: report, do not fix.
  const prefixEnd = containerPrefix(line).end;
  const marker = line.slice(prefixEnd).match(/^[ \t]*(?:[-*+]|\d{1,9}[.)])(\u00A0+)/);
  const keepEnd = marker ? prefixEnd + marker[0].length : -1;
  const keepStart = marker ? keepEnd - marker[1].length : -1;
  let output = '';
  for (const segment of segments) {
    if (segment.protected) {
      output += segment.text;
      continue;
    }
    let first = -1;
    let text = '';
    for (let i = 0; i < segment.text.length; i++) {
      const ch = segment.text[i];
      const index = segment.start + i;
      if (ch !== '\u00A0') {
        text += ch;
      } else if (index >= keepStart && index < keepEnd) {
        if (index === keepStart) {
          changes.push({
            rule: 'MDX008',
            column: index + 1,
            message: 'NBSP right after a list marker. Replacing it with a space would turn this paragraph into a list item; use a normal space if a list was intended.',
            overrides: { fixable: false }
          });
        }
        text += ch;
      } else {
        if (first === -1) first = i;
        text += ' ';
      }
    }
    if (first !== -1) {
      changes.push({
        rule: 'MDX008',
        column: segment.start + first + 1,
        message: 'Replace non-breaking spaces (U+00A0) with normal spaces; they usually come from Word or a web page paste.'
      });
    }
    output += text;
  }
  // An NBSP that is the only content of a line or list item keeps it from
  // being blank; a normal space would end the paragraph or empty the item.
  if (output !== line && isBlank(output.slice(containerPrefix(line).end))) {
    return { line, changes: [] };
  }
  return { line: output, changes };
}

function transformLine(line, lineInfo, lineNo, isEnabled) {
  const issues = [];
  if (isProtectedLine(lineInfo) || lineInfo.keepWhitespace) {
    return { line, issues };
  }

  let current = line;
  const steps = [
    (value) => fixHeading(value, isEnabled),
    (value) => fixBlockquote(value, isEnabled),
    (value) => fixListMarker(value, isEnabled),
    (value) => fixEmphasis(value, lineNo, isEnabled),
    (value) => fixNbsp(value, isEnabled)
  ];

  for (const step of steps) {
    const result = step(current);
    if (!result) continue;
    for (const change of result.changes) {
      if (isEnabled(change.rule)) {
        issues.push(makeProblem(change.rule, lineNo, change.column, change.message, change.overrides));
      }
    }
    if (result.line !== undefined) {
      current = result.line;
    }
  }

  return { line: current, issues };
}

function splitTableCells(line) {
  let value = line.trim();
  if (value.startsWith('|')) value = value.slice(1);
  if (value.endsWith('|') && !value.endsWith('\\|')) value = value.slice(0, -1);

  const cells = [];
  let current = '';
  let codeRun = 0;
  for (let i = 0; i < value.length; i++) {
    const ch = value[i];
    if (ch === '\\' && i + 1 < value.length) {
      current += ch + value[i + 1];
      i++;
      continue;
    }
    if (ch === '`') {
      let run = 1;
      while (value[i + run] === '`') run++;
      if (codeRun === 0) codeRun = run;
      else if (codeRun === run) codeRun = 0;
      current += '`'.repeat(run);
      i += run - 1;
      continue;
    }
    if (ch === '|' && codeRun === 0) {
      cells.push(current.trim());
      current = '';
      continue;
    }
    current += ch;
  }
  cells.push(current.trim());
  return cells;
}

// GFM splits cells on every pipe that is not backslash-escaped, including
// pipes inside code spans; a header and delimiter row with different counts
// are not a table at all.
function gfmCellCount(line) {
  let value = line.trim();
  if (value.startsWith('|')) value = value.slice(1);
  if (value.endsWith('|') && !value.endsWith('\\|')) value = value.slice(0, -1);
  let count = 1;
  for (let i = 0; i < value.length; i++) {
    if (value[i] === '\\') i++;
    else if (value[i] === '|') count++;
  }
  return count;
}

function isGfmTable(lines, table) {
  return gfmCellCount(lines[table.start]) === gfmCellCount(lines[table.start + 1]);
}

function isTableDelimiterRow(line) {
  if (!line.includes('|')) return false;
  const cells = splitTableCells(line);
  return cells.length > 0 && cells.every((cell) => /^:?-+:?$/.test(cell));
}

function isTableHeaderCandidate(line) {
  if (line.length === 0) return false;
  if (isIndentedCodeLine(line)) return false;
  const content = line.replace(/^ {0,3}/, '');
  if (content.startsWith('>')) return false;
  if (isAtxHeading(content) || isListLine(content) || isThematicBreak(content) || content.startsWith('```') || content.startsWith('~~~')) return false;
  return content.includes('|');
}

function isTableRow(line, lineInfo) {
  if (!lineInfo || isProtectedLine(lineInfo)) return false;
  if (isBlank(line) || isIndentedCodeLine(line)) return false;
  const content = line.replace(/^ {0,3}/, '');
  if (content.startsWith('>')) return false;
  if (isAtxHeading(content) || isListLine(content) || isThematicBreak(content)) return false;
  if (content.startsWith('```') || content.startsWith('~~~')) return false;
  return content.includes('|');
}

function findTables(lines, info) {
  const tables = [];
  for (let i = 0; i + 1 < lines.length; i++) {
    if (isProtectedLine(info[i]) || isProtectedLine(info[i + 1])) continue;
    if (!isTableHeaderCandidate(lines[i])) continue;
    if (!isTableDelimiterRow(lines[i + 1])) continue;
    const columns = splitTableCells(lines[i + 1]).length;
    let end = i + 1;
    while (end + 1 < lines.length && isTableRow(lines[end + 1], info[end + 1])) {
      end++;
    }
    tables.push({ start: i, end, columns });
    i = end;
  }
  return tables;
}

function fixTableDelimiter(line, isEnabled = () => true) {
  if (!isEnabled('MDX060')) return null;
  if (!line.includes('|')) return null;
  const quote = parseQuotePrefix(line);
  if (!isTableDelimiterRow(quote ? quote.rest : line)) return null;
  const parts = line.split('|');
  let changed = false;
  let firstColumn = 1;
  for (let i = 0; i < parts.length; i++) {
    const match = parts[i].match(/^([ \t]*)(:?)(-{1,2})(:?)([ \t]*)$/);
    if (!match) continue;
    if (!changed) {
      const offset = parts.slice(0, i).join('|').length + (i > 0 ? 1 : 0);
      firstColumn = offset + match[1].length + 1;
    }
    const hyphens = match[3];
    parts[i] = `${match[1]}${match[2]}${'-'.repeat(Math.max(0, 3 - hyphens.length))}${hyphens}${match[4]}${match[5]}`;
    changed = true;
  }
  if (!changed) return null;
  return {
    line: parts.join('|'),
    changes: [{
      rule: 'MDX060',
      column: firstColumn,
      message: 'Each table delimiter cell needs at least three hyphens.'
    }]
  };
}

function normalizeLabel(label) {
  return label.trim().replace(/\s+/g, ' ').toLowerCase();
}

function computeHeadingStructure(lines, info, isEnabled) {
  const issues = [];
  const headings = [];
  for (let i = 0; i < lines.length; i++) {
    if (isProtectedLine(info[i])) continue;
    if (/^( {0,3})#{1,6}[ \t]*$/.test(lines[i])) {
      if (isEnabled('MDX016')) {
        issues.push(makeProblem('MDX016', i + 1, 1, 'Heading has no text. Remove it or give it a title.'));
      }
      continue;
    }
    const match = lines[i].match(/^( {0,3})(#{1,6})[ \t]+(.*?)[ \t]*$/);
    if (!match) continue;
    const indent = match[1].length;
    const level = match[2].length;
    let text = match[3].replace(/[ \t]+#+[ \t]*$/, '').trim();

    if (indent > 0 && isEnabled('MDX014')) {
      issues.push(makeProblem('MDX014', i + 1, 1, 'Heading is indented. Confirm it is not a top-level heading with accidental indentation; inside a list item the indent may be intentional.', { fixable: false }));
    }
    if (text === '') {
      if (isEnabled('MDX016')) {
        issues.push(makeProblem('MDX016', i + 1, 1, 'Heading has no text. Remove it or give it a title.'));
      }
      continue;
    }
    if (/[.:;!?。：；！？]$/.test(text) && isEnabled('MDX017')) {
      issues.push(makeProblem('MDX017', i + 1, 1, 'Heading ends with sentence punctuation. Headings usually read better without a trailing period, colon, or similar mark.', { severity: 'info' }));
    }
    if (indent === 0) {
      headings.push({ level, line: i + 1, text });
    }
  }

  if (headings.length > 0 && isEnabled('MDX018') && headings[0].level !== 1) {
    issues.push(makeProblem('MDX018', headings[0].line, 1, 'The first heading is not an H1. Start the outline with one H1 unless the document intentionally starts in a section.', { severity: 'info' }));
  }

  const h1s = headings.filter((heading) => heading.level === 1);
  if (h1s.length > 1 && isEnabled('MDX019')) {
    for (const heading of h1s.slice(1)) {
      issues.push(makeProblem('MDX019', heading.line, 1, 'Document has more than one H1. Use H2 for subsections and keep one H1 for the page title.'));
    }
  }

  if (isEnabled('MDX015')) {
    let previous = null;
    for (const heading of headings) {
      if (previous && heading.level > previous.level + 1) {
        issues.push(makeProblem('MDX015', heading.line, 1, `Heading jumps from H${previous.level} to H${heading.level}. Use the next level instead of skipping one.`));
      }
      previous = heading;
    }
  }

  return issues;
}

function computeListStructure(lines, info, isEnabled) {
  const issues = [];
  const runs = new Map();

  for (let i = 0; i < lines.length; i++) {
    if (isProtectedLine(info[i])) {
      if (isTopLevelLine(lines[i]) && !info[i].fenceContent && !info[i].indentedCode) {
        runs.clear();
      }
      continue;
    }
    if (isThematicBreak(lines[i])) {
      runs.clear();
      continue;
    }
    const match = lines[i].match(/^( {0,3})([-*+])([ \t]+)(.*)$/);
    if (!match) {
      if (lines[i].trim() !== '' && !/^[ \t]/.test(lines[i])) runs.clear();
      continue;
    }
    const indent = match[1].length;
    if (!runs.has(indent)) {
      runs.set(indent, { marker: match[2], line: i + 1 });
      continue;
    }
    const run = runs.get(indent);
    if (run.marker !== match[2] && isEnabled('MDX031')) {
      issues.push(makeProblem('MDX031', i + 1, indent + 1, `List uses "${match[2]}" but the first item in this list uses "${run.marker}". A different bullet marker starts a new list in CommonMark, so this is not fixed automatically; use one marker if this is one list.`));
    }
  }

  if (isEnabled('MDX035')) {
    for (let i = 0; i < lines.length; i++) {
      if (isProtectedLine(info[i])) continue;
      if (/^(?: {0,3})(?:[-*+]|\d{1,9}[.)])[ \t]*$/.test(lines[i])) {
        issues.push(makeProblem('MDX035', i + 1, 1, 'Empty list item. Remove it or add content.'));
      }
    }
  }

  if (isEnabled('MDX032')) {
    let inList = false;
    let lastLineWasItem = false;
    for (let i = 0; i < lines.length; i++) {
      if (isProtectedLine(info[i])) {
        if (isTopLevelLine(lines[i]) && !info[i].fenceContent && !info[i].indentedCode) {
          inList = false;
          lastLineWasItem = false;
        }
        continue;
      }
      const isItem = /^(?: {0,3})(?:[-*+]|\d{1,9}[.)])[ \t]+\S/.test(lines[i]);
      if (isItem) {
        if (!inList && i > 0 && isParagraphLike(lines[i - 1], info[i - 1])) {
          issues.push(makeProblem('MDX032', i + 1, 1, 'List starts directly after a paragraph. Add a blank line before it so the list boundary is unambiguous.', { fixable: false }));
        }
        inList = true;
        lastLineWasItem = true;
        continue;
      }
      if (lines[i].trim() === '') {
        lastLineWasItem = false;
        continue;
      }
      if (/^[ \t]/.test(lines[i])) {
        continue;
      }
      if (inList && lastLineWasItem && isParagraphLike(lines[i], info[i])) {
        issues.push(makeProblem('MDX032', i + 1, 1, 'List ends directly before a paragraph, which may be read as a lazy continuation of the last item. Add a blank line after the list.', { fixable: false }));
      }
      inList = false;
      lastLineWasItem = false;
    }
  }

  return issues;
}

function computeTableStructure(lines, info, isEnabled) {
  const issues = [];
  const tables = findTables(lines, info);
  const delimiterLines = new Set(tables.filter((table) => isGfmTable(lines, table)).map((table) => table.start + 1));

  for (const table of tables) {
    const headerCells = splitTableCells(lines[table.start]);
    const delimiterCells = splitTableCells(lines[table.start + 1]);
    if (headerCells.length !== delimiterCells.length && isEnabled('MDX061')) {
      issues.push(makeProblem('MDX061', table.start + 1, 1, `Table header has ${headerCells.length} cell(s) but the delimiter row has ${delimiterCells.length}.`));
    }
    const checkCodePipes = function (lineIndex) {
      if (!isEnabled('MDX063')) return;
      const mask = buildMask(lines[lineIndex]);
      const segments = splitMasked(lines[lineIndex], mask);
      for (const segment of segments) {
        if (segment.protected && /(?<!\\)\|/.test(segment.text)) {
          issues.push(makeProblem('MDX063', lineIndex + 1, segment.start + 1, 'A pipe inside code or a URL in a table cell still splits the cell. Escape it as \\| in code, or use %7C in a URL.'));
          break;
        }
      }
    };
    checkCodePipes(table.start);
    for (let i = table.start + 2; i <= table.end; i++) {
      const cells = splitTableCells(lines[i]);
      if (cells.length !== delimiterCells.length && isEnabled('MDX061')) {
        issues.push(makeProblem('MDX061', i + 1, 1, `Table row has ${cells.length} cell(s) but the table has ${delimiterCells.length} column(s).`));
      }
      checkCodePipes(i);
    }
  }

  if (isEnabled('MDX060')) {
    for (let i = 0; i < lines.length; i++) {
      if (!delimiterLines.has(i)) continue;
      const parts = lines[i].split('|');
      for (let partIndex = 0; partIndex < parts.length; partIndex++) {
        const match = parts[partIndex].match(/^([ \t]*)(:?)(-{1,2})(:?)([ \t]*)$/);
        if (match) {
          const offset = parts.slice(0, partIndex).join('|').length + (partIndex > 0 ? 1 : 0);
          issues.push(makeProblem('MDX060', i + 1, offset + match[1].length + 1, 'Table delimiter cell has fewer than three hyphens.'));
          break;
        }
      }
    }
  }

  return issues;
}

function computeReferenceStructure(lines, info, isEnabled) {
  const issues = [];
  const definitions = new Map();

  for (let i = 0; i < lines.length; i++) {
    if (info[i].frontmatter || info[i].fenceContent || info[i].htmlBlock || info[i].indentedCode || info[i].mathBlock || info[i].opaque) continue;
    const definition = matchDefinition(lines[i]);
    if (!definition) continue;
    const match = [null, definition.label];
    const label = normalizeLabel(match[1]);
    if (!label) continue;
    if (definitions.has(label)) {
      if (isEnabled('MDX074')) {
        issues.push(makeProblem('MDX074', i + 1, 1, `Reference definition "${match[1].trim()}" is declared more than once.`));
      }
    } else {
      definitions.set(label, i + 1);
    }
  }

  for (let i = 0; i < lines.length; i++) {
    if (isProtectedLine(info[i])) continue;
    const mask = buildMask(lines[i], {
      maskBareUrls: false,
      maskLinkDestinations: false,
      maskReferenceLabels: false
    });
    const segments = splitMasked(lines[i], mask);

    for (const segment of segments) {
      if (segment.protected) continue;

      if (isEnabled('MDX071')) {
        const empty = segment.text.match(/\[[ \t]*\]\([^)]*\)|\][ \t]*\([ \t]*\)|!\[[ \t]*\]\([^)]*\)/);
        if (empty) {
          issues.push(makeProblem('MDX071', i + 1, segment.start + empty.index + 1, 'Link or image has empty text or an empty destination.'));
        }
      }
      if (isEnabled('MDX075')) {
        const image = segment.text.match(/!\[[ \t]*\]/);
        if (image) {
          issues.push(makeProblem('MDX075', i + 1, segment.start + image.index + 1, 'Image has no alt text. Add a short description, or confirm that an empty alt is correct for a decorative image.', { severity: 'warning' }));
        }
      }
      if (isEnabled('MDX072')) {
        const space = segment.text.match(/\]\s+\([^)]*\)/);
        if (space) {
          issues.push(makeProblem('MDX072', i + 1, segment.start + space.index + 1, 'There is whitespace between ] and (. CommonMark does not parse this as a link; remove the space if a link was intended.', { severity: 'info' }));
        }
      }
      if (isEnabled('MDX073')) {
        const refRe = /!?\[([^\]\n]*)\]\[([^\]\n]*)\]/g;
        let ref;
        while ((ref = refRe.exec(segment.text)) !== null) {
          const labelText = ref[2] === '' ? ref[1] : ref[2];
          const label = normalizeLabel(labelText);
          if (label && !definitions.has(label)) {
            issues.push(makeProblem('MDX073', i + 1, segment.start + ref.index + 1, `Reference link label "${labelText.trim()}" has no matching definition.`));
          }
        }
      }
    }
  }

  return issues;
}

function computeSetextAmbiguity(lines, info, isEnabled) {
  if (!isEnabled('MDX080')) return [];
  const issues = [];
  const tables = findTables(lines, info);
  const delimiterLines = new Set(tables.map((table) => table.start + 1));

  for (let i = 1; i < lines.length; i++) {
    if (isProtectedLine(info[i]) || delimiterLines.has(i)) continue;
    if (!/^ {0,3}(?:=+|-+)[ \t]*$/.test(lines[i])) continue;
    if (/^[ \t]/.test(lines[i]) || /^[ \t]/.test(lines[i - 1])) continue;
    if (lines[i - 1].trim() === '') continue;
    if (isThematicBreak(lines[i]) && /^ {0,3}(?:\*[ \t]*){3,}$/.test(lines[i])) continue;

    const previous = lines[i - 1];
    const previousInfo = info[i - 1];
    if (isProtectedLine(previousInfo) || previousInfo.fence) continue;
    if (/^ {0,3}>/.test(previous) || /^ {0,3}\|/.test(previous) || previous.includes('|')) continue;
    if (isAtxHeading(previous) || isListLine(previous) || isThematicBreak(previous)) continue;
    if (previousInfo.definition) continue;

    issues.push(makeProblem('MDX080', i + 1, 1, 'This line is a Setext heading underline for the previous paragraph, so it renders as a heading instead of a horizontal rule. Insert a blank line above it, or use *** for a rule.'));
  }
  return issues;
}

function computeFullwidthSyntax(lines, info, isEnabled) {
  if (!isEnabled('MDX084')) return [];
  const issues = [];
  for (let i = 0; i < lines.length; i++) {
    if (isProtectedLine(info[i])) continue;
    const line = lines[i];
    if (
      /^[ \t]*＃{1,6}(?:[ \t　]|[^＃\s])/.test(line) ||
      /^[ \t]*[＊＋－][ \t　]/.test(line) ||
      /！?［[^］\n]+］（[^）\n]+）/.test(line)
    ) {
      issues.push(makeProblem('MDX084', i + 1, 1, 'Full-width punctuation is being used for Markdown syntax. Use ASCII # for headings, -, *, or + for bullets, and [ ] ( ) for links and images.'));
    }
  }
  return issues;
}

// --- MDX042: CJK/full-width punctuation that breaks emphasis pairing --------
// Simulate CommonMark delimiter pairing per paragraph instead of counting
// delimiters per line, then report the runs that stay unpaired. A closing
// like **X：**Y leaves literal ** on GitHub because the full-width punctuation
// makes the delimiter run unable to close; **Note:**text keeps its CommonMark
// behaviour and is not reported.

function isEmphasisProseLine(line, lineInfo) {
  return !isBlank(line) && !isProtectedLine(lineInfo) && !lineInfo.fence;
}

function startsInlineBlockBoundary(line) {
  return isAtxHeading(line) || isListLine(line) || isThematicBreak(line) || /^ {0,3}\|/.test(line);
}

function forEachEmphasisParagraph(lines, info, callback) {
  let i = 0;
  while (i < lines.length) {
    if (!isEmphasisProseLine(lines[i], info[i])) {
      i++;
      continue;
    }
    let end = i;
    while (
      end + 1 < lines.length &&
      isEmphasisProseLine(lines[end + 1], info[end + 1]) &&
      !startsInlineBlockBoundary(lines[end + 1])
    ) {
      end++;
    }
    callback(i, end);
    i = end + 1;
  }
}

function findStarDelimiterRuns(text, mask) {
  const runs = [];
  let i = 0;
  while (i < text.length) {
    if (text[i] !== '*' || mask[i]) {
      i++;
      continue;
    }
    let end = i + 1;
    while (end < text.length && text[end] === '*' && !mask[end]) end++;
    const before = i > 0 ? text[i - 1] : null;
    const after = end < text.length ? text[end] : null;
    const flanking = delimiterFlanking(before, after);
    runs.push({
      start: i,
      end,
      len: end - i,
      canOpen: flanking.leftFlanking,
      canClose: flanking.rightFlanking,
      before,
      after
    });
    i = end;
  }
  return runs;
}

// A simplified CommonMark "process emphasis" pass over the * runs of one
// paragraph: closers pair with the nearest usable opener before them, runs in
// between become literal, and the rule of three is honoured. Runs left with
// unused length are the ones CommonMark renders as literal asterisks.
function matchEmphasisRuns(runs) {
  const items = runs.map((run) => ({ ...run, remaining: run.len, dropped: false }));
  const pairs = [];
  let index = 0;
  while (index < items.length) {
    const closer = items[index];
    if (closer.dropped || closer.remaining === 0 || !closer.canClose) {
      index++;
      continue;
    }
    let openerIndex = -1;
    for (let j = index - 1; j >= 0; j--) {
      const opener = items[j];
      if (opener.dropped || opener.remaining === 0 || !opener.canOpen) continue;
      const ruleOfThree = (opener.len + closer.len) % 3 === 0 &&
        (opener.len % 3 !== 0 || closer.len % 3 !== 0);
      if ((opener.canClose || closer.canOpen) && ruleOfThree) continue;
      openerIndex = j;
      break;
    }
    if (openerIndex === -1) {
      index++;
      continue;
    }
    const opener = items[openerIndex];
    const use = opener.remaining >= 2 && closer.remaining >= 2 ? 2 : 1;
    opener.remaining -= use;
    closer.remaining -= use;
    pairs.push({ opener, closer, use });
    for (let j = openerIndex + 1; j < index; j++) items[j].dropped = true;
    if (closer.remaining === 0) index++;
  }
  return { pairs, leftovers: items.filter((item) => !item.dropped && item.remaining > 0) };
}

function analyzeCjkEmphasis(lines, info) {
  const findings = [];
  const transforms = [];

  forEachEmphasisParagraph(lines, info, (from, to) => {
    let text = '';
    let mask = [];
    const lineStarts = [];
    for (let i = from; i <= to; i++) {
      if (i > from) {
        text += '\n';
        mask.push(true);
      }
      lineStarts.push(text.length);
      text += lines[i];
      // A loop, not push(...spread): spreading a long line's mask exceeds V8's
      // argument limit (RangeError) at about 120k code units.
      for (const masked of buildMask(lines[i])) mask.push(masked);
    }

    const locate = function (pos) {
      let offset = 0;
      for (let k = lineStarts.length - 1; k >= 0; k--) {
        if (pos >= lineStarts[k]) {
          offset = k;
          break;
        }
      }
      return { line: from + offset + 1, column: pos - lineStarts[offset] + 1 };
    };
    const lineOffsetOf = function (pos) {
      let offset = 0;
      for (let k = lineStarts.length - 1; k >= 0; k--) {
        if (pos >= lineStarts[k]) {
          offset = k;
          break;
        }
      }
      return offset;
    };

    const runs = findStarDelimiterRuns(text, mask);
    const leftovers = matchEmphasisRuns(runs).leftovers;
    if (leftovers.length === 0) return;

    // A lone leftover is literal text (a stray *, a glob like claude-*, or the
    // $-amount masking heuristic hiding the partner run), not a broken
    // emphasis. Only two leftover runs whose in-between content is attached to
    // both delimiters look like the one emphasis span CommonMark refused to
    // pair — the same "content must be attached" idea MDX040 uses.
    const attachedSpan = function (left, right) {
      const inner = text.slice(left.end, right.start);
      return inner !== '' && !/\s/u.test(inner[0]) && !/\s/u.test(inner[inner.length - 1]);
    };
    const reportable = new Set();
    for (let k = 0; k < leftovers.length; k++) {
      const previous = k > 0 ? leftovers[k - 1] : null;
      const next = k + 1 < leftovers.length ? leftovers[k + 1] : null;
      if ((previous && attachedSpan(previous, leftovers[k])) ||
        (next && attachedSpan(leftovers[k], next))) {
        reportable.add(leftovers[k]);
      }
    }
    const cjkAdjacent = function (run) {
      return (run.before !== null && CJK_ADJACENT_RE.test(run.before)) ||
        (run.after !== null && CJK_ADJACENT_RE.test(run.after));
    };

    // Safe auto-fix: a leftover closing ** whose only problem is a run of
    // movable full-width punctuation glued to its inside. Moving that
    // punctuation outside keeps the meaning (**X：**Y -> **X**：Y). One-sided
    // brackets and quotes stay inside and are only reported.
    const fixable = new Set();
    for (const closer of leftovers) {
      if (closer.len !== 2 || !reportable.has(closer) || !cjkAdjacent(closer)) continue;
      const after = closer.end < text.length ? text[closer.end] : null;
      if (isBoundaryOrWhitespace(after)) continue;
      let punctStart = closer.start;
      while (punctStart > 0 && MOVABLE_PUNCT_RE.test(text[punctStart - 1])) punctStart--;
      if (punctStart === closer.start) continue;
      let opener = null;
      for (const candidate of leftovers) {
        if (candidate === closer || candidate.start >= punctStart) continue;
        if (!candidate.canOpen || candidate.len !== 2) continue;
        if (!opener || candidate.start > opener.start) opener = candidate;
      }
      if (!opener || opener.end > punctStart) continue;
      const inner = text.slice(opener.end, punctStart);
      if (inner === '' || /\s/u.test(inner[0]) || /\s/u.test(inner[inner.length - 1])) continue;
      if (runs.some((run) => run !== opener && run !== closer && run.start >= opener.end && run.end <= closer.start)) continue;
      if (lineOffsetOf(punctStart) !== lineOffsetOf(closer.start)) continue;
      fixable.add(opener);
      fixable.add(closer);
      transforms.push({
        from,
        to,
        edits: [
          { start: punctStart, end: closer.start, text: '' },
          { start: closer.end, end: closer.end, text: text.slice(punctStart, closer.start) }
        ]
      });
    }

    // Second safe auto-fix: the pair wraps a whole full-width-quoted span
    // (這是**「引號」**文字). Both quote marks sit inside the delimiters, which
    // keeps the run from flanking; moving them outside (這是「**引號**」文字)
    // makes the pair work and keeps the meaning. Only matching quote marks
    // with attached, run-free content on one line are moved; every other
    // opening-side shape (one-sided quotes, single `*`, spans across lines or
    // other runs) is still just reported. Each side is its own length
    // preserving transform.
    for (const closer of leftovers) {
      if (closer.len !== 2 || !reportable.has(closer) || !cjkAdjacent(closer)) continue;
      const closeQuote = closer.start > 0 ? text[closer.start - 1] : null;
      const openQuote = closeQuote === null ? null : QUOTE_OPEN_BY_CLOSE.get(closeQuote);
      if (!openQuote) continue;
      let opener = null;
      for (const candidate of leftovers) {
        if (candidate === closer || candidate.start >= closer.start) continue;
        if (candidate.len !== 2 || text[candidate.end] !== openQuote) continue;
        if (!opener || candidate.start > opener.start) opener = candidate;
      }
      if (!opener) continue;
      const inner = text.slice(opener.end + 1, closer.start - 1);
      if (inner === '' || /\s/u.test(inner[0]) || /\s/u.test(inner[inner.length - 1])) continue;
      if (runs.some((run) => run !== opener && run !== closer && run.start >= opener.end && run.end <= closer.start)) continue;
      if (lineOffsetOf(opener.start) !== lineOffsetOf(closer.end)) continue;
      fixable.add(opener);
      fixable.add(closer);
      transforms.push({
        from,
        to,
        edits: [
          { start: opener.start, end: opener.end, text: '' },
          { start: opener.end + 1, end: opener.end + 1, text: '**' }
        ]
      });
      transforms.push({
        from,
        to,
        edits: [
          { start: closer.start - 1, end: closer.start, text: '' },
          { start: closer.end, end: closer.end, text: closeQuote }
        ]
      });
    }

    for (const run of leftovers) {
      if (!reportable.has(run) || !cjkAdjacent(run)) continue;
      const place = locate(run.start);
      findings.push({
        line: place.line,
        column: place.column,
        delim: '*'.repeat(run.len),
        fixable: fixable.has(run)
      });
    }
  });

  return { findings, transforms };
}

function computeCjkEmphasis(lines, info, isEnabled) {
  if (!isEnabled('MDX042')) return [];
  const issues = [];
  for (const finding of analyzeCjkEmphasis(lines, info).findings) {
    const message = `Unpaired \`${finding.delim}\` next to CJK or full-width punctuation renders literally instead of emphasis. Move the full-width punctuation outside the delimiters (\`**X：**Y\` becomes \`**X**：Y\`) or reword.`;
    issues.push(makeProblem('MDX042', finding.line, finding.column, message, finding.fixable ? {} : { fixable: false }));
  }
  return issues;
}

function applyCjkEmphasisFixes(text, isEnabled) {
  if (!isEnabled('MDX042')) return text;
  const scan = scanDocument(text, isEnabled.limits);
  const transforms = analyzeCjkEmphasis(scan.lines, scan.info).transforms;
  if (transforms.length === 0) return text;

  const lines = [...scan.lines];
  for (const transform of transforms) {
    let paragraph = lines.slice(transform.from, transform.to + 1).join('\n');
    const edits = [...transform.edits].sort((a, b) => b.start - a.start);
    for (const edit of edits) {
      paragraph = paragraph.slice(0, edit.start) + edit.text + paragraph.slice(edit.end);
    }
    const rewritten = paragraph.split('\n');
    for (let k = 0; k < rewritten.length; k++) lines[transform.from + k] = rewritten[k];
  }
  return lines.join('\n');
}

// A blank line inside a blockquote: `>`, `> >`, ... with nothing after the markers.
function isQuoteBlankLine(line) {
  const quote = parseQuotePrefix(line);
  return Boolean(quote) && quote.rest.trim() === '';
}

function computeBlankLineFixes(lines, info, scan, isEnabled) {
  const before = new Set();
  const after = new Set();
  // Line to insert instead of '' for a fence inside a blockquote, keyed by the
  // fence's opening (before) or closing (after) line. A fully blank line would
  // end the blockquote and split it in two.
  const fill = new Map();
  const issues = [];

  if (isEnabled('MDX013')) {
    for (let i = 0; i < lines.length; i++) {
      if (isProtectedLine(info[i])) continue;
      if (!/^#{1,6}[ \t]/.test(lines[i])) continue;
      if (i > 0 && isParagraphLike(lines[i - 1], info[i - 1])) {
        before.add(i);
        issues.push(makeProblem('MDX013', i + 1, 1, 'Heading is directly attached to the preceding paragraph. Add a blank line before it.'));
      }
      if (i + 1 < lines.length && isParagraphLike(lines[i + 1], info[i + 1])) {
        after.add(i);
        issues.push(makeProblem('MDX013', i + 1, 1, 'Heading is directly attached to the following paragraph. Add a blank line after it.'));
      }
    }
  }

  if (isEnabled('MDX052')) {
    for (let i = 0; i < lines.length; i++) {
      if (!info[i].fenceOpen || !isTopLevelLine(lines[i])) continue;
      // A fence inside a blockquote is separated by a `>` line at the fence's
      // depth. When the fence is indented after the markers it may belong to a
      // list item, where any blank line would make the list loose, so skip it.
      const quote = lines[i][0] === '>' ? parseQuotePrefix(lines[i]) : null;
      if (quote && /^[ \t]/.test(quote.rest)) continue;
      const quoteFill = quote ? new Array(quote.depth).fill('>').join(' ') : '';
      const separated = function (line) {
        return line.trim() === '' || (quote !== null && isQuoteBlankLine(line));
      };
      if (i > 0 && !separated(lines[i - 1])) {
        before.add(i);
        if (quote) fill.set(i, quoteFill);
        issues.push(makeProblem('MDX052', i + 1, 1, 'Fenced code block starts directly after another block. Add a blank line before it.'));
      }
      let close = i + 1;
      while (close < lines.length && !info[close].fenceClose) close++;
      if (close < lines.length) {
        if (close + 1 < lines.length && !separated(lines[close + 1])) {
          after.add(close);
          if (quote) fill.set(close, quoteFill);
          issues.push(makeProblem('MDX052', close + 1, 1, 'Fenced code block ends directly before another block. Add a blank line after it.'));
        }
        i = close;
      }
    }
  }

  if (isEnabled('MDX062')) {
    for (const table of findTables(lines, info)) {
      if (/^[ \t]/.test(lines[table.start])) continue;
      if (table.start > 0 && lines[table.start - 1].trim() !== '') {
        before.add(table.start);
        issues.push(makeProblem('MDX062', table.start + 1, 1, 'Table starts directly after another block. Add a blank line before it.'));
      }
      if (table.end + 1 < lines.length && lines[table.end + 1].trim() !== '') {
        after.add(table.end);
        issues.push(makeProblem('MDX062', table.end + 2, 1, 'Table ends directly before another block. Add a blank line after it.'));
      }
    }
  }

  if (isEnabled('MDX082') && scan.frontmatterEnd >= 0) {
    const end = scan.frontmatterEnd;
    if (end + 1 < lines.length && lines[end + 1].trim() !== '') {
      after.add(end);
      issues.push(makeProblem('MDX082', end + 2, 1, 'Frontmatter must be followed by a blank line.'));
    }
  }

  return { before, after, fill, issues };
}

function applyBlankLineFixes(text, isEnabled) {
  const scan = scanDocument(text, isEnabled.limits);
  const { before, after, fill } = computeBlankLineFixes(scan.lines, scan.info, scan, isEnabled);
  if (before.size === 0 && after.size === 0) return text;

  const needsSeparator = function (line, i) {
    if (line.trim() === '') return false;
    return !(fill.has(i) && isQuoteBlankLine(line));
  };
  const output = [];
  for (let i = 0; i < scan.lines.length; i++) {
    if (before.has(i) && output.length > 0 && needsSeparator(output[output.length - 1], i)) {
      output.push(fill.get(i) ?? '');
    }
    output.push(scan.lines[i]);
    if (after.has(i) && i + 1 < scan.lines.length && needsSeparator(scan.lines[i + 1], i)) {
      output.push(fill.get(i) ?? '');
    }
  }
  return output.join('\n');
}

function computeTrailingWhitespace(lines, info, isEnabled) {
  if (!isEnabled('MDX004')) return [];
  const issues = [];
  for (let i = 0; i < lines.length; i++) {
    const lineInfo = info[i];
    if (lineInfo.frontmatter || lineInfo.fenceContent || lineInfo.htmlBlock || lineInfo.indentedCode || lineInfo.mathBlock || lineInfo.opaque || lineInfo.keepWhitespace) continue;
    const match = lines[i].match(/[ \t]+$/);
    if (!match) continue;
    if (!isBlank(lines[i]) && /^ {2}$/.test(match[0])) continue;
    issues.push(makeProblem('MDX004', i + 1, lines[i].length - match[0].length + 1, 'Trailing whitespace. Exactly two trailing spaces create a hard line break; other trailing whitespace is noise.'));
  }
  return issues;
}

function applyTrailingWhitespace(text, isEnabled) {
  if (!isEnabled('MDX004')) return text;
  const scan = scanDocument(text, isEnabled.limits);
  const lines = scan.lines.map((line, i) => {
    const lineInfo = scan.info[i];
    if (lineInfo.frontmatter || lineInfo.fenceContent || lineInfo.htmlBlock || lineInfo.indentedCode || lineInfo.mathBlock || lineInfo.opaque || lineInfo.keepWhitespace) return line;
    const match = line.match(/[ \t]+$/);
    if (!match) return line;
    if (!isBlank(line) && /^ {2}$/.test(match[0])) return line;
    if (isBlank(line)) return '';
    if (/ {2}$/.test(match[0])) {
      return line.slice(0, line.length - match[0].length) + '  ';
    }
    return line.slice(0, line.length - match[0].length);
  });
  return lines.join('\n');
}

function computeNbspInUrls(lines, info, isEnabled) {
  if (!isEnabled('MDX008')) return [];
  const issues = [];
  const urlRe = /\b(?:https?:\/\/|ftp:\/\/|www\.)(?:[^\s<>"']|\u00A0)+/gi;
  for (let i = 0; i < lines.length; i++) {
    if (isProtectedLine(info[i])) continue;
    const mask = buildMask(lines[i], { maskBareUrls: false, maskLinkDestinations: false });
    for (const match of lines[i].matchAll(urlRe)) {
      if (mask[match.index]) continue;
      const offset = match[0].indexOf('\u00A0');
      if (offset === -1) continue;
      issues.push(makeProblem('MDX008', i + 1, match.index + offset + 1, 'NBSP inside a URL. Replace it with %20 or remove it; the fixer leaves URLs untouched to avoid breaking them.', { fixable: false }));
    }
  }
  return issues;
}

function computeZeroWidth(lines, info, isEnabled) {
  if (!isEnabled('MDX009')) return [];
  const issues = [];
  for (let i = 0; i < lines.length; i++) {
    const lineInfo = info[i];
    if (lineInfo.frontmatter || lineInfo.fenceContent || lineInfo.htmlBlock || lineInfo.indentedCode || lineInfo.mathBlock || lineInfo.opaque) continue;
    const mask = buildMask(lines[i], { maskBareUrls: false, maskLinkDestinations: false });
    const segments = splitMasked(lines[i], mask);
    for (const segment of segments) {
      if (segment.protected) continue;
      const match = segment.text.match(/[\u200B\u2060\uFEFF]/);
      if (match) {
        issues.push(makeProblem('MDX009', i + 1, segment.start + match.index + 1, 'Zero-width character found. Remove it unless it is intentional (for example, a line-break opportunity in some scripts).'));
        break;
      }
    }
  }
  return issues;
}

function computeBlankLineRuns(text, scan, isEnabled) {
  const issues = [];
  if (text.length === 0) return issues;
  const lines = scan.lines;
  const protectedBlank = function (i) {
    const lineInfo = scan.info[i];
    return Boolean(lineInfo.frontmatter || lineInfo.fenceContent || lineInfo.htmlBlock || lineInfo.indentedCode || lineInfo.mathBlock || lineInfo.opaque || lineInfo.keepWhitespace);
  };

  let i = 0;
  while (i < lines.length) {
    if (!isBlank(lines[i]) || protectedBlank(i)) {
      i++;
      continue;
    }
    let j = i;
    while (j < lines.length && isBlank(lines[j]) && !protectedBlank(j)) j++;
    const hasBefore = i > 0;
    const hasAfter = j < lines.length;

    if (!hasBefore && isEnabled('MDX006')) {
      if (hasAfter && lines[j].startsWith('\uFEFF')) {
        issues.push(makeProblem('MDX006', 1, 1, 'File starts with blank lines before a line that begins with U+FEFF. Remove the U+FEFF first; removing only the blank lines would turn it into a BOM.', { fixable: false }));
      } else {
        issues.push(makeProblem('MDX006', 1, 1, 'File starts with blank lines. Remove them so the first block starts at line 1.'));
      }
    }
    if (hasBefore && hasAfter && j - i >= 2 && isEnabled('MDX005')) {
      issues.push(makeProblem('MDX005', i + 2, 1, 'Multiple consecutive blank lines. Use one blank line between blocks.'));
    }
    i = j;
  }

  if (isEnabled('MDX003') && text.length > 0) {
    if (!text.endsWith('\n')) {
      issues.push(makeProblem('MDX003', lines.length, 1, 'File does not end with a newline.'));
    } else if (/\n{2,}$/.test(text)) {
      issues.push(makeProblem('MDX003', lines.length - 1, 1, 'File ends with extra blank lines. End it with exactly one newline.'));
    }
  }

  return issues;
}

function applyBlankLineNormalization(text, isEnabled) {
  const scan = scanDocument(text, isEnabled.limits);
  const lines = scan.lines;
  const protectedBlank = function (i) {
    const lineInfo = scan.info[i];
    return Boolean(lineInfo.frontmatter || lineInfo.fenceContent || lineInfo.htmlBlock || lineInfo.indentedCode || lineInfo.mathBlock || lineInfo.opaque || lineInfo.keepWhitespace);
  };

  const output = [];
  let i = 0;

  if (isEnabled('MDX006')) {
    while (i < lines.length && isBlank(lines[i]) && !protectedBlank(i)) i++;
    // Removing the blank lines would move a U+FEFF to the start of the file,
    // where it becomes a BOM and the line's meaning changes.
    if (i < lines.length && lines[i].startsWith('\uFEFF')) i = 0;
  }

  while (i < lines.length) {
    if (!isBlank(lines[i]) || protectedBlank(i)) {
      output.push(lines[i]);
      i++;
      continue;
    }
    let j = i;
    while (j < lines.length && isBlank(lines[j]) && !protectedBlank(j)) j++;
    const atEnd = j >= lines.length;
    if (atEnd) {
      if (!isEnabled('MDX003')) {
        for (let k = i; k < j; k++) output.push(lines[k]);
      }
    } else if (isEnabled('MDX005')) {
      output.push('');
    } else {
      for (let k = i; k < j; k++) output.push(lines[k]);
    }
    i = j;
  }

  let result = output.join('\n');
  if (isEnabled('MDX003') && result.length > 0) {
    result = result.replace(/\n*$/, '\n');
  }
  return result;
}

function computeLineLength(lines, info, isEnabled, maxLineLength) {
  if (!isEnabled('MDX090') || !(maxLineLength > 0)) return [];
  const issues = [];
  const tables = findTables(lines, info);
  const tableLines = new Set();
  for (const table of tables) {
    for (let i = table.start; i <= table.end; i++) tableLines.add(i);
  }
  for (let i = 0; i < lines.length; i++) {
    if (isProtectedLine(info[i]) || tableLines.has(i)) continue;
    const length = Array.from(lines[i]).length;
    if (length > maxLineLength) {
      issues.push(makeProblem('MDX090', i + 1, maxLineLength + 1, `Line is ${length} characters; limit is ${maxLineLength}.`, { severity: 'info' }));
    }
  }
  return issues;
}

function dedupeProblems(problems) {
  const seen = new Set();
  const result = [];
  for (const problem of problems) {
    const key = `${problem.rule}:${problem.line}:${problem.column}:${problem.message}`;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(problem);
  }
  result.sort((a, b) => a.line - b.line || a.column - b.column || a.rule.localeCompare(b.rule));
  return result;
}

function applyTableDelimiterFixes(text, isEnabled) {
  if (!isEnabled('MDX060')) return text;
  const scan = scanDocument(text, isEnabled.limits);
  const lines = [...scan.lines];
  for (const table of findTables(lines, scan.info)) {
    if (!isGfmTable(lines, table)) continue;
    const index = table.start + 1;
    const result = fixTableDelimiter(lines[index], isEnabled);
    if (result) lines[index] = result.line;
  }
  return lines.join('\n');
}

// Append without push(...items): spreading a large array exceeds V8's argument
// limit (RangeError) on documents with many problems.
function pushAll(target, items) {
  for (const item of items) target.push(item);
}

function diagnose(text, options = {}) {
  const isEnabled = createEnabled(options);
  const raw = text;
  const normalized = normalizeForAnalysis(raw);
  const issues = [];

  if (raw.startsWith('\uFEFF') && isEnabled('MDX001')) {
    issues.push(makeProblem('MDX001', 1, 1, 'File starts with a UTF-8 BOM. Remove it so the first Markdown block starts at byte 0.'));
  }
  if (/\r/.test(raw) && isEnabled('MDX002')) {
    const before = raw.slice(0, raw.search(/\r/)).split('\n');
    issues.push(makeProblem('MDX002', before.length, 1, 'File uses CRLF or CR line endings. Normalize them to LF.'));
  }

  const scan = scanDocument(normalized, isEnabled.limits);
  for (const issue of scan.issues) {
    if (isEnabled(issue.rule)) issues.push(issue);
  }

  const lineResults = scan.lines.map((line, i) => transformLine(line, scan.info[i], i + 1, isEnabled));
  for (const result of lineResults) {
    pushAll(issues, result.issues);
  }

  pushAll(issues, computeTrailingWhitespace(scan.lines, scan.info, isEnabled));
  pushAll(issues, computeNbspInUrls(scan.lines, scan.info, isEnabled));
  pushAll(issues, computeZeroWidth(scan.lines, scan.info, isEnabled));
  pushAll(issues, computeBlankLineRuns(normalized, scan, isEnabled));

  const normalizedLines = lineResults.map((result) => result.line);
  const normalizedText = normalizedLines.join('\n');
  const normalizedScan = scanDocument(normalizedText, isEnabled.limits);

  pushAll(issues, computeHeadingStructure(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeListStructure(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeTableStructure(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeReferenceStructure(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeSetextAmbiguity(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeFullwidthSyntax(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeCjkEmphasis(normalizedLines, normalizedScan.info, isEnabled));
  pushAll(issues, computeBlankLineFixes(normalizedLines, normalizedScan.info, normalizedScan, isEnabled).issues);
  pushAll(issues, computeLineLength(normalizedLines, normalizedScan.info, isEnabled, options.maxLineLength || 0));

  if (isEnabled('MDX007')) {
    for (let i = 0; i < normalizedLines.length; i++) {
      const lineInfo = normalizedScan.info[i];
      if (isProtectedLine(lineInfo)) continue;
      const mask = buildMask(normalizedLines[i], { maskBareUrls: false });
      const segments = splitMasked(normalizedLines[i], mask);
      const unprotected = segments.find((segment) => !segment.protected && segment.text.includes('\t'));
      if (unprotected) {
        const tabIndex = normalizedLines[i].indexOf('\t', unprotected.start);
        issues.push(makeProblem('MDX007', i + 1, tabIndex + 1, 'Tab character found. Use spaces for indentation and alignment; tabs render at different widths.'));
      }
    }
  }

  if (isEnabled('MDX070')) {
    for (let i = 0; i < normalizedLines.length; i++) {
      const lineInfo = normalizedScan.info[i];
      if (isProtectedLine(lineInfo)) continue;
      const mask = buildMask(normalizedLines[i], { maskBareUrls: false, maskLinkText: true });
      const segments = splitMasked(normalizedLines[i], mask);
      for (const segment of segments) {
        if (segment.protected) continue;
        const urlRe = /\b(?:https?:\/\/|ftp:\/\/|www\.)(?:[^\s<>"']|\u00A0)+/gi;
        let match;
        while ((match = urlRe.exec(segment.text)) !== null) {
          issues.push(makeProblem('MDX070', i + 1, segment.start + match.index + 1, 'Bare URL. Wrap it in <...> or use an explicit link only when that improves readability; otherwise leave it as text.', { severity: 'info' }));
        }
      }
    }
  }

  if (isEnabled('MDX050')) {
    for (let i = 0; i < normalizedLines.length; i++) {
      if (!normalizedScan.info[i].fenceOpen) continue;
      if (normalizedScan.info[i].fenceInfo === '') {
        issues.push(makeProblem('MDX050', i + 1, 1, 'Fenced code block has no language. Add one when the language is known, for example ```js.', { severity: 'info' }));
      }
    }
  }

  if (isEnabled('MDX041')) {
    for (let i = 0; i < normalizedLines.length; i++) {
      const lineInfo = normalizedScan.info[i];
      if (isProtectedLine(lineInfo)) continue;
      const mask = buildMask(normalizedLines[i], { maskBareUrls: false });
      const segments = splitMasked(normalizedLines[i], mask);
      let strongStars = 0;
      let strongUnders = 0;
      let strikes = 0;
      for (const segment of segments) {
        if (segment.protected) continue;
        strongStars += (segment.text.match(/(?<!\*)\*\*(?!\*)/g) || []).length;
        strongUnders += (segment.text.match(/(?<![\w_])__(?![\w_])/g) || []).length;
        strikes += (segment.text.match(/(?<!~)~~(?!~)/g) || []).length;
      }
      if (strongStars % 2 === 1 || strongUnders % 2 === 1 || strikes % 2 === 1) {
        issues.push(makeProblem('MDX041', i + 1, 1, 'Possible unmatched strong or strikethrough delimiter on this line. Verify that emphasis opens and closes outside code and links.'));
      }
    }
  }

  return dedupeProblems(issues.filter((issue) => isEnabled(issue.rule)));
}

// Structural problems that make fixDocument refuse the whole document,
// regardless of rules/ignoreRules.
const BLOCKING_RULES = new Set(['MDX081', 'MDX083', 'MDX053', 'MDX085', 'MDX086']);

const FRONTMATTER_TRAP = 'A fix would turn a leading --- into YAML frontmatter. Fix the first lines manually; use *** if the line was meant as a horizontal rule.';

function fixDocument(text, options = {}) {
  const isEnabled = createEnabled(options);
  const originalEol = /\r\n/.test(text) ? '\r\n' : (/\r/.test(text) ? '\r' : '\n');
  const bom = (text.match(/^\uFEFF+/) || [''])[0];
  const hadBom = bom.length > 0;
  const normalizedOriginal = normalizeForAnalysis(text);
  const encodingChanged = (hadBom && isEnabled('MDX001')) || (/\r/.test(text) && isEnabled('MDX002'));
  let working = normalizedOriginal;
  const initialScan = scanDocument(working, isEnabled.limits);
  const hadFrontmatter = initialScan.frontmatterEnd >= 0;
  const blocking = initialScan.issues.find((issue) => BLOCKING_RULES.has(issue.rule));
  if (blocking) {
    return { text, changed: false, refused: true, reason: blocking.message };
  }
  if (!hadFrontmatter && isEnabled('MDX006') && startsFrontmatterAfterLeadingBlanks(working)) {
    return { text, changed: false, refused: true, reason: FRONTMATTER_TRAP };
  }

  for (let pass = 0; pass < 6; pass++) {
    const before = working;
    const scan = scanDocument(working, isEnabled.limits);
    const blocking = scan.issues.find((issue) => BLOCKING_RULES.has(issue.rule));
    if (blocking) {
      return { text, changed: false, refused: true, reason: blocking.message };
    }
    if (!hadFrontmatter && scan.frontmatterEnd >= 0) {
      return { text, changed: false, refused: true, reason: FRONTMATTER_TRAP };
    }
    if (!hadFrontmatter && isEnabled('MDX006') && startsFrontmatterAfterLeadingBlanks(working)) {
      return { text, changed: false, refused: true, reason: FRONTMATTER_TRAP };
    }
    const lines = scan.lines.map((line, i) => transformLine(line, scan.info[i], i + 1, isEnabled).line);
    working = lines.join('\n');
    working = applyCjkEmphasisFixes(working, isEnabled);
    working = applyTableDelimiterFixes(working, isEnabled);
    working = applyBlankLineFixes(working, isEnabled);
    working = applyTrailingWhitespace(working, isEnabled);
    working = applyBlankLineNormalization(working, isEnabled);
    if (working === before) break;
  }

  if (!hadFrontmatter && scanDocument(working, isEnabled.limits).frontmatterEnd >= 0) {
    return { text, changed: false, refused: true, reason: FRONTMATTER_TRAP };
  }
  if (!hadFrontmatter && isEnabled('MDX006') && startsFrontmatterAfterLeadingBlanks(working)) {
    return { text, changed: false, refused: true, reason: FRONTMATTER_TRAP };
  }
  if (working === normalizedOriginal && !encodingChanged) {
    return { text, changed: false, refused: false, reason: '' };
  }
  if (hadBom && !isEnabled('MDX001')) {
    working = `${bom}${working}`;
  }
  if (!isEnabled('MDX002') && originalEol !== '\n') {
    working = working.replace(/\n/g, originalEol);
  }

  return { text: working, changed: working !== text, refused: false, reason: '' };
}

function expandPaths(inputs, options = {}) {
  const files = [];
  const errors = [];
  const visit = function (target) {
    let stat;
    try {
      stat = fs.statSync(target);
    } catch (error) {
      errors.push(`Cannot access ${target}: ${error.message}`);
      return;
    }
    if (stat.isFile()) {
      files.push(path.resolve(target));
      return;
    }
    if (!stat.isDirectory()) return;
    let entries;
    try {
      entries = fs.readdirSync(target, { withFileTypes: true });
    } catch (error) {
      errors.push(`Cannot read directory ${target}: ${error.message}`);
      return;
    }
    for (const entry of entries) {
      if (DEFAULT_EXCLUDES.has(entry.name)) continue;
      if (entry.isDirectory()) {
        visit(path.join(target, entry.name));
        continue;
      }
      if (entry.isFile() && /\.(?:md|markdown)$/i.test(entry.name)) {
        files.push(path.resolve(path.join(target, entry.name)));
      }
    }
  };

  for (const input of inputs) visit(input);
  return { files: [...new Set(files)].sort(), errors };
}

function readMarkdownFile(file) {
  const buffer = fs.readFileSync(file);
  if (buffer.includes(0)) {
    throw new Error('file contains NUL bytes; refusing to treat it as text');
  }
  const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
  return decoder.decode(buffer);
}

function writeFileAtomic(file, contents) {
  const target = fs.realpathSync(file);
  const stat = fs.statSync(target);
  const temp = `${target}.${process.pid}.${Date.now()}.tmp`;
  try {
    fs.writeFileSync(temp, contents, 'utf8');
    fs.chmodSync(temp, stat.mode);
    fs.renameSync(temp, target);
  } catch (error) {
    try {
      fs.unlinkSync(temp);
    } catch {
      // Best effort cleanup.
    }
    throw error;
  }
}

function formatProblem(problem) {
  const location = `${problem.line}:${problem.column}`;
  const fixable = problem.fixable ? ' (fixable)' : '';
  return `  ${location.padEnd(10)} ${problem.severity.padEnd(7)} ${problem.rule}  ${problem.message}${fixable}`;
}

function summarizeProblems(problems) {
  return {
    total: problems.length,
    errors: problems.filter((problem) => problem.severity === 'error').length,
    warnings: problems.filter((problem) => problem.severity === 'warning').length,
    info: problems.filter((problem) => problem.severity === 'info').length,
    fixable: problems.filter((problem) => problem.fixable).length
  };
}

function printHelp() {
  const lines = [
    'markdown-fixer — safe Markdown syntax checking and auto-correction',
    '',
    `Usage: node markdown-fixer.mjs [options] <file-or-directory>...`,
    '',
    'Modes:',
    '  --check             Report problems without writing (default).',
    '  --write             Apply safe fixes in place.',
    '',
    'Options:',
    '  --json              Emit machine-readable JSON.',
    '  --quiet             Suppress per-file problem lists.',
    '  --strict            Make info-level findings fail the run too.',
    '  --rule <ids>        Only enable these rules (comma-separated, repeatable).',
    '  --ignore-rule <ids> Disable these rules (comma-separated, repeatable).',
    '  --max-line-length N Enable MDX090 with a line-length limit.',
    '  --max-quote-depth N Limit blockquote nesting depth (default 32, 0 = no limit).',
    '  --on-limit <mode>   When a limit is exceeded: error (MDX086, default) or ignore.',
    '  --list-rules        Print the rule catalog and exit.',
    '  --version           Print the script version and exit.',
    '  -h, --help          Print this help and exit.',
    '',
    'Exit codes:',
    '  0  No failing problems.',
    '  1  Problems remain (or would remain in --check mode).',
    '  2  Usage, I/O, or parse error.',
    '',
    'Examples:',
    '  node markdown-fixer.mjs --check README.md docs/',
    '  node markdown-fixer.mjs --write README.md AGENTS.md',
    '  node markdown-fixer.mjs --check --json .',
    '',
    'Two trailing spaces are treated as an intentional hard line break.',
    'Fenced code, inline code, frontmatter, math, URLs, and raw HTML blocks are protected.'
  ];
  console.log(lines.join('\n'));
}

function printRules() {
  console.log('ID       Severity  Fixable  Category     Rule');
  console.log('-------  --------  -------  -----------  ------------------------------------------');
  for (const rule of RULES) {
    console.log(
      `${rule.id.padEnd(7)}  ${rule.severity.padEnd(8)}  ${(rule.fixable ? 'yes' : 'no').padEnd(7)}  ${rule.category.padEnd(11)}  ${rule.name} — ${rule.summary}`
    );
  }
}

function parseArgs(argv) {
  const options = {
    mode: 'check',
    json: false,
    quiet: false,
    strict: false,
    help: false,
    version: false,
    listRules: false,
    rules: null,
    ignoreRules: null,
    maxLineLength: 0,
    maxQuoteDepth: DEFAULT_MAX_QUOTE_DEPTH,
    onLimit: 'error',
    paths: []
  };

  const parseQuoteDepth = function (value) {
    if (typeof value !== 'string' || !/^\d{1,9}$/.test(value)) {
      throw new Error('--max-quote-depth requires a non-negative integer.');
    }
    return Number(value);
  };
  const parseOnLimit = function (value) {
    if (value !== 'error' && value !== 'ignore') {
      throw new Error('--on-limit requires error or ignore.');
    }
    return value;
  };

  const addRules = function (value, target) {
    if (!value) throw new Error('A rule list is required after --rule/--ignore-rule.');
    const ids = value.split(',').map((id) => id.trim()).filter(Boolean);
    if (ids.length === 0) throw new Error('A rule list is required after --rule/--ignore-rule.');
    for (const id of ids) {
      if (!RULE_BY_ID.has(id)) throw new Error(`Unknown rule: ${id}`);
    }
    if (!options[target]) options[target] = [];
    options[target].push(...ids);
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--check') options.mode = 'check';
    else if (arg === '--write') options.mode = 'write';
    else if (arg === '--json') options.json = true;
    else if (arg === '--quiet') options.quiet = true;
    else if (arg === '--strict') options.strict = true;
    else if (arg === '--help' || arg === '-h') options.help = true;
    else if (arg === '--version' || arg === '-v') options.version = true;
    else if (arg === '--list-rules') options.listRules = true;
    else if (arg === '--rule') addRules(argv[++i], 'rules');
    else if (arg.startsWith('--rule=')) addRules(arg.slice('--rule='.length), 'rules');
    else if (arg === '--ignore-rule') addRules(argv[++i], 'ignoreRules');
    else if (arg.startsWith('--ignore-rule=')) addRules(arg.slice('--ignore-rule='.length), 'ignoreRules');
    else if (arg === '--max-line-length') {
      const value = Number(argv[++i]);
      if (!Number.isInteger(value) || value <= 0) throw new Error('--max-line-length requires a positive integer.');
      options.maxLineLength = value;
    } else if (arg.startsWith('--max-line-length=')) {
      const value = Number(arg.slice('--max-line-length='.length));
      if (!Number.isInteger(value) || value <= 0) throw new Error('--max-line-length requires a positive integer.');
      options.maxLineLength = value;
    } else if (arg === '--max-quote-depth') {
      options.maxQuoteDepth = parseQuoteDepth(argv[++i]);
    } else if (arg.startsWith('--max-quote-depth=')) {
      options.maxQuoteDepth = parseQuoteDepth(arg.slice('--max-quote-depth='.length));
    } else if (arg === '--on-limit') {
      options.onLimit = parseOnLimit(argv[++i]);
    } else if (arg.startsWith('--on-limit=')) {
      options.onLimit = parseOnLimit(arg.slice('--on-limit='.length));
    } else if (arg === '--') {
      options.paths.push(...argv.slice(i + 1));
      break;
    } else if (arg.startsWith('-')) {
      throw new Error(`Unknown option: ${arg}`);
    } else {
      options.paths.push(arg);
    }
  }

  if (options.rules && options.rules.includes('MDX090') && options.maxLineLength === 0) {
    options.maxLineLength = 120;
  }
  return options;
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`Error: ${error.message}`);
    console.error('Run with --help for usage.');
    process.exit(2);
  }

  if (options.help) {
    printHelp();
    return;
  }
  if (options.version) {
    console.log(`markdown-fixer ${VERSION}`);
    return;
  }
  if (options.listRules) {
    printRules();
    return;
  }

  if (options.paths.length === 0) {
    if (options.mode === 'write') {
      console.error('Error: --write requires explicit files or directories. Refusing to rewrite the whole working tree implicitly.');
      process.exit(2);
    }
    options.paths.push('.');
  }

  const expanded = expandPaths(options.paths);
  if (expanded.errors.length > 0) {
    for (const error of expanded.errors) console.error(`Error: ${error}`);
    process.exitCode = 2;
    return;
  }
  if (expanded.files.length === 0) {
    if (options.json) {
      console.log(JSON.stringify({ tool: 'markdown-fixer', version: VERSION, mode: options.mode, files: [], summary: { files: 0, changed: 0, wouldChange: 0, fixed: 0, refused: 0, problems: summarizeProblems([]) } }, null, 2));
    } else {
      console.log('No Markdown files found.');
    }
    return;
  }

  const results = [];
  let ioError = false;
  for (const file of expanded.files) {
    let original;
    try {
      original = readMarkdownFile(file);
    } catch (error) {
      console.error(`Error: ${path.relative(process.cwd(), file)}: ${error.message}`);
      ioError = true;
      continue;
    }

    const before = diagnose(original, options);
    const fixResult = fixDocument(original, options);
    if (options.mode === 'write' && fixResult.changed && !fixResult.refused) {
      try {
        writeFileAtomic(file, fixResult.text);
      } catch (error) {
        console.error(`Error: cannot write ${path.relative(process.cwd(), file)}: ${error.message}`);
        ioError = true;
        continue;
      }
    }

    const after = fixResult.refused ? before : diagnose(fixResult.text, options);
    const shown = options.mode === 'write' ? after : before;
    const originalFixable = before.filter((problem) => problem.fixable).length;
    const remainingFixable = after.filter((problem) => problem.fixable).length;
    const fixed = options.mode === 'write' ? Math.max(0, originalFixable - remainingFixable) : 0;

    results.push({
      file,
      display: path.relative(process.cwd(), file) || file,
      changed: options.mode === 'write' && fixResult.changed && !fixResult.refused,
      wouldChange: fixResult.changed && !fixResult.refused,
      refused: fixResult.refused,
      reason: fixResult.reason,
      before,
      after,
      shown,
      fixed,
      wouldFix: fixResult.refused ? 0 : before.filter((problem) => problem.fixable).length
    });
  }

  if (!options.json && results.length === 0) {
    process.exitCode = ioError ? 2 : 0;
    return;
  }

  const failing = results.some((result) => result.refused || result.shown.some((problem) => {
    if (problem.severity === 'error' || problem.severity === 'warning') return true;
    return options.strict && problem.severity === 'info';
  }));

  if (options.json) {
    const allProblems = results.flatMap((result) => result.shown);
    const output = {
      tool: 'markdown-fixer',
      version: VERSION,
      mode: options.mode,
      files: results.map((result) => ({
        path: result.display,
        changed: result.changed,
        wouldChange: result.wouldChange,
        refused: result.refused,
        reason: result.reason || undefined,
        fixed: result.fixed,
        wouldFix: result.wouldFix,
        problems: result.shown.map((problem) => ({
          rule: problem.rule,
          name: problem.name,
          severity: problem.severity,
          fixable: problem.fixable,
          line: problem.line,
          column: problem.column,
          message: problem.message
        }))
      })),
      summary: {
        files: results.length,
        changed: results.filter((result) => result.changed).length,
        wouldChange: results.filter((result) => result.wouldChange).length,
        fixed: results.reduce((sum, result) => sum + result.fixed, 0),
        refused: results.filter((result) => result.refused).length,
        problems: summarizeProblems(allProblems)
      }
    };
    console.log(JSON.stringify(output, null, 2));
    process.exitCode = ioError ? 2 : (failing ? 1 : 0);
    return;
  }

  if (!options.quiet) {
    for (const result of results) {
      const header = result.display;
      console.log(header);
      if (result.refused) {
        const prefix = options.mode === 'write' ? 'Not modified' : 'Structural error';
        console.log(`  ERROR  ${prefix}: ${result.reason}`);
      }
      if (result.shown.length === 0) {
        if (!result.refused) console.log('  No problems.');
      } else {
        for (const problem of result.shown) {
          console.log(formatProblem(problem));
        }
      }
      if (options.mode === 'write' && result.changed && !result.refused) {
        console.log(`  Fixed ${result.fixed} issue(s).`);
      }
      console.log('');
    }
  }

  const allShown = results.flatMap((result) => result.shown);
  const summary = summarizeProblems(allShown);
  const changedCount = results.filter((result) => result.changed).length;
  const fixedCount = results.reduce((sum, result) => sum + result.fixed, 0);
  const refusedCount = results.filter((result) => result.refused).length;

  if (options.mode === 'write') {
    if (fixedCount > 0 || changedCount > 0) {
      console.log(`Fixed ${fixedCount} issue(s) in ${changedCount} file(s).`);
    }
    if (summary.total > 0) {
      console.log(`${summary.total} problem(s) remain: ${summary.fixable} fixable, ${summary.errors} error(s), ${summary.warnings} warning(s), ${summary.info} info.`);
    } else if (!ioError && refusedCount === 0) {
      console.log('No remaining Markdown issues.');
    }
  } else if (summary.total > 0) {
    console.log(`${summary.total} problem(s) in ${results.length} file(s): ${summary.fixable} fixable, ${summary.errors} error(s), ${summary.warnings} warning(s), ${summary.info} info.`);
  } else if (!ioError && refusedCount === 0) {
    console.log(`No Markdown issues found in ${results.length} file(s).`);
  }

  if (ioError) {
    console.log('Some files could not be read or written; see the errors above.');
  }

  if (refusedCount > 0) {
    const message = options.mode === 'write'
      ? `${refusedCount} file(s) were not modified because of structural or frontmatter ambiguity.`
      : `${refusedCount} file(s) need manual review before safe fixes.`;
    console.log(message);
  }

  process.exitCode = ioError ? 2 : (failing ? 1 : 0);
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) {
  try {
    main();
  } catch (error) {
    console.error(`Error: ${error && error.message ? error.message : error}`);
    process.exitCode = 2;
  }
}

export {
  RULES,
  VERSION,
  diagnose,
  fixDocument,
  buildMask,
  splitTableCells,
  scanDocument,
  matchDefinition,
  parseArgs
};
