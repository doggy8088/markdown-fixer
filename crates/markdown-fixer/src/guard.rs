//! 渲染保護（render guard，PRD〈6.9〉第 2 點）。Rust 專有，需要 `render-guard` feature。
//!
//! 先跑一次與 oracle 相同的 [`fix::fix_document`]，再用 `pulldown-cmark` 渲染修正前後的
//! 文件比對：
//!
//! 1. **便宜路徑**：`render(全部修正) == render(原文)` 時直接採用，輸出與 oracle 逐位元組相同。
//! 2. **修復類檢查**：只套用修復類規則（MDX008、010、030、040、042、062）得到 `rep`，
//!    以行 diff 找出修改區塊（hunk）。被修改的行所在的頂層區塊、加上前後各一個相鄰區塊
//!    可以改變；其他頂層區塊的 HTML 與（對應後的）行範圍必須完全相同。違反時找出最近的
//!    hunk 還原，最多 [`MAX_REPAIR_ROUNDS`] 輪，仍不行就還原全部修復。
//! 3. **版面類檢查**：`render(全部修正)` 必須等於 `render(只套用修復類)`。不相等時，對每條
//!    實際觸發的版面類規則 L 個別檢查 `render(修復類 + L)`（add-one），不相等就不套用 L；
//!    最後再驗證一次合併結果，仍不相等就退回只套用修復類的結果。
//!
//! 每一層都以「驗證，失敗就退一步」收尾：全部修正 → 去掉肇事規則 → 只套用（部分）修復 →
//! 原文。原文永遠安全，所以結果一定不會有未預期的渲染改變。工作量有上限：每條版面類規則
//! 最多多跑一次完整修正，修復類最多 [`MAX_REPAIR_ROUNDS`] 輪渲染。
//!
//! 渲染設定見 [`parser_options`]。數學式（`$…$`、`$$`）開啟：fixer 本來就把數學式當保護
//! 區域，不開的話數學式裡的 `*`、`_` 會被當成強調，造成與修正無關的誤報。

use std::ops::Range;

use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, html};

use crate::fix;
use crate::rules::Enabled;
use crate::{FixResult, Options, Problem, SkippedFix};

/// 版面類規則（PRD〈6.9〉）：本身不應改變渲染。
pub(crate) const LAYOUT_RULES: &[&str] = &[
    "MDX001", "MDX002", "MDX003", "MDX004", "MDX005", "MDX006", "MDX011", "MDX012", "MDX013",
    "MDX020", "MDX052", "MDX060", "MDX082",
];

/// 修復類規則（PRD〈6.9〉）：目的就是讓原本壞掉的語法正確渲染。
pub(crate) const REPAIR_RULES: &[&str] =
    &["MDX008", "MDX010", "MDX030", "MDX040", "MDX042", "MDX062"];

/// 只改行內語法的修復類規則：修正後區塊結構必須不變。
const INLINE_REPAIR_RULES: &[&str] = &["MDX008", "MDX040", "MDX042"];

/// 編碼類規則：渲染前一律先正規化 BOM 與行尾，所以它們不可能改變渲染，也永遠不放進變體的
/// `ignore_rules`（停用 MDX002 會讓 `fix_document` 還原 CRLF，變體的輸出就無法互相比較）。
const ENCODING_RULES: &[&str] = &["MDX001", "MDX002"];

/// 修復類檢查最多還原幾輪；超過就還原全部修復。
const MAX_REPAIR_ROUNDS: usize = 8;

/// Myers diff 的編輯距離上限；超過時改用貪婪重新對齊。
const MYERS_MAX_D: usize = 256;

/// 貪婪重新對齊時，往前找共同行的視窗大小。
const RESYNC_WINDOW: usize = 64;

const REASON_LAYOUT: &str = "fixes for this layout rule would change how the document renders, so none were applied in this file";
const REASON_REPAIR: &str =
    "this repair would change the rendering of blocks other than the one it fixes";
const REASON_PROTECTED: &str =
    "this repair would edit a line that renders as code, raw HTML, or frontmatter";
const REASON_STRUCTURE: &str = "this inline repair would change the block structure";
const REASON_COMBINED: &str = "layout fixes in this file would change how the document renders together, so none were applied";

/// 渲染用的 pulldown-cmark 選項：接近 GitHub Flavored Markdown。
///
/// - 表格、刪除線、任務清單、註腳：GFM 擴充。
/// - 不開 YAML metadata block：pulldown-cmark 在文件中間也接受 `---` 包起來的區塊，
///   GitHub 只認檔首的 frontmatter。檔首 frontmatter 由 [`mask_frontmatter`] 處理。
/// - `ENABLE_GFM`：GFM blockquote 標籤（`> [!NOTE]`）。
/// - 數學式：fixer 把 `$…$` 與 `$$` 區塊當保護區域，GitHub 也會渲染數學式。
/// - 不開 smart punctuation、heading attributes 等非 GFM 擴充。
pub(crate) fn parser_options() -> CmarkOptions {
    CmarkOptions::ENABLE_TABLES
        | CmarkOptions::ENABLE_STRIKETHROUGH
        | CmarkOptions::ENABLE_TASKLISTS
        | CmarkOptions::ENABLE_FOOTNOTES
        | CmarkOptions::ENABLE_GFM
        | CmarkOptions::ENABLE_MATH
}

/// 渲染前的正規化：去掉檔首 BOM，CRLF 與 CR 一律改成 LF（與 `normalize_for_analysis` 相同）。
pub(crate) fn normalize(text: &str) -> String {
    let text = text.trim_start_matches('\u{FEFF}');
    if !text.contains('\r') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            out.push('\n');
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 檔首 frontmatter（第一行是 `---`，到下一個 `---` 或 `...` 行為止）改寫成同樣行數的
/// fenced code，讓它的內容逐字比對、行號不變，也讓 [`line_paths`] 把它當成保護區域。
fn mask_frontmatter(text: &str) -> std::borrow::Cow<'_, str> {
    let mut lines = text.split('\n');
    if lines.next().map(|l| l.trim_end_matches([' ', '\t'])) != Some("---") {
        return text.into();
    }
    let Some(close) = lines.position(|l| matches!(l.trim_end_matches([' ', '\t']), "---" | "..."))
    else {
        return text.into();
    };
    let close = close + 1;
    let longest = text
        .split('\n')
        .take(close)
        .map(|l| l.split(|c| c != '`').map(str::len).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    let mut out = String::with_capacity(text.len() + 2 * fence.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if i == 0 || i == close {
            out.push_str(&fence);
        } else {
            out.push_str(line);
        }
    }
    out.into()
}

/// 把（已正規化的）文件渲染成 HTML，比對前先做「看得見的差異」正規化，見 [`visible_events`]。
pub(crate) fn render(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    html::push_html(
        &mut out,
        visible_events(text).into_iter().map(|(event, _)| event),
    );
    out
}

/// 瀏覽器會摺疊的空白。NBSP 等其他空白不摺疊。
fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000C}')
}

/// 把連續的 HTML 空白摺成一個空格。
fn collapse_space(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        if is_html_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// 每行去掉行尾空白，再去掉結尾的空行。
fn trim_line_ends(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end_matches(is_html_space));
    }
    let len = out.trim_end_matches('\n').len();
    out.truncate(len);
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Text,
    Code,
    Html,
    InlineHtml,
}

/// 解析成事件，並去掉看不見的差異，讓比對只反映讀者看得到的改變：
///
/// - 一般內文：相鄰的文字與軟換行合併，連續空白摺成一個空格（瀏覽器本來就會摺疊）。
/// - 程式碼區塊：內容保留，只去掉每行行尾空白與結尾空行（`<pre>` 裡看不見）。
/// - raw HTML 區塊：整個區塊合成一段（含 tab 展開產生的 `Text`），只去掉每行行尾空白與
///   區塊結尾的空行；區塊內部的空行數照原樣比對。
///
/// 硬換行（`<br />`）、NBSP、程式碼內容與所有標籤都照原樣比對。
fn visible_events(text: &str) -> Vec<(Event<'static>, Range<usize>)> {
    let text = mask_frontmatter(text);
    let mut out: Vec<(Event<'static>, Range<usize>)> = Vec::new();
    let mut buffer = String::new();
    let mut buffer_range = 0..0;
    let mut pending: Option<Pending> = None;
    let mut code_depth = 0usize;
    let mut html_depth = 0usize;
    let flush = |out: &mut Vec<(Event<'static>, Range<usize>)>,
                 buffer: &mut String,
                 pending: &mut Option<Pending>,
                 range: &Range<usize>| {
        let Some(kind) = pending.take() else { return };
        let text = std::mem::take(buffer);
        let event = match kind {
            Pending::Text => Event::Text(collapse_space(&text).into()),
            Pending::Code => {
                let mut code = trim_line_ends(&text);
                if !code.is_empty() {
                    code.push('\n');
                }
                Event::Text(code.into())
            }
            Pending::Html => Event::Html(trim_line_ends(&text).into()),
            Pending::InlineHtml => Event::InlineHtml(trim_line_ends(&text).into()),
        };
        out.push((event, range.clone()));
    };
    for (event, range) in Parser::new_ext(&text, parser_options()).into_offset_iter() {
        let (kind, piece): (Pending, &str) = match &event {
            // raw HTML 區塊裡，pulldown-cmark 會把 tab 展開剩下的欄位另外送成 `Text`。
            // 整個區塊合成一段逐字比對，不能拆段，否則各段的「結尾空行」會被當成看不見的差異去掉。
            Event::Text(t) | Event::Html(t) if html_depth > 0 => (Pending::Html, t),
            Event::Text(t) if code_depth > 0 => (Pending::Code, t),
            Event::Text(t) => (Pending::Text, t),
            Event::SoftBreak if code_depth == 0 => (Pending::Text, " "),
            Event::Html(t) => (Pending::Html, t),
            Event::InlineHtml(t) => (Pending::InlineHtml, t),
            _ => {
                flush(&mut out, &mut buffer, &mut pending, &buffer_range);
                match &event {
                    Event::Start(pulldown_cmark::Tag::CodeBlock(_)) => code_depth += 1,
                    Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                        code_depth = code_depth.saturating_sub(1)
                    }
                    Event::Start(pulldown_cmark::Tag::HtmlBlock) => html_depth += 1,
                    Event::End(pulldown_cmark::TagEnd::HtmlBlock) => {
                        html_depth = html_depth.saturating_sub(1)
                    }
                    _ => {}
                }
                out.push((event.into_static(), range));
                continue;
            }
        };
        if pending != Some(kind) {
            flush(&mut out, &mut buffer, &mut pending, &buffer_range);
            pending = Some(kind);
            buffer_range = range;
        }
        buffer.push_str(piece);
    }
    flush(&mut out, &mut buffer, &mut pending, &buffer_range);
    out
}

/// 一個頂層區塊：行範圍（從 0 起算，不含 `end`）與它單獨渲染的 HTML。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) lines: Range<usize>,
    pub(crate) html: String,
}

/// 每一行開頭的位元組位置（第 0 行是 0）。
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        text.bytes()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| i + 1),
    );
    starts
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&s| s <= offset).saturating_sub(1)
}

/// 取出（已正規化的）文件的頂層區塊，依出現順序排列。
pub(crate) fn top_level_blocks(text: &str) -> Vec<Block> {
    // 位元組位置以改寫過 frontmatter 的文字為準（行數不變）。
    let text = mask_frontmatter(text);
    let starts = line_starts(&text);
    let events = visible_events(&text);
    let mut blocks = Vec::new();
    let mut depth = 0usize;
    let mut begin = 0usize;
    for (i, (event, _)) in events.iter().enumerate() {
        let span = match event {
            Event::Start(_) => {
                if depth == 0 {
                    begin = i;
                }
                depth += 1;
                None
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                (depth == 0).then_some(begin..i + 1)
            }
            _ if depth == 0 => Some(i..i + 1),
            _ => None,
        };
        let Some(span) = span else { continue };
        let range = events[span.start].1.clone();
        let first = line_of(&starts, range.start);
        let last = line_of(&starts, range.end.max(range.start + 1) - 1);
        let mut html_out = String::new();
        html::push_html(
            &mut html_out,
            events[span].iter().map(|(event, _)| event.clone()),
        );
        blocks.push(Block {
            lines: first..last.max(first) + 1,
            html: html_out,
        });
    }
    blocks
}

/// 行 diff 的一個修改區塊：原文的 `a` 行換成新文字的 `b` 行（都從 0 起算）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hunk {
    pub(crate) a: Range<usize>,
    pub(crate) b: Range<usize>,
}

/// 行 diff。先去掉共同的開頭與結尾，中段用編輯距離有上限的 Myers；超過上限改用貪婪重新
/// 對齊。任何共同子序列都是合法的對齊，品質只影響 hunk 的大小（hunk 越大檢查越寬鬆）。
/// 貪婪對齊在 [`RESYNC_WINDOW`] 內找不到共同行時回傳 `None`，呼叫端要走安全的那一邊。
pub(crate) fn diff_lines(a: &[&str], b: &[&str]) -> Option<Vec<Hunk>> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let a_mid = &a[prefix..a.len() - suffix];
    let b_mid = &b[prefix..b.len() - suffix];
    let hunks = match myers(a_mid, b_mid, MYERS_MAX_D) {
        Some(hunks) => hunks,
        None => greedy(a_mid, b_mid)?,
    };
    Some(
        hunks
            .into_iter()
            .map(|h| Hunk {
                a: h.a.start + prefix..h.a.end + prefix,
                b: h.b.start + prefix..h.b.end + prefix,
            })
            .collect(),
    )
}

/// 由依序排列、互不重疊的相同行配對 `(i, j)` 產生 hunk。
fn hunks_from_matches(n: usize, m: usize, matches: &[(usize, usize)]) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    for &(x, y) in matches.iter().chain(std::iter::once(&(n, m))) {
        if x > i || y > j {
            hunks.push(Hunk { a: i..x, b: j..y });
        }
        i = x + 1;
        j = y + 1;
    }
    hunks
}

/// Myers O((N+M)D) diff；編輯距離超過 `max_d` 時回傳 `None`。
fn myers(a: &[&str], b: &[&str], max_d: usize) -> Option<Vec<Hunk>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    if n == 0 && m == 0 {
        return Some(Vec::new());
    }
    let max_d = max_d.min(a.len() + b.len()) as isize;
    let offset = max_d + 1;
    let idx = |k: isize| (k + offset) as usize;
    let mut v = vec![0isize; (2 * max_d + 3) as usize];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    for d in 0..=max_d {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d || (k != d && v[idx(k - 1)] < v[idx(k + 1)]) {
                v[idx(k + 1)]
            } else {
                v[idx(k - 1)] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[idx(k)] = x;
            if x >= n && y >= m {
                return Some(myers_backtrack(&trace, n, m, idx));
            }
            k += 2;
        }
    }
    None
}

fn myers_backtrack(
    trace: &[Vec<isize>],
    n: isize,
    m: isize,
    idx: impl Fn(isize) -> usize,
) -> Vec<Hunk> {
    let (mut x, mut y) = (n, m);
    let mut matches = Vec::new();
    for (d, v) in trace.iter().enumerate().rev() {
        let d = d as isize;
        let k = x - y;
        let prev_k = if k == -d || (k != d && v[idx(k - 1)] < v[idx(k + 1)]) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[idx(prev_k)];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y && x > 0 && y > 0 {
            x -= 1;
            y -= 1;
            matches.push((x as usize, y as usize));
        }
        if d > 0 {
            x = prev_x;
            y = prev_y;
        }
    }
    matches.reverse();
    hunks_from_matches(n as usize, m as usize, &matches)
}

/// 貪婪重新對齊：遇到不同的行，在視窗內找 `x + y` 最小、而且下一行也相同（或兩邊同時
/// 結束）的共同行。
fn greedy(a: &[&str], b: &[&str]) -> Option<Vec<Hunk>> {
    let (n, m) = (a.len(), b.len());
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    let confirmed = |x: usize, y: usize| (x >= n && y >= m) || (x < n && y < m && a[x] == b[y]);
    while i < n && j < m {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        let mut found = None;
        let to_end = (n - i) + (m - j);
        'search: for k in 1..=2 * RESYNC_WINDOW {
            if k == to_end {
                // 剩下的行全部不同：最後一個 hunk 直接延伸到結尾。
                found = Some((n - i, m - j));
                break;
            }
            for x in 0..=k {
                let y = k - x;
                if i + x < n && j + y < m && a[i + x] == b[j + y] && confirmed(i + x + 1, j + y + 1)
                {
                    found = Some((x, y));
                    break 'search;
                }
            }
            if k == 2 && i + 1 < n && j + 1 < m && a[i + 1] == b[j + 1] {
                // 沒有更好的對齊時，把它當成單行替換（修正多半是逐行改寫，例如每隔一行就改一行）。
                found = Some((1, 1));
                break;
            }
        }
        let (x, y) = found?;
        hunks.push(Hunk {
            a: i..i + x,
            b: j..j + y,
        });
        i += x;
        j += y;
    }
    if i < n || j < m {
        hunks.push(Hunk { a: i..n, b: j..m });
    }
    Some(hunks)
}

/// 原文中沒被 hunk 改到的第 `line` 行，在新文字中的行號。
fn map_line(hunks: &[Hunk], line: usize) -> usize {
    let before = hunks.partition_point(|h| h.a.end <= line);
    match before.checked_sub(1).and_then(|i| hunks.get(i)) {
        Some(h) => (line + h.b.end).saturating_sub(h.a.end),
        None => line,
    }
}

/// 修復類的區域性檢查：回傳違規的原文區塊索引。
///
/// 被 hunk 改到的區塊（hunk 落在區塊之間時，取前後兩個區塊）與它們前後各一個相鄰區塊
/// 可以改變；其他區塊在新文字中必須有行範圍（經 diff 對應後）與 HTML 都相同的區塊。
pub(crate) fn locality_violations(
    old_blocks: &[Block],
    new_blocks: &[Block],
    hunks: &[Hunk],
) -> Vec<usize> {
    let count = old_blocks.len();
    let mut allowed = vec![false; count];
    let touch = |i: usize, allowed: &mut Vec<bool>| {
        for flag in &mut allowed[i.saturating_sub(1)..(i + 2).min(count)] {
            *flag = true;
        }
    };
    for hunk in hunks {
        let first = old_blocks.partition_point(|b| b.lines.end <= hunk.a.start);
        let mut any = false;
        let mut i = first;
        while i < count && old_blocks[i].lines.start < hunk.a.end {
            touch(i, &mut allowed);
            any = true;
            i += 1;
        }
        if !any {
            if first > 0 {
                touch(first - 1, &mut allowed);
            }
            if first < count {
                touch(first, &mut allowed);
            }
        }
    }
    let mut violations = Vec::new();
    for (i, block) in old_blocks.iter().enumerate() {
        if allowed[i] {
            continue;
        }
        let start = map_line(hunks, block.lines.start);
        let end = map_line(hunks, block.lines.end - 1) + 1;
        let found = new_blocks
            .binary_search_by(|b| b.lines.start.cmp(&start))
            .ok()
            .map(|j| &new_blocks[j]);
        let same = found.is_some_and(|b| b.lines == (start..end) && b.html == block.html);
        if !same {
            violations.push(i);
        }
    }
    violations
}

/// 把 `kept` 裡的 hunk 套用到原文行上，回傳新文字與重新計算 `b` 座標後的 hunk。
fn splice(old: &[&str], new: &[&str], kept: &[Hunk]) -> (String, Vec<Hunk>) {
    let mut lines: Vec<&str> = Vec::with_capacity(old.len());
    let mut adjusted = Vec::with_capacity(kept.len());
    let mut pos = 0;
    for hunk in kept {
        lines.extend_from_slice(&old[pos..hunk.a.start]);
        let b_start = lines.len();
        lines.extend_from_slice(&new[hunk.b.clone()]);
        adjusted.push(Hunk {
            a: hunk.a.clone(),
            b: b_start..lines.len(),
        });
        pos = hunk.a.end;
    }
    lines.extend_from_slice(&old[pos..]);
    (lines.join("\n"), adjusted)
}

/// 區塊層級的標籤代碼；行內標籤（強調、連結等）回傳 `None`。
fn block_code(tag: &Tag<'_>) -> Option<u8> {
    Some(match tag {
        Tag::Paragraph => 1,
        Tag::Heading { level, .. } => 10 + *level as u8,
        Tag::BlockQuote(_) => 2,
        Tag::CodeBlock(_) => CODE_BLOCK,
        Tag::HtmlBlock => HTML_BLOCK,
        Tag::List(None) => 3,
        Tag::List(Some(_)) => 4,
        Tag::Item => 5,
        Tag::FootnoteDefinition(_) => 6,
        Tag::Table(_) => 7,
        Tag::TableHead => 8,
        Tag::TableRow => 9,
        Tag::TableCell => 20,
        Tag::MetadataBlock(_) => METADATA_BLOCK,
        Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => 21,
        _ => return None,
    })
}

const CODE_BLOCK: u8 = 30;
const HTML_BLOCK: u8 = 31;
const METADATA_BLOCK: u8 = 32;

/// 每一行所在的區塊路徑（由外到內的區塊標籤代碼）。
fn line_paths(text: &str) -> Vec<Vec<u8>> {
    let text = mask_frontmatter(text);
    let text = text.as_ref();
    let starts = line_starts(text);
    let mut paths = vec![Vec::new(); starts.len()];
    for (event, range) in Parser::new_ext(text, parser_options()).into_offset_iter() {
        let Event::Start(tag) = &event else { continue };
        let Some(code) = block_code(tag) else {
            continue;
        };
        let first = line_of(&starts, range.start);
        let last = line_of(&starts, range.end.max(range.start + 1) - 1);
        for path in paths.iter_mut().take(last + 1).skip(first) {
            path.push(code);
        }
    }
    paths
}

/// 這一行渲染成程式碼、raw HTML 或 frontmatter（fixer 的保護區域）。
fn is_protected(path: &[u8]) -> bool {
    path.iter()
        .any(|c| matches!(*c, CODE_BLOCK | HTML_BLOCK | METADATA_BLOCK))
}

/// 修復類檢查的結果（文字已正規化）。
struct RepairOutcome {
    text: String,
    /// 被還原的 hunk（原文座標）與原因。
    reverted: Vec<(Hunk, &'static str)>,
}

/// 原文第 `hunk.a` 行（從 1 起算的範圍）上的修復類問題；`problems` 必須依行號排序。
fn problems_in<'a>(problems: &[&'a Problem], hunk: &Hunk) -> Vec<&'a Problem> {
    let (lo, hi) = (hunk.a.start + 1, hunk.a.end.max(hunk.a.start + 1));
    let first = problems.partition_point(|p| p.line < lo);
    problems[first..]
        .iter()
        .copied()
        .take_while(|p| p.line <= hi)
        .collect()
}

/// 逐一檢查 hunk 本身：不得改到渲染成程式碼、raw HTML 或 frontmatter 的行；只有行內修復
/// （MDX008、040、042）的 hunk 不得改變區塊結構。回傳每個 hunk 的違規原因。
///
/// 區塊結構只在「只套用行內修復 hunk」的文字上比對，避免把其他修復（例如 MDX030 讓上一行
/// 變成清單）造成的連帶改變算到行內修復頭上。
fn hunk_violations(
    hunks: &[Hunk],
    old: &[&str],
    new: &[&str],
    old_paths: &[Vec<u8>],
    problems: &[&Problem],
) -> Vec<Option<&'static str>> {
    let protected = |line: usize| old_paths.get(line).is_some_and(|p| is_protected(p));
    let mut result: Vec<Option<&'static str>> = hunks
        .iter()
        .map(|hunk| {
            let touches_code = if hunk.a.is_empty() {
                hunk.a.start > 0 && protected(hunk.a.start - 1) && protected(hunk.a.start)
            } else {
                hunk.a.clone().any(protected)
            };
            touches_code.then_some(REASON_PROTECTED)
        })
        .collect();
    let inline: Vec<usize> = (0..hunks.len())
        .filter(|&i| {
            let hunk = &hunks[i];
            let rules = problems_in(problems, hunk);
            result[i].is_none()
                && hunk.a.len() == hunk.b.len()
                && !rules.is_empty()
                && rules.iter().all(|p| INLINE_REPAIR_RULES.contains(&p.rule))
        })
        .collect();
    if inline.is_empty() {
        return result;
    }
    let selected: Vec<Hunk> = inline.iter().map(|&i| hunks[i].clone()).collect();
    let (text, adjusted) = splice(old, new, &selected);
    let new_paths = line_paths(&text);
    for (&i, hunk) in inline.iter().zip(&adjusted) {
        let changed = hunks[i]
            .a
            .clone()
            .zip(hunk.b.clone())
            .any(|(a, b)| old_paths.get(a) != new_paths.get(b));
        if changed {
            result[i] = Some(REASON_STRUCTURE);
        }
    }
    result
}

/// 修復類檢查：還原改到保護區域、破壞區塊結構，或改變其他區塊渲染的 hunk。
fn repair_stage(original: &str, repaired: &str, problems: &[&Problem]) -> RepairOutcome {
    if original == repaired {
        return RepairOutcome {
            text: repaired.to_string(),
            reverted: Vec::new(),
        };
    }
    let old: Vec<&str> = original.split('\n').collect();
    let new: Vec<&str> = repaired.split('\n').collect();
    let revert_all = |hunks: Vec<Hunk>| RepairOutcome {
        text: original.to_string(),
        reverted: hunks.into_iter().map(|h| (h, REASON_REPAIR)).collect(),
    };
    let Some(hunks) = diff_lines(&old, &new) else {
        return revert_all(vec![Hunk {
            a: 0..old.len(),
            b: 0..new.len(),
        }]);
    };
    let old_paths = line_paths(original);
    let mut kept = Vec::with_capacity(hunks.len());
    let mut reverted: Vec<(Hunk, &'static str)> = Vec::new();
    let checks = hunk_violations(&hunks, &old, &new, &old_paths, problems);
    for (hunk, violation) in hunks.iter().cloned().zip(checks) {
        match violation {
            Some(reason) => reverted.push((hunk, reason)),
            None => kept.push(hunk),
        }
    }
    let old_blocks = top_level_blocks(original);
    for _ in 0..MAX_REPAIR_ROUNDS {
        if kept.is_empty() {
            break;
        }
        let (text, adjusted) = splice(&old, &new, &kept);
        let new_blocks = top_level_blocks(&text);
        let violations = locality_violations(&old_blocks, &new_blocks, &adjusted);
        if violations.is_empty() {
            reverted.sort_by_key(|(h, _)| h.a.start);
            return RepairOutcome { text, reverted };
        }
        let mut blamed = vec![false; kept.len()];
        for &v in &violations {
            let start = old_blocks[v].lines.start;
            let before = kept.partition_point(|h| h.a.end <= start);
            blamed[before.saturating_sub(1)] = true;
        }
        let mut next = Vec::with_capacity(kept.len());
        for (hunk, blame) in kept.into_iter().zip(blamed) {
            if blame {
                reverted.push((hunk, REASON_REPAIR));
            } else {
                next.push(hunk);
            }
        }
        kept = next;
    }
    if kept.is_empty() {
        reverted.sort_by_key(|(h, _)| h.a.start);
        return RepairOutcome {
            text: original.to_string(),
            reverted,
        };
    }
    revert_all(hunks)
}

/// 在 `options` 之外再停用 `extra` 這些規則（編碼類規則除外）。
fn without(options: &Options, extra: &[&str]) -> Options {
    let mut ignore = options.ignore_rules.clone().unwrap_or_default();
    ignore.extend(
        extra
            .iter()
            .filter(|id| !ENCODING_RULES.contains(id))
            .map(|id| id.to_string()),
    );
    Options {
        ignore_rules: Some(ignore),
        ..options.clone()
    }
}

/// 跑一次與 oracle 相同的修正；拒絕修正時回傳 `None`（變體不可用）。
fn variant(text: &str, options: &Options, extra_ignore: &[&str]) -> Option<String> {
    let result = fix::fix_document(text, &without(options, extra_ignore));
    (!result.refused).then_some(result.text)
}

/// 把正規化文字還原成原文的 BOM 與行尾，讓它能再交給 `fix_document`（編碼規則照常處理）。
fn denormalize(normalized: &str, original: &str) -> String {
    let bom_len = original.len() - original.trim_start_matches('\u{FEFF}').len();
    let eol = if original.contains("\r\n") {
        "\r\n"
    } else if original.contains('\r') {
        "\r"
    } else {
        "\n"
    };
    let mut out = String::with_capacity(normalized.len() + bom_len + normalized.len() / 16);
    out.push_str(&original[..bom_len]);
    if eol == "\n" {
        out.push_str(normalized);
    } else {
        out.push_str(&normalized.replace('\n', eol));
    }
    out
}

/// `rules` 裡可修正的問題，依行號排序（[`problems_in`] 以二分搜尋查詢）。
fn fixable_problems_of<'a>(problems: &'a [Problem], rules: &[&str]) -> Vec<&'a Problem> {
    let mut found: Vec<&Problem> = problems
        .iter()
        .filter(|p| p.fixable && rules.contains(&p.rule))
        .collect();
    found.sort_by_key(|p| p.line);
    found
}

/// 版面類規則中，在原文或只套用修復類的文字裡實際觸發的規則（依規則目錄順序）。
fn fired_layout_rules(problems: &[&[Problem]]) -> Vec<&'static str> {
    LAYOUT_RULES
        .iter()
        .copied()
        .filter(|id| !ENCODING_RULES.contains(id))
        .filter(|id| {
            problems
                .iter()
                .any(|list| list.iter().any(|p| p.fixable && p.rule == *id))
        })
        .collect()
}

/// 規則在原文（優先）或修復後文字中可修正問題的行號；都沒有時是第 1 行。
fn lines_of(problems: &[&[Problem]], rule: &str) -> Vec<usize> {
    for list in problems {
        let mut lines: Vec<usize> = list
            .iter()
            .filter(|p| p.fixable && p.rule == rule)
            .map(|p| p.line)
            .collect();
        if !lines.is_empty() {
            lines.dedup();
            return lines;
        }
    }
    vec![1]
}

/// 原文中「全部修正」與參考渲染不同的區域（行範圍，從 0 起算，不含結尾），往外擴張
/// [`EXCERPT_CONTEXT`] 行並對齊到空行。區域超過文件一半時回傳 `None`（直接在整份文件上找）。
fn culprit_window(base_n: &str, reference_n: &str, full_n: &str) -> Option<Range<usize>> {
    let reference_blocks = top_level_blocks(reference_n);
    let full_blocks = top_level_blocks(full_n);
    let a: Vec<&str> = reference_blocks.iter().map(|b| b.html.as_str()).collect();
    let b: Vec<&str> = full_blocks.iter().map(|b| b.html.as_str()).collect();
    let block_hunks = diff_lines(&a, &b)?;
    // 不同的區塊在全部修正文字中的行範圍。
    let mut lo = usize::MAX;
    let mut hi = 0;
    for hunk in &block_hunks {
        let first = hunk.b.start.min(full_blocks.len().saturating_sub(1));
        let last = hunk.b.end.max(hunk.b.start + 1).min(full_blocks.len());
        for block in full_blocks.get(first.saturating_sub(1)..(last + 1).min(full_blocks.len()))? {
            lo = lo.min(block.lines.start);
            hi = hi.max(block.lines.end);
        }
    }
    if lo >= hi {
        return None;
    }
    // 對應回原文行號。
    let old: Vec<&str> = base_n.split('\n').collect();
    let new: Vec<&str> = full_n.split('\n').collect();
    let line_hunks = diff_lines(&old, &new)?;
    let back = |line: usize, end: bool| {
        let i = line_hunks.partition_point(|h| h.b.end <= line);
        match line_hunks.get(i) {
            Some(h) if h.b.start <= line => {
                if end {
                    h.a.end
                } else {
                    h.a.start
                }
            }
            _ => match i.checked_sub(1).and_then(|j| line_hunks.get(j)) {
                Some(h) => (line + h.a.end).saturating_sub(h.b.end),
                None => line,
            },
        }
    };
    let blank = |i: usize| old.get(i).is_none_or(|l| l.trim().is_empty());
    let mut start = back(lo, false)
        .saturating_sub(EXCERPT_CONTEXT)
        .min(old.len());
    let mut end = back(hi, true)
        .saturating_add(EXCERPT_CONTEXT)
        .min(old.len());
    while start > 0 && !blank(start) {
        start -= 1;
    }
    while end < old.len() && !blank(end) {
        end += 1;
    }
    (start < end && end.saturating_sub(start).saturating_mul(2) < old.len()).then_some(start..end)
}

/// 找肇事規則時，摘錄區域前後多帶的行數。
const EXCERPT_CONTEXT: usize = 20;

/// 版面類檢查的結果。
struct LayoutOutcome {
    text: String,
    /// 被擋下的版面類規則。
    culprits: Vec<&'static str>,
    /// 個別檢查找不出肇事規則、退回只套用修復類的結果時為 `true`。
    combined: bool,
}

/// 在 [`culprit_window`] 的摘錄上找肇事的版面類規則（每條規則一次修正，但只修摘錄），
/// 再在整份文件上驗證一次。摘錄不可用或驗證失敗時回傳 `None`，呼叫端改在整份文件上找。
fn excerpt_culprits(
    base: &str,
    options: &Options,
    extra: &[&str],
    (reference, reference_n): (&str, &str),
    full: Option<&str>,
) -> Option<LayoutOutcome> {
    let base_n = normalize(base);
    let window = culprit_window(&base_n, reference_n, &normalize(full?))?;
    let lines: Vec<&str> = base_n.split('\n').collect();
    let mut excerpt = lines.get(window.clone())?.join("\n");
    excerpt.push('\n');
    let ignore_all_layout: Vec<&str> = extra.iter().chain(LAYOUT_RULES).copied().collect();
    let excerpt_reference = render(&normalize(&variant(&excerpt, options, &ignore_all_layout)?));
    let problems = crate::diagnose::diagnose(&excerpt, options);
    let fired = fired_layout_rules(&[&problems]);
    let culprits: Vec<&'static str> = fired
        .into_iter()
        .filter(|rule| {
            let ignore: Vec<&str> = ignore_all_layout
                .iter()
                .copied()
                .filter(|id| id != rule)
                .collect();
            !variant(&excerpt, options, &ignore)
                .is_some_and(|t| render(&normalize(&t)) == excerpt_reference)
        })
        .collect();
    if culprits.is_empty() {
        return None;
    }
    let ignore: Vec<&str> = extra.iter().chain(&culprits).copied().collect();
    let text = variant(base, options, &ignore)?;
    (render(&normalize(&text)) == reference).then_some(LayoutOutcome {
        text,
        culprits,
        combined: false,
    })
}

/// 版面類檢查。`base` 是要修正的文字（原文，或還原部分修復後的文字），`extra` 是額外停用的
/// 規則（還原過修復時是全部修復類規則），`reference` 是只套用修復類的渲染結果，
/// `reference_n` 是它的（正規化）文字。
/// `fired` 只在全部修正的渲染不等於 `reference` 時才呼叫（它要跑一次診斷）。
///
/// 回傳 `None` 表示連只套用編碼規則都不行，呼叫端要退回原文。
fn layout_stage(
    base: &str,
    options: &Options,
    extra: &[&str],
    (reference, reference_n): (&str, &str),
    fired: impl FnOnce() -> Vec<&'static str>,
    known_full: Option<String>,
) -> Option<LayoutOutcome> {
    let ignore_all_layout: Vec<&str> = extra.iter().chain(LAYOUT_RULES).copied().collect();
    let full = known_full.or_else(|| variant(base, options, extra));
    if let Some(full) = &full {
        if render(&normalize(full)) == reference {
            return Some(LayoutOutcome {
                text: full.clone(),
                culprits: Vec::new(),
                combined: false,
            });
        }
    }
    let full_for_window = full;
    if let Some(outcome) = excerpt_culprits(
        base,
        options,
        extra,
        (reference, reference_n),
        full_for_window.as_deref(),
    ) {
        return Some(outcome);
    }
    let fired = fired();
    let is_culprit = |rule: &str| {
        let ignore: Vec<&str> = ignore_all_layout
            .iter()
            .copied()
            .filter(|id| *id != rule)
            .collect();
        !variant(base, options, &ignore).is_some_and(|t| render(&normalize(&t)) == reference)
    };
    let mut culprits: Vec<&'static str> = fired.iter().copied().filter(|r| is_culprit(r)).collect();
    if culprits.is_empty() {
        // 診斷沒有指出肇事規則（例如多輪修正後才出現的問題）：檢查其餘啟用的版面類規則。
        let enabled = Enabled::new(options);
        culprits = LAYOUT_RULES
            .iter()
            .copied()
            .filter(|r| !ENCODING_RULES.contains(r) && !fired.contains(r) && enabled.is(r))
            .filter(|r| is_culprit(r))
            .collect();
    }
    if !culprits.is_empty() {
        let ignore: Vec<&str> = extra.iter().chain(&culprits).copied().collect();
        if let Some(text) = variant(base, options, &ignore) {
            if render(&normalize(&text)) == reference {
                return Some(LayoutOutcome {
                    text,
                    culprits,
                    combined: false,
                });
            }
        }
    }
    // 個別檢查都過、合在一起卻改變渲染：所有實際觸發的版面類規則都不套用。
    let text = variant(base, options, &ignore_all_layout)?;
    let mut all: Vec<&'static str> = fired.iter().chain(&culprits).copied().collect();
    all.sort_unstable();
    all.dedup();
    (render(&normalize(&text)) == reference).then_some(LayoutOutcome {
        text,
        culprits: all,
        combined: true,
    })
}

/// 有渲染保護的 `fix_document`。
pub(crate) fn fix_document(text: &str, options: &Options) -> FixResult {
    let full = fix::fix_document(text, options);
    if full.refused || !full.changed {
        return full;
    }
    let original_n = normalize(text);
    let original_render = render(&original_n);
    if render(&normalize(&full.text)) == original_render {
        return full;
    }
    guarded(text, options, &original_n, full.text)
}

/// 慢路徑：全部修正會改變渲染時，逐層找出可以安全套用的部分。
fn guarded(text: &str, options: &Options, original_n: &str, full: String) -> FixResult {
    let enabled = Enabled::new(options);
    let repair_enabled: Vec<&'static str> = REPAIR_RULES
        .iter()
        .copied()
        .filter(|id| enabled.is(id))
        .collect();
    let before = crate::diagnose::diagnose(text, options);
    let mut skipped: Vec<SkippedFix> = Vec::new();

    // 修復類檢查。
    let repaired = variant(text, options, LAYOUT_RULES).unwrap_or_else(|| text.to_string());
    let repaired_n = normalize(&repaired);
    let repair_problems = fixable_problems_of(&before, &repair_enabled);
    let outcome = repair_stage(original_n, &repaired_n, &repair_problems);
    for (hunk, reason) in &outcome.reverted {
        let mut inside = problems_in(&repair_problems, hunk);
        if inside.is_empty() {
            let lo = hunk.a.start + 1;
            inside.extend(
                repair_problems
                    .iter()
                    .copied()
                    .min_by_key(|p| p.line.abs_diff(lo)),
            );
        }
        for p in inside {
            skipped.push(SkippedFix {
                rule: p.rule,
                line: p.line,
                reason: reason.to_string(),
            });
        }
    }

    let (base, extra, reference_n): (String, Vec<&str>, &str) = if outcome.reverted.is_empty() {
        (text.to_string(), Vec::new(), &repaired_n)
    } else {
        (
            denormalize(&outcome.text, text),
            REPAIR_RULES.to_vec(),
            &outcome.text,
        )
    };
    let reference = render(reference_n);
    let after_text = if outcome.reverted.is_empty() {
        &repaired
    } else {
        &base
    };
    let after_repair = std::cell::OnceCell::new();
    let fired = || {
        let after = after_repair.get_or_init(|| crate::diagnose::diagnose(after_text, options));
        fired_layout_rules(&[&before, after])
    };
    let known_full = outcome.reverted.is_empty().then_some(full);
    let result = layout_stage(
        &base,
        options,
        &extra,
        (&reference, reference_n),
        fired,
        known_full,
    );
    let final_text = match result {
        Some(layout) => {
            let reason = if layout.combined {
                REASON_COMBINED
            } else {
                REASON_LAYOUT
            };
            let after: &[Problem] = after_repair.get().map_or(&[], Vec::as_slice);
            for rule in layout.culprits {
                for line in lines_of(&[&before, after], rule) {
                    skipped.push(SkippedFix {
                        rule,
                        line,
                        reason: reason.to_string(),
                    });
                }
            }
            layout.text
        }
        None => {
            // 連只套用編碼規則都失敗：退回原文，所有修正都不套用。
            skipped.clear();
            for p in before.iter().filter(|p| p.fixable) {
                skipped.push(SkippedFix {
                    rule: p.rule,
                    line: p.line,
                    reason: REASON_COMBINED.to_string(),
                });
            }
            text.to_string()
        }
    };
    skipped.sort_by(|a, b| (a.line, a.rule).cmp(&(b.line, b.rule)));
    skipped.dedup();
    FixResult {
        changed: final_text != text,
        text: final_text,
        refused: false,
        reason: String::new(),
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<&str> {
        s.split('\n').collect()
    }

    fn apply(a: &[&str], b: &[&str], hunks: &[Hunk]) -> Vec<String> {
        splice(a, b, hunks)
            .0
            .split('\n')
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn diff_reconstructs_target() {
        let cases = [
            ("a\nb\nc", "a\nb\nc"),
            ("a\nb\nc", "a\nx\nc"),
            ("a\nb\nc", "a\n\nb\nc"),
            ("a\n\nb\nc", "a\nb\nc"),
            ("", "x\ny"),
            ("x\ny", ""),
            ("t\n|a|\nb\n", "t\n|a|\n\nb\n"),
        ];
        for (a, b) in cases {
            let (la, lb) = (lines(a), lines(b));
            let hunks = diff_lines(&la, &lb).unwrap();
            assert_eq!(apply(&la, &lb, &hunks), lb, "{a:?} -> {b:?}");
        }
    }

    #[test]
    fn greedy_and_myers_agree_on_reconstruction() {
        let a: Vec<String> = (0..600).map(|i| format!("l{i}")).collect();
        let mut b = a.clone();
        for i in (0..600).step_by(3) {
            b[i] = format!("changed{i}");
        }
        b.insert(10, String::new());
        let la: Vec<&str> = a.iter().map(String::as_str).collect();
        let lb: Vec<&str> = b.iter().map(String::as_str).collect();
        assert!(myers(&la, &lb, MYERS_MAX_D).is_none());
        let hunks = diff_lines(&la, &lb).unwrap();
        assert_eq!(apply(&la, &lb, &hunks), lb);
    }

    #[test]
    fn blocks_have_line_ranges() {
        let blocks = top_level_blocks("# T\n\npara\nmore\n\n- a\n- b\n");
        let ranges: Vec<_> = blocks.iter().map(|b| b.lines.clone()).collect();
        assert_eq!(ranges, vec![0..1, 2..4, 5..7]);
        assert_eq!(blocks[0].html, "<h1>T</h1>\n");
    }

    #[test]
    fn frontmatter_is_masked_as_code_only_at_the_start() {
        assert_eq!(
            mask_frontmatter("---\na: 1\n---\nx\n"),
            "```\na: 1\n```\nx\n"
        );
        assert_eq!(
            mask_frontmatter("---\na: ```` \n...\n"),
            "`````\na: ```` \n`````\n"
        );
        assert_eq!(mask_frontmatter("x\n\n---\na\n---\n"), "x\n\n---\na\n---\n");
        assert_eq!(
            mask_frontmatter("---\nnever closed\n"),
            "---\nnever closed\n"
        );
        // 文件中間的 `---` 區塊照一般 Markdown 渲染（分隔線、Setext 標題）。
        assert!(render("x\n\n---\n* a\n---\n").contains("<hr />"));
        let blocks = top_level_blocks("---\na: 1\n---\n\n# T\n");
        assert_eq!(blocks[1].lines, 4..5);
    }

    #[test]
    fn blank_lines_inside_raw_html_are_visible() {
        // pulldown-cmark 在 HTML 區塊裡把 tab 展開剩下的欄位送成 `Text`；區塊內部的空行數
        // 仍要逐字比對（CI 上 property test 找到的案例）。
        assert_ne!(render("- <!-- >\n    \n\n\t#"), render("- <!-- >\n\n\t#"));
        assert_eq!(
            render("<div>\nx  \n</div>\n\n"),
            render("<div>\nx\n</div>\n")
        );
        let result = fix_document("- <!-- >\n    \n\n\t#", &Options::default());
        // MDX004 只清掉空白行上的空白（看不見），MDX005 合併空行會改變 HTML 內容，被擋下。
        assert_eq!(result.text, "- <!-- >\n\n\n\t#\n");
        let rules: Vec<&str> = result.skipped.iter().map(|s| s.rule).collect();
        assert_eq!(rules, ["MDX005"]);
    }

    #[test]
    fn normalize_strips_bom_and_cr() {
        assert_eq!(normalize("\u{FEFF}\u{FEFF}a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn denormalize_restores_encoding() {
        assert_eq!(
            denormalize("a\nb\n", "\u{FEFF}x\r\ny"),
            "\u{FEFF}a\r\nb\r\n"
        );
        assert_eq!(denormalize("a\nb", "x\ry"), "a\rb");
    }

    #[test]
    fn locality_allows_touched_and_adjacent() {
        let old = "#A\n\np1\n\np2\n\np3\n";
        let new = "# A\n\np1\n\np2\n\np3\n";
        let hunks = diff_lines(&lines(old), &lines(new)).unwrap();
        let v = locality_violations(&top_level_blocks(old), &top_level_blocks(new), &hunks);
        assert!(v.is_empty());
    }

    #[test]
    fn locality_flags_distant_change() {
        // 開一個沒關的 fence 會讓後面所有區塊都變成程式碼。
        let old = "p0\n\np1\n\np2\n\np3\n";
        let new = "```\np0\n\np1\n\np2\n\np3\n";
        let hunks = diff_lines(&lines(old), &lines(new)).unwrap();
        let v = locality_violations(&top_level_blocks(old), &top_level_blocks(new), &hunks);
        assert_eq!(v, vec![2, 3]);
    }

    /// 以行為單位縮小輸入，保持 `keep` 成立（ddmin 的簡化版：逐一刪行、再逐一刪區段）。
    fn minimize(text: &str, keep: &dyn Fn(&str) -> bool) -> String {
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let mut chunk = lines.len().max(1) / 2;
        while chunk >= 1 {
            let mut i = 0;
            while i < lines.len() {
                let mut trial = lines.clone();
                let end = (i + chunk).min(trial.len());
                trial.drain(i..end);
                if keep(&trial.join("\n")) {
                    lines = trial;
                } else {
                    i += chunk;
                }
            }
            chunk /= 2;
        }
        lines.join("\n")
    }

    /// 渲染稽核（PRD〈6.9〉第 3 點）。手動執行：
    ///
    /// ```sh
    /// MDFIX_AUDIT_LIST=files.txt MDFIX_AUDIT_OUT=out.md \
    ///   cargo test -p markdown-fixer --lib --release guard::tests::render_audit -- --ignored
    /// ```
    ///
    /// `files.txt` 每行一個路徑。對每份文件，以 `render_guard = false`（oracle 行為）修正，
    /// 渲染結果有變時，用 guard 分類：guard 擋下的是非預期的渲染改變（附縮小後的重現），
    /// 其餘是修復類規則的預期修復。
    #[test]
    #[ignore]
    fn render_audit() {
        use std::collections::BTreeMap;
        use std::fmt::Write as _;
        let list = std::env::var("MDFIX_AUDIT_LIST").expect("MDFIX_AUDIT_LIST");
        let out_path = std::env::var("MDFIX_AUDIT_OUT").expect("MDFIX_AUDIT_OUT");
        let files = std::fs::read_to_string(list).expect("read list");
        let options = Options::default();
        let mut intended: BTreeMap<&str, usize> = BTreeMap::new();
        let mut unintended: BTreeMap<String, Vec<(String, usize, String)>> = BTreeMap::new();
        let (mut total, mut changed_render) = (0, 0);
        for path in files.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            total += 1;
            let plain = fix::fix_document(&text, &options);
            if plain.refused || !plain.changed {
                continue;
            }
            if render(&normalize(&plain.text)) == render(&normalize(&text)) {
                continue;
            }
            changed_render += 1;
            let guarded = fix_document(&text, &options);
            if guarded.skipped.is_empty() {
                for p in crate::diagnose::diagnose(&text, &options) {
                    if p.fixable && REPAIR_RULES.contains(&p.rule) {
                        *intended.entry(p.rule).or_default() += 1;
                    }
                }
                continue;
            }
            for skip in &guarded.skipped {
                let key = format!("{} ({})", skip.rule, skip.reason);
                let entry = unintended.entry(key).or_default();
                if entry.len() >= 3 {
                    entry.push((path.to_string(), skip.line, String::new()));
                    continue;
                }
                let rule = skip.rule;
                let keep = |t: &str| {
                    fix_document(t, &options)
                        .skipped
                        .iter()
                        .any(|s| s.rule == rule)
                };
                let repro = if text.len() < 200_000 {
                    minimize(&text, &keep)
                } else {
                    String::from("(too large to minimize)")
                };
                entry.push((path.to_string(), skip.line, repro));
            }
        }
        let mut report = String::new();
        let _ = writeln!(
            report,
            "files: {total}, render changed (guard off): {changed_render}\n"
        );
        let _ = writeln!(
            report,
            "## intended repairs (render changed, guard accepted)\n"
        );
        for (rule, count) in &intended {
            let _ = writeln!(report, "- {rule}: {count} problems");
        }
        let _ = writeln!(report, "\n## caught by guard\n");
        for (key, hits) in &unintended {
            let _ = writeln!(report, "### {key}: {} hits\n", hits.len());
            for (path, line, repro) in hits {
                let _ = writeln!(report, "- {path}:{line}");
                if !repro.is_empty() {
                    let _ = writeln!(report, "\n````md\n{repro}\n````\n");
                }
            }
            let _ = writeln!(report);
        }
        std::fs::write(out_path, report).expect("write report");
    }

    /// 除錯用：印出某份文件各變體的渲染。`MDFIX_DEBUG=<檔案> cargo test ... debug_one -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn debug_one() {
        let path = std::env::var("MDFIX_DEBUG").expect("MDFIX_DEBUG");
        let text = std::fs::read_to_string(path).expect("read");
        let options = Options::default();
        let rep = variant(&text, &options, LAYOUT_RULES).unwrap_or_default();
        println!("rep: {rep:?}\n{}", render(&normalize(&rep)));
        for rule in LAYOUT_RULES {
            let ignore: Vec<&str> = LAYOUT_RULES
                .iter()
                .copied()
                .filter(|id| id != rule)
                .collect();
            if let Some(t) = variant(&text, &options, &ignore) {
                if t != rep {
                    println!(
                        "{rule}: {t:?}\nsame render: {}\n{}",
                        render(&normalize(&t)) == render(&normalize(&rep)),
                        render(&normalize(&t))
                    );
                }
            }
        }
        println!("{:?}", fix_document(&text, &options));
    }

    /// 1 MB 效能量測：`cargo test --release -p markdown-fixer --lib -- --ignored guard_one_megabyte --nocapture`
    #[test]
    #[ignore = "效能量測"]
    fn guard_one_megabyte() {
        use std::time::Instant;
        let options = Options::default();
        let mut chunk = String::new();
        let corpus = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/corpus");
        let mut dirs = vec![std::path::PathBuf::from(corpus)];
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if let Ok(text) = std::fs::read_to_string(&path) {
                    if !fix::fix_document(&text, &options).refused && !text.starts_with("---") {
                        chunk.push_str(&text);
                        chunk.push_str("\n\n");
                    }
                }
            }
        }
        let repeat = |unit: &str| {
            let mut text = String::new();
            while text.len() < 1_000_000 {
                text.push_str(unit);
            }
            text
        };
        let cases = [
            ("corpus", repeat(&chunk)),
            (
                "clean",
                repeat("# 標題\n\n一段**粗體**文字與 `code`。\n\n- a\n- b\n\n"),
            ),
            (
                "repairs",
                repeat("#標題\n\n-item\n\n** x **\n\n**標籤：**內容\n\n"),
            ),
            ("mdx031", repeat("* a\n- b\n\n段落\n\n")),
            (
                "mdx005-in-code",
                repeat("段落\n\n    code\n\n\n    more\n\n"),
            ),
            ("mdx040-list", repeat("* 說明**粗體**\n\n段落\n\n")),
            (
                "layout-culprit",
                repeat("段落 a \t \nb\n\n#標題\n\n內文\n\n\n"),
            ),
            ("corpus+culprit", {
                let mut text = repeat(&chunk);
                text.push_str("\na \t \nb\n");
                text
            }),
        ];
        for (name, text) in cases {
            let started = Instant::now();
            let plain = fix::fix_document(&text, &options);
            let plain_time = started.elapsed();
            let started = Instant::now();
            let guarded = fix_document(&text, &options);
            let guard_time = started.elapsed();
            eprintln!(
                "{name:>15}: {} bytes, oracle fix {plain_time:?} (refused {}), guarded {guard_time:?}, skipped {}",
                text.len(),
                plain.refused,
                guarded.skipped.len()
            );
        }
    }
}
