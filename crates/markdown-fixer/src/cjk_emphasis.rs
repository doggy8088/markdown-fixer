//! MDX042：CJK／全形標點旁無法配對的 `*`、`**`（JS 1440–1678 行）。
//!
//! 以段落為單位模擬 CommonMark 的 delimiter 配對，回報留下來的 run；
//! 只有「`**X：**Y` 這種收尾端前面黏著可移動的全形標點」才自動修正。

use crate::Problem;
use crate::jsstr::{self, JsString, at, is_js_space};
use crate::line_fix::{delimiter_flanking, is_boundary_or_whitespace};
use crate::mask::{MaskOptions, build_mask};
use crate::rules::{Enabled, Overrides, make_problem};
use crate::scan::{
    LineInfo, is_atx_heading, is_blank, is_list_line, is_protected_line, is_thematic_break,
    scan_document_with,
};

const STAR: u16 = b'*' as u16;

/// JS `CJK_ADJACENT_RE`（單一 code unit）。
fn is_cjk_adjacent(c: u16) -> bool {
    matches!(
        c,
        0x2E80..=0x2FFF
            | 0x3000..=0x303F
            | 0x3040..=0x30FF
            | 0x31C0..=0x31EF
            | 0x3200..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
    )
}

/// JS `MOVABLE_PUNCT_RE`：`、。，．：；！？…‥`。
fn is_movable_punct(c: u16) -> bool {
    matches!(
        c,
        0x3001 | 0x3002 | 0xFF0C | 0xFF0E | 0xFF1A | 0xFF1B | 0xFF01 | 0xFF1F | 0x2026 | 0x2025
    )
}

/// JS `isEmphasisProseLine`。
fn is_emphasis_prose_line(line: &[u16], info: &LineInfo) -> bool {
    !is_blank(line) && !is_protected_line(info) && !info.fence
}

/// JS `startsInlineBlockBoundary`。
fn starts_inline_block_boundary(line: &[u16]) -> bool {
    is_atx_heading(line)
        || is_list_line(line)
        || is_thematic_break(line)
        || crate::jsregex::re!(r"^ {0,3}\|").test(line)
}

/// JS `forEachEmphasisParagraph`：回傳每個段落的 `(from, to)`（含）。
fn emphasis_paragraphs(lines: &[JsString], info: &[LineInfo]) -> Vec<(usize, usize)> {
    let prose = |i: usize| {
        info.get(i)
            .is_some_and(|inf| is_emphasis_prose_line(&lines[i], inf))
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !prose(i) {
            i += 1;
            continue;
        }
        let mut end = i;
        while end + 1 < lines.len()
            && prose(end + 1)
            && !starts_inline_block_boundary(&lines[end + 1])
        {
            end += 1;
        }
        out.push((i, end));
        i = end + 1;
    }
    out
}

#[derive(Debug, Clone)]
struct Run {
    start: usize,
    end: usize,
    len: usize,
    can_open: bool,
    can_close: bool,
    before: Option<u16>,
    after: Option<u16>,
}

/// JS `findStarDelimiterRuns(text, mask)`。
fn find_star_delimiter_runs(text: &[u16], mask: &[bool]) -> Vec<Run> {
    let masked = |i: usize| mask.get(i).copied().unwrap_or(false);
    let mut runs = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if text[i] != STAR || masked(i) {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < text.len() && text[end] == STAR && !masked(end) {
            end += 1;
        }
        let before = if i > 0 { at(text, i - 1) } else { None };
        let after = at(text, end);
        let flanking = delimiter_flanking(before, after);
        runs.push(Run {
            start: i,
            end,
            len: end - i,
            can_open: flanking.left,
            can_close: flanking.right,
            before,
            after,
        });
        i = end;
    }
    runs
}

/// JS `matchEmphasisRuns(runs).leftovers`：回傳留下未用長度的 run 索引（依位置排序）。
///
/// 結果與 JS 的逐一反向掃描完全相同，但複雜度是 O(n)（攤銷）而不是 O(n²)：
///
/// - JS 對每個 closer 由近到遠找第一個「未 dropped、remaining > 0、can_open、且沒被
///   rule of three 排除」的 opener。rule of three 只取決於 opener 的 `len % 3` 與
///   `can_close`，所以把候選 opener 依 `(len % 3, can_close)` 分成 6 個 stack（各自依
///   索引遞增），每個 stack 內的候選對同一個 closer 的排除結論相同；最近的合格 opener
///   就是「未被排除的 stack 頂端中索引最大者」。
/// - 一個 run 在 `index` 越過它時才可能成為候選（JS 只往回看 `j < index`），此時若
///   `!dropped && remaining > 0 && can_open` 就推入對應 stack；之後只有「配對時把
///   (opener, closer) 之間全部 dropped」與「opener 的 remaining 用完」會讓它失去資格，
///   前者等於把所有 stack 中索引大於 opener 的元素彈出，後者等於彈出 opener 本身
///   （它此時必定是自己 stack 的頂端）。每個 run 最多推入、彈出各一次。
/// - dropped 區間用差分陣列在最後一次標記，避免同一區段被重複標記造成平方時間。
fn leftover_runs(runs: &[Run]) -> Vec<usize> {
    let n = runs.len();
    let mut remaining: Vec<usize> = runs.iter().map(|r| r.len).collect();
    // 差分陣列：dropped 區間 [opener + 1, closer) 的開頭 +1、結尾 -1。
    let mut dropped_diff = vec![0i64; n + 1];
    let mut stacks: [Vec<usize>; 6] = Default::default();
    let bucket = |run: &Run| (run.len % 3) * 2 + usize::from(run.can_close);
    let mut index = 0;
    while index < n {
        let closer = &runs[index];
        let opener_index = if remaining[index] == 0 || !closer.can_close {
            None
        } else {
            let mut best: Option<usize> = None;
            for (b, stack) in stacks.iter().enumerate() {
                let Some(&top) = stack.last() else {
                    continue;
                };
                let opener_len_mod = b / 2;
                let opener_can_close = b % 2 == 1;
                let rule_of_three = (opener_len_mod + closer.len) % 3 == 0
                    && (opener_len_mod != 0 || closer.len % 3 != 0);
                if (opener_can_close || closer.can_open) && rule_of_three {
                    continue;
                }
                if best.is_none_or(|current| top > current) {
                    best = Some(top);
                }
            }
            best
        };
        let Some(opener_index) = opener_index else {
            // index 越過這個 run：符合條件就成為之後 closer 的候選 opener。
            if remaining[index] > 0 && closer.can_open {
                stacks[bucket(closer)].push(index);
            }
            index += 1;
            continue;
        };
        let used = if remaining[opener_index] >= 2 && remaining[index] >= 2 {
            2
        } else {
            1
        };
        remaining[opener_index] -= used;
        remaining[index] -= used;
        if opener_index + 1 < index {
            dropped_diff[opener_index + 1] += 1;
            dropped_diff[index] -= 1;
        }
        for stack in stacks.iter_mut() {
            while stack.last().is_some_and(|&top| top > opener_index) {
                stack.pop();
            }
        }
        if remaining[opener_index] == 0 {
            let stack = &mut stacks[bucket(&runs[opener_index])];
            if stack.last() == Some(&opener_index) {
                stack.pop();
            }
        }
        if remaining[index] == 0 {
            index += 1;
        }
    }
    let mut depth = 0i64;
    let mut leftovers = Vec::new();
    for k in 0..n {
        depth += dropped_diff[k];
        if depth == 0 && remaining[k] > 0 {
            leftovers.push(k);
        }
    }
    leftovers
}

/// 舊版（逐一反向掃描）的直接移植，只給測試用來驗證 [`leftover_runs`] 與 JS 等價。
#[cfg(test)]
fn leftover_runs_naive(runs: &[Run]) -> Vec<usize> {
    let mut remaining: Vec<usize> = runs.iter().map(|r| r.len).collect();
    let mut dropped = vec![false; runs.len()];
    let mut index = 0;
    while index < runs.len() {
        let closer = &runs[index];
        if dropped[index] || remaining[index] == 0 || !closer.can_close {
            index += 1;
            continue;
        }
        let mut opener_index = None;
        for j in (0..index).rev() {
            let opener = &runs[j];
            if dropped[j] || remaining[j] == 0 || !opener.can_open {
                continue;
            }
            let rule_of_three =
                (opener.len + closer.len) % 3 == 0 && (opener.len % 3 != 0 || closer.len % 3 != 0);
            if (opener.can_close || closer.can_open) && rule_of_three {
                continue;
            }
            opener_index = Some(j);
            break;
        }
        let Some(opener_index) = opener_index else {
            index += 1;
            continue;
        };
        let used = if remaining[opener_index] >= 2 && remaining[index] >= 2 {
            2
        } else {
            1
        };
        remaining[opener_index] -= used;
        remaining[index] -= used;
        for flag in dropped.iter_mut().take(index).skip(opener_index + 1) {
            *flag = true;
        }
        if remaining[index] == 0 {
            index += 1;
        }
    }
    (0..runs.len())
        .filter(|&k| !dropped[k] && remaining[k] > 0)
        .collect()
}

/// 一個 MDX042 發現（JS `findings[]`）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    line: usize,
    column: usize,
    delim_len: usize,
    fixable: bool,
}

/// 段落內的編輯（JS `transforms[].edits[]`），索引是段落文字（以 `\n` 串接）的 UTF-16 位置。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edit {
    start: usize,
    end: usize,
    text: JsString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Transform {
    from: usize,
    to: usize,
    edits: Vec<Edit>,
}

fn attached(inner: &[u16]) -> bool {
    match (inner.first(), inner.last()) {
        (Some(&a), Some(&b)) => !is_js_space(a) && !is_js_space(b),
        _ => false,
    }
}

/// JS `analyzeCjkEmphasis(lines, info)`。
///
/// 結果與 JS 完全相同，但每個段落是 O(n log n)，不是 JS 的 O(n²)：
///
/// - JS 對每個候選 closer 重掃全部 leftovers 找「start < punctStart、can_open、
///   len == 2」中最靠右的 opener。`[punctStart, closer.start)` 全是可移動的全形標點、
///   沒有 `*`，不可能有 run 從那裡開始，所以條件等於「在 leftovers 中排在 closer 之前」；
///   事先算好每個位置之前最後一個合格 leftover 即可。
/// - JS 用 `runs.some(...)` 檢查 opener 與 closer 之間有沒有其他 run。runs 依位置排序、
///   互不重疊，所以這等於兩者在 runs 中的索引不相鄰。
/// - `lineOffsetOf`／`locate` 的反向線性搜尋改成對遞增的 `line_starts` 做二分搜尋。
fn analyze(lines: &[JsString], info: &[LineInfo]) -> (Vec<Finding>, Vec<Transform>) {
    let mut findings = Vec::new();
    let mut transforms = Vec::new();

    for (from, to) in emphasis_paragraphs(lines, info) {
        // 沒有 `*` 的段落不會有 delimiter run。
        if !lines[from..=to].iter().any(|l| l.contains(&STAR)) {
            continue;
        }
        let mut text: JsString = Vec::new();
        let mut mask: Vec<bool> = Vec::new();
        let mut line_starts = Vec::new();
        for (k, line) in lines.iter().enumerate().take(to + 1).skip(from) {
            if k > from {
                text.push(b'\n' as u16);
                mask.push(true);
            }
            line_starts.push(text.len());
            text.extend_from_slice(line);
            mask.extend(build_mask(line, MaskOptions::default()));
        }

        // line_starts 嚴格遞增且第一個是 0：最後一個 `<= pos` 的位置。
        let line_offset_of =
            |pos: usize| -> usize { line_starts.partition_point(|&s| s <= pos).saturating_sub(1) };
        let locate = |pos: usize| -> (usize, usize) {
            let offset = line_offset_of(pos);
            (from + offset + 1, pos - line_starts[offset] + 1)
        };

        let runs = find_star_delimiter_runs(&text, &mask);
        let leftovers = leftover_runs(&runs);
        if leftovers.is_empty() {
            continue;
        }

        let attached_span = |left: usize, right: usize| -> bool {
            attached(jsstr::slice(&text, runs[left].end, runs[right].start))
        };
        let mut reportable = vec![false; runs.len()];
        for k in 0..leftovers.len() {
            let current = leftovers[k];
            let previous = k.checked_sub(1).map(|p| leftovers[p]);
            let next = leftovers.get(k + 1).copied();
            if previous.is_some_and(|p| attached_span(p, current))
                || next.is_some_and(|n| attached_span(current, n))
            {
                reportable[current] = true;
            }
        }
        let cjk_adjacent = |run: &Run| {
            run.before.is_some_and(is_cjk_adjacent) || run.after.is_some_and(is_cjk_adjacent)
        };

        // eligible_before[k]：leftovers[..k] 中最後一個 can_open 且 len == 2 的 run 索引。
        let mut eligible_before = Vec::with_capacity(leftovers.len());
        let mut last_eligible: Option<usize> = None;
        for &idx in &leftovers {
            eligible_before.push(last_eligible);
            if runs[idx].can_open && runs[idx].len == 2 {
                last_eligible = Some(idx);
            }
        }

        let mut fixable = vec![false; runs.len()];
        for (k, &closer_idx) in leftovers.iter().enumerate() {
            let closer = &runs[closer_idx];
            if closer.len != 2 || !reportable[closer_idx] || !cjk_adjacent(closer) {
                continue;
            }
            if is_boundary_or_whitespace(at(&text, closer.end)) {
                continue;
            }
            let mut punct_start = closer.start;
            while punct_start > 0 && is_movable_punct(text[punct_start - 1]) {
                punct_start -= 1;
            }
            if punct_start == closer.start {
                continue;
            }
            let Some(opener_idx) = eligible_before[k] else {
                continue;
            };
            let opener = &runs[opener_idx];
            if opener.end > punct_start {
                continue;
            }
            if !attached(jsstr::slice(&text, opener.end, punct_start)) {
                continue;
            }
            // 中間有其他 run（JS `runs.some(...)`）。
            if closer_idx > opener_idx + 1 {
                continue;
            }
            if line_offset_of(punct_start) != line_offset_of(closer.start) {
                continue;
            }
            fixable[opener_idx] = true;
            fixable[closer_idx] = true;
            transforms.push(Transform {
                from,
                to,
                edits: vec![
                    Edit {
                        start: punct_start,
                        end: closer.start,
                        text: Vec::new(),
                    },
                    Edit {
                        start: closer.end,
                        end: closer.end,
                        text: jsstr::slice(&text, punct_start, closer.start).to_vec(),
                    },
                ],
            });
        }

        for &idx in &leftovers {
            let run = &runs[idx];
            if !reportable[idx] || !cjk_adjacent(run) {
                continue;
            }
            let (line, column) = locate(run.start);
            findings.push(Finding {
                line,
                column,
                delim_len: run.len,
                fixable: fixable[idx],
            });
        }
    }

    (findings, transforms)
}

/// JS `computeCjkEmphasis(lines, info, isEnabled)`。
pub(crate) fn compute_cjk_emphasis(
    lines: &[JsString],
    info: &[LineInfo],
    enabled: &Enabled,
) -> Vec<Problem> {
    if !enabled.is("MDX042") {
        return Vec::new();
    }
    analyze(lines, info)
        .0
        .into_iter()
        .map(|f| {
            let delim = "*".repeat(f.delim_len);
            let message = format!(
                "Unpaired `{delim}` next to CJK or full-width punctuation renders literally instead of emphasis. Move the full-width punctuation outside the delimiters (`**X：**Y` becomes `**X**：Y`) or reword."
            );
            let overrides = if f.fixable {
                Overrides::NONE
            } else {
                Overrides::NOT_FIXABLE
            };
            make_problem("MDX042", f.line, f.column, message, overrides)
        })
        .collect()
}

/// JS `applyCjkEmphasisFixes(text, isEnabled)`。
///
/// JS 對每個 transform 重新 join／split 整個段落，並把「以原始段落計算的偏移量」
/// 依序套用在已修改的文字上，大量 transform 時是平方時間。這裡改成在整份文件上
/// 原地改寫，結果相同的理由：
///
/// - `analyze` 產生的 transform 只有兩個 edit：刪除 `[p, c)` 與在 `e`（`c < e`）插入
///   原本的標點 `T`，`T` 的長度是 `c - p`。依 start 由大到小套用在任何目前內容
///   `buf` 上，結果都是 `buf[..p] + buf[c..e] + T + buf[e..]`，長度不變，只動到
///   `[p, e)`；只要 `buf[p..e]` 與 `T` 都沒有 `\n`，段落的行數與每行長度也不變。
/// - 因此 JS 的 join → 改寫 → split → 寫回各行，等於直接改寫整份文件中段落起點
///   加上偏移量的那一段，而且後面的 transform 看到的行與 JS 完全一樣。
///
/// 每個 transform 套用前都會檢查上述前提；萬一不成立，就把目前結果切回各行，
/// 改用與 JS 相同的逐一 join／split 處理剩下的 transform。
pub(crate) fn apply_cjk_emphasis_fixes(text: &[u16], enabled: &Enabled) -> JsString {
    if !enabled.is("MDX042") {
        return text.to_vec();
    }
    let scan = scan_document_with(text, enabled.limits);
    let transforms = analyze(&scan.lines, &scan.info).1;
    if transforms.is_empty() {
        return text.to_vec();
    }

    apply_transforms(&scan.lines, &transforms)
}

/// 依序套用 transforms 並回傳 `lines.join('\n')`；能原地改寫的就原地改寫。
fn apply_transforms(lines: &[JsString], transforms: &[Transform]) -> JsString {
    let mut out = jsstr::join_lines(lines);
    let mut line_starts = Vec::with_capacity(lines.len());
    let mut pos = 0;
    for line in lines {
        line_starts.push(pos);
        pos += line.len() + 1;
    }
    for (k, transform) in transforms.iter().enumerate() {
        if !rotate_in_place(&mut out, lines, &line_starts, transform) {
            let mut lines = jsstr::split_lines(&out);
            apply_transforms_sequentially(&mut lines, &transforms[k..]);
            return jsstr::join_lines(&lines);
        }
    }
    out
}

/// 在整份文件 `out` 上原地套用一個 transform；前提不成立時不動 `out` 並回傳 `false`。
///
/// `lines` 是原始各行，只用來算段落長度；transform 不改變任何行的長度，所以
/// `line_starts` 與段落長度在整個套用過程中都有效。
fn rotate_in_place(
    out: &mut [u16],
    lines: &[JsString],
    line_starts: &[usize],
    transform: &Transform,
) -> bool {
    const LF: u16 = b'\n' as u16;
    let [remove, insert] = transform.edits.as_slice() else {
        return false;
    };
    let (p, c, e) = (remove.start, remove.end, insert.start);
    if !remove.text.is_empty()
        || insert.end != e
        || p >= c
        || c > e
        || insert.text.len() != c - p
        || insert.text.contains(&LF)
    {
        return false;
    }
    let (Some(&base), Some(&last_start), Some(last)) = (
        line_starts.get(transform.from),
        line_starts.get(transform.to),
        lines.get(transform.to),
    ) else {
        return false;
    };
    if transform.from > transform.to || e > last_start + last.len() - base {
        return false;
    }
    let Some(window) = out.get_mut(base + p..base + e) else {
        return false;
    };
    if window.contains(&LF) {
        return false;
    }
    // [p, e) 的 `X..X` + `Y..Y`（`[c, e)`）變成 `Y..Y` + T。
    window.copy_within(c - p.., 0);
    let tail = e - c;
    window[tail..].copy_from_slice(&insert.text);
    true
}

/// JS 的逐一套用：每個 transform 都重新 join 段落、依 start 由大到小套用 edit、再 split 寫回。
fn apply_transforms_sequentially(lines: &mut Vec<JsString>, transforms: &[Transform]) {
    for transform in transforms {
        // 與 JS 相同：同一段落的多個 transform 依序套用在「已更新」的行上，
        // 但偏移量是以原始段落計算的。
        let Some(slice) = lines.get(transform.from..=transform.to) else {
            continue;
        };
        let mut paragraph = jsstr::join_lines(slice);
        let mut edits = transform.edits.clone();
        edits.sort_by_key(|e| std::cmp::Reverse(e.start));
        for edit in &edits {
            let head = jsstr::slice(&paragraph, 0, edit.start);
            let tail = jsstr::slice_from(&paragraph, edit.end);
            paragraph = jsstr::concat(&[head, &edit.text, tail]);
        }
        for (k, line) in jsstr::split_lines(&paragraph).into_iter().enumerate() {
            let target = transform.from + k;
            if target < lines.len() {
                lines[target] = line;
            } else {
                lines.push(line);
            }
        }
    }
}

/// 舊版 MDX042 分析與套用的直接移植（與 JS 同形狀、平方時間），只給測試驗證等價。
#[cfg(test)]
mod naive {
    use super::*;

    /// 舊版 [`analyze`]（逐一重掃 leftovers／runs、線性找行）的直接移植。
    pub(super) fn analyze_naive(
        lines: &[JsString],
        info: &[LineInfo],
    ) -> (Vec<Finding>, Vec<Transform>) {
        let mut findings = Vec::new();
        let mut transforms = Vec::new();

        for (from, to) in emphasis_paragraphs(lines, info) {
            // 沒有 `*` 的段落不會有 delimiter run。
            if !lines[from..=to].iter().any(|l| l.contains(&STAR)) {
                continue;
            }
            let mut text: JsString = Vec::new();
            let mut mask: Vec<bool> = Vec::new();
            let mut line_starts = Vec::new();
            for (k, line) in lines.iter().enumerate().take(to + 1).skip(from) {
                if k > from {
                    text.push(b'\n' as u16);
                    mask.push(true);
                }
                line_starts.push(text.len());
                text.extend_from_slice(line);
                mask.extend(build_mask(line, MaskOptions::default()));
            }

            let line_offset_of =
                |pos: usize| -> usize { line_starts.iter().rposition(|&s| pos >= s).unwrap_or(0) };
            let locate = |pos: usize| -> (usize, usize) {
                let offset = line_offset_of(pos);
                (from + offset + 1, pos - line_starts[offset] + 1)
            };

            let runs = find_star_delimiter_runs(&text, &mask);
            let leftovers = leftover_runs(&runs);
            if leftovers.is_empty() {
                continue;
            }

            let attached_span = |left: usize, right: usize| -> bool {
                attached(jsstr::slice(&text, runs[left].end, runs[right].start))
            };
            let mut reportable = vec![false; runs.len()];
            for k in 0..leftovers.len() {
                let current = leftovers[k];
                let previous = k.checked_sub(1).map(|p| leftovers[p]);
                let next = leftovers.get(k + 1).copied();
                if previous.is_some_and(|p| attached_span(p, current))
                    || next.is_some_and(|n| attached_span(current, n))
                {
                    reportable[current] = true;
                }
            }
            let cjk_adjacent = |run: &Run| {
                run.before.is_some_and(is_cjk_adjacent) || run.after.is_some_and(is_cjk_adjacent)
            };

            let mut fixable = vec![false; runs.len()];
            for &closer_idx in &leftovers {
                let closer = &runs[closer_idx];
                if closer.len != 2 || !reportable[closer_idx] || !cjk_adjacent(closer) {
                    continue;
                }
                if is_boundary_or_whitespace(at(&text, closer.end)) {
                    continue;
                }
                let mut punct_start = closer.start;
                while punct_start > 0 && is_movable_punct(text[punct_start - 1]) {
                    punct_start -= 1;
                }
                if punct_start == closer.start {
                    continue;
                }
                let mut opener: Option<usize> = None;
                for &cand_idx in &leftovers {
                    let candidate = &runs[cand_idx];
                    if cand_idx == closer_idx || candidate.start >= punct_start {
                        continue;
                    }
                    if !candidate.can_open || candidate.len != 2 {
                        continue;
                    }
                    if opener.is_none_or(|o| candidate.start > runs[o].start) {
                        opener = Some(cand_idx);
                    }
                }
                let Some(opener_idx) = opener else {
                    continue;
                };
                let opener = &runs[opener_idx];
                if opener.end > punct_start {
                    continue;
                }
                if !attached(jsstr::slice(&text, opener.end, punct_start)) {
                    continue;
                }
                // JS 以物件身分比較 `run !== opener`（opener 是複本，永遠為 true），
                // 但 opener／closer 本身本來就不會落在 [opener.end, closer.start] 之內。
                if runs
                    .iter()
                    .any(|run| run.start >= opener.end && run.end <= closer.start)
                {
                    continue;
                }
                if line_offset_of(punct_start) != line_offset_of(closer.start) {
                    continue;
                }
                fixable[opener_idx] = true;
                fixable[closer_idx] = true;
                transforms.push(Transform {
                    from,
                    to,
                    edits: vec![
                        Edit {
                            start: punct_start,
                            end: closer.start,
                            text: Vec::new(),
                        },
                        Edit {
                            start: closer.end,
                            end: closer.end,
                            text: jsstr::slice(&text, punct_start, closer.start).to_vec(),
                        },
                    ],
                });
            }

            for &idx in &leftovers {
                let run = &runs[idx];
                if !reportable[idx] || !cjk_adjacent(run) {
                    continue;
                }
                let (line, column) = locate(run.start);
                findings.push(Finding {
                    line,
                    column,
                    delim_len: run.len,
                    fixable: fixable[idx],
                });
            }
        }

        (findings, transforms)
    }

    /// 舊版 [`apply_cjk_emphasis_fixes`]（每個 transform 重新 join／split 段落）的直接移植。
    pub(super) fn apply_naive(text: &[u16], enabled: &Enabled) -> JsString {
        if !enabled.is("MDX042") {
            return text.to_vec();
        }
        let scan = scan_document_with(text, enabled.limits);
        let transforms = analyze_naive(&scan.lines, &scan.info).1;
        if transforms.is_empty() {
            return text.to_vec();
        }

        let mut lines = scan.lines.clone();
        for transform in transforms {
            // 與 JS 相同：同一段落的多個 transform 依序套用在「已更新」的行上，
            // 但偏移量是以原始段落計算的。
            let Some(slice) = lines.get(transform.from..=transform.to) else {
                continue;
            };
            let mut paragraph = jsstr::join_lines(slice);
            let mut edits = transform.edits.clone();
            edits.sort_by_key(|e| std::cmp::Reverse(e.start));
            for edit in &edits {
                let head = jsstr::slice(&paragraph, 0, edit.start);
                let tail = jsstr::slice_from(&paragraph, edit.end);
                paragraph = jsstr::concat(&[head, &edit.text, tail]);
            }
            for (k, line) in jsstr::split_lines(&paragraph).into_iter().enumerate() {
                let target = transform.from + k;
                if target < lines.len() {
                    lines[target] = line;
                } else {
                    lines.push(line);
                }
            }
        }
        jsstr::join_lines(&lines)
    }
}

#[cfg(test)]
mod leftover_equivalence_tests {
    use super::{Run, leftover_runs, leftover_runs_naive};

    /// 小型決定性亂數（xorshift），避免為了測試加相依。
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    fn run(len: usize, can_open: bool, can_close: bool) -> Run {
        Run {
            start: 0,
            end: len,
            len,
            can_open,
            can_close,
            before: None,
            after: None,
        }
    }

    #[test]
    fn stack_matcher_equals_naive_backward_scan() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for case in 0..20_000 {
            let n = (rng.next() % 24) as usize;
            let runs: Vec<Run> = (0..n)
                .map(|_| {
                    let r = rng.next();
                    run(1 + (r % 7) as usize, r & 0x100 != 0, r & 0x200 != 0)
                })
                .collect();
            assert_eq!(
                leftover_runs(&runs),
                leftover_runs_naive(&runs),
                "case {case}: {:?}",
                runs.iter()
                    .map(|r| (r.len, r.can_open, r.can_close))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn closer_only_runs_are_linear() {
        let runs: Vec<Run> = (0..200_000).map(|_| run(1, false, true)).collect();
        let started = std::time::Instant::now();
        let leftovers = leftover_runs(&runs);
        assert_eq!(leftovers.len(), runs.len());
        // 時間上限只在 release build 斷言，避免 debug CI 誤報。
        assert!(cfg!(debug_assertions) || started.elapsed() < std::time::Duration::from_secs(2));
    }
}

#[cfg(test)]
mod linear_equivalence_tests {
    use super::naive::{analyze_naive, apply_naive};
    use super::{
        Edit, Transform, analyze, apply_cjk_emphasis_fixes, apply_transforms,
        apply_transforms_sequentially,
    };
    use crate::jsstr::{self, u};
    use crate::rules::Enabled;
    use crate::scan::scan_document;

    /// 小型決定性亂數（xorshift），避免為了測試加相依。
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
            items[(self.next() % items.len() as u64) as usize]
        }
    }

    /// 可修正的形狀放前面並重複出現，提高命中率。
    const UNITS: &[&str] = &[
        "**甲：**乙",
        "**甲：**乙",
        "**標題，。**內文",
        "**甲！？**乙",
        "**甲…**乙",
        "*甲：*乙",
        "**甲：**乙：**丙",
        "**「引號」**文字",
        "**",
        "*",
        "***",
        "****",
        "甲",
        "乙丙",
        "abc",
        "：",
        "，",
        "。",
        "！",
        "…",
        "、",
        "「",
        "」",
        "（",
        " ",
        " ",
        "\t",
        "\n",
        "\n",
        "\n\n",
        "\n# ",
        "\n- ",
        "\n| ",
        "\n> ",
        "\n```\n",
        "`code`",
        "`**甲：**`",
        "`",
        "$",
        "$5 ",
        "\\*",
        "\\",
        "_",
        "<b>",
        "[連結](http://x)",
        "😀",
        "\u{3000}",
    ];

    fn document(rng: &mut Rng) -> String {
        let n = 1 + (rng.next() % 40) as usize;
        (0..n).map(|_| rng.pick(UNITS)).collect()
    }

    #[test]
    fn linear_analyze_and_apply_equal_naive_port() {
        let enabled = Enabled::new(&crate::Options::default());
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        let mut total_transforms = 0usize;
        let mut max_per_paragraph = 0usize;
        for case in 0..30_000 {
            let doc = document(&mut rng);
            let text = u(&doc);
            let scan = scan_document(&text);
            let fast = analyze(&scan.lines, &scan.info);
            let slow = analyze_naive(&scan.lines, &scan.info);
            assert_eq!(fast, slow, "case {case}: {doc:?}");
            total_transforms += fast.1.len();
            let mut per_paragraph = std::collections::HashMap::new();
            for t in &fast.1 {
                *per_paragraph.entry(t.from).or_insert(0usize) += 1;
            }
            max_per_paragraph =
                max_per_paragraph.max(per_paragraph.values().copied().max().unwrap_or(0));
            assert_eq!(
                apply_cjk_emphasis_fixes(&text, &enabled),
                apply_naive(&text, &enabled),
                "case {case}: {doc:?}"
            );
        }
        // 確認產生器真的有覆蓋到修正路徑，而不是全部沒有 transform。
        assert!(
            total_transforms > 10_000,
            "only {total_transforms} transforms"
        );
        assert!(
            max_per_paragraph >= 3,
            "max {max_per_paragraph} per paragraph"
        );
    }

    #[test]
    fn fixed_shapes_equal_naive_port() {
        let enabled = Enabled::new(&crate::Options::default());
        let cases = [
            "**甲：**乙 ".repeat(50) + "\n",
            "*甲：*乙 ".repeat(50) + "\n",
            ("**甲：**乙 ".repeat(5) + "\n").repeat(10),
            "**甲：**乙：**丙：**丁\n".to_string(),
            "**甲：**乙 plain *text* **ok** ".repeat(20),
            "**甲：**乙\n\n**丙，**丁\n".to_string(),
        ];
        for doc in &cases {
            let text = u(doc);
            let scan = scan_document(&text);
            assert_eq!(
                analyze(&scan.lines, &scan.info),
                analyze_naive(&scan.lines, &scan.info),
                "{doc:?}"
            );
            assert_eq!(
                apply_cjk_emphasis_fixes(&text, &enabled),
                apply_naive(&text, &enabled),
                "{doc:?}"
            );
        }
    }

    /// 原地改寫的前提不成立時（跨行、長度不符、超出段落、重疊），要退回與 JS
    /// 相同的逐一套用，而且結果一致。`analyze` 不會產生這些 transform，所以直接手造。
    #[test]
    fn fallback_matches_sequential_application() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        let alphabet: Vec<u16> = u("ab：*\n");
        for case in 0..20_000 {
            let len = (rng.next() % 16) as usize;
            let text: Vec<u16> = (0..len)
                .map(|_| alphabet[(rng.next() % alphabet.len() as u64) as usize])
                .collect();
            let lines = jsstr::split_lines(&text);
            let transforms: Vec<Transform> = (0..1 + rng.next() % 4)
                .map(|_| {
                    let from = (rng.next() % (lines.len() as u64 + 1)) as usize;
                    let to = from + (rng.next() % 3) as usize;
                    let p = (rng.next() % 18) as usize;
                    let c = p + (rng.next() % 4) as usize;
                    let e = c + (rng.next() % 4) as usize;
                    let t_len = if rng.next() % 4 == 0 {
                        (rng.next() % 4) as usize
                    } else {
                        c - p
                    };
                    let t: Vec<u16> = (0..t_len)
                        .map(|_| alphabet[(rng.next() % alphabet.len() as u64) as usize])
                        .collect();
                    Transform {
                        from,
                        to,
                        edits: vec![
                            Edit {
                                start: p,
                                end: c,
                                text: Vec::new(),
                            },
                            Edit {
                                start: e,
                                end: e,
                                text: t,
                            },
                        ],
                    }
                })
                .collect();
            let mut expected = lines.clone();
            apply_transforms_sequentially(&mut expected, &transforms);
            assert_eq!(
                apply_transforms(&lines, &transforms),
                jsstr::join_lines(&expected),
                "case {case}: {text:?} {transforms:?}"
            );
        }
    }
}
