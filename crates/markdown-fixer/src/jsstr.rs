//! JS 字串語意的輔助函式。
//!
//! Oracle 以 JS 字串（UTF-16 code unit 序列）運算：索引、`length`、遮罩陣列、
//! `text[i]` 取到的「字元」都是 code unit，emoji 會被拆成兩個 lone surrogate。
//! 為了逐位元組一致，行內處理一律以 `&[u16]` 表示一行文字，只在函式庫邊界
//! 與 `String` 互轉。欄號因此天生就是 UTF-16 單位，不需要另外換算。

use crate::unicode_tables::PUNCT_OR_SYMBOL;

/// JS 字串（UTF-16 code unit 序列）。
pub(crate) type JsString = Vec<u16>;

/// `&str` 轉成 UTF-16。
pub(crate) fn u(s: &str) -> JsString {
    s.encode_utf16().collect()
}

/// UTF-16 轉回 `String`。正常流程不會產生 lone surrogate；萬一有，以 U+FFFD 取代，不 panic。
pub(crate) fn to_string(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// JS 正規表示式 `\s`（也就是 `String.prototype.trim()` 去掉的字元）。
///
/// 包含 U+FEFF 與 U+3000，但不含 U+0085；與 Rust `char::is_whitespace` 不同。
pub(crate) fn is_js_space(c: u16) -> bool {
    matches!(
        c,
        0x09..=0x0D
            | 0x20
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

/// JS 正規表示式 `\w`（只有 ASCII）。
pub(crate) fn is_word(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

/// JS 正規表示式 `\d`（只有 ASCII）。
pub(crate) fn is_digit(c: u16) -> bool {
    (0x30..=0x39).contains(&c)
}

/// `/[\p{P}\p{S}]/u.test(ch)`，`ch` 是單一 code unit（lone surrogate 一律為 false）。
pub(crate) fn is_punct_or_symbol(c: u16) -> bool {
    PUNCT_OR_SYMBOL
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// `String.prototype.trim()`。
pub(crate) fn trim(s: &[u16]) -> &[u16] {
    let start = s.iter().position(|&c| !is_js_space(c)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&c| !is_js_space(c))
        .map_or(start, |i| i + 1);
    s.get(start..end).unwrap_or(&[])
}

/// `s.trim() === ''`。
pub(crate) fn is_trim_empty(s: &[u16]) -> bool {
    s.iter().all(|&c| is_js_space(c))
}

/// `[ \t]*$` 的 `isBlank`（只看空格與 tab，與 `trim` 不同）。
pub(crate) fn is_blank(s: &[u16]) -> bool {
    s.iter().all(|&c| c == 0x20 || c == 0x09)
}

/// `split('\n')`：保留最後一個空字串。
pub(crate) fn split_lines(s: &[u16]) -> Vec<JsString> {
    split_by(s, 0x0A)
}

/// `split(<單一字元>)`。
pub(crate) fn split_by(s: &[u16], sep: u16) -> Vec<JsString> {
    s.split(|&c| c == sep).map(<[u16]>::to_vec).collect()
}

/// `join(<單一字元>)`。
pub(crate) fn join_by(parts: &[JsString], sep: u16) -> JsString {
    let total = parts.iter().map(Vec::len).sum::<usize>() + parts.len();
    let mut out = Vec::with_capacity(total);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.extend_from_slice(part);
    }
    out
}

/// `lines.join('\n')`。
pub(crate) fn join_lines(lines: &[JsString]) -> JsString {
    join_by(lines, 0x0A)
}

/// `s.startsWith(lit)`，`lit` 為 Rust 字串常值。
pub(crate) fn starts_with(s: &[u16], lit: &str) -> bool {
    lit.encode_utf16()
        .enumerate()
        .all(|(i, c)| s.get(i) == Some(&c))
}

/// `s.endsWith(lit)`。
pub(crate) fn ends_with(s: &[u16], lit: &str) -> bool {
    let lit = u(lit);
    s.ends_with(&lit)
}

/// `s.indexOf(needle, from)`；找不到回傳 `None`。
pub(crate) fn index_of(s: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if from > s.len() {
        return if needle.is_empty() {
            Some(s.len())
        } else {
            None
        };
    }
    if needle.is_empty() {
        return Some(from);
    }
    s.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// `s.includes(lit)`。
pub(crate) fn includes(s: &[u16], lit: &str) -> bool {
    index_of(s, &u(lit), 0).is_some()
}

/// 安全的 `s.slice(start, end)`（超出範圍時夾住，不 panic）。
pub(crate) fn slice(s: &[u16], start: usize, end: usize) -> &[u16] {
    let end = end.min(s.len());
    let start = start.min(end);
    &s[start..end]
}

/// 安全的 `s.slice(start)`。
pub(crate) fn slice_from(s: &[u16], start: usize) -> &[u16] {
    &s[start.min(s.len())..]
}

/// `s[i]`；超出範圍時為 `None`（JS 的 `undefined`）。
pub(crate) fn at(s: &[u16], i: usize) -> Option<u16> {
    s.get(i).copied()
}

/// 把多段 UTF-16 串接起來。
pub(crate) fn concat(parts: &[&[u16]]) -> JsString {
    let mut out = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        out.extend_from_slice(p);
    }
    out
}

/// `String.prototype.toLowerCase()`。
///
/// JS 與 Rust 都採用 Unicode 預設（非語系）小寫對應，含 final sigma 規則。
pub(crate) fn to_lower_case(s: &[u16]) -> JsString {
    u(&to_string(s).to_lowercase())
}

/// `Array.from(s).length`：以 code point 計數（lone surrogate 各算 1）。
pub(crate) fn code_point_len(s: &[u16]) -> usize {
    char::decode_utf16(s.iter().copied()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_matches_js() {
        assert!(is_js_space(0xFEFF));
        assert!(is_js_space(0x3000));
        assert!(is_js_space(0xA0));
        assert!(!is_js_space(0x85));
        assert!(!is_js_space(0x200B));
    }

    #[test]
    fn trim_strips_bom_and_ideographic_space() {
        assert_eq!(trim(&u("\u{FEFF} a \u{3000}")), u("a").as_slice());
        assert!(trim(&u("   ")).is_empty());
        assert!(is_trim_empty(&u("\u{FEFF}")));
    }

    #[test]
    fn split_keeps_trailing_empty() {
        assert_eq!(split_lines(&u("a\n")).len(), 2);
        assert_eq!(split_lines(&u("")).len(), 1);
    }

    #[test]
    fn punct_table() {
        assert!(is_punct_or_symbol(u16::from(b'*')));
        assert!(is_punct_or_symbol(0xFF1A)); // ：
        assert!(is_punct_or_symbol(0x3002)); // 。
        assert!(!is_punct_or_symbol(u16::from(b'a')));
        assert!(!is_punct_or_symbol(0x4E2D)); // 中
        assert!(!is_punct_or_symbol(0xD83D)); // lone surrogate
    }

    #[test]
    fn code_points() {
        assert_eq!(code_point_len(&u("a😀中")), 3);
    }
}
