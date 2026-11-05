//! `diagnose`：JS `diagnose(text, options)`（1992–2097 行）與 `dedupeProblems`（1931 行）的移植。
//!
//! 問題清單的串接順序必須與 JS 完全相同：`dedupeProblems` 用穩定排序，
//! 同一行同一欄同一規則的問題會保留串接時的先後。

use std::collections::HashSet;

use crate::cjk_emphasis::compute_cjk_emphasis;
use crate::jsregex::re;
use crate::jsstr::{JsString, join_lines, u};
use crate::line_fix::transform_line;
use crate::mask::{MaskOptions, build_mask, split_masked, url_re};
use crate::rules::{Enabled, Overrides, make_problem};
use crate::scan::{LineInfo, Scan, is_protected_line, normalize_for_analysis, scan_document};
use crate::structure::{
    compute_blank_line_runs, compute_fullwidth_syntax, compute_heading_structure,
    compute_line_length, compute_list_structure, compute_nbsp_in_urls, compute_reference_structure,
    compute_setext_ambiguity, compute_table_structure, compute_trailing_whitespace,
    compute_zero_width, index_of_tab,
};
use crate::{Options, Problem, Severity};

const CR: u16 = b'\r' as u16;
const LF: u16 = b'\n' as u16;
const TAB: u16 = b'\t' as u16;

/// `computeBlankLineFixes(lines, info, scan, isEnabled).issues`。
fn blank_line_fix_issues(
    lines: &[JsString],
    info: &[LineInfo],
    scan: &Scan,
    enabled: &Enabled,
) -> Vec<Problem> {
    crate::blank_lines::compute_blank_line_fixes(lines, info, scan, enabled).issues
}

/// JS `dedupeProblems`：以 `rule:line:column:message` 去重，再依行、欄、規則穩定排序。
pub(crate) fn dedupe_problems(problems: Vec<Problem>) -> Vec<Problem> {
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(problems.len());
    for problem in problems {
        let key = (
            problem.rule,
            problem.line,
            problem.column,
            problem.message.clone(),
        );
        if seen.insert(key) {
            result.push(problem);
        }
    }
    // 規則 ID 都是 ASCII，`localeCompare` 等同位元組比較；`sort_by` 是穩定排序。
    result.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then(a.column.cmp(&b.column))
            .then(a.rule.cmp(b.rule))
    });
    result
}

/// JS `diagnose(text, options)`。
pub(crate) fn diagnose(text: &str, options: &Options) -> Vec<Problem> {
    let enabled = Enabled::new(options);
    let raw = u(text);
    let normalized = normalize_for_analysis(&raw);
    let mut issues: Vec<Problem> = Vec::new();

    if raw.first() == Some(&0xFEFF) && enabled.is("MDX001") {
        issues.push(make_problem(
            "MDX001",
            1,
            1,
            "File starts with a UTF-8 BOM. Remove it so the first Markdown block starts at byte 0.",
            Overrides::NONE,
        ));
    }
    if let Some(first_cr) = raw.iter().position(|&c| c == CR) {
        if enabled.is("MDX002") {
            // `raw.slice(0, raw.search(/\r/)).split('\n').length`
            let line = raw[..first_cr].iter().filter(|&&c| c == LF).count() + 1;
            issues.push(make_problem(
                "MDX002",
                line,
                1,
                "File uses CRLF or CR line endings. Normalize them to LF.",
                Overrides::NONE,
            ));
        }
    }

    let scan = scan_document(&normalized);
    issues.extend(scan.issues.iter().filter(|p| enabled.is(p.rule)).cloned());

    let line_results: Vec<_> = scan
        .lines
        .iter()
        .zip(&scan.info)
        .enumerate()
        .map(|(i, (line, info))| transform_line(line, info, i + 1, &enabled))
        .collect();
    for result in &line_results {
        issues.extend(result.issues.iter().cloned());
    }

    issues.extend(compute_trailing_whitespace(
        &scan.lines,
        &scan.info,
        &enabled,
    ));
    issues.extend(compute_nbsp_in_urls(&scan.lines, &scan.info, &enabled));
    issues.extend(compute_zero_width(&scan.lines, &scan.info, &enabled));
    issues.extend(compute_blank_line_runs(&normalized, &scan, &enabled));

    let lines: Vec<JsString> = line_results.into_iter().map(|r| r.line).collect();
    let normalized_scan = scan_document(&join_lines(&lines));
    let info = &normalized_scan.info;

    issues.extend(compute_heading_structure(&lines, info, &enabled));
    issues.extend(compute_list_structure(&lines, info, &enabled));
    issues.extend(compute_table_structure(&lines, info, &enabled));
    issues.extend(compute_reference_structure(&lines, info, &enabled));
    issues.extend(compute_setext_ambiguity(&lines, info, &enabled));
    issues.extend(compute_fullwidth_syntax(&lines, info, &enabled));
    issues.extend(compute_cjk_emphasis(&lines, info, &enabled));
    issues.extend(blank_line_fix_issues(
        &lines,
        info,
        &normalized_scan,
        &enabled,
    ));
    issues.extend(compute_line_length(
        &lines,
        info,
        &enabled,
        options.max_line_length,
    ));

    if enabled.is("MDX007") {
        for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
            if is_protected_line(line_info) {
                continue;
            }
            let mask = build_mask(line, MaskOptions::NO_BARE_URLS);
            let unprotected = split_masked(line, &mask)
                .into_iter()
                .find(|s| !s.protected && s.text.contains(&TAB));
            if let Some(segment) = unprotected {
                // 找得到 segment 就一定找得到 tab；保底用 segment 起點，避免 panic。
                let tab_index = index_of_tab(line, segment.start).unwrap_or(segment.start);
                issues.push(make_problem(
                    "MDX007",
                    i + 1,
                    tab_index + 1,
                    "Tab character found. Use spaces for indentation and alignment; tabs render at different widths.",
                    Overrides::NONE,
                ));
            }
        }
    }

    if enabled.is("MDX070") {
        let mask_options = MaskOptions {
            mask_link_text: true,
            ..MaskOptions::NO_BARE_URLS
        };
        for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
            if is_protected_line(line_info) {
                continue;
            }
            let mask = build_mask(line, mask_options);
            for segment in split_masked(line, &mask) {
                if segment.protected {
                    continue;
                }
                for m in url_re().find_iter(segment.text) {
                    issues.push(make_problem(
                        "MDX070",
                        i + 1,
                        segment.start + m.start() + 1,
                        "Bare URL. Wrap it in <...> or use an explicit link only when that improves readability; otherwise leave it as text.",
                        Overrides::severity(Severity::Info),
                    ));
                }
            }
        }
    }

    if enabled.is("MDX050") {
        for (i, line_info) in info.iter().enumerate().take(lines.len()) {
            if line_info.fence_open && line_info.fence_info.is_empty() {
                issues.push(make_problem(
                    "MDX050",
                    i + 1,
                    1,
                    "Fenced code block has no language. Add one when the language is known, for example ```js.",
                    Overrides::severity(Severity::Info),
                ));
            }
        }
    }

    if enabled.is("MDX041") {
        for (i, (line, line_info)) in lines.iter().zip(info).enumerate() {
            if is_protected_line(line_info) {
                continue;
            }
            let mask = build_mask(line, MaskOptions::NO_BARE_URLS);
            let mut strong_stars = 0;
            let mut strong_unders = 0;
            let mut strikes = 0;
            for segment in split_masked(line, &mask) {
                if segment.protected {
                    continue;
                }
                strong_stars += re!(r"(?<!\*)\*\*(?!\*)").find_iter(segment.text).len();
                strong_unders += re!(r"(?<![\w_])__(?![\w_])").find_iter(segment.text).len();
                strikes += re!(r"(?<!~)~~(?!~)").find_iter(segment.text).len();
            }
            if strong_stars % 2 == 1 || strong_unders % 2 == 1 || strikes % 2 == 1 {
                issues.push(make_problem(
                    "MDX041",
                    i + 1,
                    1,
                    "Possible unmatched strong or strikethrough delimiter on this line. Verify that emphasis opens and closes outside code and links.",
                    Overrides::NONE,
                ));
            }
        }
    }

    issues.retain(|p| enabled.is(p.rule));
    dedupe_problems(issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(rules: &[&str]) -> Options {
        Options {
            rules: Some(rules.iter().map(|r| r.to_string()).collect()),
            ignore_rules: None,
            max_line_length: 10,
            ..Default::default()
        }
    }

    fn brief(problems: &[Problem]) -> Vec<String> {
        problems
            .iter()
            .map(|p| format!("{}@{}:{}", p.rule, p.line, p.column))
            .collect()
    }

    /// 預期值由 oracle 產生：`diagnose(text, { rules, maxLineLength: 10 })`。
    #[test]
    fn inline_loops() {
        let text = "a\tb `c\td`\n`x`\ty\nvisit https://a.com and www.b.org.\n[https://c.com](https://c.com)\n```\ncode\n```\n**bold\n***x** __u__ a__b ~~s~ ~~~\n";
        let problems = diagnose(text, &opts(&["MDX007", "MDX070", "MDX050", "MDX041"]));
        assert_eq!(
            brief(&problems),
            [
                "MDX007@1:2",
                "MDX007@2:4",
                "MDX070@3:7",
                "MDX070@3:25",
                "MDX050@5:1",
                "MDX041@8:1",
                "MDX041@9:1"
            ]
        );
    }

    #[test]
    fn bom_and_crlf() {
        let problems = diagnose("\u{FEFF}a\nb\r\nc", &opts(&["MDX001", "MDX002"]));
        assert_eq!(brief(&problems), ["MDX001@1:1", "MDX002@2:1"]);
    }

    #[test]
    fn dedupe_keeps_first_and_sorts_stably() {
        let a = make_problem("MDX041", 2, 1, "x", Overrides::NONE);
        let b = make_problem("MDX007", 2, 1, "y", Overrides::NONE);
        let c = make_problem("MDX041", 1, 5, "z", Overrides::NONE);
        let d = make_problem("MDX007", 2, 1, "w", Overrides::NONE);
        let out = dedupe_problems(vec![a.clone(), b.clone(), a.clone(), c.clone(), d.clone()]);
        assert_eq!(out, vec![c, b, d, a]);
    }

    /// 與 oracle 比對整份 `diagnose` 輸出的探針（平常不執行，不依賴 Node）：
    /// `MDFIX_DIAG_IN=<每行一個檔案路徑的清單> MDFIX_DIAG_OUT=<輸出檔> cargo test -p markdown-fixer --lib -- --ignored diagnose_probe`
    #[test]
    #[ignore]
    fn diagnose_probe() {
        let (Ok(input), Ok(output)) = (
            std::env::var("MDFIX_DIAG_IN"),
            std::env::var("MDFIX_DIAG_OUT"),
        ) else {
            return;
        };
        let list = std::fs::read_to_string(input).unwrap_or_default();
        let mut out = String::new();
        for path in list.lines().filter(|l| !l.is_empty()) {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            out.push_str(&format!("== {path}\n"));
            for p in diagnose(&text, &Options::default()) {
                out.push_str(&format!(
                    "{}|{}|{}|{}|{}|{}|{}\n",
                    p.rule,
                    p.name,
                    p.severity.as_str(),
                    p.fixable,
                    p.line,
                    p.column,
                    p.message
                ));
            }
        }
        let _ = std::fs::write(output, out);
    }
}
