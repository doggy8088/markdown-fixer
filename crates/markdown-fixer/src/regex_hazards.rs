//! 正規表示式效能防護測試（只在測試中編譯）。
//!
//! 對本 crate（core 擁有的模組）用到的每一個 JS 正規表示式，餵入 10k code unit 的
//! 病態輸入（巢狀量詞、重疊選擇、相鄰重疊量詞最容易觸發的形狀），確認比對時間
//! 有上限。`jsregex` 在步數預算用完後會改用記憶化回溯，所以這裡應該全部是線性等級。

use std::time::{Duration, Instant};

use crate::jsregex::Regex;
use crate::jsstr::u;

/// 每個（樣式, 輸入）的時間上限（release）。
const LIMIT: Duration = Duration::from_millis(50);

/// 只在 release build 斷言時間；debug build 未最佳化、CI 機器又慢，只印出耗時避免誤報。
fn check_time(elapsed: Duration, limit: Duration, what: &dyn std::fmt::Display) {
    if cfg!(debug_assertions) {
        if elapsed > limit {
            eprintln!("（debug，不斷言）{what} 花了 {elapsed:?}");
        }
    } else {
        assert!(elapsed < limit, "{what} took {elapsed:?}");
    }
}

fn patterns() -> Vec<(String, &'static str)> {
    vec![
        (String::from(r"!?\[[^\]\n]*\]\("), ""),
        (String::from(r"!?\[[^\]\n]*\]\[[^\]\n]*\]"), ""),
        (
            String::from(r"(?<![*\w])\*([ \t]+)([^*\n]*?\S)\*(?![*\w])"),
            "",
        ),
        (
            String::from(r"(?<![*\w])\*(\S[^*\n]*?)([ \t]+)\*(?![*\w])"),
            "",
        ),
        (
            String::from(r"(?<![\w_])__([ \t]+)([^_\n]*?\S)([ \t]*)__(?![\w_])"),
            "",
        ),
        (
            String::from(r"(?<![\w_])__(\S[^_\n]*?)([ \t]+)__(?![\w_])"),
            "",
        ),
        (
            String::from(r"(?<![\w_])_([ \t]+)([^_\n]*?\S)_(?![\w_])"),
            "",
        ),
        (
            String::from(r"(?<![\w_])_(\S[^_\n]*?)([ \t]+)_(?![\w_])"),
            "",
        ),
        (
            String::from(r"(?<![\w*])\*\*([ \t]+)([^*\n]*?\S)([ \t]*)\*\*(?!\*)"),
            "",
        ),
        (
            String::from(r"(?<![\w*])\*\*(\S[^*\n]*?)([ \t]+)\*\*(?!\*)"),
            "",
        ),
        (String::from(r"(?<!\\)\*"), ""),
        (String::from(r"\]\[[^\]\n]*\]"), ""),
        (String::from(r"\$(?![\s$])[^$\n]*?(?<![\s\\])\$"), ""),
        (String::from(r"\$\$[\s\S]+?\$\$"), ""),
        (
            String::from(r"\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b"),
            "i",
        ),
        (String::from(r"\s+"), ""),
        (
            String::from(r"^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$"),
            "",
        ),
        (String::from(r"^ {0,3}(`{3,}|~{3,})[ \t]*$"), ""),
        (String::from(r"^ {0,3}\|"), ""),
        (String::from(r"^ {0,3}#"), ""),
        (String::from(r"^ {0,3}#{1,6}(?:[ \t]|$)"), ""),
        (String::from(r"^ {0,3}<!--"), ""),
        (String::from(r"^ {0,3}<![A-Za-z]"), ""),
        (String::from(r"^ {0,3}<!\[CDATA\["), ""),
        (String::from(r"^ {0,3}<\?"), ""),
        (
            String::from(r"^ {0,3}<\/?[A-Za-z][\w-]*(?:\s+[^>\n]*)?\/?>[ \t]*$"),
            "",
        ),
        (String::from(r"^---[ \t]*$"), ""),
        (String::from(r"^:?-+:?$"), ""),
        (String::from(r"^( {0,3})([-*+])([ \t]+)(.*)$"), ""),
        (
            String::from(r"^( {0,3})([-*+]|\d{1,9}[.)])([ \t]+)(.*)$"),
            "",
        ),
        (String::from(r"^( {0,3})([-+])(\S.*)$"), ""),
        (String::from(r"^( {0,3})(\*)(\S.*)$"), ""),
        (String::from(r"^( {0,3})(\d{1,9})([.)])([^\d\s].*)$"), ""),
        (String::from(r"^( {0,3})(`{3,}|~{3,})(.*)$"), ""),
        (String::from(r"^( {0,3})\$\$(.*)$"), ""),
        (
            String::from(r"^(?: {0,3})(?:[-*+]|\d{1,9}[.)])[ \t]+\S"),
            "",
        ),
        (
            String::from(r"^(?: {0,3}>[ \t]?)* {0,3}\[([^\]\n]+)\]:[ \t]*(?:<[^>\n]*>|\S+)"),
            "",
        ),
        (String::from(r"^(?:---|\.\.\.)[ \t]*$"), ""),
        (String::from(r"^(.*?)[ \t]+(#+)[ \t]*$"), ""),
        (String::from(r"^([ \t]*)(:?)(-{1,2})(:?)([ \t]*)$"), ""),
        (String::from(r"^[A-Za-z]$"), ""),
        (String::from(r"^[A-Za-z0-9_.-]+[ \t]*:"), ""),
        (String::from(r"^\[[ xX]\][ \t]+(.*)$"), ""),
        (String::from(r"^#{1,6}[ \t]"), ""),
        (
            format!(r"^ {{0,3}}<({})(?=[\s>])", crate::scan::RAW_HTML_TAGS),
            "i",
        ),
        (
            format!(
                r"^ {{0,3}}</?(?:{})(?=[\s/>]|$)",
                crate::scan::HTML_BLOCK_TAGS
            ),
            "i",
        ),
        (String::from(r"</script\s*>"), "i"),
        (
            String::from(r#"\b(?:https?:\/\/|ftp:\/\/|www\.)(?:[^\s<>"']|\u00A0)+"#),
            "i",
        ),
    ]
}

/// 約 10k code unit 的病態輸入。
fn inputs() -> Vec<String> {
    const N: usize = 10_000;
    let units = [
        "> ", ">", " ", "\t", "*", "**", "* ", "** ", "_", "__ ", "-", "- ", "#", " #", "[", "]",
        "](", "](<", "$", "$$", "$ ", "`", "``", "<", "< ", "a", "a.", "a@", "@a.", "|", ":", "\\",
        "1.", "~", "\u{a0}", "😀", "> [", "\" ", "' ", "](\"", "http://", "www.a",
    ];
    let mut out = Vec::new();
    for unit in units {
        let reps = N / unit.chars().count().max(1);
        let body = unit.repeat(reps);
        out.push(body.clone());
        out.push(format!("{body}q"));
        out.push(format!("q{body}"));
        out.push(format!("**{body}x"));
        out.push(format!("<a{body}"));
        out.push(format!("# a{body}x"));
        out.push(format!("a@{body}"));
        out.push(format!("{body}**"));
    }
    out.push(format!("{}q", "> ".repeat(40)));
    out.push(format!("{}q", ">".repeat(5_000)));
    out.push(format!("** {}a", " ".repeat(N)));
    out.push(format!("a{}x", " ".repeat(N)));
    out.push(format!("x{}#y", " ".repeat(N)));
    out.push(format!("<a{}x", " ".repeat(N)));
    out.push("a.".repeat(N / 2) + "@" + &"a.".repeat(N / 4));
    out
}

#[test]
fn every_ported_regex_is_bounded_on_pathological_input() {
    let inputs: Vec<Vec<u16>> = inputs().iter().map(|s| u(s)).collect();
    let mut worst = (Duration::ZERO, String::new(), 0usize);
    for (pattern, flags) in patterns() {
        let re = Regex::new(&pattern, flags).expect("pattern compiles");
        for (k, input) in inputs.iter().enumerate() {
            let started = Instant::now();
            let _ = re.find_iter(input);
            let elapsed = started.elapsed();
            if elapsed > worst.0 {
                worst = (elapsed, pattern.clone(), k);
            }
            check_time(
                elapsed,
                LIMIT,
                &format!("regex {pattern:?} on input #{k} (len {})", input.len()),
            );
        }
    }
    // `<[^>\n]{1,500}>` 已改由線性的 `mask::angle_spans` 實作，改測整個 `build_mask`。
    for (k, input) in inputs.iter().enumerate() {
        let started = Instant::now();
        let _ = crate::mask::build_mask(input, crate::mask::MaskOptions::default());
        check_time(
            started.elapsed(),
            LIMIT,
            &format!("build_mask on input #{k}"),
        );
    }
    eprintln!("slowest: {:?} {:?} input #{}", worst.0, worst.1, worst.2);
}

#[test]
fn deep_blockquote_definition_scan_is_fast() {
    // 原本會指數級回溯的形狀（oracle 在 40 層時卡住）。
    let line = u(&format!("{}q", "> ".repeat(5_000)));
    let started = Instant::now();
    assert_eq!(crate::scan::match_definition(&line), None);
    check_time(started.elapsed(), LIMIT, &"match_definition");
    let line = u(&format!("{}[x]: y", "> ".repeat(5_000)));
    assert_eq!(crate::scan::match_definition(&line), Some((10_001, 10_002)));
}

#[test]
fn deep_nesting_fix_document_is_fast() {
    let text = format!("{} 深層引言\n", ">".repeat(400));
    let started = Instant::now();
    let _ = crate::fix_document(&text, &crate::Options::default());
    check_time(started.elapsed(), LIMIT * 4, &"deep nesting fix_document");
}

#[test]
#[ignore = "效能量測：cargo test --release -p markdown-fixer --lib -- --ignored one_megabyte --nocapture"]
fn one_megabyte_fix_document() {
    let mut chunk = String::new();
    for dir in ["../../fixtures/corpus", "../../docs"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                chunk.push_str(&text);
                chunk.push('\n');
            }
        }
    }
    if chunk.is_empty() {
        chunk =
            "# 標題\n\n一段**粗體：**文字 https://example.com 與 `code`。\n\n- a\n- b\n\n".into();
    }
    let mut text = String::new();
    while text.len() < 1_000_000 {
        text.push_str(&chunk);
    }
    let started = Instant::now();
    let fixed = crate::fix_document(&text, &crate::Options::default());
    let fix_time = started.elapsed();
    let started = Instant::now();
    let problems = crate::diagnose(&text, &crate::Options::default());
    eprintln!(
        "{} bytes: fix_document {:?} (changed {}, refused {}), diagnose {:?} ({} problems)",
        text.len(),
        fix_time,
        fixed.changed,
        fixed.refused,
        started.elapsed(),
        problems.len()
    );
}

#[test]
#[ignore = "效能剖析：cargo test --release -p markdown-fixer --lib -- --ignored phase_profile --nocapture"]
fn phase_profile() {
    use crate::jsstr::u;
    let mut chunk = String::new();
    if let Ok(entries) = std::fs::read_dir("../../docs") {
        for entry in entries.flatten() {
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                chunk.push_str(&text);
            }
        }
    }
    let mut text = String::new();
    while text.len() < 1_000_000 {
        text.push_str(&chunk);
    }
    let t = u(&text);
    let enabled = crate::rules::Enabled::new(&crate::Options::default());
    let time = |name: &str, f: &mut dyn FnMut()| {
        let s = Instant::now();
        f();
        eprintln!("{name:>28}: {:?}", s.elapsed());
    };
    let scan = crate::scan::scan_document(&t);
    time("scan_document", &mut || {
        let _ = crate::scan::scan_document(&t);
    });
    time("build_mask default", &mut || {
        for l in &scan.lines {
            let _ = crate::mask::build_mask(l, crate::mask::MaskOptions::default());
        }
    });
    time("transform_line", &mut || {
        for (i, (l, info)) in scan.lines.iter().zip(&scan.info).enumerate() {
            let _ = crate::line_fix::transform_line(l, info, i + 1, &enabled);
        }
    });
    time("fix_heading", &mut || {
        for l in &scan.lines {
            let _ = crate::line_fix::fix_heading(l, &enabled);
        }
    });
    time("fix_emphasis", &mut || {
        for l in &scan.lines {
            let _ = crate::line_fix::fix_emphasis(l, &enabled);
        }
    });
    time("cjk apply", &mut || {
        let _ = crate::cjk_emphasis::apply_cjk_emphasis_fixes(&t, &enabled);
    });
    time("table apply", &mut || {
        let _ = crate::table::apply_table_delimiter_fixes(&t, &enabled);
    });
    time("list style", &mut || {
        let _ = crate::blank_lines::fix_list_style(&t, &enabled);
    });
    time("blank fixes", &mut || {
        let _ = crate::blank_lines::apply_blank_line_fixes(&t, &enabled);
    });
    time("trailing ws", &mut || {
        let _ = crate::blank_lines::apply_trailing_whitespace(&t, &enabled);
    });
    time("normalization", &mut || {
        let _ = crate::blank_lines::apply_blank_line_normalization(&t, &enabled);
    });
}

#[test]
fn pathological_documents_are_fast() {
    let cases = [
        format!("{}q", "> ".repeat(40)),
        format!("{}q\n", ">".repeat(400)),
        "[](".repeat(3_333),
        "**".repeat(5_000),
        "* ".repeat(5_000) + "x",
        "`".repeat(10_000),
        "$ ".repeat(5_000),
        "<".repeat(10_000),
        "a@".repeat(5_000),
        "_ ".repeat(5_000) + "_",
        "# ".repeat(5_000) + "x",
        "|".repeat(10_000) + "\n" + &"|-".repeat(5_000),
        // review-2：只能 close 的強調 run 曾讓 MDX042 的配對退化成平方時間。
        "a* ".repeat(32_000) + "a\n",
        "甲* ".repeat(32_000) + "甲\n",
        "a** ".repeat(24_000) + "a\n",
    ];
    for text in &cases {
        let started = Instant::now();
        let _ = crate::fix_document(text, &crate::Options::default());
        let fix = started.elapsed();
        let started = Instant::now();
        let _ = crate::diagnose(text, &crate::Options::default());
        let diag = started.elapsed();
        let label: String = text.chars().take(12).collect();
        eprintln!("{:>8?} {:>8?} {:?}", fix, diag, label);
        check_time(fix + diag, LIMIT * 4, &format!("{label:?}"));
    }
}
