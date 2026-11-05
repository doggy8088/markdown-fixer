//! Rust 版專有旗標（PRD〈6.4.2〉）的整合測試：`--stdin`、`--exit-zero`、`--detect`。

mod common;

use common::{TempDir, run};
use serde_json::Value;

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

const MESSY: &str = "#Title\nBody with trailing space   \n\n\n- a\n* b\n";
const REFUSED: &str = "---\ntitle: x\n#Title\n";

// ---- --stdin 與路徑、--detect 與路徑的衝突 ----

#[test]
fn stdin_cannot_be_combined_with_paths() {
    let tmp = TempDir::new("stdin-paths");
    tmp.write("a.md", "# A\n");
    let out = run(&tmp.path, &["--stdin", "a.md"], Some(b"# A\n"));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
    assert_eq!(
        text(&out.stderr),
        "Error: --stdin cannot be combined with file or directory arguments.\nRun with --help for usage.\n"
    );
}

#[test]
fn detect_cannot_be_combined_with_paths() {
    let tmp = TempDir::new("detect-paths");
    let out = run(&tmp.path, &["--detect", "a.md"], Some(b"# A\n"));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
    assert!(text(&out.stderr).starts_with("Error: --detect "));
}

#[test]
fn help_lists_rust_only_flags_in_a_separate_block() {
    let tmp = TempDir::new("help");
    let out = run(&tmp.path, &["--help"], None);
    assert_eq!(out.status.code(), Some(0));
    let help = text(&out.stdout);
    let block = help
        .split("\n\n")
        .find(|block| block.starts_with("Rust-only options"))
        .expect("Rust-only block");
    for flag in ["--stdin", "--exit-zero", "--detect"] {
        assert!(block.contains(flag), "{flag} missing from {block}");
    }
    // --help 優先於 --stdin，不讀標準輸入。
    let out = run(&tmp.path, &["--stdin", "--help"], None);
    assert_eq!(out.status.code(), Some(0));
}

// ---- --stdin 的輸入錯誤：結束碼 2、stdout 不輸出任何東西 ----

#[test]
fn stdin_invalid_utf8_exits_2_with_empty_stdout() {
    let tmp = TempDir::new("stdin-utf8");
    for args in [
        &["--stdin"][..],
        &["--stdin", "--write"],
        &["--stdin", "--write", "--quiet", "--exit-zero"],
        &["--detect"],
    ] {
        let out = run(&tmp.path, args, Some(&[0x23, 0x20, 0xff, 0xfe, b'\n']));
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert_eq!(
            text(&out.stderr),
            "Error: <stdin>: The encoded data was not valid for encoding utf-8\n"
        );
    }
}

#[test]
fn stdin_nul_bytes_exit_2_with_empty_stdout() {
    let tmp = TempDir::new("stdin-nul");
    let out = run(&tmp.path, &["--stdin", "--write"], Some(b"# T\0\n"));
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_eq!(
        text(&out.stderr),
        "Error: <stdin>: file contains NUL bytes; refusing to treat it as text\n"
    );
}

// ---- --stdin --check：與檔案模式相同的輸出，只是顯示名稱為 <stdin> ----

/// 以檔案模式跑同樣的內容，把顯示名稱換成 `<stdin>` 後應與 `--stdin` 輸出相同。
fn assert_same_as_file_mode(contents: &str, extra: &[&str]) {
    let tmp = TempDir::new("stdin-vs-file");
    tmp.write("doc.md", contents);
    let mut file_args = extra.to_vec();
    file_args.push("doc.md");
    let file_out = run(&tmp.path, &file_args, None);
    let mut stdin_args = extra.to_vec();
    stdin_args.push("--stdin");
    let stdin_out = run(&tmp.path, &stdin_args, Some(contents.as_bytes()));
    let expected = text(&file_out.stdout)
        .replace("\"path\": \"doc.md\"", "\"path\": \"<stdin>\"")
        .replace("doc.md\n", "<stdin>\n");
    assert_eq!(text(&stdin_out.stdout), expected, "{extra:?}");
    assert_eq!(stdin_out.status.code(), file_out.status.code(), "{extra:?}");
    assert_eq!(text(&stdin_out.stderr), "");
}

#[test]
fn stdin_check_matches_file_mode() {
    for contents in [MESSY, REFUSED, "# Clean\n", "Bare https://example.com.\n"] {
        assert_same_as_file_mode(contents, &[]);
        assert_same_as_file_mode(contents, &["--check", "--json"]);
        assert_same_as_file_mode(contents, &["--quiet"]);
        assert_same_as_file_mode(contents, &["--strict"]);
        assert_same_as_file_mode(contents, &["--rule", "MDX010,MDX004"]);
    }
}

// ---- --stdin --write：完整文字到 stdout，報告到 stderr ----

#[test]
fn stdin_write_prints_fixed_text_and_reports_to_stderr() {
    let tmp = TempDir::new("stdin-write");
    tmp.write("doc.md", MESSY);
    let file_out = run(&tmp.path, &["--write", "doc.md"], None);
    let fixed = std::fs::read_to_string(tmp.join("doc.md")).unwrap();

    let out = run(&tmp.path, &["--stdin", "--write"], Some(MESSY.as_bytes()));
    assert_eq!(text(&out.stdout), fixed);
    assert_eq!(out.status.code(), file_out.status.code());
    assert_eq!(
        text(&out.stderr),
        text(&file_out.stdout).replace("doc.md\n", "<stdin>\n")
    );
}

#[test]
fn stdin_write_quiet_prints_only_text() {
    let tmp = TempDir::new("stdin-quiet");
    let out = run(
        &tmp.path,
        &["--stdin", "--write", "--quiet"],
        Some(b"#Title\n"),
    );
    assert_eq!(text(&out.stdout), "# Title\n");
    assert_eq!(text(&out.stderr), "");
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn stdin_write_json_report_goes_to_stderr() {
    let tmp = TempDir::new("stdin-json");
    let out = run(
        &tmp.path,
        &["--stdin", "--write", "--json"],
        Some(b"#Title\n"),
    );
    assert_eq!(text(&out.stdout), "# Title\n");
    let report: Value = serde_json::from_slice(&out.stderr).expect("json on stderr");
    assert_eq!(report["mode"], "write");
    assert_eq!(report["files"][0]["path"], "<stdin>");
    assert_eq!(report["files"][0]["changed"], true);
    assert_eq!(report["summary"]["changed"], 1);
}

#[test]
fn stdin_write_unchanged_text_is_echoed_byte_for_byte() {
    let tmp = TempDir::new("stdin-unchanged");
    // 片段：沒有檔尾換行，搭配 --ignore-rule MDX003（剪貼簿用法）。
    for input in ["# Title\n\nBody.\n", "plain snippet without newline", ""] {
        let out = run(
            &tmp.path,
            &[
                "--stdin",
                "--write",
                "--quiet",
                "--exit-zero",
                "--ignore-rule",
                "MDX003",
            ],
            Some(input.as_bytes()),
        );
        assert_eq!(out.status.code(), Some(0), "{input:?}");
        assert_eq!(text(&out.stdout), input);
        assert_eq!(text(&out.stderr), "");
    }
}

#[test]
fn stdin_write_preserves_bom_and_crlf_when_rules_are_disabled() {
    let tmp = TempDir::new("stdin-crlf");
    let input = "\u{FEFF}#Title\r\nBody\r\n";
    let out = run(
        &tmp.path,
        &["--stdin", "--write", "--ignore-rule=MDX001,MDX002"],
        Some(input.as_bytes()),
    );
    assert_eq!(text(&out.stdout), "\u{FEFF}# Title\r\n\r\nBody\r\n");
}

#[test]
fn stdin_write_refused_prints_original_and_reason() {
    let tmp = TempDir::new("stdin-refused");
    let out = run(&tmp.path, &["--stdin", "--write"], Some(REFUSED.as_bytes()));
    assert_eq!(text(&out.stdout), REFUSED);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains(
        "  ERROR  Not modified: Unclosed YAML frontmatter: no closing --- or ... line was found."
    ));

    let quiet = run(
        &tmp.path,
        &["--stdin", "--write", "--quiet", "--exit-zero"],
        Some(REFUSED.as_bytes()),
    );
    assert_eq!(text(&quiet.stdout), REFUSED);
    assert_eq!(quiet.status.code(), Some(0));
    assert_eq!(
        text(&quiet.stderr),
        "<stdin>: Not modified: Unclosed YAML frontmatter: no closing --- or ... line was found.\n"
    );
}

#[test]
fn stdin_write_does_not_need_paths() {
    let tmp = TempDir::new("stdin-nopaths");
    let out = run(&tmp.path, &["--write", "--stdin"], Some(b"# T\n"));
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(text(&out.stdout), "# T\n");
}

// ---- --exit-zero ----

#[test]
fn exit_zero_turns_failures_into_success() {
    let tmp = TempDir::new("exit-zero");
    tmp.write("doc.md", MESSY);
    let normal = run(&tmp.path, &["doc.md"], None);
    assert_eq!(normal.status.code(), Some(1));
    let zero = run(&tmp.path, &["--exit-zero", "doc.md"], None);
    assert_eq!(zero.status.code(), Some(0));
    assert_eq!(zero.stdout, normal.stdout);

    tmp.write("refused.md", REFUSED);
    let refused = run(&tmp.path, &["--exit-zero", "--write", "refused.md"], None);
    assert_eq!(refused.status.code(), Some(0));

    let strict = run(
        &tmp.path,
        &["--exit-zero", "--stdin", "--strict"],
        Some(b"Bare https://example.com.\n"),
    );
    assert_eq!(strict.status.code(), Some(0));
}

#[test]
fn exit_zero_keeps_usage_and_io_errors() {
    let tmp = TempDir::new("exit-zero-errors");
    assert_eq!(
        run(&tmp.path, &["--exit-zero", "--bogus"], None)
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&tmp.path, &["--exit-zero", "missing.md"], None)
            .status
            .code(),
        Some(2)
    );
    tmp.write("bad.md", [0xff]);
    assert_eq!(
        run(&tmp.path, &["--exit-zero", "bad.md"], None)
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&tmp.path, &["--exit-zero", "--write"], None)
            .status
            .code(),
        Some(2)
    );
}

// ---- --detect ----

const MARKDOWN: &str = "# Project\n\nSome text with a [link](https://example.com).\n\n- one\n- two\n\n```sh\nmake\n```\n";
const PYTHON: &str = "#!/usr/bin/env python3\nimport sys\n\n\ndef main():\n    print(sys.argv)\n\n\nif __name__ == '__main__':\n    main()\n";

#[test]
fn detect_prints_markdown_or_not_markdown() {
    let tmp = TempDir::new("detect");
    let yes = run(&tmp.path, &["--detect"], Some(MARKDOWN.as_bytes()));
    assert_eq!(text(&yes.stdout), "markdown\n");
    assert_eq!(yes.status.code(), Some(0));

    let no = run(&tmp.path, &["--detect"], Some(PYTHON.as_bytes()));
    assert_eq!(text(&no.stdout), "not-markdown\n");
    assert_eq!(no.status.code(), Some(1));

    let no_zero = run(
        &tmp.path,
        &["--detect", "--exit-zero"],
        Some(PYTHON.as_bytes()),
    );
    assert_eq!(text(&no_zero.stdout), "not-markdown\n");
    assert_eq!(no_zero.status.code(), Some(0));

    // --stdin 可有可無。
    let with_stdin = run(
        &tmp.path,
        &["--stdin", "--detect"],
        Some(MARKDOWN.as_bytes()),
    );
    assert_eq!(text(&with_stdin.stdout), "markdown\n");
}

#[test]
fn detect_json_outputs_detection() {
    let tmp = TempDir::new("detect-json");
    let out = run(
        &tmp.path,
        &["--detect", "--json"],
        Some(MARKDOWN.as_bytes()),
    );
    assert_eq!(out.status.code(), Some(0));
    let detection: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(detection["is_markdown"], true);
    assert!(detection["score"].as_i64().unwrap_or(0) >= 3);
    let signals = detection["signals"].as_array().expect("signals");
    assert!(!signals.is_empty());
    assert!(text(&out.stdout).ends_with("}\n"));
}
