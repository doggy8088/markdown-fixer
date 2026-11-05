//! 行內保護遮罩：`buildMask`、`splitMasked`、`replaceWithOffsets`（JS 544–686 行）。
//!
//! 遮罩以 UTF-16 code unit 為單位，`mask[i]` 為 true 表示該位置屬於 inline
//! code、跳脫字元、數學式、連結目的地、角括號、裸 URL 等不能改寫的區域。

use crate::jsregex::{Captures, Regex, re};
use crate::jsstr::at;

/// `buildMask` 的選項；[`Default`] 與 JS 不帶 `options` 時相同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MaskOptions {
    pub(crate) mask_bare_urls: bool,
    pub(crate) mask_link_destinations: bool,
    pub(crate) mask_reference_labels: bool,
    pub(crate) mask_link_text: bool,
}

impl Default for MaskOptions {
    fn default() -> Self {
        MaskOptions {
            mask_bare_urls: true,
            mask_link_destinations: true,
            mask_reference_labels: true,
            mask_link_text: false,
        }
    }
}

impl MaskOptions {
    /// `{ maskBareUrls: false }`。
    pub(crate) const NO_BARE_URLS: MaskOptions = MaskOptions {
        mask_bare_urls: false,
        mask_link_destinations: true,
        mask_reference_labels: true,
        mask_link_text: false,
    };
}

/// 裸 URL 樣式（JS 多處重複的 `/\b(?:https?:\/\/|ftp:\/\/|www\.)(?:[^\s<>"']|\u00A0)+/gi`）。
pub(crate) fn url_re() -> &'static Regex {
    re!(
        r#"\b(?:https?:\/\/|ftp:\/\/|www\.)(?:[^\s<>"']|\u00A0)+"#,
        "i"
    )
}

const BACKSLASH: u16 = b'\\' as u16;
const BACKTICK: u16 = b'`' as u16;

/// JS `buildMask(line, options)`。
pub(crate) fn build_mask(line: &[u16], options: MaskOptions) -> Vec<bool> {
    let len = line.len();
    let mut mask = vec![false; len];

    fn mark(mask: &mut [bool], start: usize, end: usize) {
        let to = end.min(mask.len());
        for slot in mask.iter_mut().take(to).skip(start) {
            *slot = true;
        }
    }

    // 跳脫字元：`\` 與下一個 code unit。
    let mut i = 0;
    while i + 1 < len {
        if line[i] == BACKSLASH && !mask[i] {
            mark(&mut mask, i, i + 2);
            i += 1;
        }
        i += 1;
    }

    // inline code：長度相同的 backtick run 配對。
    let mut cursor = 0;
    while cursor < len {
        if line[cursor] != BACKTICK || mask[cursor] {
            cursor += 1;
            continue;
        }
        let mut run = 1;
        while at(line, cursor + run) == Some(BACKTICK) {
            run += 1;
        }
        let mut scan = cursor + run;
        let mut close = None;
        while scan < len {
            if line[scan] == BACKTICK {
                let mut close_run = 1;
                while at(line, scan + close_run) == Some(BACKTICK) {
                    close_run += 1;
                }
                if close_run == run {
                    close = Some(scan);
                    break;
                }
                scan += close_run;
            } else {
                scan += 1;
            }
        }
        match close {
            Some(close) => {
                mark(&mut mask, cursor, close + run);
                cursor = close + run;
            }
            None => cursor += run,
        }
    }

    let mark_regex = |mask: &mut Vec<bool>, regex: &Regex| {
        for caps in regex.find_iter(line) {
            if !mask.get(caps.start()).copied().unwrap_or(false) {
                mark(mask, caps.start(), caps.end());
            }
        }
    };

    // 以下各樣式都必須含有某個字元才可能匹配；先檢查可省下大部分逐位置比對，結果不變。
    let has = |c: u8| line.contains(&u16::from(c));

    if has(b'$') {
        mark_regex(&mut mask, re!(r"\$\$[\s\S]+?\$\$"));
        mark_regex(&mut mask, re!(r"\$(?![\s$])[^$\n]*?(?<![\s\\])\$"));
    }

    if options.mask_link_destinations {
        let mut i = 0;
        while i + 1 < len {
            if line[i] != b']' as u16 || line[i + 1] != b'(' as u16 || mask[i] {
                i += 1;
                continue;
            }
            let mut depth = 1i32;
            let mut scan = i + 2;
            let mut in_angle = false;
            let mut quote: Option<u16> = None;
            while scan < len {
                let ch = line[scan];
                if let Some(q) = quote {
                    if ch == q && line[scan - 1] != BACKSLASH {
                        quote = None;
                    }
                } else if in_angle {
                    if ch == b'>' as u16 {
                        in_angle = false;
                    }
                } else if ch == b'<' as u16 {
                    in_angle = true;
                } else if ch == b'"' as u16 || ch == b'\'' as u16 {
                    quote = Some(ch);
                } else if ch == b'(' as u16 {
                    depth += 1;
                } else if ch == b')' as u16 {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                scan += 1;
            }
            if depth == 0 && scan < len {
                mark(&mut mask, i + 1, scan + 1);
                i = scan;
            }
            i += 1;
        }
    }

    if options.mask_reference_labels && has(b']') {
        mark_regex(&mut mask, re!(r"\]\[[^\]\n]*\]"));
    }
    if options.mask_link_text && has(b'[') {
        mark_regex(&mut mask, re!(r"!?\[[^\]\n]*\]\("));
        mark_regex(&mut mask, re!(r"!?\[[^\]\n]*\]\[[^\]\n]*\]"));
    }
    if has(b'<') {
        for (start, end) in angle_spans(line) {
            if !mask.get(start).copied().unwrap_or(false) {
                mark(&mut mask, start, end);
            }
        }
    }

    if options.mask_bare_urls {
        // `https?://`、`ftp://` 含 `:`；`www.` 含 `.`。
        if has(b':') || has(b'.') {
            mark_regex(&mut mask, url_re());
        }
        if has(b'@') {
            mark_regex(
                &mut mask,
                re!(r"\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b", "i"),
            );
        }
    }

    mask
}

/// `/<[^>\n]{1,500}>/g` 的所有比對範圍（`[start, end)`），與 regex 逐一比對的結果相同。
///
/// 因為 `[^>\n]` 不含 `>`，從 `<` 起算的比對成立若且唯若「之後第一個 `>` 或 `\n`」
/// 是 `>`，且距離在 1–500 之間；比對成功後從 `end` 繼續、失敗則從下一個位置繼續。
/// 以預先算好的「下一個 `>`／`\n` 位置」讓每個起點 O(1)，避免 regex 在一長串 `<`
/// 上每個起點各走 500 步。
fn angle_spans(line: &[u16]) -> Vec<(usize, usize)> {
    const LT: u16 = b'<' as u16;
    const GT: u16 = b'>' as u16;
    const LF: u16 = b'\n' as u16;
    let len = line.len();
    let mut next_stop = vec![len; len + 1];
    for i in (0..len).rev() {
        next_stop[i] = if line[i] == GT || line[i] == LF {
            i
        } else {
            next_stop[i + 1]
        };
    }
    let mut spans = Vec::new();
    let mut i = 0;
    while i < len {
        if line[i] == LT {
            let stop = next_stop[i + 1];
            let run = stop - (i + 1);
            if stop < len && line[stop] == GT && (1..=500).contains(&run) {
                spans.push((i, stop + 1));
                i = stop + 1;
                continue;
            }
        }
        i += 1;
    }
    spans
}

/// `splitMasked` 的一段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Segment<'a> {
    pub(crate) text: &'a [u16],
    /// 在整行中的起點（UTF-16 index）。
    pub(crate) start: usize,
    pub(crate) protected: bool,
}

/// JS `splitMasked(line, mask)`：把一行切成連續的保護／未保護區段。
pub(crate) fn split_masked<'a>(line: &'a [u16], mask: &[bool]) -> Vec<Segment<'a>> {
    let mut segments = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let prot = mask.get(i).copied().unwrap_or(false);
        let mut j = i + 1;
        while j < line.len() && mask.get(j).copied().unwrap_or(false) == prot {
            j += 1;
        }
        segments.push(Segment {
            text: &line[i..j],
            start: i,
            protected: prot,
        });
        i = j;
    }
    segments
}

/// JS `replaceWithOffsets(text, regex, replacer)`：全域比對，`replacer` 拿到比對與起點。
pub(crate) fn replace_with_offsets(
    text: &[u16],
    regex: &Regex,
    mut replacer: impl FnMut(&Captures, usize) -> Vec<u16>,
) -> Vec<u16> {
    regex.replace_with(text, true, |caps| replacer(caps, caps.start()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsstr::u;

    fn mask_str(line: &str, options: MaskOptions) -> String {
        build_mask(&u(line), options)
            .iter()
            .map(|&b| if b { '1' } else { '0' })
            .collect()
    }

    #[test]
    fn split_segments() {
        let line = u("a`b`c");
        let mask = build_mask(&line, MaskOptions::default());
        let segs = split_masked(&line, &mask);
        assert_eq!(segs.len(), 3);
        assert!(segs[1].protected);
        assert_eq!(segs[2].start, 4);
    }

    #[test]
    fn angle_spans_match_regex() {
        let re = Regex::new(r"<[^>\n]{1,500}>", "").expect("regex");
        let long = format!("<{}>", "a".repeat(500));
        let longer = format!("<{}>", "a".repeat(501));
        for s in [
            "<a> <> <<b>> < c > <\n>",
            "<<<<",
            "a<b<c>d>e",
            long.as_str(),
            longer.as_str(),
            "<😀>",
            "",
        ] {
            let line = u(s);
            let expected: Vec<(usize, usize)> = re
                .find_iter(&line)
                .iter()
                .map(|c| (c.start(), c.end()))
                .collect();
            assert_eq!(angle_spans(&line), expected, "{s:?}");
        }
    }

    #[test]
    fn simple_masks() {
        assert_eq!(mask_str(r"a\*b", MaskOptions::default()), "0110");
        assert_eq!(mask_str("x <b> y", MaskOptions::default()), "0011100");
    }
}
