//! 單行修正：標題、blockquote、清單標記、強調內側空白、NBSP，以及把它們串起來的
//! `transformLine`（JS 687–1027 行）。

use crate::Problem;
use crate::jsregex::{Captures, Regex, re};
use crate::jsstr::{self, JsString, at, concat, is_js_space, is_punct_or_symbol, trim};
use crate::mask::{MaskOptions, Segment, build_mask, replace_with_offsets, split_masked};
use crate::rules::{Enabled, Overrides, make_problem};
use crate::scan::{
    LineInfo, container_prefix, is_protected_line, is_thematic_break, parse_quote_prefix,
};

const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const GT: u16 = b'>' as u16;
const HASH: u16 = b'#' as u16;
const NBSP: u16 = 0xA0;

/// 單一修正步驟回報的變更（JS `changes[]` 元素）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    pub(crate) rule: &'static str,
    pub(crate) column: usize,
    pub(crate) message: String,
    /// JS `change.overrides`（例如只報告不修正的 `{ fixable: false }`）。
    pub(crate) overrides: Overrides,
}

/// 修正步驟的結果（JS `{ line, changes }`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepResult {
    pub(crate) line: JsString,
    pub(crate) changes: Vec<Change>,
}

fn change(rule: &'static str, column: usize, message: impl Into<String>) -> Change {
    Change {
        rule,
        column,
        message: message.into(),
        overrides: Overrides::NONE,
    }
}

fn unchanged(line: &[u16]) -> StepResult {
    StepResult {
        line: line.to_vec(),
        changes: Vec::new(),
    }
}

/// JS `fixHeading(line, isEnabled)`。
pub(crate) fn fix_heading(line: &[u16], enabled: &Enabled) -> Option<StepResult> {
    if !re!(r"^ {0,3}#").test(line) {
        return None;
    }
    let indent_len = line.iter().take_while(|&&c| c == SPACE).count();
    let indent = &line[..indent_len];
    let after_indent = &line[indent_len..];
    let hashes_len = after_indent.iter().take_while(|&&c| c == HASH).count();
    let hashes = &after_indent[..hashes_len];
    if hashes_len > 6 || at(after_indent, hashes_len) == Some(HASH) {
        return None;
    }

    let rest = &after_indent[hashes_len..];
    if !rest.is_empty() && rest[0] != SPACE && rest[0] != TAB {
        if !enabled.is("MDX010") {
            return None;
        }
        let h = jsstr::to_string(hashes);
        return Some(StepResult {
            line: concat(&[indent, hashes, &[SPACE], rest]),
            changes: vec![change(
                "MDX010",
                indent_len + hashes_len + 1,
                format!("Add a space after the heading marker: \"{h} \" rather than \"{h}\"."),
            )],
        });
    }

    let gap_len = rest.iter().take_while(|&&c| c == SPACE || c == TAB).count();
    let original_gap = &rest[..gap_len];
    let mut gap: JsString = original_gap.to_vec();
    let mut content: JsString = rest[gap_len..].to_vec();
    let mut changes = Vec::new();

    if gap.len() > 1 && !content.is_empty() && enabled.is("MDX011") {
        changes.push(change(
            "MDX011",
            indent_len + hashes_len + 1,
            "Use exactly one space after the ATX heading marker.",
        ));
        gap = vec![SPACE];
    }

    if let Some(caps) = re!(r"^(.*?)[ \t]+(#+)[ \t]*$").exec(&content) {
        let body = caps.str(&content, 1);
        if !jsstr::is_trim_empty(body) && enabled.is("MDX012") {
            changes.push(change(
                "MDX012",
                indent_len + hashes_len + original_gap.len() + body.len() + 1,
                "Remove optional closing hashes from the heading; they add noise and are easy to miscount.",
            ));
            let trimmed_len = body.len()
                - body
                    .iter()
                    .rev()
                    .take_while(|&&c| c == SPACE || c == TAB)
                    .count();
            content = body[..trimmed_len].to_vec();
        }
    }

    if jsstr::is_trim_empty(&content) {
        if enabled.is("MDX011") && gap.len() > 1 {
            gap.clear();
        }
        return Some(StepResult {
            line: concat(&[indent, hashes, &gap]),
            changes,
        });
    }

    if gap.is_empty() {
        gap = vec![SPACE];
    }
    Some(StepResult {
        line: concat(&[indent, hashes, &gap, &content]),
        changes,
    })
}

/// JS `fixBlockquote(line, isEnabled)`。
/// JS `startsIndentSensitiveBlock`：在引言裡意義取決於縮排的區塊（fence 與清單標記）。
fn starts_indent_sensitive_block(content: &[u16]) -> bool {
    re!(r"^(?:`{3,}|~{3,}|(?:[-*+]|\d{1,9}[.)])(?:[ \t]|$))").test(content)
}

pub(crate) fn fix_blockquote(line: &[u16], enabled: &Enabled) -> Option<StepResult> {
    if !enabled.is("MDX020") {
        return None;
    }
    let quote = parse_quote_prefix(line)?;
    // parse_quote_prefix 停在 `>  >`；另一個標記前多一個空格仍是同一串巢狀，所以一次吃完整串。
    // 否則 fix_document 每一輪只合併一層，結果不冪等。
    let mut depth = quote.depth;
    let mut content = quote.rest;
    while content.first() == Some(&SPACE) && content.get(1) == Some(&GT) {
        let Some(inner) = parse_quote_prefix(&content[1..]) else {
            break;
        };
        depth += inner.depth;
        content = inner.rest;
    }
    let gap_len = content
        .iter()
        .take_while(|&&c| c == SPACE || c == TAB)
        .count();
    if content[..gap_len].contains(&TAB) {
        return Some(unchanged(line));
    }
    // 標記自己的空格之後再多的空白，兩個以上時可能是清單項目的延續；fence（決定程式碼
    // 要扣掉多少縮排）與清單標記（決定項目內容的欄位）前面的空白也有意義。只移除其他內容
    // 前的單一空格，或後面沒有內容的至多三個空格。
    let blank_content = gap_len == content.len();
    if (blank_content && gap_len <= 3)
        || (gap_len == 1 && !starts_indent_sensitive_block(&content[1..]))
    {
        content = &content[gap_len..];
    }

    let mut fixed: JsString = quote.leading.to_vec();
    for k in 0..depth {
        if k > 0 {
            fixed.push(SPACE);
        }
        fixed.push(GT);
    }
    if !content.is_empty() {
        fixed.push(SPACE);
        fixed.extend_from_slice(content);
    }

    if fixed == line {
        return Some(unchanged(line));
    }
    Some(StepResult {
        line: fixed,
        changes: vec![change(
            "MDX020",
            quote.leading.len() + 1,
            "Put exactly one space after each blockquote marker; preserve four or more spaces only when they are content indentation.",
        )],
    })
}

/// JS `fixListMarker(line, isEnabled)`。
pub(crate) fn fix_list_marker(line: &[u16], enabled: &Enabled) -> Option<StepResult> {
    if !enabled.is("MDX030") || is_thematic_break(line) {
        return None;
    }

    if let Some(caps) = re!(r"^( {0,3})([-+])(\S.*)$").exec(line) {
        let indent = caps.str(line, 1);
        let marker = caps.str(line, 2);
        let rest = caps.str(line, 3);
        let first = rest.first().copied();
        if first == marker.first().copied() || first.is_some_and(jsstr::is_digit) {
            return None;
        }
        let m = jsstr::to_string(marker);
        return Some(StepResult {
            line: concat(&[indent, marker, &[SPACE], rest]),
            changes: vec![change(
                "MDX030",
                indent.len() + 2,
                format!("Add a space after the \"{m}\" list marker."),
            )],
        });
    }

    if let Some(caps) = re!(r"^( {0,3})(\*)(\S.*)$").exec(line) {
        let indent = caps.str(line, 1);
        let rest = caps.str(line, 3);
        if re!(r"(?<!\\)\*").test(rest) {
            return None;
        }
        return Some(StepResult {
            line: concat(&[indent, &jsstr::u("* "), rest]),
            changes: vec![change(
                "MDX030",
                indent.len() + 2,
                "Add a space after the \"*\" list marker.",
            )],
        });
    }

    if let Some(caps) = re!(r"^( {0,3})(\d{1,9})([.)])([^\d\s].*)$").exec(line) {
        let indent = caps.str(line, 1);
        let number = caps.str(line, 2);
        let delim = caps.str(line, 3);
        let rest = caps.str(line, 4);
        if re!(r"^[A-Za-z]$").test(rest) {
            return None;
        }
        let label = jsstr::to_string(&concat(&[number, delim]));
        return Some(StepResult {
            line: concat(&[indent, number, delim, &[SPACE], rest]),
            changes: vec![change(
                "MDX030",
                indent.len() + number.len() + 2,
                format!("Add a space after the \"{label}\" list marker."),
            )],
        });
    }

    None
}

/// JS `isBoundaryOrWhitespace(ch)`：`None` 代表行內內容的開頭或結尾。
pub(crate) fn is_boundary_or_whitespace(ch: Option<u16>) -> bool {
    ch.is_none_or(is_js_space)
}

/// JS `isUnicodePunct(ch)`。
pub(crate) fn is_unicode_punct(ch: Option<u16>) -> bool {
    !is_boundary_or_whitespace(ch) && ch.is_some_and(is_punct_or_symbol)
}

/// CommonMark 的 flanking 判斷（JS `delimiterFlanking`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Flanking {
    pub(crate) left: bool,
    pub(crate) right: bool,
}

/// JS `delimiterFlanking(before, after)`。
pub(crate) fn delimiter_flanking(before: Option<u16>, after: Option<u16>) -> Flanking {
    let before_ws = is_boundary_or_whitespace(before);
    let after_ws = is_boundary_or_whitespace(after);
    let before_punct = is_unicode_punct(before);
    let after_punct = is_unicode_punct(after);
    Flanking {
        left: !after_ws && (!after_punct || before_ws || before_punct),
        right: !before_ws && (!before_punct || after_ws || after_punct),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WsSide {
    After,
    Before,
}

const MSG_BOLD_STAR: &str = "Remove spaces just inside the bold (**) delimiters.";
const MSG_BOLD_UNDER: &str = "Remove spaces just inside the bold (__) delimiters.";
const MSG_ITALIC_STAR: &str = "Remove the space just inside the italic (*) delimiter. Single-star spacing is fixed only when the other side is attached.";
const MSG_ITALIC_UNDER: &str = "Remove the space just inside the italic (_) delimiter. Single-underscore spacing is fixed only when the other side is attached.";

fn trim_pairs() -> [(&'static Regex, &'static str, WsSide, &'static str); 8] {
    [
        (
            re!(r"(?<![\w*])\*\*([ \t]+)([^*\n]*?\S)([ \t]*)\*\*(?!\*)"),
            "**",
            WsSide::After,
            MSG_BOLD_STAR,
        ),
        (
            re!(r"(?<![\w*])\*\*(\S[^*\n]*?)([ \t]+)\*\*(?!\*)"),
            "**",
            WsSide::Before,
            MSG_BOLD_STAR,
        ),
        (
            re!(r"(?<![\w_])__([ \t]+)([^_\n]*?\S)([ \t]*)__(?![\w_])"),
            "__",
            WsSide::After,
            MSG_BOLD_UNDER,
        ),
        (
            re!(r"(?<![\w_])__(\S[^_\n]*?)([ \t]+)__(?![\w_])"),
            "__",
            WsSide::Before,
            MSG_BOLD_UNDER,
        ),
        (
            re!(r"(?<![*\w])\*([ \t]+)([^*\n]*?\S)\*(?![*\w])"),
            "*",
            WsSide::After,
            MSG_ITALIC_STAR,
        ),
        (
            re!(r"(?<![*\w])\*(\S[^*\n]*?)([ \t]+)\*(?![*\w])"),
            "*",
            WsSide::Before,
            MSG_ITALIC_STAR,
        ),
        (
            re!(r"(?<![\w_])_([ \t]+)([^_\n]*?\S)_(?![\w_])"),
            "_",
            WsSide::After,
            MSG_ITALIC_UNDER,
        ),
        (
            re!(r"(?<![\w_])_(\S[^_\n]*?)([ \t]+)_(?![\w_])"),
            "_",
            WsSide::Before,
            MSG_ITALIC_UNDER,
        ),
    ]
}

/// JS `fixEmphasis(line, lineNo, isEnabled)`：移除強調符號內側的空白。
pub(crate) fn fix_emphasis(line: &[u16], enabled: &Enabled) -> StepResult {
    if !enabled.is("MDX040") {
        return unchanged(line);
    }
    // 八個樣式都需要 `*` 或 `_`；沒有時結果必然不變。
    if !line
        .iter()
        .any(|&c| c == u16::from(b'*') || c == u16::from(b'_'))
    {
        return unchanged(line);
    }
    // 清單標記的 `*` 不是強調符號；移除它後面的空白會讓清單項目變成段落。
    let (_, marker_stars) = container_prefix(line);
    let mask = build_mask(line, MaskOptions::NO_BARE_URLS);
    let segments = split_masked(line, &mask);
    let mut changes = Vec::new();
    let mut output: JsString = Vec::with_capacity(line.len());

    for segment in &segments {
        if segment.protected {
            output.extend_from_slice(segment.text);
            continue;
        }
        let mut text: JsString = segment.text.to_vec();
        for (regex, delimiter, ws_side, message) in trim_pairs() {
            text = trim_pair(
                line,
                segment,
                &marker_stars,
                &text,
                regex,
                delimiter,
                ws_side,
                message,
                &mut changes,
            );
        }
        output.extend_from_slice(&text);
    }

    StepResult {
        line: output,
        changes,
    }
}

#[allow(clippy::too_many_arguments)]
fn trim_pair(
    line: &[u16],
    segment: &Segment<'_>,
    marker_stars: &[usize],
    text: &[u16],
    regex: &Regex,
    delimiter: &str,
    ws_side: WsSide,
    message: &'static str,
    changes: &mut Vec<Change>,
) -> JsString {
    // `charBefore`／`charAfter` 讀的是這一輪取代前的 `text`，超出時退回原始整行。
    let char_before = |start: usize| -> Option<u16> {
        if start > 0 {
            return at(text, start - 1);
        }
        if segment.start > 0 {
            return at(line, segment.start - 1);
        }
        None
    };
    let char_after = |end: usize| -> Option<u16> {
        if end < text.len() {
            return at(text, end);
        }
        let segment_end = segment.start + segment.text.len();
        if segment_end < line.len() {
            return at(line, segment_end);
        }
        None
    };
    let delim = jsstr::u(delimiter);
    let (content_index, ws_index) = match ws_side {
        WsSide::After => (2, 1),
        WsSide::Before => (1, 2),
    };
    replace_with_offsets(text, regex, |caps: &Captures, start| {
        let whole = caps.str(text, 0);
        let size = delim.len();
        let close_start = start + whole.len() - size;
        let content = trim(caps.str(text, content_index));
        let ws = caps.str(text, ws_index);
        let opener_before = char_before(start);
        let closer_after = char_after(close_start + size);
        let opener_original = delimiter_flanking(opener_before, ws.first().copied());
        let opener_repaired = delimiter_flanking(opener_before, content.first().copied());
        let closer_original = delimiter_flanking(ws.last().copied(), closer_after);
        let closer_repaired = delimiter_flanking(content.last().copied(), closer_after);
        let allowed = !marker_stars.contains(&(segment.start + start))
            && opener_repaired.left
            && closer_repaired.right
            && match ws_side {
                WsSide::After => !opener_original.right,
                WsSide::Before => !closer_original.left,
            };
        if !allowed {
            return whole.to_vec();
        }
        changes.push(change("MDX040", segment.start + start + 1, message));
        concat(&[&delim, content, &delim])
    })
}

/// JS `fixNbsp(line, isEnabled)`：保護區域外的 NBSP 換成一般空格。
pub(crate) fn fix_nbsp(line: &[u16], enabled: &Enabled) -> StepResult {
    if !enabled.is("MDX008") || !line.contains(&NBSP) {
        return unchanged(line);
    }
    let mask = build_mask(line, MaskOptions::default());
    let segments = split_masked(line, &mask);
    let mut changes = Vec::new();
    // NBSP 對清單標記不算空白，所以 `*<NBSP>文字` 是段落；換成一般空格會變成清單項目，只報告不修。
    let (prefix_end, _) = container_prefix(line);
    let keep = re!(r"^[ \t]*(?:[-*+]|\d{1,9}[.)])(\u00A0+)")
        .exec(&line[prefix_end.min(line.len())..])
        .and_then(|caps| caps.get(1))
        .map(|(start, end)| (prefix_end + start, prefix_end + end));
    let mut output: JsString = Vec::with_capacity(line.len());
    for segment in &segments {
        if segment.protected {
            output.extend_from_slice(segment.text);
            continue;
        }
        let mut first = None;
        for (i, &c) in segment.text.iter().enumerate() {
            let index = segment.start + i;
            if c != NBSP {
                output.push(c);
            } else if let Some((keep_start, _)) = keep.filter(|&(a, b)| index >= a && index < b) {
                if index == keep_start {
                    changes.push(Change {
                        rule: "MDX008",
                        column: index + 1,
                        message: "NBSP right after a list marker. Replacing it with a space would turn this paragraph into a list item; use a normal space if a list was intended.".to_string(),
                        overrides: Overrides::NOT_FIXABLE,
                    });
                }
                output.push(c);
            } else {
                first.get_or_insert(i);
                output.push(SPACE);
            }
        }
        if let Some(i) = first {
            changes.push(change(
                "MDX008",
                segment.start + i + 1,
                "Replace non-breaking spaces (U+00A0) with normal spaces; they usually come from Word or a web page paste.",
            ));
        }
    }
    // NBSP 是該行或清單項目唯一的內容時，它讓這一行不是空行；換成一般空格會結束段落或清空項目。
    let (content_start, _) = container_prefix(line);
    if output != line && crate::scan::is_blank(&output[content_start.min(output.len())..]) {
        return unchanged(line);
    }
    StepResult {
        line: output,
        changes,
    }
}

/// `transformLine` 的結果（JS `{ line, issues }`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineResult {
    pub(crate) line: JsString,
    pub(crate) issues: Vec<Problem>,
}

/// JS `transformLine(line, lineInfo, lineNo, isEnabled)`：回傳修正後的行與問題清單。
pub(crate) fn transform_line(
    line: &[u16],
    info: &LineInfo,
    line_no: usize,
    enabled: &Enabled,
) -> LineResult {
    let mut issues = Vec::new();
    if is_protected_line(info) || info.keep_whitespace {
        return LineResult {
            line: line.to_vec(),
            issues,
        };
    }

    let mut current: JsString = line.to_vec();
    for step in 0..5 {
        let result = match step {
            0 => fix_heading(&current, enabled),
            1 => fix_blockquote(&current, enabled),
            2 => fix_list_marker(&current, enabled),
            3 => Some(fix_emphasis(&current, enabled)),
            _ => Some(fix_nbsp(&current, enabled)),
        };
        let Some(result) = result else {
            continue;
        };
        for c in result.changes {
            if enabled.is(c.rule) {
                issues.push(make_problem(
                    c.rule,
                    line_no,
                    c.column,
                    c.message,
                    c.overrides,
                ));
            }
        }
        current = result.line;
    }
    LineResult {
        line: current,
        issues,
    }
}
