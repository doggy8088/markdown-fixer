//! 空行與行尾空白的修正：`computeBlankLineFixes`、`applyBlankLineFixes`、
//! `applyTrailingWhitespace`、`applyBlankLineNormalization`、`fixListStyle`
//! （JS 1679–1759、1775–1791、1870–1911、1956–1990 行）。

use std::collections::{BTreeSet, HashMap};

use crate::Problem;
use crate::jsregex::re;
use crate::jsstr::{self, JsString, is_trim_empty};
use crate::rules::{Enabled, Overrides, make_problem};
use crate::scan::{
    LineInfo, Scan, is_blank, is_paragraph_like, is_protected_block, is_protected_line,
    is_thematic_break, is_top_level_line, scan_document,
};
use crate::table::find_tables;

const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;

/// `computeBlankLineFixes` 的結果：要在前／後補空行的行索引與對應問題。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BlankLineFixes {
    pub(crate) before: BTreeSet<usize>,
    pub(crate) after: BTreeSet<usize>,
    pub(crate) issues: Vec<Problem>,
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
            if i > 0 && non_empty(i - 1) {
                fixes.before.insert(i);
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
                if non_empty(close + 1) {
                    fixes.after.insert(close);
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
    let scan = scan_document(text);
    let fixes = compute_blank_line_fixes(&scan.lines, &scan.info, &scan, enabled);
    if fixes.before.is_empty() && fixes.after.is_empty() {
        return text.to_vec();
    }
    let lines = &scan.lines;
    let mut output: Vec<JsString> = Vec::with_capacity(lines.len() + 8);
    for i in 0..lines.len() {
        if fixes.before.contains(&i) && output.last().is_some_and(|l| !is_trim_empty(l)) {
            output.push(Vec::new());
        }
        output.push(lines[i].clone());
        if fixes.after.contains(&i) && lines.get(i + 1).is_some_and(|l| !is_trim_empty(l)) {
            output.push(Vec::new());
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
    let scan = scan_document(text);
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
    let scan = scan_document(text);
    let lines = &scan.lines;
    let removable = |i: usize| is_blank(&lines[i]) && !is_protected_block(&scan.info[i]);

    let mut output: Vec<JsString> = Vec::with_capacity(lines.len());
    let mut i = 0;
    if enabled.is("MDX006") {
        while i < lines.len() && removable(i) {
            i += 1;
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

/// JS `fixListStyle(text, isEnabled)`（MDX031）：同一層清單統一用第一個項目的符號。
pub(crate) fn fix_list_style(text: &[u16], enabled: &Enabled) -> JsString {
    if !enabled.is("MDX031") {
        return text.to_vec();
    }
    let scan = scan_document(text);
    let mut lines = scan.lines.clone();
    let mut runs: HashMap<usize, u16> = HashMap::new();

    for (i, info) in scan.info.iter().enumerate() {
        let line = &scan.lines[i];
        if is_protected_line(info) {
            if is_top_level_line(line) && !info.fence_content && !info.indented_code {
                runs.clear();
            }
            continue;
        }
        if is_thematic_break(line) {
            runs.clear();
            continue;
        }
        let Some(caps) = re!(r"^( {0,3})([-*+])([ \t]+)(.*)$").exec(line) else {
            if !is_trim_empty(line) && !starts_with_space_or_tab(line) {
                runs.clear();
            }
            continue;
        };
        let indent = caps.str(line, 1);
        let marker = caps.str(line, 2).first().copied().unwrap_or(b'-' as u16);
        match runs.get(&indent.len()) {
            None => {
                runs.insert(indent.len(), marker);
            }
            Some(&canonical) if canonical != marker => {
                lines[i] =
                    jsstr::concat(&[indent, &[canonical], caps.str(line, 3), caps.str(line, 4)]);
            }
            Some(_) => {}
        }
    }

    jsstr::join_lines(&lines)
}
