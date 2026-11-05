//! JS 字串與數值語意的小工具：`String.prototype.trim()`、`Number(string)`、
//! `String.prototype.padEnd()` 與 `Array.prototype.sort()` 的字串比較。
//!
//! oracle 的參數解析與輸出格式直接依賴這些語意，Rust 標準函式庫的對應功能
//! 行為不同（例如 `str::trim` 不去 U+FEFF、`str::parse::<f64>` 接受 `inf`），
//! 所以在這裡逐一照 ECMAScript 規格實作。

use std::cmp::Ordering;

/// JS `WhiteSpace` 加上 `LineTerminator`，也就是 `trim()` 與 `Number()` 會去掉的字元。
///
/// 與 `char::is_whitespace` 的差異：JS 包含 U+FEFF，不含 U+0085 與 U+001C～U+001F。
pub fn is_js_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// JS `String.prototype.trim()`。
pub fn js_trim(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// JS `Number(string)`（ECMAScript `StringToNumber`）。不合法時回傳 NaN。
pub fn js_number(value: &str) -> f64 {
    let text = js_trim(value);
    if text.is_empty() {
        return 0.0;
    }
    let bytes = text.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'0' {
        let radix = match bytes[1] {
            b'x' | b'X' => Some(16),
            b'o' | b'O' => Some(8),
            b'b' | b'B' => Some(2),
            _ => None,
        };
        if let Some(radix) = radix {
            return parse_radix(&text[2..], radix);
        }
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    let negative = text.starts_with('-');
    if unsigned == "Infinity" {
        return if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    if !is_decimal_literal(unsigned) {
        return f64::NAN;
    }
    text.parse::<f64>().unwrap_or(f64::NAN)
}

/// `StrUnsignedDecimalLiteral`：`digits [. digits?] | . digits`，後面可接指數。
fn is_decimal_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut frac_digits = 0;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - frac_start;
    }
    if int_digits == 0 && frac_digits == 0 {
        return false;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == bytes.len()
}

fn parse_radix(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut value = 0.0_f64;
    for ch in digits.chars() {
        match ch.to_digit(radix) {
            Some(digit) => value = value * f64::from(radix) + f64::from(digit),
            None => return f64::NAN,
        }
    }
    value
}

/// JS `Number.isInteger(value) && value > 0`，成立時回傳飽和轉換後的 `usize`。
pub fn positive_integer(value: f64) -> Option<usize> {
    if value.is_finite() && value.trunc() == value && value > 0.0 {
        // `as` 對超出範圍的浮點數做飽和轉換；行長上限再大也只是「永不超過」。
        Some(value as usize)
    } else {
        None
    }
}

/// JS `String.prototype.padEnd(width)`，寬度以 UTF-16 code unit 計算。
pub fn pad_end(value: &str, width: usize) -> String {
    let len = value.encode_utf16().count();
    let mut out = String::with_capacity(value.len() + width.saturating_sub(len));
    out.push_str(value);
    for _ in len..width {
        out.push(' ');
    }
    out
}

/// JS `Array.prototype.sort()` 的預設字串比較：依 UTF-16 code unit 字典序。
pub fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_follows_js() {
        assert_eq!(js_trim("\u{FEFF} MDX010\u{00A0}"), "MDX010");
        assert_eq!(js_trim("\u{3000}x\u{2028}"), "x");
        // U+0085 不是 JS 空白。
        assert_eq!(js_trim("\u{0085}x"), "\u{0085}x");
    }

    #[test]
    fn number_follows_js() {
        assert_eq!(js_number("80"), 80.0);
        assert_eq!(js_number(" 80 "), 80.0);
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number("  "), 0.0);
        assert_eq!(js_number("0x50"), 80.0);
        assert_eq!(js_number("0X50"), 80.0);
        assert_eq!(js_number("0o17"), 15.0);
        assert_eq!(js_number("0b101"), 5.0);
        assert_eq!(js_number("1e2"), 100.0);
        assert_eq!(js_number("80.0"), 80.0);
        assert_eq!(js_number("80."), 80.0);
        assert_eq!(js_number(".5"), 0.5);
        assert_eq!(js_number("+80"), 80.0);
        assert_eq!(js_number("-80"), -80.0);
        assert_eq!(js_number("Infinity"), f64::INFINITY);
        assert_eq!(js_number("-Infinity"), f64::NEG_INFINITY);
        for bad in [
            "abc", "0x", "-0x10", "+0x10", "0xg", "inf", "nan", "1_000", "1e", ".", "1.2.3",
            "80px", "infinity", "0b2", "--80",
        ] {
            assert!(js_number(bad).is_nan(), "{bad:?} should be NaN");
        }
    }

    #[test]
    fn positive_integer_rules() {
        assert_eq!(positive_integer(80.0), Some(80));
        assert_eq!(positive_integer(0.0), None);
        assert_eq!(positive_integer(-1.0), None);
        assert_eq!(positive_integer(8.5), None);
        assert_eq!(positive_integer(f64::NAN), None);
        assert_eq!(positive_integer(f64::INFINITY), None);
        assert_eq!(positive_integer(1e300), Some(usize::MAX));
    }

    #[test]
    fn pad_end_counts_utf16() {
        assert_eq!(pad_end("1:2", 10), "1:2       ");
        assert_eq!(pad_end("warning", 7), "warning");
        assert_eq!(pad_end("toolongvalue", 3), "toolongvalue");
        assert_eq!(pad_end("😀", 3), "😀 ");
    }

    #[test]
    fn utf16_order_differs_from_bytes() {
        // U+FF5E（BMP）的 UTF-8 位元組大於 U+1F600，但 UTF-16 code unit 較大的是 U+FF5E。
        let bmp = "\u{FF5E}";
        let astral = "\u{1F600}";
        assert_eq!(bmp.cmp(astral), Ordering::Less);
        assert_eq!(utf16_cmp(bmp, astral), Ordering::Greater);
    }
}
