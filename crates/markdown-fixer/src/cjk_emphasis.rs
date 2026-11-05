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
    scan_document,
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
pub(crate) fn apply_cjk_emphasis_fixes(text: &[u16], enabled: &Enabled) -> JsString {
    if !enabled.is("MDX042") {
        return text.to_vec();
    }
    let scan = scan_document(text);
    let transforms = analyze(&scan.lines, &scan.info).1;
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
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
