//! 行導向掃描：`scanDocument` 與它用到的小工具（JS 131–543 行）。
//!
//! 所有行都以 UTF-16（[`JsString`]）保存，索引與 JS 相同。

use crate::jsregex::{Regex, re};
use crate::jsstr::{self, JsString, at, is_blank as js_is_blank, slice_from, trim};
use crate::rules::{Overrides, make_problem};
use crate::{DEFAULT_MAX_QUOTE_DEPTH, LimitPolicy, Options, Problem};

const BACKTICK: u16 = b'`' as u16;
const GT: u16 = b'>' as u16;
const SPACE: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;
const BOM: u16 = 0xFEFF;
const OPEN_BRACKET: u16 = b'[' as u16;
const CLOSE_BRACKET: u16 = b']' as u16;
const COLON: u16 = b':' as u16;
const LF: u16 = b'\n' as u16;

/// JS `RAW_HTML_TAGS`。
pub(crate) const RAW_HTML_TAGS: &str =
    "script|pre|style|textarea|xmp|iframe|noembed|noframes|noscript|plaintext";

/// JS `HTML_BLOCK_TAGS`（以 `|` 串接）。
pub(crate) const HTML_BLOCK_TAGS: &str = "address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h1|h2|h3|h4|h5|h6|head|header|hr|html|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|section|source|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul";

/// `scanDocument` 每一行的資訊（JS `info[i]`；`line` 欄位請改用 [`Scan::lines`]）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LineInfo {
    pub(crate) index: usize,
    pub(crate) blank: bool,
    pub(crate) frontmatter: bool,
    pub(crate) fence: bool,
    pub(crate) fence_open: bool,
    pub(crate) fence_close: bool,
    pub(crate) fence_content: bool,
    /// 已 `trim()` 的 info string。
    pub(crate) fence_info: JsString,
    /// `` ` `` 或 `~`；不是 fence 開頭時為 `None`（JS 的 `''`）。
    pub(crate) fence_char: Option<u16>,
    pub(crate) fence_len: usize,
    pub(crate) html_block: bool,
    pub(crate) html_start: bool,
    /// `raw`、`comment`、`pi`、`cdata`、`declaration`、`type6`、`type7`；非 HTML 開頭為 `""`。
    pub(crate) html_kind: &'static str,
    pub(crate) indented_code: bool,
    pub(crate) definition: bool,
    pub(crate) math_block: bool,
    pub(crate) math_open: bool,
    pub(crate) math_close: bool,
    /// 引言深度超過 [`Limits::max_quote_depth`] 的不透明行（JS `opaque`）：不是任何區塊的開頭、
    /// 不是 definition、不修改也不診斷。
    pub(crate) opaque: bool,
}

/// `scanDocument` 的結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scan {
    /// `text.split('\n')`。
    pub(crate) lines: Vec<JsString>,
    pub(crate) info: Vec<LineInfo>,
    /// 掃描時發現的結構問題（MDX081、MDX053、MDX051、MDX085、MDX083）。
    pub(crate) issues: Vec<Problem>,
    /// frontmatter 結尾行索引；沒有（或未閉合）時為 `None`（JS 的 `-1`）。
    pub(crate) frontmatter_end: Option<usize>,
}

/// JS `normalizeForAnalysis`：去掉開頭所有 BOM，CRLF／CR 換成 LF。
pub(crate) fn normalize_for_analysis(text: &[u16]) -> JsString {
    let start = text.iter().position(|&c| c != BOM).unwrap_or(text.len());
    let text = slice_from(text, start);
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if c == b'\r' as u16 {
            out.push(b'\n' as u16);
            if at(text, i + 1) == Some(b'\n' as u16) {
                i += 1;
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// JS `countIndent`：tab 對齊到 4 的倍數。
pub(crate) fn count_indent(value: &[u16]) -> usize {
    let mut columns = 0;
    // JS 以 code point 疊代；這裡只會傳入空白與 tab，與 code unit 疊代相同。
    for ch in char::decode_utf16(value.iter().copied()) {
        columns += if ch == Ok('\t') { 4 - (columns % 4) } else { 1 };
    }
    columns
}

fn leading_space_tab(line: &[u16]) -> &[u16] {
    let n = line.iter().take_while(|&&c| c == SPACE || c == TAB).count();
    &line[..n]
}

/// JS `isIndentedCodeLine`。
pub(crate) fn is_indented_code_line(line: &[u16]) -> bool {
    count_indent(leading_space_tab(line)) >= 4
}

/// JS `isBlank`：只含空格與 tab。
pub(crate) fn is_blank(line: &[u16]) -> bool {
    js_is_blank(line)
}

/// JS `startsFrontmatterAfterLeadingBlanks`。
pub(crate) fn starts_frontmatter_after_leading_blanks(text: &[u16]) -> bool {
    let lines = jsstr::split_lines(text);
    let mut i = 0;
    while i < lines.len() && is_blank(&lines[i]) {
        i += 1;
    }
    if i == 0 || i >= lines.len() {
        return false;
    }
    let line = &lines[i];
    let start = line.iter().position(|&c| c != BOM).unwrap_or(line.len());
    re!(r"^---[ \t]*$").test(&line[start..])
}

/// JS `isThematicBreak`。
pub(crate) fn is_thematic_break(line: &[u16]) -> bool {
    re!(r"^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$").test(line)
}

/// JS `isAtxHeading`。
pub(crate) fn is_atx_heading(line: &[u16]) -> bool {
    if !re!(r"^ {0,3}#{1,6}(?:[ \t]|$)").test(line) {
        return false;
    }
    let indent = line.iter().take_while(|&&c| c == SPACE).count();
    let hashes = slice_from(line, indent)
        .iter()
        .take_while(|&&c| c == b'#' as u16)
        .count();
    hashes <= 6 && at(line, indent + hashes) != Some(b'#' as u16)
}

/// JS `isListLine`。
pub(crate) fn is_list_line(line: &[u16]) -> bool {
    re!(r"^(?: {0,3})(?:[-*+]|\d{1,9}[.)])[ \t]+\S").test(line)
}

/// JS `isTopLevelLine`。
pub(crate) fn is_top_level_line(line: &[u16]) -> bool {
    !line.is_empty() && line[0] != SPACE && line[0] != TAB
}

/// JS `isParagraphLike`。
pub(crate) fn is_paragraph_like(line: &[u16], info: &LineInfo) -> bool {
    if is_blank(line) {
        return false;
    }
    if matches!(line.first(), Some(&c) if c == SPACE || c == TAB || c == GT) {
        return false;
    }
    if is_protected_line(info) {
        return false;
    }
    if is_atx_heading(line) || is_list_line(line) || is_thematic_break(line) {
        return false;
    }
    if info.definition {
        return false;
    }
    if line.first() == Some(&(b'|' as u16)) {
        return false;
    }
    if jsstr::starts_with(line, "```") || jsstr::starts_with(line, "~~~") {
        return false;
    }
    true
}

/// HTML 區塊的結束條件（JS `closeRe`）。
#[derive(Debug, Clone)]
enum HtmlClose {
    /// `</tag\s*>`，不分大小寫。
    Raw(Regex),
    /// 含有這個字面字串。
    Contains(&'static str),
    /// `^[ \t]*$`（type 6／7 以空行結束）。
    BlankLine,
}

impl HtmlClose {
    fn test(&self, line: &[u16]) -> bool {
        match self {
            HtmlClose::Raw(re) => re.test(line),
            HtmlClose::Contains(lit) => jsstr::includes(line, lit),
            HtmlClose::BlankLine => is_blank(line),
        }
    }
}

#[derive(Debug, Clone)]
struct HtmlStart {
    kind: &'static str,
    close: HtmlClose,
    closed_on_same_line: bool,
    type7: bool,
}

fn raw_html_open_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::constant(&format!(r"^ {{0,3}}<({RAW_HTML_TAGS})(?=[\s>]|$)"), "i"))
}

fn html_block_open_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::constant(
            &format!(r"^ {{0,3}}</?(?:{HTML_BLOCK_TAGS})(?=[\s/>]|$)"),
            "i",
        )
    })
}

/// JS `detectHtmlBlockStart`。
fn detect_html_block_start(line: &[u16]) -> Option<HtmlStart> {
    if let Some(caps) = raw_html_open_re().exec(line) {
        let tag = jsstr::to_string(caps.str(line, 1));
        // 標籤名稱只會是 RAW_HTML_TAGS 裡的 ASCII 字，可以安全放進樣式。
        let close = Regex::constant(&format!(r"</{tag}\s*>"), "i");
        let closed = close.test(slice_from(line, caps.end()));
        return Some(HtmlStart {
            kind: "raw",
            close: HtmlClose::Raw(close),
            closed_on_same_line: closed,
            type7: false,
        });
    }
    if re!(r"^ {0,3}<!--").test(line) {
        let open = jsstr::index_of(line, &jsstr::u("<!--"), 0).unwrap_or(0);
        return Some(HtmlStart {
            kind: "comment",
            close: HtmlClose::Contains("-->"),
            closed_on_same_line: jsstr::index_of(line, &jsstr::u("-->"), open + 4).is_some(),
            type7: false,
        });
    }
    if re!(r"^ {0,3}<\?").test(line) {
        let open = jsstr::index_of(line, &jsstr::u("<?"), 0).unwrap_or(0);
        return Some(HtmlStart {
            kind: "pi",
            close: HtmlClose::Contains("?>"),
            closed_on_same_line: jsstr::includes(slice_from(line, open + 2), "?>"),
            type7: false,
        });
    }
    if re!(r"^ {0,3}<!\[CDATA\[").test(line) {
        let open = jsstr::index_of(line, &jsstr::u("<![CDATA["), 0).unwrap_or(0);
        return Some(HtmlStart {
            kind: "cdata",
            close: HtmlClose::Contains("]]>"),
            closed_on_same_line: jsstr::includes(slice_from(line, open + 9), "]]>"),
            type7: false,
        });
    }
    if re!(r"^ {0,3}<![A-Za-z]").test(line) {
        let open = jsstr::index_of(line, &jsstr::u("<!"), 0).unwrap_or(0);
        return Some(HtmlStart {
            kind: "declaration",
            close: HtmlClose::Contains(">"),
            closed_on_same_line: jsstr::includes(slice_from(line, open + 2), ">"),
            type7: false,
        });
    }
    if html_block_open_re().test(line) {
        return Some(HtmlStart {
            kind: "type6",
            close: HtmlClose::BlankLine,
            closed_on_same_line: false,
            type7: false,
        });
    }
    if re!(r"^ {0,3}<\/?[A-Za-z][\w-]*(?:\s+[^>\n]*)?\/?>[ \t]*$").test(line) {
        return Some(HtmlStart {
            kind: "type7",
            close: HtmlClose::BlankLine,
            closed_on_same_line: false,
            type7: true,
        });
    }
    None
}

/// JS `parseQuotePrefix` 的結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuotePrefix<'a> {
    pub(crate) depth: usize,
    /// 開頭 0–3 個空格。
    pub(crate) leading: &'a [u16],
    pub(crate) rest: &'a [u16],
    pub(crate) prefix_length: usize,
}

/// JS `parseQuotePrefix`。
pub(crate) fn parse_quote_prefix(line: &[u16]) -> Option<QuotePrefix<'_>> {
    let leading_len = line.iter().take(3).take_while(|&&c| c == SPACE).count();
    let mut pos = leading_len;
    let mut depth = 0;
    while at(line, pos) == Some(GT) {
        depth += 1;
        pos += 1;
        if at(line, pos) == Some(SPACE) {
            pos += 1;
        }
        if at(line, pos) != Some(GT) {
            break;
        }
    }
    if depth == 0 {
        return None;
    }
    Some(QuotePrefix {
        depth,
        leading: &line[..leading_len],
        rest: slice_from(line, pos),
        prefix_length: pos,
    })
}

struct QuoteFence {
    quote_depth: usize,
    ch: u16,
    len: usize,
    info: JsString,
    raw_info: JsString,
}

fn fence_re() -> &'static Regex {
    re!(r"^( {0,3})(`{3,}|~{3,})(.*)$")
}

/// JS `parseQuoteFence`。
fn parse_quote_fence(line: &[u16]) -> Option<QuoteFence> {
    let quote = parse_quote_prefix(line)?;
    let caps = fence_re().exec(quote.rest)?;
    let marker = caps.str(quote.rest, 2);
    let raw_info = caps.str(quote.rest, 3);
    Some(QuoteFence {
        quote_depth: quote.depth,
        ch: *marker.first()?,
        len: marker.len(),
        info: trim(raw_info).to_vec(),
        raw_info: raw_info.to_vec(),
    })
}

struct MathOpen {
    quote_depth: usize,
    closed: bool,
}

/// JS `parseMathOpen`：去掉 blockquote、清單與 task list 前綴後看是否以 `$$` 開頭。
fn parse_math_open(line: &[u16]) -> Option<MathOpen> {
    let mut rest: &[u16] = line;
    let mut quote_depth = 0;
    let mut in_list = false;
    for _ in 0..16 {
        if let Some(quote) = parse_quote_prefix(rest) {
            rest = quote.rest;
            quote_depth += quote.depth;
            continue;
        }
        if let Some(caps) = re!(r"^( {0,3})([-*+]|\d{1,9}[.)])([ \t]+)(.*)$").exec(rest) {
            rest = caps.str(rest, 4);
            in_list = true;
            continue;
        }
        if in_list {
            if let Some(caps) = re!(r"^\[[ xX]\][ \t]+(.*)$").exec(rest) {
                rest = caps.str(rest, 1);
                continue;
            }
        }
        break;
    }
    let caps = re!(r"^( {0,3})\$\$(.*)$").exec(rest)?;
    Some(MathOpen {
        quote_depth,
        closed: jsstr::includes(caps.str(rest, 2), "$$"),
    })
}

struct OpenFence {
    ch: u16,
    len: usize,
    start: usize,
    quote_depth: usize,
}

/// JS `isClosingFence`。
fn is_closing_fence(line: &[u16], fence: &OpenFence) -> bool {
    let mut rest = line;
    if fence.quote_depth > 0 {
        match parse_quote_prefix(line) {
            Some(quote) if quote.depth >= fence.quote_depth => rest = quote.rest,
            _ => return false,
        }
    }
    match re!(r"^ {0,3}(`{3,}|~{3,})[ \t]*$").exec(rest) {
        Some(caps) => {
            let marker = caps.str(rest, 1);
            marker.first() == Some(&fence.ch) && marker.len() >= fence.len
        }
        None => false,
    }
}

struct OpenHtml {
    kind: &'static str,
    close: HtmlClose,
    start: usize,
}

struct OpenMath {
    start: usize,
    quote_depth: usize,
}

const MSG_MDX053: &str =
    "Invalid fenced-code opener: a backtick fence info string cannot contain backticks.";

/// JS `scanDocument(text)`；`text` 應先經過 [`normalize_for_analysis`]。
/// 一次 `diagnose`／`fix_document` 共用的資源上限（JS `createLimits`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    /// 引言巢狀深度上限；0 表示不限制。
    pub(crate) max_quote_depth: usize,
    pub(crate) on_limit: LimitPolicy,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_quote_depth: DEFAULT_MAX_QUOTE_DEPTH,
            on_limit: LimitPolicy::Error,
        }
    }
}

impl Limits {
    pub(crate) fn new(options: &Options) -> Limits {
        Limits {
            max_quote_depth: options.max_quote_depth,
            on_limit: options.on_limit,
        }
    }
}

/// JS `scanQuoteMarkers`：以 `/^(?: {0,3}>[ \t]?)*/` 貪婪的方式數引言標記。
///
/// 每一輪是至多 3 個空格、`>`、再一個可有可無的空格或 tab。貪婪吃法能走到任何回溯路徑
/// 走得到的每一個 `>`，所以回傳的深度是最大值、結束位置是最遠的位置。線性時間。
pub(crate) fn scan_quote_markers(line: &[u16]) -> (usize, usize) {
    let mut pos = 0;
    let mut depth = 0;
    loop {
        let mut p = pos;
        while p - pos < 3 && line.get(p) == Some(&SPACE) {
            p += 1;
        }
        if line.get(p) != Some(&GT) {
            break;
        }
        p += 1;
        if matches!(line.get(p), Some(&c) if c == SPACE || c == TAB) {
            p += 1;
        }
        pos = p;
        depth += 1;
    }
    (depth, pos)
}

fn resource_limit_message(depth: usize, limit: usize) -> String {
    format!(
        "Blockquote nesting depth {depth} exceeds the limit of {limit}. Reduce the nesting, raise --max-quote-depth (0 = no limit), or use --on-limit ignore to leave such lines untouched."
    )
}

/// 以預設資源上限掃描（只給測試用；函式庫內一律用 [`scan_document_with`] 傳入選項的上限）。
#[cfg(test)]
pub(crate) fn scan_document(text: &[u16]) -> Scan {
    scan_document_with(text, Limits::default())
}

/// JS `scanDocument(text, limits)`。
pub(crate) fn scan_document_with(text: &[u16], limits: Limits) -> Scan {
    let lines = jsstr::split_lines(text);
    let mut info: Vec<LineInfo> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| LineInfo {
            index,
            blank: is_blank(line),
            ..LineInfo::default()
        })
        .collect();
    let mut issues = Vec::new();
    let mut frontmatter_end = None;

    let first_line: &[u16] = match lines.first() {
        Some(l) if l.first() == Some(&BOM) => &l[1..],
        Some(l) => l,
        None => &[],
    };
    if re!(r"^---[ \t]*$").test(first_line) {
        let close = (1..lines.len()).find(|&i| re!(r"^(?:---|\.\.\.)[ \t]*$").test(&lines[i]));
        if let Some(close) = close {
            frontmatter_end = Some(close);
            for item in info.iter_mut().take(close + 1) {
                item.frontmatter = true;
            }
        } else {
            let first_content = lines
                .iter()
                .skip(1)
                .take(11)
                .find(|line| !jsstr::is_trim_empty(line));
            if first_content.is_some_and(|l| re!(r"^[A-Za-z0-9_.-]+[ \t]*:").test(l)) {
                issues.push(make_problem(
                    "MDX081",
                    1,
                    1,
                    "Unclosed YAML frontmatter: no closing --- or ... line was found.",
                    Overrides::NONE,
                ));
                let mut end = lines.len() - 1;
                for (i, line) in lines.iter().enumerate().skip(1) {
                    if jsstr::is_trim_empty(line) {
                        end = i - 1;
                        break;
                    }
                }
                for item in info.iter_mut().take(end + 1) {
                    item.frontmatter = true;
                }
            }
        }
    }

    let mut fence: Option<OpenFence> = None;
    let mut html: Option<OpenHtml> = None;
    let mut math: Option<OpenMath> = None;
    let mut in_indented = false;

    let next_non_blank = |start: usize| -> Option<usize> {
        (start + 1..lines.len()).find(|&i| !is_blank(&lines[i]))
    };

    for i in 0..lines.len() {
        let line = &lines[i][..];
        if info[i].frontmatter {
            continue;
        }

        if let Some(open) = &fence {
            info[i].fence = true;
            info[i].fence_content = true;
            if is_closing_fence(line, open) {
                info[i].fence_close = true;
                info[i].fence_content = false;
                fence = None;
            }
            continue;
        }

        if let Some(open) = &html {
            info[i].html_block = true;
            if open.close.test(line) {
                html = None;
            }
            continue;
        }

        if let Some(open) = &math {
            // `$$` 區塊在同一層 blockquote 深度出現 `$$` 時結束。
            if open.quote_depth > 0 {
                match parse_quote_prefix(line) {
                    Some(quote) if quote.depth >= open.quote_depth => {
                        info[i].math_block = true;
                        if jsstr::includes(quote.rest, "$$") {
                            info[i].math_close = true;
                            math = None;
                        }
                        continue;
                    }
                    _ => {
                        issues.push(make_problem(
                            "MDX085",
                            open.start + 1,
                            1,
                            "Unclosed $$ math block: the blockquote ended before a closing $$ was found.",
                            Overrides::NONE,
                        ));
                        math = None;
                    }
                }
            } else {
                info[i].math_block = true;
                if jsstr::includes(line, "$$") {
                    info[i].math_close = true;
                    math = None;
                }
                continue;
            }
        }

        if info[i].blank {
            if in_indented {
                match next_non_blank(i) {
                    Some(next) if is_indented_code_line(&lines[next]) => {
                        info[i].indented_code = true;
                    }
                    _ => in_indented = false,
                }
            }
            continue;
        }

        if in_indented {
            if is_indented_code_line(line) {
                info[i].indented_code = true;
                continue;
            }
            in_indented = false;
        }

        // 超過 maxQuoteDepth 的行是不透明的：不是區塊開頭也不是 definition，不修改也不診斷。
        // `error` 模式另外回報 MDX086，讓 fix_document 拒絕修正。
        if limits.max_quote_depth > 0 {
            let (depth, _) = scan_quote_markers(line);
            if depth > limits.max_quote_depth {
                info[i].opaque = true;
                if limits.on_limit != LimitPolicy::Ignore {
                    issues.push(make_problem(
                        "MDX086",
                        i + 1,
                        1,
                        resource_limit_message(depth, limits.max_quote_depth),
                        Overrides::NONE,
                    ));
                }
                continue;
            }
        }

        let can_start_indented_code = if i == 0 {
            true
        } else {
            let previous = &info[i - 1];
            let previous_is_indented_continuation =
                is_indented_code_line(&lines[i - 1]) && !previous.indented_code;
            previous.blank
                || previous.indented_code
                || (!previous_is_indented_continuation
                    && !is_paragraph_like(&lines[i - 1], previous))
        };
        if is_indented_code_line(line) && can_start_indented_code {
            in_indented = true;
            info[i].indented_code = true;
            continue;
        }

        if let Some(quote) = parse_quote_prefix(line) {
            if is_indented_code_line(quote.rest) {
                info[i].indented_code = true;
                continue;
            }
        }

        if let Some(open) = parse_math_open(line) {
            info[i].math_block = true;
            if !open.closed {
                info[i].math_open = true;
                math = Some(OpenMath {
                    start: i,
                    quote_depth: open.quote_depth,
                });
            }
            continue;
        }

        if let Some(caps) = fence_re().exec(line) {
            let marker = caps.str(line, 2);
            let ch = marker.first().copied().unwrap_or(BACKTICK);
            let raw_info = caps.str(line, 3);
            if ch == BACKTICK && raw_info.contains(&BACKTICK) {
                issues.push(make_problem(
                    "MDX053",
                    i + 1,
                    1,
                    MSG_MDX053,
                    Overrides::NONE,
                ));
            } else {
                let item = &mut info[i];
                item.fence = true;
                item.fence_open = true;
                item.fence_info = trim(raw_info).to_vec();
                item.fence_char = Some(ch);
                item.fence_len = marker.len();
                fence = Some(OpenFence {
                    ch,
                    len: marker.len(),
                    start: i,
                    quote_depth: 0,
                });
                continue;
            }
        }

        if let Some(qf) = parse_quote_fence(line) {
            if qf.ch == BACKTICK && qf.raw_info.contains(&BACKTICK) {
                issues.push(make_problem(
                    "MDX053",
                    i + 1,
                    1,
                    MSG_MDX053,
                    Overrides::NONE,
                ));
            } else {
                let item = &mut info[i];
                item.fence = true;
                item.fence_open = true;
                item.fence_info = qf.info;
                item.fence_char = Some(qf.ch);
                item.fence_len = qf.len;
                fence = Some(OpenFence {
                    ch: qf.ch,
                    len: qf.len,
                    start: i,
                    quote_depth: qf.quote_depth,
                });
                continue;
            }
        }

        if let Some(start) = detect_html_block_start(line) {
            if !start.type7 || i == 0 || info[i - 1].blank {
                let item = &mut info[i];
                item.html_block = true;
                item.html_start = true;
                item.html_kind = start.kind;
                if !start.closed_on_same_line {
                    html = Some(OpenHtml {
                        kind: start.kind,
                        close: start.close,
                        start: i,
                    });
                }
                continue;
            }
        }

        if is_definition_line(line) {
            info[i].definition = true;
        }
    }

    if let Some(open) = fence {
        let kind = if open.ch == BACKTICK {
            "backtick"
        } else {
            "tilde"
        };
        issues.push(make_problem(
            "MDX051",
            open.start + 1,
            1,
            format!("Unclosed {kind} code fence."),
            Overrides::NONE,
        ));
    }
    if let Some(open) = math {
        issues.push(make_problem(
            "MDX085",
            open.start + 1,
            1,
            "Unclosed $$ math block: no closing $$ was found.",
            Overrides::NONE,
        ));
    }
    if let Some(open) = html {
        if open.kind == "comment" {
            issues.push(make_problem(
                "MDX083",
                open.start + 1,
                1,
                "Unclosed HTML comment: no closing --> was found.",
                Overrides::NONE,
            ));
        }
    }

    Scan {
        lines,
        info,
        issues,
        frontmatter_end,
    }
}

/// v1.0.0 的參考定義行樣式（`scanDocument` 與 `computeReferenceStructure` 共用）。
///
/// `(?: {0,3}>[ \t]?)*` 與後面的 ` {0,3}` 互相重疊，在 `> > > … q` 這種輸入上純回溯是
/// 指數級。v1.1.0 改用線性的 [`match_definition`]；這個樣式只留給等價測試。
#[cfg(test)]
pub(crate) fn definition_re() -> &'static Regex {
    re!(r"^(?: {0,3}>[ \t]?)* {0,3}\[([^\]\n]+)\]:[ \t]*(?:<[^>\n]*>|\S+)")
}

/// 參考定義行（JS `matchDefinition`，等價於
/// `/^(?: {0,3}>[ \t]?)* {0,3}\[([^\]\n]+)\]:[ \t]*(?:<[^>\n]*>|\S+)/`）。
///
/// 先以 [`scan_quote_markers`] 貪婪地吃完引言前綴，再比對其餘部分，線性時間。是定義行時
/// 回傳 label（第 1 組）在整行中的 UTF-16 範圍 `(start, end)`。
pub(crate) fn match_definition(line: &[u16]) -> Option<(usize, usize)> {
    let (_, mut pos) = scan_quote_markers(line);
    let indent_start = pos;
    while pos - indent_start < 3 && line.get(pos) == Some(&SPACE) {
        pos += 1;
    }
    if line.get(pos) != Some(&OPEN_BRACKET) {
        return None;
    }
    let label_start = pos + 1;
    let mut close = label_start;
    while close < line.len() && line[close] != CLOSE_BRACKET && line[close] != LF {
        close += 1;
    }
    if close == label_start
        || line.get(close) != Some(&CLOSE_BRACKET)
        || line.get(close + 1) != Some(&COLON)
    {
        return None;
    }
    let mut dest = close + 2;
    while matches!(line.get(dest), Some(&c) if c == SPACE || c == TAB) {
        dest += 1;
    }
    match line.get(dest) {
        Some(&c) if !jsstr::is_js_space(c) => Some((label_start, close)),
        _ => None,
    }
}

fn is_definition_line(line: &[u16]) -> bool {
    match_definition(line).is_some()
}

/// JS `isProtectedLine`。
pub(crate) fn is_protected_line(info: &LineInfo) -> bool {
    info.frontmatter
        || info.fence_content
        || info.html_block
        || info.indented_code
        || info.math_block
        || info.definition
        || info.opaque
}

/// 不含 `definition` 的保護判斷（JS 多處的 `frontmatter || fenceContent || htmlBlock || indentedCode || mathBlock || opaque`）。
pub(crate) fn is_protected_block(info: &LineInfo) -> bool {
    info.frontmatter
        || info.fence_content
        || info.html_block
        || info.indented_code
        || info.math_block
        || info.opaque
}

#[cfg(test)]
mod definition_equivalence_tests {
    //! v1.1.0 的線性 [`match_definition`] 與 v1.0.0 的 regex（[`definition_re`]）逐位相同。
    //! jsregex 會在步數超過預算後改用記憶化回溯，所以這裡的引言深度可以超過 JS 測試的 12 層。
    use super::*;

    fn check(line: &[u16]) {
        let expected = definition_re().exec(line).and_then(|c| c.get(1));
        assert_eq!(
            match_definition(line),
            expected,
            "{:?}",
            jsstr::to_string(line)
        );
        let greedy = re!(r"^(?: {0,3}>[ \t]?)*")
            .exec(line)
            .and_then(|c| c.get(0));
        let (depth, end) = scan_quote_markers(line);
        assert_eq!(Some((0, end)), greedy, "{:?}", jsstr::to_string(line));
        assert_eq!(depth, line[..end].iter().filter(|&&c| c == GT).count());
    }

    #[test]
    fn exhaustive_short_lines() {
        let alphabet: [u16; 10] = [
            SPACE,
            TAB,
            GT,
            OPEN_BRACKET,
            CLOSE_BRACKET,
            COLON,
            b'x' as u16,
            b'<' as u16,
            0xA0,
            LF,
        ];
        let mut line = Vec::new();
        fn walk(line: &mut Vec<u16>, alphabet: &[u16], depth: usize) {
            check(line);
            if depth == 0 {
                return;
            }
            for &c in alphabet {
                line.push(c);
                walk(line, alphabet, depth - 1);
                line.pop();
            }
        }
        walk(&mut line, &alphabet, 5);
    }

    #[test]
    fn structured_lines() {
        let mut seed: u64 = 1101;
        let mut random = |n: u64| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) % n) as usize
        };
        let pick = |items: &[&str], k: usize| jsstr::u(items[k % items.len()]);
        for _ in 0..20_000 {
            let mut line: JsString = Vec::new();
            for _ in 0..random(25) {
                line.extend(std::iter::repeat_n(SPACE, random(5)));
                line.push(GT);
                line.extend(pick(&["", "", " ", "\t", "  "], random(5)));
            }
            line.extend(std::iter::repeat_n(SPACE, random(5)));
            line.extend(pick(&["[", "[", "[", "", ">", "\t["], random(6)));
            line.extend(pick(
                &["", "a", "label", "a b", "]", "\n", "\u{3000}"],
                random(7),
            ));
            line.extend(pick(&["]:", "]:", "]", "]: ", "] :", ""], random(6)));
            line.extend(pick(&["", " ", "\t", "  ", "\u{a0}"], random(5)));
            line.extend(pick(
                &["https://x", "<>", "<a b>", "<", "", " ", "\u{3000}", "x y"],
                random(8),
            ));
            check(&line);
        }
    }

    #[test]
    fn deep_lines_are_linear() {
        for line in [
            format!("{}q", "> ".repeat(5000)),
            format!("{} q", ">".repeat(5000)),
            format!("{}[a]: https://x", "> ".repeat(5000)),
        ] {
            let line = jsstr::u(&line);
            let started = std::time::Instant::now();
            let _ = match_definition(&line);
            assert_eq!(scan_quote_markers(&line).0, 5000);
            // 時間上限只在 release build 斷言，避免 debug CI 誤報。
            assert!(
                cfg!(debug_assertions) || started.elapsed() < std::time::Duration::from_secs(1)
            );
        }
    }
}
