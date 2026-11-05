//! 結構診斷：只回報、不修正的整份文件檢查（JS 1139–1430、1761–1830、1913 行附近）。
//!
//! 每個函式對應 oracle 的一個 `compute*` 函式，輸入是 `scanDocument` 切好的行
//! （UTF-16）與每行的 [`LineInfo`]，回傳的問題順序與 JS 完全相同。欄號一律是
//! UTF-16 index + 1。正規表示式盡量保留 JS 原文，交給 [`crate::jsregex`] 比對，
//! 讓 `\s`、`.`、lookbehind 等語意與 JS 一致。

use std::collections::{HashMap, HashSet};

use crate::Problem;
use crate::Severity;
use crate::jsregex::re;
use crate::jsstr::{
    JsString, code_point_len, index_of, is_js_space, is_trim_empty, to_string, trim, u,
};
use crate::mask::{MaskOptions, build_mask, split_masked};
use crate::rules::{Enabled, Overrides, make_problem};
use crate::scan::{
    LineInfo, Scan, is_atx_heading, is_blank, is_list_line, is_paragraph_like, is_protected_block,
    is_protected_line, is_thematic_break, is_top_level_line, match_definition,
};
use crate::table::{find_tables, normalize_label, split_table_cells};

const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const PIPE: u16 = b'|' as u16;

/// `/^[ \t]/.test(line)`。
fn starts_with_space_or_tab(line: &[u16]) -> bool {
    matches!(line.first(), Some(&c) if c == SPACE || c == TAB)
}

/// `{ maskBareUrls: false, maskLinkDestinations: false }`（MDX008、MDX009 共用）。
const NO_URLS_NO_DESTINATIONS: MaskOptions = MaskOptions {
    mask_bare_urls: false,
    mask_link_destinations: false,
    mask_reference_labels: true,
    mask_link_text: false,
};

// --- 線性比對器 -------------------------------------------------------------
// 下列函式取代 oracle 中在回溯式 regex 引擎上會退化成平方時間的樣式（JS 端同樣是
// 平方，但 V8 的常數小很多）。每個函式的結果必須與原樣式逐位相同，測試
// `matchers` 會在小字母表上窮舉所有短字串，與 `jsregex` 跑原樣式的結果比對。

const HASH: u16 = b'#' as u16;
const LBRACKET: u16 = b'[' as u16;
const RBRACKET: u16 = b']' as u16;
const LPAREN: u16 = b'(' as u16;
const RPAREN: u16 = b')' as u16;
const BANG: u16 = b'!' as u16;
const LF: u16 = b'\n' as u16;

/// JS `.` 不能比對的行終止字元（與 `jsregex` 的 `Dot` 相同）。
fn is_line_terminator(c: u16) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

fn is_space_or_tab(c: u16) -> bool {
    c == SPACE || c == TAB
}

fn leading_count(s: &[u16], pred: impl Fn(u16) -> bool) -> usize {
    s.iter().take_while(|&&c| pred(c)).count()
}

fn trailing_count(s: &[u16], pred: impl Fn(u16) -> bool) -> usize {
    s.iter().rev().take_while(|&&c| pred(c)).count()
}

/// `match_atx_heading` 的結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtxHeading {
    indent: usize,
    level: usize,
    /// 第 3 組（標題文字）的範圍。
    text: (usize, usize),
}

/// `/^( {0,3})(#{1,6})[ \t]+(.*?)[ \t]*$/.exec(line)` 的線性版。
///
/// `[ \t]+` 貪婪吃完整段空白，`(.*?)` 取最短、讓剩下的只有行尾空白；`.` 不跨
/// 行終止字元，所以其後只要出現行終止字元就整體失敗。
fn match_atx_heading(line: &[u16]) -> Option<AtxHeading> {
    let indent = leading_count(line, |c| c == SPACE);
    if indent > 3 {
        return None;
    }
    let level = leading_count(&line[indent..], |c| c == HASH);
    if !(1..=6).contains(&level) {
        return None;
    }
    let after = indent + level;
    let ws = leading_count(&line[after..], is_space_or_tab);
    if ws == 0 {
        return None;
    }
    let start = after + ws;
    let rest = &line[start..];
    if rest.iter().any(|&c| is_line_terminator(c)) {
        return None;
    }
    let end = line.len() - trailing_count(rest, is_space_or_tab);
    Some(AtxHeading {
        indent,
        level,
        text: (start, end),
    })
}

/// `text.replace(/[ \t]+#+[ \t]*$/, '')` 的線性版：去掉結尾的 closing `#` 序列。
fn strip_closing_hashes(text: &[u16]) -> &[u16] {
    let body = &text[..text.len() - trailing_count(text, is_space_or_tab)];
    let hashes = trailing_count(body, |c| c == HASH);
    if hashes == 0 {
        return text;
    }
    let before = &body[..body.len() - hashes];
    let ws = trailing_count(before, is_space_or_tab);
    if ws == 0 {
        return text;
    }
    &text[..before.len() - ws]
}

/// `/^( {0,3})([-*+])([ \t]+)(.*)$/.exec(line)` 的線性版：回傳 (縮排, marker)。
fn match_bullet(line: &[u16]) -> Option<(usize, u16)> {
    let indent = leading_count(line, |c| c == SPACE);
    if indent > 3 {
        return None;
    }
    let marker = *line.get(indent)?;
    if !matches!(marker, 0x2D | 0x2A | 0x2B) {
        return None;
    }
    let rest = &line[indent + 1..];
    let ws = leading_count(rest, is_space_or_tab);
    if ws == 0 || rest.iter().any(|&c| is_line_terminator(c)) {
        return None;
    }
    Some((indent, marker))
}

/// `[ \t]*` 之後的位置。
fn skip_space_or_tab(text: &[u16], from: usize) -> usize {
    from + leading_count(text.get(from..).unwrap_or(&[]), is_space_or_tab)
}

/// MDX071 `/\[[ \t]*\]\([^)]*\)|\][ \t]*\([ \t]*\)|!\[[ \t]*\]\([^)]*\)/` 第一個比對的起點。
fn find_empty_link(text: &[u16]) -> Option<usize> {
    // `[^)]*\)` 成立 ⇔ 位置 p 之後還有 `)`。
    let last_close = text.iter().rposition(|&c| c == RPAREN);
    let close_from = |p: usize| last_close.is_some_and(|q| q >= p);
    let empty_text_link = |b: usize| {
        if text.get(b) != Some(&LBRACKET) {
            return false;
        }
        let p = skip_space_or_tab(text, b + 1);
        text.get(p) == Some(&RBRACKET) && text.get(p + 1) == Some(&LPAREN) && close_from(p + 2)
    };
    let empty_destination = |s: usize| {
        if text.get(s) != Some(&RBRACKET) {
            return false;
        }
        let p = skip_space_or_tab(text, s + 1);
        if text.get(p) != Some(&LPAREN) {
            return false;
        }
        let q = skip_space_or_tab(text, p + 1);
        text.get(q) == Some(&RPAREN)
    };
    (0..text.len()).find(|&s| {
        empty_text_link(s) || empty_destination(s) || (text[s] == BANG && empty_text_link(s + 1))
    })
}

/// MDX072 `/\]\s+\([^)]*\)/` 第一個比對的起點。
fn find_spaced_link(text: &[u16]) -> Option<usize> {
    let last_close = text.iter().rposition(|&c| c == RPAREN);
    (0..text.len()).find(|&s| {
        if text[s] != RBRACKET {
            return false;
        }
        let ws = leading_count(&text[s + 1..], is_js_space);
        let p = s + 1 + ws;
        ws > 0 && text.get(p) == Some(&LPAREN) && last_close.is_some_and(|q| q > p)
    })
}

/// 對每個位置 p，回傳 `p` 之後（含）第一個 `stop` 字元的位置，沒有則為 `len`。
fn next_stop(text: &[u16], stop: impl Fn(u16) -> bool) -> Vec<usize> {
    let mut next = vec![text.len(); text.len() + 1];
    for p in (0..text.len()).rev() {
        next[p] = if stop(text[p]) { p } else { next[p + 1] };
    }
    next
}

/// `/!?\[([^\]\n]*)\]\[([^\]\n]*)\]/g` 的一個比對。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FullReference {
    start: usize,
    first: (usize, usize),
    second: (usize, usize),
}

/// MDX073 `/!?\[([^\]\n]*)\]\[([^\]\n]*)\]/g` 全部比對的線性版。
fn find_full_references(text: &[u16]) -> Vec<FullReference> {
    let next = next_stop(text, |c| c == RBRACKET || c == LF);
    let label_end = |from: usize| {
        let q = *next.get(from)?;
        (text.get(q) == Some(&RBRACKET)).then_some(q)
    };
    let match_at = |s: usize| -> Option<(FullReference, usize)> {
        let b = if text[s] == BANG { s + 1 } else { s };
        if text.get(b) != Some(&LBRACKET) {
            return None;
        }
        let q1 = label_end(b + 1)?;
        if text.get(q1 + 1) != Some(&LBRACKET) {
            return None;
        }
        let q2 = label_end(q1 + 2)?;
        let found = FullReference {
            start: s,
            first: (b + 1, q1),
            second: (q1 + 2, q2),
        };
        Some((found, q2 + 1))
    };
    let mut out = Vec::new();
    let mut s = 0;
    while s < text.len() {
        match match_at(s) {
            Some((found, end)) => {
                out.push(found);
                s = end;
            }
            None => s += 1,
        }
    }
    out
}

/// MDX084 `/！?［[^］\n]+］（[^）\n]+）/.test(line)` 的線性版（`！?` 不影響是否比對成功）。
fn has_fullwidth_link(line: &[u16]) -> bool {
    const OPEN_BRACKET: u16 = 0xFF3B; // ［
    const CLOSE_BRACKET: u16 = 0xFF3D; // ］
    const OPEN_PAREN: u16 = 0xFF08; // （
    const CLOSE_PAREN: u16 = 0xFF09; // ）
    let bracket = next_stop(line, |c| c == CLOSE_BRACKET || c == LF);
    let paren = next_stop(line, |c| c == CLOSE_PAREN || c == LF);
    (0..line.len()).any(|b| {
        if line[b] != OPEN_BRACKET {
            return false;
        }
        let q = bracket[b + 1];
        if q <= b + 1 || line.get(q) != Some(&CLOSE_BRACKET) || line.get(q + 1) != Some(&OPEN_PAREN)
        {
            return false;
        }
        let Some(&r) = paren.get(q + 2) else {
            return false;
        };
        r > q + 2 && line.get(r) == Some(&CLOSE_PAREN)
    })
}

/// JS `computeHeadingStructure`：MDX014–MDX019。
pub(crate) fn compute_heading_structure(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    struct Heading {
        level: usize,
        line: usize,
    }
    let mut issues = Vec::new();
    let mut headings: Vec<Heading> = Vec::new();

    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_line(line_info) {
            continue;
        }
        if re!(r"^( {0,3})#{1,6}[ \t]*$").test(line) {
            if enabled.is("MDX016") {
                issues.push(make_problem(
                    "MDX016",
                    i + 1,
                    1,
                    "Heading has no text. Remove it or give it a title.",
                    Overrides::NONE,
                ));
            }
            continue;
        }
        let Some(heading) = match_atx_heading(line) else {
            continue;
        };
        let (indent, level) = (heading.indent, heading.level);
        let text = trim(strip_closing_hashes(&line[heading.text.0..heading.text.1]));

        if indent > 0 && enabled.is("MDX014") {
            issues.push(make_problem(
                "MDX014",
                i + 1,
                1,
                "Heading is indented. Confirm it is not a top-level heading with accidental indentation; inside a list item the indent may be intentional.",
                Overrides::NOT_FIXABLE,
            ));
        }
        if text.is_empty() {
            if enabled.is("MDX016") {
                issues.push(make_problem(
                    "MDX016",
                    i + 1,
                    1,
                    "Heading has no text. Remove it or give it a title.",
                    Overrides::NONE,
                ));
            }
            continue;
        }
        // `/[.:;!?。：；！？]$/`：只看最後一個 code unit。
        let ends_with_punct = text
            .last()
            .is_some_and(|c| u(".:;!?。：；！？").contains(c));
        if ends_with_punct && enabled.is("MDX017") {
            issues.push(make_problem(
                "MDX017",
                i + 1,
                1,
                "Heading ends with sentence punctuation. Headings usually read better without a trailing period, colon, or similar mark.",
                Overrides::severity(Severity::Info),
            ));
        }
        if indent == 0 {
            headings.push(Heading { level, line: i + 1 });
        }
    }

    if let Some(first) = headings.first() {
        if enabled.is("MDX018") && first.level != 1 {
            issues.push(make_problem(
                "MDX018",
                first.line,
                1,
                "The first heading is not an H1. Start the outline with one H1 unless the document intentionally starts in a section.",
                Overrides::severity(Severity::Info),
            ));
        }
    }

    let h1s: Vec<&Heading> = headings.iter().filter(|h| h.level == 1).collect();
    if h1s.len() > 1 && enabled.is("MDX019") {
        for heading in &h1s[1..] {
            issues.push(make_problem(
                "MDX019",
                heading.line,
                1,
                "Document has more than one H1. Use H2 for subsections and keep one H1 for the page title.",
                Overrides::NONE,
            ));
        }
    }

    if enabled.is("MDX015") {
        let mut previous: Option<&Heading> = None;
        for heading in &headings {
            if let Some(prev) = previous {
                if heading.level > prev.level + 1 {
                    issues.push(make_problem(
                        "MDX015",
                        heading.line,
                        1,
                        format!(
                            "Heading jumps from H{} to H{}. Use the next level instead of skipping one.",
                            prev.level, heading.level
                        ),
                        Overrides::NONE,
                    ));
                }
            }
            previous = Some(heading);
        }
    }

    issues
}

/// JS `computeListStructure`：MDX031、MDX035、MDX032。
pub(crate) fn compute_list_structure(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    let mut issues = Vec::new();
    // indent → 這個縮排層第一個項目的 marker（JS 的 `runs` Map）。
    let mut runs: HashMap<usize, u16> = HashMap::new();

    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_line(line_info) {
            if is_top_level_line(line) && !line_info.fence_content && !line_info.indented_code {
                runs.clear();
            }
            continue;
        }
        if is_thematic_break(line) {
            runs.clear();
            continue;
        }
        let Some((indent, marker)) = match_bullet(line) else {
            if !is_trim_empty(line) && !starts_with_space_or_tab(line) {
                runs.clear();
            }
            continue;
        };
        let Some(&first) = runs.get(&indent) else {
            runs.insert(indent, marker);
            continue;
        };
        if first != marker && enabled.is("MDX031") {
            issues.push(make_problem(
                "MDX031",
                i + 1,
                indent + 1,
                format!(
                    "List uses \"{}\" but the first item in this list uses \"{}\". Use one bullet marker per list.",
                    to_string(&[marker]),
                    to_string(&[first])
                ),
                Overrides::NONE,
            ));
        }
    }

    if enabled.is("MDX035") {
        for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
            if is_protected_line(line_info) {
                continue;
            }
            if re!(r"^(?: {0,3})(?:[-*+]|\d{1,9}[.)])[ \t]*$").test(line) {
                issues.push(make_problem(
                    "MDX035",
                    i + 1,
                    1,
                    "Empty list item. Remove it or add content.",
                    Overrides::NONE,
                ));
            }
        }
    }

    if enabled.is("MDX032") {
        let mut in_list = false;
        let mut last_line_was_item = false;
        for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
            if is_protected_line(line_info) {
                if is_top_level_line(line) && !line_info.fence_content && !line_info.indented_code {
                    in_list = false;
                    last_line_was_item = false;
                }
                continue;
            }
            if is_list_line(line) {
                let after_paragraph = i > 0
                    && match (lines.get(i - 1), info.get(i - 1)) {
                        (Some(prev), Some(prev_info)) => is_paragraph_like(prev, prev_info),
                        _ => false,
                    };
                if !in_list && after_paragraph {
                    issues.push(make_problem(
                        "MDX032",
                        i + 1,
                        1,
                        "List starts directly after a paragraph. Add a blank line before it so the list boundary is unambiguous.",
                        Overrides::NOT_FIXABLE,
                    ));
                }
                in_list = true;
                last_line_was_item = true;
                continue;
            }
            if is_trim_empty(line) {
                last_line_was_item = false;
                continue;
            }
            if starts_with_space_or_tab(line) {
                continue;
            }
            if in_list && last_line_was_item && is_paragraph_like(line, line_info) {
                issues.push(make_problem(
                    "MDX032",
                    i + 1,
                    1,
                    "List ends directly before a paragraph, which may be read as a lazy continuation of the last item. Add a blank line after the list.",
                    Overrides::NOT_FIXABLE,
                ));
            }
            in_list = false;
            last_line_was_item = false;
        }
    }

    issues
}

/// JS `computeTableStructure`：MDX061、MDX063、MDX060。
pub(crate) fn compute_table_structure(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    let mut issues = Vec::new();
    let tables = find_tables(lines, info);
    let delimiter_lines: HashSet<usize> = tables.iter().map(|t| t.start + 1).collect();
    let empty: JsString = Vec::new();
    let line_at = |i: usize| lines.get(i).unwrap_or(&empty);

    let check_code_pipes = |issues: &mut Vec<Problem>, line_index: usize| {
        if !enabled.is("MDX063") {
            return;
        }
        let line = line_at(line_index);
        let mask = build_mask(line, MaskOptions::default());
        for segment in split_masked(line, &mask) {
            if segment.protected && re!(r"(?<!\\)\|").test(segment.text) {
                issues.push(make_problem(
                    "MDX063",
                    line_index + 1,
                    segment.start + 1,
                    "A pipe inside code or a URL in a table cell still splits the cell. Escape it as \\| in code, or use %7C in a URL.",
                    Overrides::NONE,
                ));
                break;
            }
        }
    };

    for table in &tables {
        let header_cells = split_table_cells(line_at(table.start)).len();
        let delimiter_cells = split_table_cells(line_at(table.start + 1)).len();
        if header_cells != delimiter_cells && enabled.is("MDX061") {
            issues.push(make_problem(
                "MDX061",
                table.start + 1,
                1,
                format!(
                    "Table header has {header_cells} cell(s) but the delimiter row has {delimiter_cells}."
                ),
                Overrides::NONE,
            ));
        }
        check_code_pipes(&mut issues, table.start);
        for i in table.start + 2..=table.end {
            let cells = split_table_cells(line_at(i)).len();
            if cells != delimiter_cells && enabled.is("MDX061") {
                issues.push(make_problem(
                    "MDX061",
                    i + 1,
                    1,
                    format!(
                        "Table row has {cells} cell(s) but the table has {delimiter_cells} column(s)."
                    ),
                    Overrides::NONE,
                ));
            }
            check_code_pipes(&mut issues, i);
        }
    }

    if enabled.is("MDX060") {
        for (i, line) in lines.iter().enumerate() {
            if !delimiter_lines.contains(&i) {
                continue;
            }
            // `parts.slice(0, k).join('|').length + (k > 0 ? 1 : 0)` 就是第 k 段的起點。
            let mut offset = 0;
            for part in line.split(|&c| c == PIPE) {
                if let Some(caps) = re!(r"^([ \t]*)(:?)(-{1,2})(:?)([ \t]*)$").exec(part) {
                    issues.push(make_problem(
                        "MDX060",
                        i + 1,
                        offset + caps.str(part, 1).len() + 1,
                        "Table delimiter cell has fewer than three hyphens.",
                        Overrides::NONE,
                    ));
                    break;
                }
                offset += part.len() + 1;
            }
        }
    }

    issues
}

/// JS `computeReferenceStructure`：MDX074、MDX071、MDX075、MDX072、MDX073。
pub(crate) fn compute_reference_structure(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    let mut issues = Vec::new();
    let mut definitions: HashSet<JsString> = HashSet::new();

    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_block(line_info) {
            continue;
        }
        let Some((label_start, label_end)) = match_definition(line) else {
            continue;
        };
        let raw_label = &line[label_start..label_end];
        let label = normalize_label(raw_label);
        if label.is_empty() {
            continue;
        }
        if definitions.contains(&label) {
            if enabled.is("MDX074") {
                issues.push(make_problem(
                    "MDX074",
                    i + 1,
                    1,
                    format!(
                        "Reference definition \"{}\" is declared more than once.",
                        to_string(trim(raw_label))
                    ),
                    Overrides::NONE,
                ));
            }
        } else {
            definitions.insert(label);
        }
    }

    let mask_options = MaskOptions {
        mask_bare_urls: false,
        mask_link_destinations: false,
        mask_reference_labels: false,
        mask_link_text: false,
    };
    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_line(line_info) {
            continue;
        }
        let mask = build_mask(line, mask_options);
        for segment in split_masked(line, &mask) {
            if segment.protected {
                continue;
            }
            let text = segment.text;

            if enabled.is("MDX071") {
                if let Some(start) = find_empty_link(text) {
                    issues.push(make_problem(
                        "MDX071",
                        i + 1,
                        segment.start + start + 1,
                        "Link or image has empty text or an empty destination.",
                        Overrides::NONE,
                    ));
                }
            }
            if enabled.is("MDX075") {
                if let Some(m) = re!(r"!\[[ \t]*\]").exec(text) {
                    issues.push(make_problem(
                        "MDX075",
                        i + 1,
                        segment.start + m.start() + 1,
                        "Image has no alt text. Add a short description, or confirm that an empty alt is correct for a decorative image.",
                        Overrides::severity(Severity::Warning),
                    ));
                }
            }
            if enabled.is("MDX072") {
                if let Some(start) = find_spaced_link(text) {
                    issues.push(make_problem(
                        "MDX072",
                        i + 1,
                        segment.start + start + 1,
                        "There is whitespace between ] and (. CommonMark does not parse this as a link; remove the space if a link was intended.",
                        Overrides::severity(Severity::Info),
                    ));
                }
            }
            if enabled.is("MDX073") {
                for m in find_full_references(text) {
                    let second = &text[m.second.0..m.second.1];
                    let label_text = if second.is_empty() {
                        &text[m.first.0..m.first.1]
                    } else {
                        second
                    };
                    let label = normalize_label(label_text);
                    if !label.is_empty() && !definitions.contains(&label) {
                        issues.push(make_problem(
                            "MDX073",
                            i + 1,
                            segment.start + m.start + 1,
                            format!(
                                "Reference link label \"{}\" has no matching definition.",
                                to_string(trim(label_text))
                            ),
                            Overrides::NONE,
                        ));
                    }
                }
            }
        }
    }

    issues
}

/// JS `computeSetextAmbiguity`：MDX080。
pub(crate) fn compute_setext_ambiguity(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    if !enabled.is("MDX080") {
        return Vec::new();
    }
    let mut issues = Vec::new();
    let tables = find_tables(lines, info);
    let delimiter_lines: HashSet<usize> = tables.iter().map(|t| t.start + 1).collect();

    for i in 1..lines.len().min(info.len()) {
        let (line, previous) = (&lines[i], &lines[i - 1]);
        let previous_info = &info[i - 1];
        if is_protected_line(&info[i]) || delimiter_lines.contains(&i) {
            continue;
        }
        if !re!(r"^ {0,3}(?:=+|-+)[ \t]*$").test(line) {
            continue;
        }
        if starts_with_space_or_tab(line) || starts_with_space_or_tab(previous) {
            continue;
        }
        if is_trim_empty(previous) {
            continue;
        }
        if is_thematic_break(line) && re!(r"^ {0,3}(?:\*[ \t]*){3,}$").test(line) {
            continue;
        }
        if is_protected_line(previous_info) || previous_info.fence {
            continue;
        }
        if re!(r"^ {0,3}>").test(previous)
            || re!(r"^ {0,3}\|").test(previous)
            || previous.contains(&PIPE)
        {
            continue;
        }
        if is_atx_heading(previous) || is_list_line(previous) || is_thematic_break(previous) {
            continue;
        }
        if previous_info.definition {
            continue;
        }
        issues.push(make_problem(
            "MDX080",
            i + 1,
            1,
            "This line is a Setext heading underline for the previous paragraph, so it renders as a heading instead of a horizontal rule. Insert a blank line above it, or use *** for a rule.",
            Overrides::NONE,
        ));
    }
    issues
}

/// JS `computeFullwidthSyntax`：MDX084。
pub(crate) fn compute_fullwidth_syntax(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    if !enabled.is("MDX084") {
        return Vec::new();
    }
    let mut issues = Vec::new();
    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_line(line_info) {
            continue;
        }
        if re!(r"^[ \t]*＃{1,6}(?:[ \t　]|[^＃\s])").test(line)
            || re!(r"^[ \t]*[＊＋－][ \t　]").test(line)
            || has_fullwidth_link(line)
        {
            issues.push(make_problem(
                "MDX084",
                i + 1,
                1,
                "Full-width punctuation is being used for Markdown syntax. Use ASCII # for headings, -, *, or + for bullets, and [ ] ( ) for links and images.",
                Overrides::NONE,
            ));
        }
    }
    issues
}

/// JS `computeTrailingWhitespace`：MDX004。
pub(crate) fn compute_trailing_whitespace(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    if !enabled.is("MDX004") {
        return Vec::new();
    }
    let mut issues = Vec::new();
    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_block(line_info) {
            continue;
        }
        // `/[ \t]+$/` 的比對就是行尾連續的空格與 tab。
        let trailing = line
            .iter()
            .rev()
            .take_while(|&&c| c == SPACE || c == TAB)
            .count();
        if trailing == 0 {
            continue;
        }
        let run = &line[line.len() - trailing..];
        if !is_blank(line) && run == [SPACE, SPACE] {
            continue;
        }
        issues.push(make_problem(
            "MDX004",
            i + 1,
            line.len() - trailing + 1,
            "Trailing whitespace. Exactly two trailing spaces create a hard line break; other trailing whitespace is noise.",
            Overrides::NONE,
        ));
    }
    issues
}

/// JS `computeNbspInUrls`：MDX008。
pub(crate) fn compute_nbsp_in_urls(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    if !enabled.is("MDX008") {
        return Vec::new();
    }
    let mut issues = Vec::new();
    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_line(line_info) {
            continue;
        }
        let mask = build_mask(line, NO_URLS_NO_DESTINATIONS);
        for m in crate::mask::url_re().find_iter(line) {
            if mask.get(m.start()).copied().unwrap_or(false) {
                continue;
            }
            let Some(offset) = m.str(line, 0).iter().position(|&c| c == 0x00A0) else {
                continue;
            };
            issues.push(make_problem(
                "MDX008",
                i + 1,
                m.start() + offset + 1,
                "NBSP inside a URL. Replace it with %20 or remove it; the fixer leaves URLs untouched to avoid breaking them.",
                Overrides::NOT_FIXABLE,
            ));
        }
    }
    issues
}

/// JS `computeZeroWidth`：MDX009。
pub(crate) fn compute_zero_width(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    if !enabled.is("MDX009") {
        return Vec::new();
    }
    let mut issues = Vec::new();
    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_block(line_info) {
            continue;
        }
        let mask = build_mask(line, NO_URLS_NO_DESTINATIONS);
        for segment in split_masked(line, &mask) {
            if segment.protected {
                continue;
            }
            if let Some(index) = segment
                .text
                .iter()
                .position(|&c| matches!(c, 0x200B | 0x2060 | 0xFEFF))
            {
                issues.push(make_problem(
                    "MDX009",
                    i + 1,
                    segment.start + index + 1,
                    "Zero-width character found. Remove it unless it is intentional (for example, a line-break opportunity in some scripts).",
                    Overrides::NONE,
                ));
                break;
            }
        }
    }
    issues
}

/// JS `computeBlankLineRuns(text, scan, isEnabled)`：MDX006、MDX005、MDX003。
pub(crate) fn compute_blank_line_runs(
    text: &[u16],
    scan: &Scan,
    enabled: &Enabled,
) -> Vec<Problem> {
    let mut issues = Vec::new();
    if text.is_empty() {
        return issues;
    }
    let lines = &scan.lines;
    let protected_blank = |i: usize| scan.info.get(i).is_some_and(is_protected_block);
    let blank_run_line = |i: usize| is_blank(&lines[i]) && !protected_blank(i);

    let mut i = 0;
    while i < lines.len() {
        if !blank_run_line(i) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < lines.len() && blank_run_line(j) {
            j += 1;
        }
        let has_before = i > 0;
        let has_after = j < lines.len();

        if !has_before && enabled.is("MDX006") {
            issues.push(make_problem(
                "MDX006",
                1,
                1,
                "File starts with blank lines. Remove them so the first block starts at line 1.",
                Overrides::NONE,
            ));
        }
        if has_before && has_after && j - i >= 2 && enabled.is("MDX005") {
            issues.push(make_problem(
                "MDX005",
                i + 2,
                1,
                "Multiple consecutive blank lines. Use one blank line between blocks.",
                Overrides::NONE,
            ));
        }
        i = j;
    }

    if enabled.is("MDX003") {
        const LF: u16 = b'\n' as u16;
        if text.last() != Some(&LF) {
            issues.push(make_problem(
                "MDX003",
                lines.len(),
                1,
                "File does not end with a newline.",
                Overrides::NONE,
            ));
        } else if text.ends_with(&[LF, LF]) {
            issues.push(make_problem(
                "MDX003",
                lines.len().saturating_sub(1),
                1,
                "File ends with extra blank lines. End it with exactly one newline.",
                Overrides::NONE,
            ));
        }
    }

    issues
}

/// JS `computeLineLength`：MDX090。長度以 code point 計（`Array.from(line).length`）。
pub(crate) fn compute_line_length(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
    max_line_length: usize,
) -> Vec<Problem> {
    if !enabled.is("MDX090") || max_line_length == 0 {
        return Vec::new();
    }
    let mut issues = Vec::new();
    let mut table_lines = HashSet::new();
    for table in find_tables(lines, info) {
        table_lines.extend(table.start..=table.end);
    }
    for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
        if is_protected_line(line_info) || table_lines.contains(&i) {
            continue;
        }
        let length = code_point_len(line);
        if length > max_line_length {
            issues.push(make_problem(
                "MDX090",
                i + 1,
                max_line_length + 1,
                format!("Line is {length} characters; limit is {max_line_length}."),
                Overrides::severity(Severity::Info),
            ));
        }
    }
    issues
}

/// `lines[i].indexOf('\t', from)`（給 diagnose 的 MDX007 用）。
pub(crate) fn index_of_tab(line: &[u16], from: usize) -> Option<usize> {
    index_of(line, &[TAB], from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Options;
    use crate::diagnose::dedupe_problems;
    use crate::jsstr::u;
    use crate::scan::scan_document;

    /// 只啟用 `rules` 時，行內修正不會改動任何行，所以直接在第一次 scan 上跑全部結構檢查。
    /// 預期值由 oracle 產生：`diagnose(text, { rules, maxLineLength: 10 })`。
    fn run(text: &str, rules: &[&str]) -> Vec<String> {
        let options = Options {
            rules: Some(rules.iter().map(|r| r.to_string()).collect()),
            ignore_rules: None,
            max_line_length: 10,
        };
        let enabled = Enabled::new(&options);
        let text = u(text);
        let scan = scan_document(&text);
        let (lines, info) = (&scan.lines, &scan.info);
        let mut issues = Vec::new();
        issues.extend(compute_trailing_whitespace(lines, info, &enabled));
        issues.extend(compute_nbsp_in_urls(lines, info, &enabled));
        issues.extend(compute_zero_width(lines, info, &enabled));
        issues.extend(compute_blank_line_runs(&text, &scan, &enabled));
        issues.extend(compute_heading_structure(lines, info, &enabled));
        issues.extend(compute_list_structure(lines, info, &enabled));
        issues.extend(compute_table_structure(lines, info, &enabled));
        issues.extend(compute_reference_structure(lines, info, &enabled));
        issues.extend(compute_setext_ambiguity(lines, info, &enabled));
        issues.extend(compute_fullwidth_syntax(lines, info, &enabled));
        issues.extend(compute_line_length(lines, info, &enabled, 10));
        dedupe_problems(issues)
            .iter()
            .map(|p| format!("{}@{}:{}", p.rule, p.line, p.column))
            .collect()
    }

    #[test]
    fn headings() {
        let text = "## Start\n#\n  # Indented:\n#### Jump!\n# One\n# Two ##\n##   ###\n# 中文。\n```\n# no\n```\n";
        let rules = ["MDX014", "MDX015", "MDX016", "MDX017", "MDX018", "MDX019"];
        assert_eq!(
            run(text, &rules),
            [
                "MDX018@1:1",
                "MDX016@2:1",
                "MDX014@3:1",
                "MDX017@3:1",
                "MDX015@4:1",
                "MDX017@4:1",
                "MDX019@6:1",
                "MDX017@8:1",
                "MDX019@8:1"
            ]
        );
    }

    #[test]
    fn lists() {
        let text =
            "Para\n- a\n* b\n+ c\n  * nested\n  - nested2\nlazy\n\n-\n1.\n- x\n\n***\n* y\n- z\n";
        assert_eq!(
            run(text, &["MDX031", "MDX035", "MDX032"]),
            [
                "MDX032@2:1",
                "MDX031@3:1",
                "MDX031@4:1",
                "MDX031@6:3",
                "MDX032@7:1",
                "MDX035@9:1",
                "MDX035@10:1",
                "MDX032@11:1",
                "MDX031@15:1"
            ]
        );
    }

    #[test]
    fn tables() {
        let text = "| a | b |\n| - | -- |\n| `x|y` | 2 | 3 |\n| 1 |\n\nx | y\n:-|---\n";
        assert_eq!(
            run(text, &["MDX060", "MDX061", "MDX063"]),
            [
                "MDX060@2:3",
                "MDX061@3:1",
                "MDX063@3:3",
                "MDX061@4:1",
                "MDX060@7:1"
            ]
        );
    }

    #[test]
    fn references() {
        let text = "[Foo]: /a\n[foo]:  /b\n[ ]: /c\nSee [x][Foo] and [y][missing] and [Bar][] and ![](img.png)\n[t] (u) and [](x) and [a]()\n> [q]: /q\n[z][q]\n";
        let rules = ["MDX071", "MDX072", "MDX073", "MDX074", "MDX075"];
        assert_eq!(
            run(text, &rules),
            [
                "MDX074@2:1",
                "MDX073@4:18",
                "MDX073@4:35",
                "MDX071@4:47",
                "MDX075@4:47",
                "MDX072@5:3",
                "MDX071@5:13"
            ]
        );
    }

    #[test]
    fn setext() {
        let text =
            "Para\n---\n\nText\n===\n- item\n---\n| a |\n---\n  x\n---\nfoo\n***\n[d]: /x\n---\n";
        assert_eq!(run(text, &["MDX080"]), ["MDX080@2:1", "MDX080@5:1"]);
    }

    #[test]
    fn fullwidth() {
        let text = "＃ 標題\n＃＃＃\n＊ 項目\n－　項目\n見［連結］（網址）\n＃abc\n";
        assert_eq!(
            run(text, &["MDX084"]),
            [
                "MDX084@1:1",
                "MDX084@3:1",
                "MDX084@4:1",
                "MDX084@5:1",
                "MDX084@6:1"
            ]
        );
    }

    #[test]
    fn trailing_whitespace() {
        let text = "a  \nb \nc\t\n   \n  \nd   \n```\ne  \t\n```\n";
        assert_eq!(
            run(text, &["MDX004"]),
            [
                "MDX004@2:2",
                "MDX004@3:2",
                "MDX004@4:1",
                "MDX004@5:1",
                "MDX004@6:2"
            ]
        );
    }

    #[test]
    fn nbsp_in_urls() {
        let text = "see https://ex.com/a\u{A0}b and www.x.com\u{A0}y\n`https://a.com/\u{A0}`\n[l](https://a.com/x\u{A0}y)\n";
        assert_eq!(
            run(text, &["MDX008"]),
            ["MDX008@1:21", "MDX008@1:37", "MDX008@3:20"]
        );
    }

    #[test]
    fn zero_width() {
        let text = "a\u{200B}b\u{2060}c\n`\u{200B}` d\u{FEFF}\n\u{FEFF}\n";
        assert_eq!(
            run(text, &["MDX009"]),
            ["MDX009@1:2", "MDX009@2:6", "MDX009@3:1"]
        );
    }

    #[test]
    fn blank_line_runs() {
        let rules = ["MDX003", "MDX005", "MDX006"];
        assert_eq!(
            run("\n\na\n\n\n\nb\n\n\n", &rules),
            ["MDX006@1:1", "MDX005@5:1", "MDX003@9:1"]
        );
        assert_eq!(run("a\nb", &rules), ["MDX003@2:1"]);
        assert!(run("", &rules).is_empty());
    }

    #[test]
    fn line_length_counts_code_points() {
        let text = "short\n中文中文中文中文中文中文\n😀😀😀😀😀😀😀😀😀😀😀\n| a | b |\n| --- | --- |\n| 1234567890123 | x |\n";
        assert_eq!(run(text, &["MDX090"]), ["MDX090@2:11", "MDX090@3:11"]);
    }
}

#[cfg(test)]
mod timing {
    use super::*;
    use crate::Options;
    use crate::jsstr::u;
    use crate::scan::scan_document;

    /// 各結構檢查的耗時探針：`MDFIX_DIAG_IN=<檔案清單> cargo test --release -- --ignored structure_timing --nocapture`。
    #[test]
    #[ignore]
    fn structure_timing() {
        let Ok(input) = std::env::var("MDFIX_DIAG_IN") else {
            return;
        };
        let enabled = Enabled::new(&Options {
            max_line_length: 80,
            ..Options::default()
        });
        for path in std::fs::read_to_string(input).unwrap_or_default().lines() {
            let text = u(&std::fs::read_to_string(path).unwrap_or_default());
            let scan = scan_document(&text);
            let (l, i, e) = (&scan.lines, &scan.info, &enabled);
            let fns: [(&str, &dyn Fn() -> Vec<Problem>); 11] = [
                ("trailing", &|| compute_trailing_whitespace(l, i, e)),
                ("nbsp", &|| compute_nbsp_in_urls(l, i, e)),
                ("zw", &|| compute_zero_width(l, i, e)),
                ("blank", &|| compute_blank_line_runs(&text, &scan, e)),
                ("heading", &|| compute_heading_structure(l, i, e)),
                ("list", &|| compute_list_structure(l, i, e)),
                ("table", &|| compute_table_structure(l, i, e)),
                ("ref", &|| compute_reference_structure(l, i, e)),
                ("setext", &|| compute_setext_ambiguity(l, i, e)),
                ("fullwidth", &|| compute_fullwidth_syntax(l, i, e)),
                ("len", &|| compute_line_length(l, i, e, 80)),
            ];
            let mut report = String::new();
            for (name, f) in fns {
                let t = std::time::Instant::now();
                let _ = f();
                let ms = t.elapsed().as_millis();
                if ms >= 5 {
                    report.push_str(&format!(" {name}={ms}ms"));
                }
            }
            eprintln!("{path}:{report}");
        }
    }
}

#[cfg(test)]
mod matchers {
    //! 線性比對器與 oracle 原樣式（交給 `jsregex` 執行）的窮舉等價測試，以及長輸入的效能測試。
    use super::*;
    use crate::jsregex::Regex;

    /// 列舉 `alphabet` 上長度 0..=`max` 的所有字串。
    fn for_all(alphabet: &str, max: usize, mut f: impl FnMut(&[u16])) {
        let alpha = u(alphabet);
        let mut buf: Vec<u16> = Vec::new();
        fn go(alpha: &[u16], max: usize, buf: &mut Vec<u16>, f: &mut dyn FnMut(&[u16])) {
            f(buf);
            if buf.len() == max {
                return;
            }
            for &c in alpha {
                buf.push(c);
                go(alpha, max, buf, f);
                buf.pop();
            }
        }
        go(&alpha, max, &mut buf, &mut f);
    }

    fn first_start(re: &Regex, s: &[u16]) -> Option<usize> {
        re.exec(s).map(|m| m.start())
    }

    #[test]
    fn atx_heading_matches_regex() {
        let re = Regex::constant(r"^( {0,3})(#{1,6})[ \t]+(.*?)[ \t]*$", "");
        for_all(" \t#a\u{2028}", 7, |s| {
            let expected = re.exec(s).map(|m| AtxHeading {
                indent: m.str(s, 1).len(),
                level: m.str(s, 2).len(),
                text: m.get(3).unwrap_or((0, 0)),
            });
            assert_eq!(match_atx_heading(s), expected, "{s:?}");
        });
    }

    #[test]
    fn closing_hashes_match_regex() {
        let re = Regex::constant(r"[ \t]+#+[ \t]*$", "");
        for_all(" \t#a", 7, |s| {
            assert_eq!(strip_closing_hashes(s), re.replace(s, false, &[]), "{s:?}");
        });
    }

    #[test]
    fn bullet_matches_regex() {
        let re = Regex::constant(r"^( {0,3})([-*+])([ \t]+)(.*)$", "");
        for_all(" \t-*a\u{2029}", 6, |s| {
            let expected = re
                .exec(s)
                .map(|m| (m.str(s, 1).len(), m.str(s, 2).first().copied().unwrap_or(0)));
            assert_eq!(match_bullet(s), expected, "{s:?}");
        });
    }

    #[test]
    fn empty_link_matches_regex() {
        let re = Regex::constant(
            r"\[[ \t]*\]\([^)]*\)|\][ \t]*\([ \t]*\)|!\[[ \t]*\]\([^)]*\)",
            "",
        );
        for_all("[]()! \t", 6, |s| {
            assert_eq!(find_empty_link(s), first_start(&re, s), "{s:?}");
        });
    }

    #[test]
    fn spaced_link_matches_regex() {
        let re = Regex::constant(r"\]\s+\([^)]*\)", "");
        for_all("[]() \u{A0}\u{3000}", 6, |s| {
            assert_eq!(find_spaced_link(s), first_start(&re, s), "{s:?}");
        });
    }

    #[test]
    fn full_references_match_regex() {
        let re = Regex::constant(r"!?\[([^\]\n]*)\]\[([^\]\n]*)\]", "");
        for_all("![]a\n", 8, |s| {
            let expected: Vec<FullReference> = re
                .find_iter(s)
                .iter()
                .map(|m| FullReference {
                    start: m.start(),
                    first: m.get(1).unwrap_or((0, 0)),
                    second: m.get(2).unwrap_or((0, 0)),
                })
                .collect();
            assert_eq!(find_full_references(s), expected, "{s:?}");
        });
    }

    #[test]
    fn fullwidth_link_matches_regex() {
        let re = Regex::constant(r"！?［[^］\n]+］（[^）\n]+）", "");
        for_all("！［］（）a\n", 7, |s| {
            assert_eq!(has_fullwidth_link(s), re.test(s), "{s:?}");
        });
    }

    /// 10k 字元的病態輸入必須在 50 ms 內跑完（debug build 也一樣）。
    #[test]
    fn pathological_inputs_are_fast() {
        let n = 10_000;
        let cases = [
            format!("# a{}b", " ".repeat(n)),
            format!("# a{}b #", " ".repeat(n)),
            format!("- {}\u{2028}", " ".repeat(n)),
            "[](".repeat(n / 3),
            "] (".repeat(n / 3),
            "[".repeat(n),
            "[a][".repeat(n / 4),
            "［".repeat(n),
            "［a］（".repeat(n / 4),
            format!("{}q", "> ".repeat(40)),
            format!("{}q", "> ".repeat(n / 2)),
        ];
        let enabled = Enabled::new(&crate::Options::default());
        for text in &cases {
            let lines = vec![u(text)];
            let info = crate::scan::scan_document(&lines[0]).info;
            let t = std::time::Instant::now();
            let _ = compute_heading_structure(&lines, &info, &enabled);
            let _ = compute_list_structure(&lines, &info, &enabled);
            let _ = compute_reference_structure(&lines, &info, &enabled);
            let _ = compute_fullwidth_syntax(&lines, &info, &enabled);
            let ms = t.elapsed().as_millis();
            assert!(ms < 50, "{ms} ms for {:?}…", &text[..text.len().min(12)]);
        }
    }
}
