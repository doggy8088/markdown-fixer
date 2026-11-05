//! 輸出格式：`--help`、`--list-rules`、每個問題的文字行、摘要與 `--json`。
//! 對應 oracle 的 `printHelp`、`printRules`、`formatProblem`、`summarizeProblems`
//! 與 `main` 後半段。所有輸出文字與 oracle 逐位元組相同。

use std::io::{self, Write};

use markdown_fixer::{Problem, Rule, Severity};
use serde::Serialize;

use crate::args::{CliOptions, Mode};
use crate::jsstr::pad_end;

/// CLI 版本（`--version` 與 JSON 的 `version` 欄位）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// oracle 的 `printHelp`；`Usage:` 與 `Examples:` 行把 `node markdown-fixer.mjs` 換成
/// `markdown-fixer`（PRD〈5.4〉第 3 項），另外插入 Rust 版專有旗標的區塊。
pub fn help_text() -> String {
    let lines = [
        "markdown-fixer — safe Markdown syntax checking and auto-correction",
        "",
        "Usage: markdown-fixer [options] <file-or-directory>...",
        "",
        "Modes:",
        "  --check             Report problems without writing (default).",
        "  --write             Apply safe fixes in place.",
        "",
        "Options:",
        "  --json              Emit machine-readable JSON.",
        "  --quiet             Suppress per-file problem lists.",
        "  --strict            Make info-level findings fail the run too.",
        "  --rule <ids>        Only enable these rules (comma-separated, repeatable).",
        "  --ignore-rule <ids> Disable these rules (comma-separated, repeatable).",
        "  --max-line-length N Enable MDX090 with a line-length limit.",
        "  --list-rules        Print the rule catalog and exit.",
        "  --version           Print the script version and exit.",
        "  -h, --help          Print this help and exit.",
        "",
        RUST_ONLY_HELP,
        "",
        "Exit codes:",
        "  0  No failing problems.",
        "  1  Problems remain (or would remain in --check mode).",
        "  2  Usage, I/O, or parse error.",
        "",
        "Examples:",
        "  markdown-fixer --check README.md docs/",
        "  markdown-fixer --write README.md AGENTS.md",
        "  markdown-fixer --check --json .",
        "",
        "Two trailing spaces are treated as an intentional hard line break.",
        "Fenced code, inline code, frontmatter, math, URLs, and raw HTML blocks are protected.",
    ];
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Rust 版專有旗標的說明區塊（oracle 沒有）。golden 比對時整段移除。
pub const RUST_ONLY_HELP: &str = "\
Rust-only options (not in the reference markdown-fixer.mjs):
  --stdin             Read Markdown from standard input (shown as <stdin>).
                      With --write, print the complete text to stdout and
                      the report to stderr.
  --exit-zero         Exit 0 unless a usage or I/O error occurs (exit 2).
  --detect            Only report whether standard input looks like Markdown:
                      print markdown or not-markdown (exit 0 or 1).";

/// oracle 的 `printRules`。
pub fn rules_text(rules: &[Rule]) -> String {
    let mut text = String::new();
    text.push_str("ID       Severity  Fixable  Category     Rule\n");
    text.push_str(
        "-------  --------  -------  -----------  ------------------------------------------\n",
    );
    for rule in rules {
        text.push_str(&format!(
            "{}  {}  {}  {}  {} — {}\n",
            pad_end(rule.id, 7),
            pad_end(rule.severity.as_str(), 8),
            pad_end(if rule.fixable { "yes" } else { "no" }, 7),
            pad_end(rule.category, 11),
            rule.name,
            rule.summary
        ));
    }
    text
}

/// oracle 的 `formatProblem`。
pub fn format_problem(problem: &Problem) -> String {
    let location = format!("{}:{}", problem.line, problem.column);
    let fixable = if problem.fixable { " (fixable)" } else { "" };
    format!(
        "  {} {} {}  {}{}",
        pad_end(&location, 10),
        pad_end(problem.severity.as_str(), 7),
        problem.rule,
        problem.message,
        fixable
    )
}

/// oracle 的 `summarizeProblems`；欄位順序即 JSON 欄位順序。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub fixable: usize,
}

pub fn summarize<'a>(problems: impl IntoIterator<Item = &'a Problem>) -> Summary {
    let mut summary = Summary::default();
    for problem in problems {
        summary.total += 1;
        match problem.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.info += 1,
        }
        if problem.fixable {
            summary.fixable += 1;
        }
    }
    summary
}

/// 一個檔案（或 `<stdin>`）的處理結果，對應 oracle `results` 的元素。
#[derive(Debug, Clone)]
pub struct FileResult {
    pub display: String,
    pub changed: bool,
    pub would_change: bool,
    pub refused: bool,
    pub reason: String,
    pub shown: Vec<Problem>,
    pub fixed: usize,
    pub would_fix: usize,
}

/// 是否有讓結束碼為 1 的結果。
pub fn is_failing(results: &[FileResult], strict: bool) -> bool {
    results.iter().any(|result| {
        result.refused
            || result.shown.iter().any(|problem| match problem.severity {
                Severity::Error | Severity::Warning => true,
                Severity::Info => strict,
            })
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    tool: &'static str,
    version: &'static str,
    mode: &'static str,
    files: Vec<JsonFile<'a>>,
    summary: JsonSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonFile<'a> {
    path: &'a str,
    changed: bool,
    would_change: bool,
    refused: bool,
    // JS 的 `result.reason || undefined`：空字串時省略。
    #[serde(skip_serializing_if = "str::is_empty")]
    reason: &'a str,
    fixed: usize,
    would_fix: usize,
    problems: Vec<JsonProblem<'a>>,
}

#[derive(Serialize)]
struct JsonProblem<'a> {
    rule: &'a str,
    name: &'a str,
    severity: &'static str,
    fixable: bool,
    line: usize,
    column: usize,
    message: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSummary {
    files: usize,
    changed: usize,
    would_change: usize,
    fixed: usize,
    refused: usize,
    problems: Summary,
}

/// `JSON.stringify(output, null, 2)`，不含結尾換行。
pub fn json_report(results: &[FileResult], mode: Mode) -> String {
    let output = JsonOutput {
        tool: "markdown-fixer",
        version: VERSION,
        mode: mode.as_str(),
        files: results
            .iter()
            .map(|result| JsonFile {
                path: &result.display,
                changed: result.changed,
                would_change: result.would_change,
                refused: result.refused,
                reason: &result.reason,
                fixed: result.fixed,
                would_fix: result.would_fix,
                problems: result
                    .shown
                    .iter()
                    .map(|problem| JsonProblem {
                        rule: problem.rule,
                        name: problem.name,
                        severity: problem.severity.as_str(),
                        fixable: problem.fixable,
                        line: problem.line,
                        column: problem.column,
                        message: &problem.message,
                    })
                    .collect(),
            })
            .collect(),
        summary: JsonSummary {
            files: results.len(),
            changed: results.iter().filter(|r| r.changed).count(),
            would_change: results.iter().filter(|r| r.would_change).count(),
            fixed: results.iter().map(|r| r.fixed).sum(),
            refused: results.iter().filter(|r| r.refused).count(),
            problems: summarize(results.iter().flat_map(|r| r.shown.iter())),
        },
    };
    // 這些型別的序列化不會失敗；萬一失敗就輸出空物件，不 panic。
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

/// oracle `main` 的文字報告：每個檔案的問題清單（`--quiet` 時略過）與摘要行。
pub fn write_text_report(
    out: &mut dyn Write,
    results: &[FileResult],
    options: &CliOptions,
    io_error: bool,
) -> io::Result<()> {
    let write_mode = options.mode == Mode::Write;
    if !options.quiet {
        for result in results {
            writeln!(out, "{}", result.display)?;
            if result.refused {
                let prefix = if write_mode {
                    "Not modified"
                } else {
                    "Structural error"
                };
                writeln!(out, "  ERROR  {prefix}: {}", result.reason)?;
            }
            if result.shown.is_empty() {
                if !result.refused {
                    writeln!(out, "  No problems.")?;
                }
            } else {
                for problem in &result.shown {
                    writeln!(out, "{}", format_problem(problem))?;
                }
            }
            if write_mode && result.changed && !result.refused {
                writeln!(out, "  Fixed {} issue(s).", result.fixed)?;
            }
            writeln!(out)?;
        }
    }

    let summary = summarize(results.iter().flat_map(|r| r.shown.iter()));
    let changed_count = results.iter().filter(|r| r.changed).count();
    let fixed_count: usize = results.iter().map(|r| r.fixed).sum();
    let refused_count = results.iter().filter(|r| r.refused).count();

    if write_mode {
        if fixed_count > 0 || changed_count > 0 {
            writeln!(
                out,
                "Fixed {fixed_count} issue(s) in {changed_count} file(s)."
            )?;
        }
        if summary.total > 0 {
            writeln!(
                out,
                "{} problem(s) remain: {} fixable, {} error(s), {} warning(s), {} info.",
                summary.total, summary.fixable, summary.errors, summary.warnings, summary.info
            )?;
        } else if !io_error && refused_count == 0 {
            writeln!(out, "No remaining Markdown issues.")?;
        }
    } else if summary.total > 0 {
        writeln!(
            out,
            "{} problem(s) in {} file(s): {} fixable, {} error(s), {} warning(s), {} info.",
            summary.total,
            results.len(),
            summary.fixable,
            summary.errors,
            summary.warnings,
            summary.info
        )?;
    } else if !io_error && refused_count == 0 {
        writeln!(
            out,
            "No Markdown issues found in {} file(s).",
            results.len()
        )?;
    }

    if io_error {
        writeln!(
            out,
            "Some files could not be read or written; see the errors above."
        )?;
    }

    if refused_count > 0 {
        if write_mode {
            writeln!(
                out,
                "{refused_count} file(s) were not modified because of structural or frontmatter ambiguity."
            )?;
        } else {
            writeln!(
                out,
                "{refused_count} file(s) need manual review before safe fixes."
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(
        rule: &'static str,
        severity: Severity,
        fixable: bool,
        line: usize,
        column: usize,
    ) -> Problem {
        Problem {
            rule,
            name: "name",
            severity,
            fixable,
            line,
            column,
            message: "Message \"quoted\" 中文.".to_string(),
        }
    }

    fn result(display: &str, shown: Vec<Problem>) -> FileResult {
        FileResult {
            display: display.to_string(),
            changed: false,
            would_change: false,
            refused: false,
            reason: String::new(),
            shown,
            fixed: 0,
            would_fix: 0,
        }
    }

    #[test]
    fn formats_problem_like_oracle() {
        let p = problem("MDX010", Severity::Warning, true, 1, 1);
        assert_eq!(
            format_problem(&p),
            "  1:1        warning MDX010  Message \"quoted\" 中文. (fixable)"
        );
        let p = problem("MDX090", Severity::Info, false, 12345, 678901);
        assert_eq!(
            format_problem(&p),
            "  12345:678901 info    MDX090  Message \"quoted\" 中文."
        );
    }

    #[test]
    fn summarizes() {
        let problems = [
            problem("MDX010", Severity::Warning, true, 1, 1),
            problem("MDX051", Severity::Error, false, 2, 1),
            problem("MDX070", Severity::Info, false, 3, 1),
        ];
        assert_eq!(
            summarize(&problems),
            Summary {
                total: 3,
                errors: 1,
                warnings: 1,
                info: 1,
                fixable: 1
            }
        );
    }

    #[test]
    fn json_matches_oracle_shape() {
        let mut refused = result(
            "b.md",
            vec![problem("MDX081", Severity::Error, false, 1, 1)],
        );
        refused.refused = true;
        refused.reason = "Frontmatter is not closed.".to_string();
        let results = [
            result(
                "a.md",
                vec![problem("MDX010", Severity::Warning, true, 1, 1)],
            ),
            refused,
            result("c.md", Vec::new()),
        ];
        let json = json_report(&results, Mode::Check);
        let expected = format!(
            r#"{{
  "tool": "markdown-fixer",
  "version": "{VERSION}",
  "mode": "check",
  "files": [
    {{
      "path": "a.md",
      "changed": false,
      "wouldChange": false,
      "refused": false,
      "fixed": 0,
      "wouldFix": 0,
      "problems": [
        {{
          "rule": "MDX010",
          "name": "name",
          "severity": "warning",
          "fixable": true,
          "line": 1,
          "column": 1,
          "message": "Message \"quoted\" 中文."
        }}
      ]
    }},
    {{
      "path": "b.md",
      "changed": false,
      "wouldChange": false,
      "refused": true,
      "reason": "Frontmatter is not closed.",
      "fixed": 0,
      "wouldFix": 0,
      "problems": [
        {{
          "rule": "MDX081",
          "name": "name",
          "severity": "error",
          "fixable": false,
          "line": 1,
          "column": 1,
          "message": "Message \"quoted\" 中文."
        }}
      ]
    }},
    {{
      "path": "c.md",
      "changed": false,
      "wouldChange": false,
      "refused": false,
      "fixed": 0,
      "wouldFix": 0,
      "problems": []
    }}
  ],
  "summary": {{
    "files": 3,
    "changed": 0,
    "wouldChange": 0,
    "fixed": 0,
    "refused": 1,
    "problems": {{
      "total": 2,
      "errors": 1,
      "warnings": 1,
      "info": 0,
      "fixable": 1
    }}
  }}
}}"#
        );
        assert_eq!(json, expected);
    }

    #[test]
    fn empty_json_matches_oracle_literal() {
        let json = json_report(&[], Mode::Write);
        assert!(json.contains("\"mode\": \"write\""));
        assert!(json.contains("\"files\": [],"));
        assert!(json.contains("\"total\": 0,"));
    }

    #[test]
    fn json_escapes_like_js() {
        let mut p = problem("MDX009", Severity::Warning, false, 1, 1);
        p.message = "a\u{1}b\u{7f}c\u{2028}d\te\\".to_string();
        let json = json_report(&[result("x.md", vec![p])], Mode::Check);
        assert!(json.contains(r#""message": "a\u0001b"#));
        assert!(json.contains("b\u{7f}c\u{2028}d\\te\\\\\""));
    }

    #[test]
    fn text_report_check_and_write() {
        let mut refused = result(
            "b.md",
            vec![problem("MDX081", Severity::Error, false, 1, 1)],
        );
        refused.refused = true;
        refused.reason = "Reason.".to_string();
        let results = [
            result("a.md", vec![problem("MDX070", Severity::Info, false, 1, 7)]),
            refused,
        ];
        let mut out = Vec::new();
        write_text_report(&mut out, &results, &CliOptions::default(), false).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "a.md\n  1:7        info    MDX070  Message \"quoted\" 中文.\n\n\
             b.md\n  ERROR  Structural error: Reason.\n  1:1        error   MDX081  Message \"quoted\" 中文.\n\n\
             2 problem(s) in 2 file(s): 0 fixable, 1 error(s), 0 warning(s), 1 info.\n\
             1 file(s) need manual review before safe fixes.\n"
        );

        let mut fixed = result("a.md", Vec::new());
        fixed.changed = true;
        fixed.fixed = 2;
        let options = CliOptions {
            mode: Mode::Write,
            ..CliOptions::default()
        };
        let mut out = Vec::new();
        write_text_report(&mut out, &[fixed], &options, false).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "a.md\n  No problems.\n  Fixed 2 issue(s).\n\nFixed 2 issue(s) in 1 file(s).\nNo remaining Markdown issues.\n"
        );

        let quiet = CliOptions {
            quiet: true,
            ..CliOptions::default()
        };
        let mut out = Vec::new();
        write_text_report(&mut out, &[result("a.md", Vec::new())], &quiet, true).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Some files could not be read or written; see the errors above.\n"
        );
    }

    #[test]
    fn help_keeps_oracle_lines() {
        let help = help_text();
        assert!(help.starts_with(
            "markdown-fixer — safe Markdown syntax checking and auto-correction\n\nUsage: markdown-fixer [options] <file-or-directory>...\n"
        ));
        assert!(help.ends_with("raw HTML blocks are protected.\n"));
        assert!(!help.contains("node markdown-fixer.mjs"));
        assert!(help.contains("\nExamples:\n  markdown-fixer --check README.md docs/\n"));
        assert!(help.contains("\n\nRust-only options"));
    }
}
