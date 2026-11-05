//! Markdown 內容偵測。規格見 PRD.md〈6.6 Markdown 偵測〉。
//!
//! 剪貼簿與 `dedent-paste` 拿到的文字可能是程式碼、log、資料檔或純文字，只有
//! 判定為 Markdown 時才值得套用 [`crate::fix_document`]。誤判的代價很高（例如
//! 把 Python 腳本裡刻意留的多個空行合併掉），所以這裡的判斷刻意**保守**：
//! 寧可漏判，也不要誤判。
//!
//! # 做法
//!
//! 1. 只看前 [`MAX_LINES`](crate::detect::MAX_LINES) 行，每行的行內掃描最多看前 [`MAX_INLINE_BYTES`](crate::detect::MAX_INLINE_BYTES)
//!    位元組，整體工作量對輸入長度是線性的。
//! 2. 先找出檔首 frontmatter 與**成對**的 fenced code 區塊。fence 內的內容不參與
//!    任何正向或反向訊號的判斷。沒有成對的 fence 不算訊號，其後的行照常參與
//!    反向訊號檢查（避免程式碼裡出現一個 ```` ``` ```` 就躲過程式碼判斷）。
//! 3. 逐行收集正向訊號（每種只計一次分數）與反向訊號。
//! 4. 任一反向訊號成立就判定為非 Markdown。
//!
//! # 正向訊號與權重
//!
//! | 訊號 | 權重 |
//! | --- | --- |
//! | `fenced-code` | 3 |
//! | `table` | 3 |
//! | `atx-heading` | 2 |
//! | `link` | 2 |
//! | `list` | 1 |
//! | `task-list` | 2 |
//! | `emphasis` | 1 |
//! | `blockquote` | 1 |
//! | `reference-definition` | 2 |
//! | `frontmatter` | 2 |
//! | `syntax-error` | 2 |
//!
//! `syntax-error` 的權重比 PRD 建議的 1 高：`**標籤：**內容` 這類兩、三行的 AI
//! 輸出片段只有 `emphasis` 與 `syntax-error` 兩種訊號，權重 1 時永遠到不了門檻。
//! 為了抵銷提高權重的風險，`#標題` 型的筆誤必須和 ATX 標題一樣，後面接空行、
//! 檔尾或另一個 Markdown 區塊才算數，否則 `#安裝套件` 這種緊貼指令的 shell
//! 註解會被當成標題。
//!
//! # 判定規則
//!
//! - 只有一行非空白內容：只有命中 `syntax-error` 且沒有反向訊號時才算 Markdown。
//! - 多行：分數至少 3，且至少兩種訊號或命中權重 3 的訊號，同時沒有反向訊號。
//!
//! 反向訊號（shebang、前置處理指令、連續的 `#` 註解區塊、`"""` 多行字串界線、
//! 程式碼宣告與程式碼特徵、log、stack trace、diff、JSON、YAML（含 block
//! scalar）、TOML、XML）不會出現在 [`Detection::signals`](crate::detect::Detection::signals)，`signals` 只列正向訊號。
//!
//! 已知的取捨：Docusaurus／MDX 文件有 `import X from '...'` 這種 JavaScript 行，
//! 會被當成程式碼而判定為非 Markdown。

/// 最多檢查的行數。超過的部分不影響判斷。
pub const MAX_LINES: usize = 2_000;

/// 每行行內掃描（連結、強調、inline code）最多檢查的位元組數。
pub const MAX_INLINE_BYTES: usize = 4_096;

/// 整份文字行內掃描的位元組總預算。
const INLINE_BUDGET: usize = 256 * 1024;

/// 偵測結果。
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Detection {
    /// 是否判定為 Markdown。
    pub is_markdown: bool,
    /// 正向訊號分數總和。
    pub score: i32,
    /// 命中的訊號名稱（穩定字串，例如 `"atx-heading"`、`"fenced-code"`）。
    ///
    /// 只包含正向訊號，順序固定為模組文件表格的順序，不重複。
    pub signals: Vec<&'static str>,
}

const FENCED_CODE: usize = 0;
const TABLE: usize = 1;
const ATX_HEADING: usize = 2;
const LINK: usize = 3;
const LIST: usize = 4;
const TASK_LIST: usize = 5;
const EMPHASIS: usize = 6;
const BLOCKQUOTE: usize = 7;
const REFERENCE_DEFINITION: usize = 8;
const FRONTMATTER: usize = 9;
const SYNTAX_ERROR: usize = 10;
const SIGNAL_COUNT: usize = 11;

/// 正向訊號的名稱與權重，索引對應上面的常數。
const SIGNALS: [(&str, i32); SIGNAL_COUNT] = [
    ("fenced-code", 3),
    ("table", 3),
    ("atx-heading", 2),
    ("link", 2),
    ("list", 1),
    ("task-list", 2),
    ("emphasis", 1),
    ("blockquote", 1),
    ("reference-definition", 2),
    ("frontmatter", 2),
    ("syntax-error", 2),
];

/// 保守地判斷一段文字是否「看起來像 Markdown」。規格見 PRD.md〈Markdown 偵測〉。
///
/// 純函式：不 panic、不做 I/O，工作量對輸入長度線性，且只檢查前 [`MAX_LINES`] 行。
pub fn looks_like_markdown(text: &str) -> Detection {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let lines: Vec<&str> = text
        .split('\n')
        .take(MAX_LINES)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();

    let mut hits = [false; SIGNAL_COUNT];
    let nonblank = lines.iter().filter(|line| !is_blank(line)).count();
    if nonblank == 0 {
        return Detection {
            is_markdown: false,
            score: 0,
            signals: Vec::new(),
        };
    }

    let body_start = match frontmatter_end(&lines) {
        Some(end) => {
            if lines[end + 1..].iter().any(|line| !is_blank(line)) {
                hits[FRONTMATTER] = true;
            }
            end + 1
        }
        None => 0,
    };

    let in_fence = mark_fences(&lines, body_start);
    if in_fence.iter().any(|inside| *inside) {
        hits[FENCED_CODE] = true;
    }

    let negative = scan_lines(&lines, body_start, &in_fence, &mut hits);

    let mut score = 0;
    let mut signals = Vec::new();
    let mut has_heavy_signal = false;
    for (index, (name, weight)) in SIGNALS.iter().enumerate() {
        if hits[index] {
            score += weight;
            signals.push(*name);
            has_heavy_signal |= *weight >= 3;
        }
    }

    let is_markdown = !negative
        && if nonblank == 1 {
            hits[SYNTAX_ERROR]
        } else {
            score >= 3 && (signals.len() >= 2 || has_heavy_signal)
        };

    Detection {
        is_markdown,
        score,
        signals,
    }
}

/// 反向訊號的計數。
#[derive(Default)]
struct Negatives {
    /// 一出現就否決的強烈訊號（shebang、前置處理指令、diff、stack trace 標記、
    /// 明確的程式碼宣告、XML 宣告、JSON 陣列開頭）。
    veto: bool,
    /// 參與比例計算的行數（fence 與 frontmatter 以外的非空白行）。
    considered: usize,
    /// 以 `#` 開頭的行（可能是註解），不列入資料檔比例的分母。
    hash_lines: usize,
    weak_code: usize,
    log_lines: usize,
    stack_frames: usize,
    yaml_plain_keys: usize,
    yaml_list_keys: usize,
    yaml_list_scalars: usize,
    toml_lines: usize,
    tag_lines: usize,
}

impl Negatives {
    fn any(&self, first_content: Option<&str>) -> bool {
        if self.veto || self.stack_frames >= 2 {
            return true;
        }
        let considered = self.considered;
        if considered == 0 {
            return false;
        }
        // 弱程式碼特徵至少 20%。
        if self.weak_code >= 1 && self.weak_code * 5 >= considered {
            return true;
        }
        // log 行至少 30%。
        if self.log_lines >= 1 && self.log_lines * 10 >= considered * 3 {
            return true;
        }

        // YAML／TOML：扣掉 `#` 開頭的行後至少 60%。YAML 至少要有一行不在清單裡的
        // `key:`，避免把 `- fix: ...` 這種 changelog 清單當成 YAML。
        let data_lines = considered.saturating_sub(self.hash_lines).max(1);
        let yaml_keys = self.yaml_plain_keys + self.yaml_list_keys;
        let yaml_lines = yaml_keys + self.yaml_list_scalars;
        if self.yaml_plain_keys >= 1 && yaml_lines >= 2 && yaml_lines * 10 >= data_lines * 6 {
            return true;
        }
        if self.toml_lines >= 2 && self.toml_lines * 10 >= data_lines * 6 {
            return true;
        }

        if let Some(first) = first_content {
            if first.starts_with('{') {
                return true;
            }
            if first.starts_with('<') && self.tag_lines * 2 >= considered {
                return true;
            }
        }
        false
    }
}

/// 逐行收集正向與反向訊號，回傳反向訊號是否成立。
fn scan_lines(
    lines: &[&str],
    body_start: usize,
    in_fence: &[bool],
    hits: &mut [bool; SIGNAL_COUNT],
) -> bool {
    let mut neg = Negatives::default();

    // shebang 只看整段文字的第一個非空白行。
    if lines
        .iter()
        .find(|line| !is_blank(line))
        .is_some_and(|line| trim(line).starts_with("#!"))
    {
        neg.veto = true;
    }

    let first_content = (body_start..lines.len())
        .find(|&i| !in_fence[i] && !is_blank(lines[i]))
        .map(|i| trim(lines[i]));
    if let Some(first) = first_content {
        let head = first.get(..9).unwrap_or(first).to_ascii_lowercase();
        if head.starts_with("<?xml") || head.starts_with("<!doctype") {
            neg.veto = true;
        }
        if looks_like_json_array_start(first, lines, in_fence, body_start) {
            neg.veto = true;
        }
    }

    let mut previous_was_list_item = false;
    let mut inline_budget = INLINE_BUDGET;
    // 連續的 `#` 行（中間沒有空行）與其中是否有 `#`、`#foo` 這種不像標題的行。
    let mut hash_run = 0usize;
    let mut hash_run_has_comment = false;

    for i in body_start..lines.len() {
        if in_fence[i] {
            previous_was_list_item = false;
            hash_run = 0;
            continue;
        }
        let line = lines[i];
        if is_blank(line) {
            hash_run = 0;
            continue;
        }
        neg.considered += 1;

        let indent = indent_width(line);
        let t = trim(line);

        // ---- 反向訊號：不論是否像 Markdown 結構都檢查 ----
        if is_preprocessor_directive(t)
            || is_diff_marker(t)
            || is_stack_marker(t)
            || is_triple_quote_line(t)
        {
            neg.veto = true;
        }

        // 註解區塊：連續三行以上的 `#`，或連續兩行且其中一行是 `#`／`#foo`。
        // Markdown 很少連續放三個標題，也不會有空標題。
        if t.starts_with('#') && !t.starts_with("#[") && !t.starts_with("#![") {
            hash_run += 1;
            hash_run_has_comment |= is_bare_hash_comment(t);
            if hash_run >= 3 || (hash_run >= 2 && hash_run_has_comment) {
                neg.veto = true;
            }
        } else {
            hash_run = 0;
            hash_run_has_comment = false;
        }
        if is_stack_frame(t) {
            neg.stack_frames += 1;
        }
        if is_log_line(t) {
            neg.log_lines += 1;
        }
        if t.starts_with('#') {
            neg.hash_lines += 1;
        } else {
            match yaml_key_line(t) {
                Some(true) => neg.yaml_list_keys += 1,
                Some(false) => {
                    neg.yaml_plain_keys += 1;
                    // `key: |`、`key: >-` 這種 YAML block scalar 只會出現在 YAML。
                    if is_yaml_block_scalar_key(t) {
                        neg.veto = true;
                    }
                }
                None => {
                    if is_yaml_scalar_item(t) {
                        neg.yaml_list_scalars += 1;
                    }
                }
            }
            if is_toml_line(t) {
                neg.toml_lines += 1;
            }
        }
        if t.starts_with('<') {
            neg.tag_lines += 1;
        }

        // ---- 正向訊號 ----
        let heading = if indent <= 3 {
            atx_heading_text(t)
        } else {
            None
        };
        let list_rest = list_item_rest(t);
        let quote = indent <= 3 && is_blockquote(t);
        let reference = indent <= 3 && is_reference_definition(t);

        if let Some(heading_text) = heading {
            if !hits[ATX_HEADING]
                && !heading_text_looks_like_code(heading_text)
                && block_context_follows(lines, in_fence, i)
            {
                hits[ATX_HEADING] = true;
            }
        } else if indent <= 3 && is_hash_cjk_heading(t) && block_context_follows(lines, in_fence, i)
        {
            hits[SYNTAX_ERROR] = true;
        }

        if let Some(rest) = list_rest {
            if previous_was_list_item {
                hits[LIST] = true;
            }
            if is_task_box(rest) {
                hits[TASK_LIST] = true;
            }
            previous_was_list_item = true;
        } else {
            previous_was_list_item = false;
            if is_dash_cjk(t) {
                hits[SYNTAX_ERROR] = true;
            }
        }

        if quote {
            hits[BLOCKQUOTE] = true;
        }
        if reference {
            hits[REFERENCE_DEFINITION] = true;
        }

        if !hits[TABLE] && t.contains('|') && i + 1 < lines.len() && !in_fence[i + 1] {
            let next = trim(lines[i + 1]);
            if !is_delimiter_row(t)
                && is_delimiter_row(next)
                && table_cell_count(t) == table_cell_count(next)
            {
                hits[TABLE] = true;
            }
        }

        if inline_budget > 0 && !(hits[LINK] && hits[EMPHASIS] && hits[SYNTAX_ERROR]) {
            let slice = prefix_at_boundary(line, MAX_INLINE_BYTES.min(inline_budget));
            inline_budget = inline_budget.saturating_sub(slice.len().max(1));
            let inline = scan_inline(slice);
            hits[LINK] |= inline.link;
            hits[EMPHASIS] |= inline.emphasis;
            hits[SYNTAX_ERROR] |= inline.cjk_emphasis_error;
        }

        // ---- 程式碼特徵：Markdown 結構行與 `#` 註解行不檢查 ----
        let structural = heading.is_some()
            || list_rest.is_some()
            || quote
            || reference
            || t.starts_with('|')
            || (t.starts_with('#') && !t.starts_with("#[") && !t.starts_with("#!["));
        if !structural {
            if is_strong_code(t) {
                neg.veto = true;
            } else if is_weak_code(t) || line.starts_with('\t') {
                // tab 開頭的非清單行：Makefile recipe、Go 程式碼。
                neg.weak_code += 1;
            }
        }
    }

    neg.any(first_content)
}

// ---------------------------------------------------------------------------
// 共用小工具
// ---------------------------------------------------------------------------

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// 去掉前後的空白、tab 與 BOM。
fn trim(line: &str) -> &str {
    line.trim_matches(|c: char| c == '\u{FEFF}' || c.is_whitespace())
}

/// 行首縮排寬度；tab 視為 4 格。
fn indent_width(line: &str) -> usize {
    let mut width = 0;
    for byte in line.bytes() {
        match byte {
            b' ' => width += 1,
            b'\t' => width += 4,
            _ => break,
        }
    }
    width
}

/// 取前 `max` 位元組，往前退到字元邊界。
fn prefix_at_boundary(line: &str, max: usize) -> &str {
    if line.len() <= max {
        return line;
    }
    let mut end = max;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2E80..=0x2FFF
            | 0x3000..=0x303F
            | 0x3040..=0x30FF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFF00..=0xFFEF
    )
}

/// 緊接在收尾 `**` 前面、後面又接著文字時，會讓 CommonMark 無法配對的全形標點。
fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '：' | '，'
            | '。'
            | '！'
            | '？'
            | '；'
            | '、'
            | '）'
            | '」'
            | '』'
            | '】'
            | '〉'
            | '》'
            | '”'
            | '’'
            | '…'
    )
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

/// 取出開頭的識別字，回傳 `(識別字, 其餘)`。
fn take_ident(s: &str) -> Option<(&str, &str)> {
    let first = s.chars().next()?;
    if !is_ident_start(first) {
        return None;
    }
    let end = s.find(|c: char| !is_ident_char(c)).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

/// `a.b.c`、`a::b`、`a-b/c` 這類沒有空白的路徑。
fn is_ident_path(s: &str) -> bool {
    s.chars().next().is_some_and(is_ident_start)
        && s.chars()
            .all(|c| is_ident_char(c) || matches!(c, '.' | ':' | '-' | '/' | '*'))
}

fn starts_with_digits(s: &str, count: usize) -> bool {
    s.len() >= count && s.as_bytes()[..count].iter().all(u8::is_ascii_digit)
}

// ---------------------------------------------------------------------------
// frontmatter 與 fenced code
// ---------------------------------------------------------------------------

/// 檔首 `---` 到下一個 `---` 或 `...`，中間至少一行 `key: value`。回傳結尾行索引。
fn frontmatter_end(lines: &[&str]) -> Option<usize> {
    if lines.first().map(|line| line.trim_end()) != Some("---") {
        return None;
    }
    let mut has_key = false;
    for (index, line) in lines.iter().enumerate().skip(1) {
        let line = line.trim_end();
        if line == "---" || line == "..." {
            return (has_key && index >= 2).then_some(index);
        }
        if yaml_key_line(trim(line)).is_some() {
            has_key = true;
        }
    }
    None
}

/// fence 開頭：任意縮排後至少三個 `` ` `` 或 `~`；backtick fence 的 info string
/// 不得含 backtick。容許任意縮排是為了涵蓋清單項目裡的 fence。
fn fence_open(line: &str) -> Option<(u8, usize)> {
    let t = line.trim_start_matches([' ', '\t']);
    let marker = *t.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let run = t.bytes().take_while(|b| *b == marker).count();
    if run < 3 || (marker == b'`' && t[run..].contains('`')) {
        return None;
    }
    Some((marker, run))
}

fn fence_close(line: &str, marker: u8, open_len: usize) -> bool {
    let t = line.trim_start_matches([' ', '\t']);
    let run = t.bytes().take_while(|b| *b == marker).count();
    run >= open_len && t[run..].trim().is_empty()
}

/// 標出成對 fence（含開頭與結尾行）涵蓋的行。遇到沒有成對的 fence 就停止尋找，
/// 其後的行維持「不在 fence 內」，讓反向訊號照常檢查。每行最多被看兩次。
fn mark_fences(lines: &[&str], start: usize) -> Vec<bool> {
    let mut in_fence = vec![false; lines.len()];
    let mut i = start;
    while i < lines.len() {
        if let Some((marker, len)) = fence_open(lines[i]) {
            match (i + 1..lines.len()).find(|&j| fence_close(lines[j], marker, len)) {
                Some(close) => {
                    in_fence[i..=close].fill(true);
                    i = close + 1;
                    continue;
                }
                None => break,
            }
        }
        i += 1;
    }
    in_fence
}

// ---------------------------------------------------------------------------
// 區塊層級的 Markdown 結構
// ---------------------------------------------------------------------------

/// ATX 標題：`#`～`######` 後接空白與非空文字。回傳標題文字。
fn atx_heading_text(t: &str) -> Option<&str> {
    let hashes = t.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim();
    (!text.is_empty()).then_some(text)
}

/// 標題文字看起來像被註解掉的程式碼。
fn heading_text_looks_like_code(text: &str) -> bool {
    text.ends_with([';', '{', '}', ')', '(', '\\'])
        || text.contains(" = ")
        || text.contains("()")
        || text.contains("=>")
        || text.contains("&&")
        || text.contains("||")
}

/// `#標題`：井號後直接接 CJK 字元。
fn is_hash_cjk_heading(t: &str) -> bool {
    let hashes = t.bytes().take_while(|b| *b == b'#').count();
    (1..=6).contains(&hashes) && t[hashes..].chars().next().is_some_and(is_cjk)
}

/// `-項目`：清單符號後直接接 CJK 字元。
fn is_dash_cjk(t: &str) -> bool {
    let mut chars = t.chars();
    matches!(chars.next(), Some('-' | '+')) && chars.next().is_some_and(is_cjk)
}

/// 標題後面是否接空行、檔尾或另一個 Markdown 區塊。shell／Python 的 `#` 註解
/// 通常緊貼著程式碼，用這個條件排除。
fn block_context_follows(lines: &[&str], in_fence: &[bool], index: usize) -> bool {
    let Some(next) = lines.get(index + 1) else {
        return true;
    };
    if is_blank(next) || in_fence[index + 1] {
        return true;
    }
    let t = trim(next);
    atx_heading_text(t).is_some()
        || list_item_rest(t).is_some()
        || is_blockquote(t)
        || t.starts_with('|')
}

/// 清單項目：`- `、`* `、`+ `、`1. `、`1) `，回傳符號後的內容。
fn list_item_rest(t: &str) -> Option<&str> {
    let bytes = t.as_bytes();
    let marker_end = match bytes.first()? {
        b'-' | b'*' | b'+' => 1,
        b'0'..=b'9' => {
            let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
            if digits > 9 || !matches!(bytes.get(digits), Some(b'.' | b')')) {
                return None;
            }
            digits + 1
        }
        _ => return None,
    };
    match bytes.get(marker_end) {
        Some(b' ' | b'\t') => Some(t[marker_end + 1..].trim_start()),
        _ => None,
    }
}

fn is_task_box(rest: &str) -> bool {
    let Some(after) = rest
        .strip_prefix("[ ]")
        .or_else(|| rest.strip_prefix("[x]"))
        .or_else(|| rest.strip_prefix("[X]"))
    else {
        return false;
    };
    after.is_empty() || after.starts_with([' ', '\t'])
}

/// `> ` 或單獨的 `>`；排除 Python REPL 的 `>>>`。
fn is_blockquote(t: &str) -> bool {
    if t.starts_with(">>>") {
        return false;
    }
    match t.strip_prefix('>') {
        Some(rest) => rest.is_empty() || rest.starts_with([' ', '\t', '>']),
        None => false,
    }
}

/// `[label]: url` 或註腳定義 `[^1]: 文字`。
fn is_reference_definition(t: &str) -> bool {
    let Some(rest) = t.strip_prefix('[') else {
        return false;
    };
    let Some(close) = rest.find("]:") else {
        return false;
    };
    let label = &rest[..close];
    if label.trim().is_empty() || label.contains(['[', ']']) {
        return false;
    }
    let after = &rest[close + 2..];
    let destination = after.trim();
    if destination.is_empty() {
        return false;
    }
    if label.starts_with('^') {
        return true;
    }
    if !after.starts_with([' ', '\t']) {
        return false;
    }
    let first = destination.split_whitespace().next().unwrap_or("");
    looks_like_destination(first.trim_start_matches('<').trim_end_matches('>'))
}

/// 連結目的地看起來像 URL 或路徑。
fn looks_like_destination(dest: &str) -> bool {
    if dest.is_empty() || dest.contains(char::is_whitespace) {
        return false;
    }
    let lower = dest.get(..8).unwrap_or(dest).to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("ftp://")
        || dest.starts_with(['/', '#'])
        || dest.contains('.')
}

/// 表格分隔列：`| --- | :---: |`，每格都是 `:?-+:?`；只有一格時必須有 `|`。
fn is_delimiter_row(t: &str) -> bool {
    if !t.contains('-')
        || !t
            .bytes()
            .all(|b| matches!(b, b'|' | b':' | b'-' | b' ' | b'\t'))
    {
        return false;
    }
    let mut cells = 0;
    for cell in strip_outer_pipes(t).split('|') {
        let cell = cell.trim();
        let core = cell.strip_prefix(':').unwrap_or(cell);
        let core = core.strip_suffix(':').unwrap_or(core);
        if core.is_empty() || !core.bytes().all(|b| b == b'-') {
            return false;
        }
        cells += 1;
    }
    cells > 1 || t.contains('|')
}

fn strip_outer_pipes(t: &str) -> &str {
    let t = t.strip_prefix('|').unwrap_or(t);
    if t.ends_with('|') && !t.ends_with("\\|") {
        &t[..t.len() - 1]
    } else {
        t
    }
}

/// 表格列的儲存格數（忽略 `\|`）。
fn table_cell_count(t: &str) -> usize {
    let bytes = strip_outer_pipes(t).as_bytes();
    1 + bytes
        .iter()
        .enumerate()
        .filter(|(index, byte)| **byte == b'|' && (*index == 0 || bytes[index - 1] != b'\\'))
        .count()
}

// ---------------------------------------------------------------------------
// 行內掃描：連結、粗體、inline code、CJK 粗體筆誤
// ---------------------------------------------------------------------------

#[derive(Default)]
struct InlineHits {
    link: bool,
    emphasis: bool,
    cjk_emphasis_error: bool,
}

/// 由左到右單次掃描一行，每個位置最多被看常數次。
fn scan_inline(line: &str) -> InlineHits {
    let bytes = line.as_bytes();
    let mut hits = InlineHits::default();
    // (backtick 數, 內容起點)
    let mut code_open: Option<(usize, usize)> = None;
    // 粗體內容起點
    let mut bold_open: Option<usize> = None;
    // `[` 之後的位置
    let mut bracket_open: Option<usize> = None;
    let mut i = 0;

    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'`' {
            let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
            match code_open {
                Some((open_len, start)) if open_len == run => {
                    if !line[start..i].trim().is_empty() {
                        hits.emphasis = true;
                    }
                    code_open = None;
                }
                Some(_) => {}
                None => code_open = Some((run, i + run)),
            }
            i += run;
            continue;
        }
        if code_open.is_some() {
            i += 1;
            continue;
        }

        match byte {
            b'*' => {
                let run = bytes[i..].iter().take_while(|b| **b == b'*').count();
                if run >= 2 {
                    let prev = line[..i].chars().next_back();
                    let next = line[i + run..].chars().next();
                    let left = next.is_some_and(|c| !c.is_whitespace())
                        && !prev.is_some_and(|c| c.is_ascii_alphanumeric());
                    let right = prev.is_some_and(|c| !c.is_whitespace())
                        && !next.is_some_and(|c| c.is_ascii_alphanumeric());
                    match bold_open {
                        Some(start) if right && start < i => {
                            hits.emphasis = true;
                            if prev.is_some_and(is_cjk_punctuation)
                                && next.is_some_and(char::is_alphanumeric)
                            {
                                hits.cjk_emphasis_error = true;
                            }
                            bold_open = None;
                        }
                        _ if left => bold_open = Some(i + run),
                        _ => {}
                    }
                }
                i += run;
                continue;
            }
            b'[' => {
                let prev = line[..i].chars().next_back();
                let allowed = !prev.is_some_and(|c| is_ident_char(c) || c == ']' || c == ')');
                bracket_open = allowed.then_some(i + 1);
            }
            b']' => {
                if let Some(start) = bracket_open.take() {
                    if start < i && bytes.get(i + 1) == Some(&b'(') && !hits.link {
                        let (found, resume) = parse_link_destination(line, i + 2);
                        hits.link |= found;
                        i = resume.max(i + 1);
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    hits
}

/// 從 `(` 之後解析連結目的地。回傳 `(是否為合法連結, 繼續掃描的位置)`。
///
/// 失敗時從終止位置繼續掃描：中間的任何 `](` 都會停在同一個終止位置，
/// 不必重掃，維持線性時間。
fn parse_link_destination(line: &str, start: usize) -> (bool, usize) {
    let bytes = line.as_bytes();
    if bytes.get(start) == Some(&b'<') {
        return match line[start..].find('>') {
            Some(offset) => {
                let end = start + offset;
                let found = bytes.get(end + 1) == Some(&b')')
                    && looks_like_destination(&line[start + 1..end].replace(' ', "%20"));
                (found, end + 1)
            }
            None => (false, bytes.len()),
        };
    }
    let mut end = start;
    while end < bytes.len() && !matches!(bytes[end], b')' | b' ' | b'\t') {
        end += 1;
    }
    let dest = &line[start..end];
    match bytes.get(end) {
        Some(b')') => (looks_like_destination(dest), end + 1),
        Some(_) => {
            let titled = line[end..].trim_start().starts_with(['"', '\'']);
            (titled && looks_like_destination(dest), end)
        }
        None => (false, end),
    }
}

// ---------------------------------------------------------------------------
// 反向訊號
// ---------------------------------------------------------------------------

/// `#include`、`#define`、`#pragma`、`#region` 等前置處理指令。
fn is_preprocessor_directive(t: &str) -> bool {
    const DIRECTIVES: [&str; 16] = [
        "include",
        "define",
        "pragma",
        "region",
        "endregion",
        "ifdef",
        "ifndef",
        "endif",
        "undef",
        "import",
        "if",
        "elif",
        "else",
        "error",
        "warning",
        "line",
    ];
    let Some(rest) = t.strip_prefix('#') else {
        return false;
    };
    DIRECTIVES.iter().any(|directive| {
        rest.strip_prefix(directive)
            .is_some_and(|after| after.is_empty() || after.starts_with([' ', '\t', '<', '"', '(']))
    })
}

/// Python／TOML 多行字串的 `"""`、`'''` 界線。Markdown 的 fence 外不會出現。
fn is_triple_quote_line(t: &str) -> bool {
    ["\"\"\"", "'''"]
        .iter()
        .any(|quote| t.starts_with(quote) || t.ends_with(quote))
}

/// `#` 單獨一行，或 `#` 後直接接 ASCII 文字（`#foo`）：註解而不是標題。
fn is_bare_hash_comment(t: &str) -> bool {
    match t.strip_prefix('#') {
        Some(rest) => {
            rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_graphic() && c != '#')
        }
        None => false,
    }
}

/// unified diff 的標頭行。
fn is_diff_marker(t: &str) -> bool {
    t.starts_with("diff --git ")
        || (t.starts_with("@@ -") && t[4..].contains(" @@"))
        || t.starts_with("--- a/")
        || t.starts_with("+++ b/")
        || t.starts_with("new file mode ")
        || t.starts_with("deleted file mode ")
        || t.strip_prefix("index ").is_some_and(|rest| {
            let hex = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
            hex >= 7 && rest[hex..].starts_with("..")
        })
}

/// 明確的 stack trace／panic 標記。
fn is_stack_marker(t: &str) -> bool {
    t.starts_with("Traceback (most recent call last)")
        || t.starts_with("Exception in thread ")
        || t.starts_with("Caused by: ")
        || t.starts_with("Uncaught ")
        || t.starts_with("--- End of ")
        || (t.starts_with("thread '") && t.contains("' panicked at"))
        || t.strip_prefix("goroutine ")
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// 堆疊框架行：`at foo (file.js:1:2)`、`at a.b.C.d(C.java:42)`、
/// `File "x.py", line 3, in f`。
fn is_stack_frame(t: &str) -> bool {
    if let Some(rest) = t.strip_prefix("File \"") {
        return rest.contains("\", line ");
    }
    let Some(rest) = t.strip_prefix("at ") else {
        return false;
    };
    rest.ends_with("(native)")
        || rest.ends_with("(Native Method)")
        || rest.ends_with("(Unknown Source)")
        || rest.contains(":line ")
        || rest
            .as_bytes()
            .windows(2)
            .any(|pair| pair[0] == b':' && pair[1].is_ascii_digit())
}

/// log 行：行首是時間戳記或記錄層級。
fn is_log_line(t: &str) -> bool {
    let s = t.strip_prefix('[').unwrap_or(t);
    let b = s.as_bytes();
    // 2024-01-02 10:00、2024-01-02T10:00、2024/01/02 10:00
    if b.len() >= 16
        && starts_with_digits(s, 4)
        && matches!(b[4], b'-' | b'/')
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b[4]
        && b[8..10].iter().all(u8::is_ascii_digit)
        && matches!(b[10], b' ' | b'T')
        && b[11..13].iter().all(u8::is_ascii_digit)
        && b[13] == b':'
    {
        return true;
    }
    // 10:00:00
    if b.len() >= 8
        && b[..2].iter().all(u8::is_ascii_digit)
        && b[2] == b':'
        && b[3..5].iter().all(u8::is_ascii_digit)
        && b[5] == b':'
        && b[6..8].iter().all(u8::is_ascii_digit)
    {
        return true;
    }
    // syslog：Jan 12 10:00:00
    if b.len() >= 15
        && b[0].is_ascii_uppercase()
        && b[1].is_ascii_lowercase()
        && b[2].is_ascii_lowercase()
        && b[3] == b' '
        && (b[4] == b' ' || b[4].is_ascii_digit())
        && b[5].is_ascii_digit()
        && b[6] == b' '
        && b[7..9].iter().all(u8::is_ascii_digit)
        && b[9] == b':'
    {
        return true;
    }
    const LEVELS: [&str; 9] = [
        "INFO", "WARN", "WARNING", "ERROR", "DEBUG", "TRACE", "FATAL", "CRITICAL", "NOTICE",
    ];
    LEVELS.iter().any(|level| {
        s.strip_prefix(level)
            .is_some_and(|after| after.starts_with([']', ':', ' ', '\t']))
    })
}

/// YAML 的 `key: value`／`key:` 行。回傳 `Some(是否以 "- " 開頭)`。
fn yaml_key_line(t: &str) -> Option<bool> {
    let (s, in_list) = match t.strip_prefix("- ") {
        Some(rest) => (rest.trim_start(), true),
        None => (t, false),
    };
    let after_key = if let Some(rest) = s.strip_prefix('"') {
        &rest[rest.find('"')? + 1..]
    } else if let Some(rest) = s.strip_prefix('\'') {
        &rest[rest.find('\'')? + 1..]
    } else {
        if !s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            return None;
        }
        let end = s
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
            .unwrap_or(s.len());
        &s[end..]
    };
    let rest = after_key.strip_prefix(':')?;
    (rest.is_empty() || rest.starts_with([' ', '\t'])).then_some(in_list)
}

/// YAML 清單裡的純量 `- value`；以 Markdown 行內語法開頭的項目不算。
fn is_yaml_scalar_item(t: &str) -> bool {
    t.strip_prefix("- ")
        .is_some_and(|rest| !rest.starts_with(['*', '`', '[']) && !rest.contains("]("))
}

/// `key: |`、`key: >-` 等 YAML block scalar 開頭。
fn is_yaml_block_scalar_key(t: &str) -> bool {
    t.rsplit_once(':')
        .is_some_and(|(_, value)| matches!(value.trim(), "|" | ">" | "|-" | ">-" | "|+" | ">+"))
}

/// TOML／INI／.env 的 `key = value`、`[section]`、`[[array]]`。
fn is_toml_line(t: &str) -> bool {
    if let Some(inner) = t.strip_prefix('[') {
        let inner = inner.strip_prefix('[').unwrap_or(inner);
        let Some(close) = inner.find(']') else {
            return false;
        };
        let name = &inner[..close];
        let tail = inner[close..].trim_start_matches(']');
        return !name.is_empty()
            && tail.trim().is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '"' | ' '));
    }
    if !t.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '"') {
        return false;
    }
    let key_end = t
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '"')))
        .unwrap_or(t.len());
    let rest = t[key_end..].trim_start();
    rest.starts_with('=') && !rest.starts_with("==")
}

/// 整份文字以 JSON 陣列開頭（`[` 開頭但不是 README 的 badge 連結）。
fn looks_like_json_array_start(
    first: &str,
    lines: &[&str],
    in_fence: &[bool],
    start: usize,
) -> bool {
    if !first.starts_with('[') {
        return false;
    }
    if first == "["
        || first.starts_with("[{")
        || first.starts_with("[\"")
        || first.starts_with("[]")
        || first[1..].starts_with(|c: char| c.is_ascii_digit())
    {
        return true;
    }
    let last = (start..lines.len())
        .rev()
        .find(|&i| !in_fence[i] && !is_blank(lines[i]))
        .map(|i| trim(lines[i]));
    !first.contains("](") && last.is_some_and(|line| line.ends_with(']') && !line.ends_with("]]"))
}

/// 只有程式碼才會有的宣告形狀，一行就足以否決。
fn is_strong_code(t: &str) -> bool {
    // Python def／class
    if let Some(rest) = t
        .strip_prefix("async def ")
        .or_else(|| t.strip_prefix("def "))
    {
        return t.ends_with(':')
            && take_ident(rest).is_some_and(|(_, after)| after.starts_with('('));
    }
    if let Some(rest) = t.strip_prefix("class ") {
        return take_ident(rest).is_some() && t.ends_with([':', '{']);
    }
    if t.starts_with("if __name__ ==") || t.starts_with("<?php") || t.starts_with("module.exports")
    {
        return true;
    }
    if let Some(rest) = t.strip_prefix("import ") {
        return is_import_rest(rest);
    }
    if let Some(rest) = t.strip_prefix("from ") {
        let mut tokens = rest.split_whitespace();
        return tokens.next().is_some_and(is_ident_path) && tokens.next() == Some("import");
    }
    if let Some(rest) = t.strip_prefix("package ") {
        return is_ident_path(rest.strip_suffix(';').unwrap_or(rest));
    }
    if (t.starts_with("use ") && t.ends_with(';'))
        || ((t.starts_with("#[") || t.starts_with("#![")) && t.ends_with(']'))
    {
        return true;
    }

    // 函式宣告
    const FN_KEYWORDS: [&str; 10] = [
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "async fn ",
        "pub async fn ",
        "func ",
        "function ",
        "async function ",
        "function* ",
        "export function ",
    ];
    for keyword in FN_KEYWORDS {
        if let Some(rest) = t.strip_prefix(keyword) {
            // Go 的 method receiver：func (r *T) Name(
            let rest = if keyword == "func " && rest.starts_with('(') {
                rest.find(") ").map_or(rest, |end| &rest[end + 2..])
            } else {
                rest
            };
            if take_ident(rest).is_some_and(|(_, after)| after.starts_with(['(', '<'])) {
                return true;
            }
        }
    }
    if t.starts_with("function(") || t.starts_with("function (") {
        return true;
    }

    // const／let／var 宣告
    for keyword in [
        "const ",
        "let ",
        "let mut ",
        "var ",
        "export const ",
        "export let ",
    ] {
        if let Some((_, after)) = t.strip_prefix(keyword).and_then(take_ident) {
            let after = after.trim_start();
            let assigns = after.starts_with('=') && !after.starts_with("==");
            let typed = after.starts_with(':') && after.contains(" =");
            if assigns || typed {
                return true;
            }
        }
    }

    // export default ...、export { ... }、export class ...
    const EXPORTS: [&str; 6] = [
        "export default ",
        "export class ",
        "export interface ",
        "export type ",
        "export {",
        "export * ",
    ];
    if EXPORTS.iter().any(|prefix| t.starts_with(prefix)) {
        return true;
    }

    // Java／C# 的存取修飾詞
    const MODIFIED: [&str; 14] = [
        "class ",
        "static ",
        "void ",
        "interface ",
        "enum ",
        "final ",
        "abstract ",
        "async ",
        "override ",
        "readonly ",
        "int ",
        "string ",
        "bool ",
        "record ",
    ];
    for modifier in ["public ", "private ", "protected ", "internal "] {
        if let Some(rest) = t.strip_prefix(modifier) {
            if MODIFIED.iter().any(|next| rest.starts_with(next)) {
                return true;
            }
        }
    }

    // C 風格的函式宣告：int main(
    for ty in [
        "int ",
        "void ",
        "char ",
        "bool ",
        "double ",
        "float ",
        "long ",
        "unsigned ",
        "size_t ",
    ] {
        if let Some(rest) = t.strip_prefix(ty) {
            let rest = rest.trim_start_matches('*');
            if take_ident(rest).is_some_and(|(_, after)| after.starts_with('(')) {
                return true;
            }
        }
    }

    false
}

/// `import` 之後的內容像程式碼：`os`、`a.b as c`、`a, b`、`{ x } from "y"`、`"x";`。
fn is_import_rest(rest: &str) -> bool {
    let rest = rest.trim();
    if rest.ends_with(';') || rest.starts_with(['{', '*', '"', '\'']) || rest.starts_with("type ") {
        return true;
    }
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if !rest.contains(',') && !tokens.contains(&"as") {
        return tokens.len() == 1 && is_ident_path(tokens[0]);
    }
    tokens
        .iter()
        .all(|token| *token == "as" || is_ident_path(token.trim_end_matches(',')))
}

/// 常見於程式碼、shell 與指令輸出，但單獨一行不足以否決的特徵。
fn is_weak_code(t: &str) -> bool {
    if t.ends_with([';', '{', '}', '(', '[', '\\']) || t.starts_with(['}', ')', ']']) {
        return true;
    }
    // 散文不會出現的運算子組合（minified JS、C 系語言）。
    const OPERATORS: [&str; 6] = ["){", "!==", "===", "&&", "||", "$("];
    if OPERATORS.iter().any(|op| t.contains(op)) {
        return true;
    }
    const PREFIXES: [&str; 28] = [
        "//", "/*", "*/", "<?", "$ ", "% ", ">>> ", "elif ", "except", "raise ", "yield ",
        "await ", "if (", "for (", "while (", "switch (", "if [", "set -", "self.", "this.",
        "console.", "print(", "std::", "&& ", "ifeq ", "ifneq ", ".PHONY:", "include ",
    ];
    if PREFIXES.iter().any(|prefix| t.starts_with(prefix)) {
        return true;
    }
    const EXACT: [&str; 14] = [
        "fi", "done", "esac", "then", "do", "else", "else:", "try:", "finally:", "pass", "break",
        "continue", "end", "endif",
    ];
    if EXACT.contains(&t) || t.contains("; then") || t.contains("; do") {
        return true;
    }
    if (t.starts_with("for ") && t.contains(" in ") && t.ends_with(':'))
        || (t.starts_with("with ") && t.contains(" as ") && t.ends_with(':'))
    {
        return true;
    }
    // PowerShell 提示符：PS C:\>
    if t.starts_with("PS ") && t.contains('>') {
        return true;
    }
    // 裝飾器：@app.route(
    if let Some(rest) = t.strip_prefix('@') {
        let end = rest
            .find(|c: char| !(is_ident_char(c) || c == '.'))
            .unwrap_or(rest.len());
        if end > 0 && (end == rest.len() || rest[end..].starts_with('(')) {
            return true;
        }
    }
    if let Some((first, rest)) = t.split_once(' ') {
        // shell 指令（第一個參數不是中文，排除「git 是…」這類句子）
        const COMMANDS: [&str; 23] = [
            "npm", "npx", "pnpm", "yarn", "cargo", "git", "brew", "pip", "pip3", "python3",
            "docker", "kubectl", "curl", "wget", "sudo", "chmod", "mkdir", "cd", "ls", "echo",
            "apt", "apt-get", "rustup",
        ];
        if COMMANDS.contains(&first)
            && rest
                .split_whitespace()
                .next()
                .is_some_and(|arg| !arg.chars().any(is_cjk))
        {
            return true;
        }
        // Dockerfile 與 SQL 的大寫關鍵字
        const UPPER: [&str; 18] = [
            "FROM",
            "RUN",
            "COPY",
            "WORKDIR",
            "ENV",
            "EXPOSE",
            "CMD",
            "ENTRYPOINT",
            "ARG",
            "LABEL",
            "SELECT",
            "INSERT",
            "UPDATE",
            "DELETE",
            "CREATE",
            "ALTER",
            "DROP",
            "WHERE",
        ];
        if UPPER.contains(&first) {
            return true;
        }
    }
    if let Some(rest) = t.strip_prefix("export ") {
        if take_ident(rest).is_some_and(|(_, after)| after.starts_with('=')) {
            return true;
        }
    }
    // JSON 片段："key": value
    if t.starts_with('"') && t.contains("\":") {
        return true;
    }
    // 指派：x = 1、FOO=bar、x += 1、x := 1、a.b[0] = c
    if let Some((_, after)) = take_ident(t) {
        let after = after
            .trim_start_matches(|c: char| is_ident_char(c) || matches!(c, '.' | '[' | ']'))
            .trim_start();
        for op in ["+=", "-=", ":=", "?=", "="] {
            if after
                .strip_prefix(op)
                .is_some_and(|value| !value.starts_with('='))
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all(expected: bool, cases: &[(&str, &str)]) {
        let mut failures = Vec::new();
        for (name, text) in cases {
            let detection = looks_like_markdown(text);
            if detection.is_markdown != expected {
                failures.push(format!("{name}: {detection:?}"));
            }
        }
        assert!(
            failures.is_empty(),
            "expected is_markdown = {expected} for:\n{}",
            failures.join("\n")
        );
    }

    // -----------------------------------------------------------------------
    // 正例
    // -----------------------------------------------------------------------

    const MARKDOWN: &[(&str, &str)] = &[
        (
            "zh-TW AI 回答：標題、清單、粗體、表格",
            r#"好的，以下是重點整理：

## 主要功能

1. **自動縮排**：移除共同縮排
2. **Markdown 修正**：修正常見語法錯誤

| 項目 | 說明 |
| --- | --- |
| 速度 | 很快 |
"#,
        ),
        (
            "English README with badges and fenced code",
            r#"[![CI](https://github.com/x/y/actions/workflows/ci.yml/badge.svg)](https://github.com/x/y/actions)

# dedent-paste

Paste clipboard text with common indentation removed.

## Install

```sh
cargo install dedent-paste
```

See [the docs](https://example.com/docs) for details.
"#,
        ),
        ("單行 **標籤：**內容", "**標籤：**內容"),
        ("兩行 **標籤：**內容", "**優點：**速度快\n**缺點：**很慢"),
        (
            "三行粗體標籤加清單",
            "**重點：**這是說明\n- 第一項\n- 第二項",
        ),
        (
            "只有表格",
            "| Name | Value |\n|------|-------|\n| a | 1 |\n| b | 2 |\n",
        ),
        (
            "task list",
            "## TODO\n\n- [ ] 寫測試\n- [x] 寫程式\n- [ ] 寫文件\n",
        ),
        (
            "fenced Python inside prose",
            r#"Run this script:

```python
import os

def main():
    print(os.getcwd())
```

It prints the working directory.
"#,
        ),
        (
            "Hugo frontmatter",
            r#"---
title: "Hello"
date: 2024-01-01
tags: [rust]
---

# Hello

這是一篇 **部落格** 文章。
"#,
        ),
        (
            "reference links",
            r#"## Links

Read the [guide][guide] and the [FAQ][faq].

[guide]: https://example.com/guide
[faq]: https://example.com/faq
"#,
        ),
        (
            "blockquote, list and link",
            r#"> **Note**
> This is important.

- See [docs](./docs/README.md)
- Ask on [GitHub](https://github.com)
"#,
        ),
        ("單行 #標題", "#標題"),
        ("單行 -項目", "-項目"),
        (
            "CHANGELOG with conventional commit bullets",
            r#"# Changelog

## [0.6.1] - 2026-10-01

- fix: handle CRLF input
- feat: add logs subcommand

## [0.6.0] - 2026-09-01

- feat: image to text
"#,
        ),
        (
            "zh-TW 會議記錄",
            r#"# 週會記錄

## 出席

- Will
- Amy

## 決議

1. 下週發佈 v1.0
2. 補齊**文件**
"#,
        ),
        (
            "AI 回答：清單項目裡的 fenced code",
            r#"Here is how to set it up:

1. **Install** the package:

   ```sh
   npm install foo
   ```

2. **Configure** it in `foo.config.js`.
"#,
        ),
        ("#標題 後接空行與清單", "#安裝步驟\n\n- 下載\n- 執行\n"),
        ("CRLF Markdown", "# Title\r\n\r\n- one\r\n- two\r\n"),
        (
            "GitHub alert",
            "> [!NOTE]\n> Useful information.\n\n## Usage\n\nRun `foo --help`.\n",
        ),
        (
            "英文 chat 回答：標題直接接內文",
            r#"### Summary
The change fixes the bug.

### Details

- Added a **guard** for empty input
- Updated `README.md`
"#,
        ),
        (
            "footnote definition",
            "結論如下[^1]，詳見文件。\n\n- 第一點\n- 第二點\n\n[^1]: 資料來源：內部報告。\n",
        ),
        ("BOM 開頭的 Markdown", "\u{FEFF}# 標題\n\n- 項目\n- 項目\n"),
    ];

    #[test]
    fn detects_markdown_documents() {
        assert_all(true, MARKDOWN);
    }

    /// 刻意保守造成的漏判：只有低權重訊號的短片段不算 Markdown。改動權重前先確認
    /// 這些案例是否真的該翻轉。
    const KNOWN_FALSE_NEGATIVES: &[(&str, &str)] = &[
        (
            "nested list with emphasis",
            "- **Backend**\n  - Rust\n  - Go\n- **Frontend**\n  - TypeScript\n",
        ),
        (
            "only reference definitions",
            "[a]: https://a.example\n[b]: https://b.example\n",
        ),
        ("list plus inline code", "- run `make`\n- run `make test`\n"),
        (
            "Markdown with an unfenced command",
            "## Install\n\nnpm install foo\n\n- then run it\n- done\n",
        ),
    ];

    #[test]
    fn conservative_false_negatives_stay_negative() {
        assert_all(false, KNOWN_FALSE_NEGATIVES);
    }

    // -----------------------------------------------------------------------
    // 反例：Python
    // -----------------------------------------------------------------------

    const PYTHON: &[(&str, &str)] = &[
        (
            "script with comments",
            r#"# Utility script

import os
import sys


def main():
    # print the cwd
    print(os.getcwd())


if __name__ == "__main__":
    main()
"#,
        ),
        (
            "dedented function body",
            "x = compute(y)\nfor i in range(3):\n    total += i\nreturn total\n",
        ),
        (
            "docstring with Markdown and a fenced example",
            r#"class Parser:
    """Parse things.

    Args:
        - `text`: the **input**
        - `strict`: fail fast

    ```python
    Parser().parse("x")
    ```
    """

    def parse(self, text):
        return text
"#,
        ),
        (
            "中文註解沒有空格",
            "#計算總和\n\ntotal = 0\nfor n in nums:\n    total += n\n",
        ),
        (
            "REPL session",
            ">>> import math\n>>> math.sqrt(2)\n1.4142135623730951\n>>> [x for x in range(3)]\n[0, 1, 2]\n",
        ),
        (
            "Flask app with decorators",
            r#"from flask import Flask

app = Flask(__name__)

@app.route("/")
def index():
    return "hello"
"#,
        ),
        (
            "heading-like comment, kwargs",
            "# Settings\n\nDEFAULTS = dict(**base, **extra)\nTIMEOUT = 30\n",
        ),
        (
            "dataclass",
            r#"from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int
"#,
        ),
        (
            "pytest",
            "import pytest\n\ndef test_add():\n    assert add(1, 2) == 3\n",
        ),
        (
            "async def with type hints",
            "async def fetch(url: str) -> bytes:\n    async with session.get(url) as r:\n        return await r.read()\n",
        ),
        (
            "Python with unclosed fence in a string",
            "TEMPLATE = \"\"\"\n```\n## Title\n\n- item\n- item\n\"\"\"\ndef render():\n    return TEMPLATE\n",
        ),
    ];

    #[test]
    fn python_is_not_markdown() {
        assert_all(false, PYTHON);
    }

    // -----------------------------------------------------------------------
    // 反例：shell
    // -----------------------------------------------------------------------

    const SHELL: &[(&str, &str)] = &[
        (
            "shebang script",
            "#!/usr/bin/env bash\n# Build\n\nset -euo pipefail\n\n## Steps\n\n- one\n- two\n",
        ),
        (
            "comments directly above commands",
            "# Install deps\nnpm install\n# Build\nnpm run build\n# Test\ncargo test\n",
        ),
        (
            "comments separated by blank lines",
            "# Install\n\nbrew install foo\n\n# Configure\n\nfoo --init\n",
        ),
        (
            "heredoc with > continuation",
            "$ cat <<EOF\n> hello **world**\n> EOF\n",
        ),
        (
            "if/then/fi",
            "if [ -f .env ]; then\n  source .env\nfi\nexport PORT=3000\n",
        ),
        (
            "exports",
            "export PATH=\"$HOME/.cargo/bin:$PATH\"\nexport EDITOR=vim\n",
        ),
        ("#中文 後接空行與 backtick 指令", "#安裝\n\necho `date`\n"),
        ("#中文 緊貼指令", "#安裝套件\nnpm install\n"),
        (
            "Dockerfile",
            "FROM rust:1.85 AS build\nWORKDIR /app\nCOPY . .\nRUN cargo build --release \\\n    --locked\nCMD [\"/app/target/release/app\"]\n",
        ),
        (
            "Makefile",
            ".PHONY: build test\n\nbuild:\n\tcargo build --release\n\ntest:\n\tcargo test\n",
        ),
        (
            "PowerShell",
            "PS C:\\> Get-ChildItem\nPS C:\\> Set-Location src\n",
        ),
        ("for loop", "for f in *.md; do\n  echo \"$f\"\ndone\n"),
    ];

    #[test]
    fn shell_is_not_markdown() {
        assert_all(false, SHELL);
    }

    // -----------------------------------------------------------------------
    // 反例：其他程式語言
    // -----------------------------------------------------------------------

    const CODE: &[(&str, &str)] = &[
        (
            "Rust with doc comments",
            r#"/// Adds numbers.
///
/// # Examples
///
/// ```
/// assert_eq!(add(1, 2), 3);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
        ),
        (
            "Rust use and derive",
            "use std::fmt;\n\n#[derive(Debug, Clone)]\nstruct Point {\n    x: i32,\n}\n",
        ),
        (
            "JavaScript",
            "const fs = require('fs');\n\nfunction read(path) {\n  return fs.readFileSync(path, 'utf8');\n}\n",
        ),
        (
            "TypeScript",
            "import { foo } from './foo';\n\nexport interface Options {\n  name: string;\n}\n",
        ),
        (
            "C with #include",
            "#include <stdio.h>\n\nint main(void) {\n    printf(\"hi\\n\");\n    return 0;\n}\n",
        ),
        (
            "Go",
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n",
        ),
        (
            "Java",
            "public class Hello {\n    public static void main(String[] args) {\n        System.out.println(\"hi\");\n    }\n}\n",
        ),
        (
            "C# with #region",
            "#region Helpers\nprivate int Add(int a, int b) => a + b;\n#endregion\n",
        ),
        (
            "SQL",
            "SELECT id, name\nFROM users\nWHERE active = 1\nORDER BY name;\n",
        ),
        ("CSS", ".button {\n  color: red;\n  padding: 4px;\n}\n"),
        (
            "JS callback with array index call",
            "handlers[name](event);\nitems.forEach((x) => {\n  log(x);\n});\n",
        ),
        (
            "Python-style exponent and markdown-ish comment",
            "# Result\n\nresult = 2**10 + 3**2\nprint(result)\n",
        ),
    ];

    #[test]
    fn source_code_is_not_markdown() {
        assert_all(false, CODE);
    }

    // -----------------------------------------------------------------------
    // 反例：log 與 stack trace
    // -----------------------------------------------------------------------

    const LOGS: &[(&str, &str)] = &[
        (
            "ISO timestamps",
            "2026-11-05T10:00:00Z [info] started\n2026-11-05T10:00:01Z [warn] **slow** request\n",
        ),
        (
            "space-separated timestamps",
            "2026-11-05 10:00:00 INFO server started\n2026-11-05 10:00:02 ERROR failed: `x`\n- retry\n- retry\n",
        ),
        (
            "syslog",
            "Nov  5 10:00:00 host sshd[1]: Accepted key\nNov  5 10:00:01 host sshd[1]: session opened\n",
        ),
        (
            "bracketed levels",
            "[INFO] Building\n[WARN] Deprecated API\n[ERROR] Failed\n",
        ),
        ("time-only", "10:00:01 Compiling\n10:00:05 Done\n"),
        (
            "Java stack trace",
            "Exception in thread \"main\" java.lang.NullPointerException\n\tat com.foo.Bar.baz(Bar.java:42)\n\tat com.foo.Main.main(Main.java:10)\n",
        ),
        (
            "Node stack trace",
            "TypeError: x is not a function\n    at foo (/app/index.js:10:5)\n    at Object.<anonymous> (/app/index.js:20:1)\n",
        ),
        (
            "Python traceback",
            "Traceback (most recent call last):\n  File \"app.py\", line 3, in <module>\n    main()\nValueError: bad\n",
        ),
        (
            "Rust panic",
            "thread 'main' panicked at src/main.rs:2:5:\nexplicit panic\nnote: run with `RUST_BACKTRACE=1`\n",
        ),
        (
            "Go panic",
            "panic: runtime error\n\ngoroutine 1 [running]:\nmain.main()\n\t/app/main.go:5 +0x1d\n",
        ),
        (
            "GitHub Actions log",
            "2026-11-05T10:00:00.1234567Z ##[group]Run cargo test\n2026-11-05T10:00:00.2Z cargo test\n2026-11-05T10:00:09.0Z ##[endgroup]\n",
        ),
        (
            "C# stack trace",
            "   at Foo.Bar() in C:\\src\\Foo.cs:line 10\n   at Foo.Main() in C:\\src\\Foo.cs:line 3\n",
        ),
    ];

    #[test]
    fn logs_are_not_markdown() {
        assert_all(false, LOGS);
    }

    // -----------------------------------------------------------------------
    // 反例：JSON
    // -----------------------------------------------------------------------

    const JSON: &[(&str, &str)] = &[
        (
            "object",
            "{\n  \"name\": \"x\",\n  \"version\": \"1.0.0\"\n}\n",
        ),
        ("array of objects", "[\n  {\"id\": 1},\n  {\"id\": 2}\n]\n"),
        (
            "inline array",
            "[{\"a\": \"**b**\"}, {\"c\": \"[d](e.f)\"}]",
        ),
        ("array of numbers", "[1, 2, 3]\n"),
        ("JSON lines", "{\"level\":\"info\"}\n{\"level\":\"warn\"}\n"),
        (
            "package.json",
            r#"{
  "name": "foo",
  "scripts": {
    "test": "node --test"
  }
}
"#,
        ),
        (
            "tsconfig with comments",
            "{\n  // compiler options\n  \"compilerOptions\": {\"strict\": true}\n}\n",
        ),
        ("empty object", "{}\n"),
        ("minified", "{\"a\":[1,2,{\"b\":\"# c\\n- d\\n- e\"}]}"),
        (
            "fragment without braces",
            "\"name\": \"foo\",\n\"description\": \"A **tool**\",\n\"version\": \"1.0\"\n",
        ),
        ("array of strings", "[\n  \"# one\",\n  \"- two\"\n]\n"),
    ];

    #[test]
    fn json_is_not_markdown() {
        assert_all(false, JSON);
    }

    // -----------------------------------------------------------------------
    // 反例：YAML、TOML、INI、XML、diff
    // -----------------------------------------------------------------------

    const YAML: &[(&str, &str)] = &[
        (
            "GitHub Actions workflow",
            r#"name: CI
on:
  push:
    branches: [main]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test
"#,
        ),
        (
            "docker-compose with comments",
            r#"# Services

services:
  web:
    image: nginx
    ports:
      - "80:80"
"#,
        ),
        (
            "Kubernetes manifest",
            "apiVersion: v1\nkind: Pod\nmetadata:\n  name: web\nspec:\n  containers:\n    - name: web\n      image: nginx\n",
        ),
        (
            "Ansible playbook",
            "- name: setup\n  hosts: all\n  tasks:\n    - name: install\n      apt:\n        name: git\n",
        ),
        (
            "multi-document stream",
            "---\na: 1\nb: 2\n---\nc: 3\nd: 4\n",
        ),
        (
            "mkdocs.yml",
            "site_name: Docs\nnav:\n  - Home: index.md\n  - Guide: guide.md\ntheme:\n  name: material\n",
        ),
        (
            "list of scalars under a key",
            "# Allowed hosts\n\nhosts:\n  - a.example.com\n  - b.example.com\n  - c.example.com\n",
        ),
        (
            "OpenAPI",
            "openapi: 3.0.0\ninfo:\n  title: API\n  version: 1.0.0\npaths:\n  /users:\n    get:\n      summary: List **users**\n",
        ),
        (
            "simple key-values",
            "host: localhost\nport: 8080\ndebug: true\n",
        ),
        (
            "gitlab-ci",
            "stages:\n  - build\n  - test\n\nbuild:\n  stage: build\n  script:\n    - make\n",
        ),
    ];

    #[test]
    fn yaml_is_not_markdown() {
        assert_all(false, YAML);
    }

    const DATA_AND_MARKUP: &[(&str, &str)] = &[
        (
            "Cargo.toml",
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        ),
        (
            "pyproject with array table",
            "[project]\nname = \"foo\"\n\n[[tool.mypy.overrides]]\nmodule = \"x\"\n",
        ),
        ("INI", "; comment\n[core]\neditor = vim\npager = less\n"),
        (
            ".env",
            "DATABASE_URL=postgres://localhost/db\nSECRET_KEY=abc\nDEBUG=1\n",
        ),
        (
            "XML",
            "<?xml version=\"1.0\"?>\n<project>\n  <name>foo</name>\n</project>\n",
        ),
        (
            "HTML page",
            "<!DOCTYPE html>\n<html>\n<body>\n<h1>Title</h1>\n<p>**not markdown**</p>\n</body>\n</html>\n",
        ),
        (
            "SVG",
            "<svg xmlns=\"http://www.w3.org/2000/svg\">\n  <rect width=\"10\"/>\n</svg>\n",
        ),
        (
            "git diff",
            r#"diff --git a/README.md b/README.md
index 1234567..89abcde 100644
--- a/README.md
+++ b/README.md
@@ -1,3 +1,4 @@
 # Title
+
- old item
+- new item
"#,
        ),
        (
            "bare hunk",
            "@@ -10,4 +10,5 @@ fn main() {\n-    old();\n+    new();\n",
        ),
    ];

    #[test]
    fn data_files_and_diffs_are_not_markdown() {
        assert_all(false, DATA_AND_MARKUP);
    }

    // -----------------------------------------------------------------------
    // 反例：純文字與終端機輸出
    // -----------------------------------------------------------------------

    const PLAIN: &[(&str, &str)] = &[
        (
            "English paragraph",
            "The quick brown fox jumps over the lazy dog. It was a sunny day,\nand everyone was happy.\n",
        ),
        (
            "中文段落",
            "今天天氣很好，我們去公園散步。\n晚上回家吃飯，然後看電影。\n",
        ),
        (
            "email with quote and list",
            r#"Hi Will,

Thanks for the update. A few notes:

- item one
- item two

> On Monday you wrote:
> please review

Best,
Amy
"#,
        ),
        (
            "URL only",
            "https://github.com/doggy8088/dedent-paste/pull/42\n",
        ),
        ("single sentence", "Please review the attached file."),
        (
            "single line with bold but no syntax error",
            "This is **important** and see [docs](https://x.y).",
        ),
        (
            "ls -l output",
            "total 16\n-rw-r--r--  1 will  staff  1024 Nov  5 10:00 README.md\ndrwxr-xr-x  4 will  staff   128 Nov  5 10:00 src\n",
        ),
        (
            "psql output",
            " id | name\n----+------\n  1 | foo\n  2 | bar\n(2 rows)\n",
        ),
        (
            "git log",
            "commit 7d298db1a2b3c4d5e6f7\nAuthor: Will <will@example.com>\nDate:   Thu Nov 5 10:00:00 2026 +0800\n\n    chore: release v0.6.1\n",
        ),
        (
            "git status",
            "On branch main\nChanges not staged for commit:\n  (use \"git add <file>...\" to update what will be committed)\n\tmodified:   src/lib.rs\n",
        ),
        ("CSV", "name,age,city\nAmy,30,Taipei\nBob,25,Tainan\n"),
        ("numbered plain items", "1. buy milk\n2. call mom\n"),
        (
            "cargo build output",
            "   Compiling foo v0.1.0 (/app)\nwarning: unused variable: `x`\n    Finished `dev` profile in 1.2s\n",
        ),
        (
            "kubectl output",
            "NAME    READY   STATUS    RESTARTS   AGE\nweb-1   1/1     Running   0          5d\n",
        ),
        ("whitespace only", "   \n\t\n"),
        ("empty", ""),
        (
            "address",
            "Will Huang\nNo. 1, Section 1\nTaipei City\nTaiwan\n",
        ),
        (
            "footnote-style asterisks",
            "Prices exclude tax*\n*Except in some regions\n",
        ),
        (
            "meeting notes as key-values",
            "Date: 2026-11-05\nAttendees: Amy, Bob\nTopic: release\n",
        ),
        (
            "single line with heading marker and space",
            "# just a comment",
        ),
    ];

    #[test]
    fn plain_text_and_terminal_output_are_not_markdown() {
        assert_all(false, PLAIN);
    }

    // -----------------------------------------------------------------------
    // 訊號名稱與分數
    // -----------------------------------------------------------------------

    #[test]
    fn reports_exact_signals_in_catalog_order() {
        let detection = looks_like_markdown(MARKDOWN[0].1);
        assert_eq!(
            detection,
            Detection {
                is_markdown: true,
                score: 3 + 2 + 1 + 1,
                signals: vec!["table", "atx-heading", "list", "emphasis"],
            }
        );

        let detection = looks_like_markdown("**優點：**速度快\n**缺點：**很慢");
        assert_eq!(detection.signals, vec!["emphasis", "syntax-error"]);
        assert_eq!(detection.score, 3);
        assert!(detection.is_markdown);
    }

    #[test]
    fn reports_every_signal_name() {
        let text = r#"---
title: x
---

# Title

- [ ] task
- item

> quote

| a | b |
| - | - |
| 1 | 2 |

See [link](https://x.y) and **bold**.

**標籤：**內容

[ref]: https://example.com

```
code
```
"#;
        let detection = looks_like_markdown(text);
        assert!(detection.is_markdown, "{detection:?}");
        assert_eq!(
            detection.signals,
            SIGNALS.iter().map(|(name, _)| *name).collect::<Vec<_>>()
        );
        assert_eq!(detection.score, SIGNALS.iter().map(|(_, w)| w).sum::<i32>());
    }

    #[test]
    fn negative_detection_still_reports_positive_signals() {
        let detection = looks_like_markdown("#!/bin/sh\n# Title\n\n- a\n- b\n");
        assert!(!detection.is_markdown);
        assert_eq!(detection.signals, vec!["atx-heading", "list"]);
    }

    #[test]
    fn single_line_needs_a_syntax_error() {
        assert!(!looks_like_markdown("| a | b |").is_markdown);
        assert!(!looks_like_markdown("## Heading").is_markdown);
        assert!(looks_like_markdown("##標題\n").is_markdown);
        assert!(looks_like_markdown("\n\n**說明：**文字\n\n").is_markdown);
        assert!(!looks_like_markdown("#!標題").is_markdown);
    }

    #[test]
    fn unpaired_fence_is_not_a_signal_and_keeps_vetoes() {
        let detection = looks_like_markdown("```\nimport os\n\n- a\n- b\n");
        assert!(!detection.signals.contains(&"fenced-code"));
        assert!(!detection.is_markdown);
    }

    #[test]
    fn code_inside_fences_does_not_veto() {
        let text = "## Example\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n";
        let detection = looks_like_markdown(text);
        assert!(detection.is_markdown, "{detection:?}");
        assert_eq!(detection.signals, vec!["fenced-code", "atx-heading"]);
    }

    #[test]
    fn heading_needs_block_context() {
        // 緊貼程式碼的 `#` 註解不算標題。
        let detection = looks_like_markdown("# Setup\nfoo --init\n# Run\nfoo\n");
        assert!(detection.signals.is_empty(), "{detection:?}");
        let detection = looks_like_markdown("# Setup\n\nfoo --init\n");
        assert_eq!(detection.signals, vec!["atx-heading"]);
        assert!(!detection.is_markdown);
    }

    #[test]
    fn link_requires_a_url_like_destination() {
        assert!(!scan_inline("handlers[name](event)").link);
        assert!(!scan_inline("see [x](y)").link);
        assert!(scan_inline("see [x](./y.md)").link);
        assert!(scan_inline("![logo](logo.png)").link);
        assert!(scan_inline("[a](<https://x.y/a b>)").link);
        assert!(scan_inline("[a](https://x.y \"title\")").link);
    }

    #[test]
    fn bold_pairing_ignores_python_operators() {
        assert!(!scan_inline("f(**a, **b)").emphasis);
        assert!(!scan_inline("x = 2**10 + 3**2").emphasis);
        assert!(scan_inline("a **bold** b").emphasis);
        assert!(scan_inline("**標籤：**內容").cjk_emphasis_error);
        assert!(!scan_inline("**標籤**：內容").cjk_emphasis_error);
        assert!(!scan_inline("**標籤：** 內容").cjk_emphasis_error);
        assert!(!scan_inline("`**標籤：**內容`").cjk_emphasis_error);
        // 收尾 `**` 後面接標點時是合法的 right-flanking，不是筆誤。
        assert!(!scan_inline("是**「重點」**，所以").cjk_emphasis_error);
    }

    #[test]
    fn table_requires_matching_delimiter_row() {
        assert!(looks_like_markdown("a | b\n--- | ---\n1 | 2\n").is_markdown);
        assert!(!looks_like_markdown("a | b | c\n--- | ---\n").is_markdown);
        assert!(!looks_like_markdown("Title\n---\ntext\n").is_markdown);
    }

    // -----------------------------------------------------------------------
    // 邊界與效能
    // -----------------------------------------------------------------------

    #[test]
    fn ignores_lines_after_the_cap() {
        let mut text = "plain words here\n".repeat(MAX_LINES);
        text.push_str("| a | b |\n| - | - |\n");
        assert!(looks_like_markdown(&text).signals.is_empty());
    }

    #[test]
    fn huge_inputs_finish_quickly() {
        let started = std::time::Instant::now();
        let long_line = "[a](".repeat(500_000);
        let _ = looks_like_markdown(&long_line);
        let many_stars = "**x".repeat(500_000);
        let _ = looks_like_markdown(&many_stars);
        let many_fences = "````\n```\n".repeat(50_000);
        let _ = looks_like_markdown(&many_fences);
        let big_markdown = "# T\n\n- a\n- b\n\n".repeat(200_000);
        assert!(looks_like_markdown(&big_markdown).is_markdown);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn odd_inputs_do_not_panic() {
        let inputs = [
            "\u{FEFF}",
            "\r",
            "\r\n\r\n",
            "#",
            "######",
            "####### x",
            "[",
            "]:",
            "[]:",
            "[^]: x",
            "`",
            "``` ```",
            "**",
            "***",
            "|",
            "|-|",
            "- [",
            "1.",
            "999999999999. x",
            "> ",
            "---",
            "---\n---",
            "---\na: 1\n",
            "<",
            "{",
            "2026-1",
            "[2026-11-05",
            "Nov",
            "at :",
            "index abc",
            "@@ -",
            "é**é**é",
            "\u{3000}**全形空白**\u{3000}",
            "👍🏽 **emoji** [👍](👍.png)",
            "\u{0}\u{1}\u{7f}",
        ];
        for input in inputs {
            let _ = looks_like_markdown(input);
            let _ = looks_like_markdown(&format!("{input}\n{input}\n"));
        }
    }
}
