//! 空行與行尾空白的修正：`computeBlankLineFixes`、`applyBlankLineFixes`、
//! `applyTrailingWhitespace`、`applyBlankLineNormalization`。MDX031 自 v1.1.0 起只回報不修正，
//! 所以沒有 `fixListStyle`。

use std::collections::{BTreeSet, HashMap};

use crate::Problem;
use crate::jsregex::re;
use crate::jsstr::{self, JsString, is_trim_empty};
use crate::rules::{Enabled, Overrides, make_problem};
use crate::scan::{
    LineInfo, Scan, is_blank, is_paragraph_like, is_protected_block, is_protected_line,
    is_top_level_line, parse_quote_prefix, scan_document_with,
};
use crate::table::find_tables;

const SPACE: u16 = b' ' as u16;
const GT: u16 = b'>' as u16;
const TAB: u16 = b'\t' as u16;

/// `computeBlankLineFixes` 的結果：要在前／後補空行的行索引與對應問題。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BlankLineFixes {
    pub(crate) before: BTreeSet<usize>,
    pub(crate) after: BTreeSet<usize>,
    /// 引言裡的 fence 要插入的行（`>`、`> >`…），以 fence 的開頭行（before）或結尾行（after）
    /// 為鍵；不在表中的位置插入完全空白的行。完全空白的行會結束引言，把它拆成兩個。
    pub(crate) fill: HashMap<usize, JsString>,
    pub(crate) issues: Vec<Problem>,
}

/// JS `isQuoteBlankLine`：引言裡的空行（`>`、`> >`…，標記後面只有空白）。
fn is_quote_blank_line(line: &[u16]) -> bool {
    parse_quote_prefix(line).is_some_and(|q| is_trim_empty(q.rest))
}

fn starts_with_space_or_tab(line: &[u16]) -> bool {
    matches!(line.first(), Some(&c) if c == SPACE || c == TAB)
}

/// JS `computeBlankLineFixes(lines, info, scan, isEnabled)`（MDX013、MDX052、MDX062、MDX082）。
pub(crate) fn compute_blank_line_fixes(
    lines: &[JsString],
    info: &[LineInfo],
    scan: &Scan,
    enabled: &Enabled,
) -> BlankLineFixes {
    let mut fixes = BlankLineFixes::default();
    let non_empty = |i: usize| lines.get(i).is_some_and(|l| !is_trim_empty(l));

    if enabled.is("MDX013") {
        for i in 0..lines.len() {
            if is_protected_line(&info[i]) || !re!(r"^#{1,6}[ \t]").test(&lines[i]) {
                continue;
            }
            if i > 0 && is_paragraph_like(&lines[i - 1], &info[i - 1]) {
                fixes.before.insert(i);
                fixes.issues.push(make_problem(
                    "MDX013",
                    i + 1,
                    1,
                    "Heading is directly attached to the preceding paragraph. Add a blank line before it.",
                    Overrides::NONE,
                ));
            }
            if i + 1 < lines.len() && is_paragraph_like(&lines[i + 1], &info[i + 1]) {
                fixes.after.insert(i);
                fixes.issues.push(make_problem(
                    "MDX013",
                    i + 1,
                    1,
                    "Heading is directly attached to the following paragraph. Add a blank line after it.",
                    Overrides::NONE,
                ));
            }
        }
    }

    if enabled.is("MDX052") {
        let mut i = 0;
        while i < lines.len() {
            if !info[i].fence_open || !is_top_level_line(&lines[i]) {
                i += 1;
                continue;
            }
            // 引言裡的 fence 以該深度的 `>` 行分隔。標記後面還有縮排時 fence 可能屬於清單項目，
            // 任何空行都會讓清單變鬆散，所以略過。
            let quote = if lines[i].first() == Some(&GT) {
                parse_quote_prefix(&lines[i])
            } else {
                None
            };
            if quote
                .as_ref()
                .is_some_and(|q| starts_with_space_or_tab(q.rest))
            {
                i += 1;
                continue;
            }
            let quote_fill: Option<JsString> = quote.as_ref().map(|q| {
                let mut fill = Vec::with_capacity(q.depth * 2);
                for d in 0..q.depth {
                    if d > 0 {
                        fill.push(SPACE);
                    }
                    fill.push(GT);
                }
                fill
            });
            let separated = |k: usize| {
                lines.get(k).is_none_or(|l| {
                    is_trim_empty(l) || (quote_fill.is_some() && is_quote_blank_line(l))
                })
            };
            if i > 0 && !separated(i - 1) {
                fixes.before.insert(i);
                if let Some(fill) = &quote_fill {
                    fixes.fill.insert(i, fill.clone());
                }
                fixes.issues.push(make_problem(
                    "MDX052",
                    i + 1,
                    1,
                    "Fenced code block starts directly after another block. Add a blank line before it.",
                    Overrides::NONE,
                ));
            }
            let mut close = i + 1;
            while close < lines.len() && !info[close].fence_close {
                close += 1;
            }
            if close < lines.len() {
                if close + 1 < lines.len() && !separated(close + 1) {
                    fixes.after.insert(close);
                    if let Some(fill) = &quote_fill {
                        fixes.fill.insert(close, fill.clone());
                    }
                    fixes.issues.push(make_problem(
                        "MDX052",
                        close + 1,
                        1,
                        "Fenced code block ends directly before another block. Add a blank line after it.",
                        Overrides::NONE,
                    ));
                }
                i = close;
            }
            i += 1;
        }
    }

    if enabled.is("MDX062") {
        for table in find_tables(lines, info) {
            if starts_with_space_or_tab(&lines[table.start]) {
                continue;
            }
            if table.start > 0 && non_empty(table.start - 1) {
                fixes.before.insert(table.start);
                fixes.issues.push(make_problem(
                    "MDX062",
                    table.start + 1,
                    1,
                    "Table starts directly after another block. Add a blank line before it.",
                    Overrides::NONE,
                ));
            }
            if non_empty(table.end + 1) {
                fixes.after.insert(table.end);
                fixes.issues.push(make_problem(
                    "MDX062",
                    table.end + 2,
                    1,
                    "Table ends directly before another block. Add a blank line after it.",
                    Overrides::NONE,
                ));
            }
        }
    }

    if enabled.is("MDX082") {
        if let Some(end) = scan.frontmatter_end {
            if non_empty(end + 1) {
                fixes.after.insert(end);
                fixes.issues.push(make_problem(
                    "MDX082",
                    end + 2,
                    1,
                    "Frontmatter must be followed by a blank line.",
                    Overrides::NONE,
                ));
            }
        }
    }

    fixes
}

/// JS `applyBlankLineFixes(text, isEnabled)`。
pub(crate) fn apply_blank_line_fixes(text: &[u16], enabled: &Enabled) -> JsString {
    let scan = scan_document_with(text, enabled.limits);
    let fixes = compute_blank_line_fixes(&scan.lines, &scan.info, &scan, enabled);
    if fixes.before.is_empty() && fixes.after.is_empty() {
        return text.to_vec();
    }
    let lines = &scan.lines;
    let needs_separator = |line: &[u16], i: usize| {
        !is_trim_empty(line) && !(fixes.fill.contains_key(&i) && is_quote_blank_line(line))
    };
    let fill_for = |i: usize| fixes.fill.get(&i).cloned().unwrap_or_default();
    let mut output: Vec<JsString> = Vec::with_capacity(lines.len() + 8);
    for i in 0..lines.len() {
        if fixes.before.contains(&i) && output.last().is_some_and(|l| needs_separator(l, i)) {
            output.push(fill_for(i));
        }
        output.push(lines[i].clone());
        if fixes.after.contains(&i) && lines.get(i + 1).is_some_and(|l| needs_separator(l, i)) {
            output.push(fill_for(i));
        }
    }
    jsstr::join_lines(&output)
}

/// 行尾 `[ \t]+` 的長度。
fn trailing_ws_len(line: &[u16]) -> usize {
    line.iter()
        .rev()
        .take_while(|&&c| c == SPACE || c == TAB)
        .count()
}

/// JS `applyTrailingWhitespace(text, isEnabled)`：保留剛好兩個空格的 hard break。
pub(crate) fn apply_trailing_whitespace(text: &[u16], enabled: &Enabled) -> JsString {
    if !enabled.is("MDX004") {
        return text.to_vec();
    }
    let scan = scan_document_with(text, enabled.limits);
    let lines: Vec<JsString> = scan
        .lines
        .iter()
        .zip(&scan.info)
        .map(|(line, info)| {
            if is_protected_block(info) {
                return line.clone();
            }
            let ws = trailing_ws_len(line);
            if ws == 0 {
                return line.clone();
            }
            let tail = &line[line.len() - ws..];
            let two_spaces = tail.len() == 2 && tail.iter().all(|&c| c == SPACE);
            if !is_blank(line) && two_spaces {
                return line.clone();
            }
            if is_blank(line) {
                return Vec::new();
            }
            let head = &line[..line.len() - ws];
            if tail.ends_with(&[SPACE, SPACE]) {
                return jsstr::concat(&[head, &[SPACE, SPACE]]);
            }
            head.to_vec()
        })
        .collect();
    jsstr::join_lines(&lines)
}

/// JS `applyBlankLineNormalization(text, isEnabled)`（MDX006、MDX005、MDX003）。
pub(crate) fn apply_blank_line_normalization(text: &[u16], enabled: &Enabled) -> JsString {
    let scan = scan_document_with(text, enabled.limits);
    let lines = &scan.lines;
    let removable = |i: usize| is_blank(&lines[i]) && !is_protected_block(&scan.info[i]);

    let mut output: Vec<JsString> = Vec::with_capacity(lines.len());
    let mut i = 0;
    if enabled.is("MDX006") {
        while i < lines.len() && removable(i) {
            i += 1;
        }
        // 移除這些空行會讓 U+FEFF 落在檔首、變成 BOM，該行的意義也跟著改變。
        if lines.get(i).is_some_and(|l| l.first() == Some(&0xFEFF)) {
            i = 0;
        }
    }

    while i < lines.len() {
        if !removable(i) {
            output.push(lines[i].clone());
            i += 1;
            continue;
        }
        let mut j = i;
        while j < lines.len() && removable(j) {
            j += 1;
        }
        let at_end = j >= lines.len();
        if at_end {
            if !enabled.is("MDX003") {
                output.extend(lines[i..j].iter().cloned());
            }
        } else if enabled.is("MDX005") {
            output.push(Vec::new());
        } else {
            output.extend(lines[i..j].iter().cloned());
        }
        i = j;
    }

    let mut result = jsstr::join_lines(&output);
    if enabled.is("MDX003") && !result.is_empty() {
        // `result.replace(/\n*$/, '\n')`：把結尾連續的 `\n` 換成恰好一個。
        while result.last() == Some(&(b'\n' as u16)) {
            result.pop();
        }
        result.push(b'\n' as u16);
    }
    result
}
