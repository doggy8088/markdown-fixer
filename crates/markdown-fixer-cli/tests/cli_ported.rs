//! 移植自 `reference/js/markdown-fixer.test.mjs` 的 CLI 層級案例。
//! 每個測試的註解標註原案例名稱；`parseArgs` 案例在 `src/args.rs` 的單元測試。

mod common;

use common::{TempDir, run};
use serde_json::Value;

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

// 原案例：CLI reports version, rules, and check results
#[test]
fn reports_version_rules_and_check_results() {
    let tmp = TempDir::new("version");
    let version = run(&tmp.path, &["--version"], None);
    assert_eq!(version.status.code(), Some(0));
    assert_eq!(
        stdout(&version),
        format!("markdown-fixer {}\n", env!("CARGO_PKG_VERSION"))
    );

    let list = run(&tmp.path, &["--list-rules"], None);
    assert_eq!(list.status.code(), Some(0));
    let text = stdout(&list);
    assert!(text.contains("MDX001"));
    assert!(text.contains("MDX085"));
    assert!(text.contains("MDX090"));

    tmp.write("doc.md", "#Title\n");
    let check = run(&tmp.path, &["--check", "doc.md"], None);
    assert_eq!(check.status.code(), Some(1));
    assert!(stdout(&check).contains("MDX010"));
}

// 原案例：CLI writes fixes, supports JSON, and reports refusals
#[test]
fn writes_fixes_supports_json_and_reports_refusals() {
    let tmp = TempDir::new("write-json");
    let file = tmp.write("doc.md", "#Title\n");
    let json_check = run(&tmp.path, &["--check", "--json", "doc.md"], None);
    assert_eq!(json_check.status.code(), Some(1));
    let check: Value = serde_json::from_slice(&json_check.stdout).expect("json");
    assert_eq!(check["summary"]["files"], 1);
    assert_eq!(check["summary"]["changed"], 0);
    assert_eq!(check["summary"]["wouldChange"], 1);
    assert_eq!(check["files"][0]["problems"][0]["rule"], "MDX010");

    let write = run(&tmp.path, &["--write", "--json", "doc.md"], None);
    assert_eq!(write.status.code(), Some(0));
    let written: Value = serde_json::from_slice(&write.stdout).expect("json");
    assert_eq!(written["summary"]["changed"], 1);
    assert!(written["summary"]["fixed"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "# Title\n");

    let refused_file = tmp.write("refused.md", "---\ntitle: x\n#Title\n");
    let refused = run(&tmp.path, &["--write", "refused.md"], None);
    assert_eq!(refused.status.code(), Some(1));
    assert!(stdout(&refused).contains("Not modified"));
    assert_eq!(
        std::fs::read_to_string(&refused_file).unwrap(),
        "---\ntitle: x\n#Title\n"
    );

    let refused_json = run(&tmp.path, &["--check", "--json", "refused.md"], None);
    assert_eq!(refused_json.status.code(), Some(1));
    let output: Value = serde_json::from_slice(&refused_json.stdout).expect("json");
    assert_eq!(output["files"][0]["refused"], true);
    assert_eq!(output["files"][0]["wouldChange"], false);
    assert_eq!(output["files"][0]["wouldFix"], 0);
}

// 原案例：CLI requires explicit paths for --write and honors --strict
#[test]
fn requires_explicit_paths_for_write_and_honors_strict() {
    let tmp = TempDir::new("strict");
    let no_paths = run(&tmp.path, &["--write"], None);
    assert_eq!(no_paths.status.code(), Some(2));
    assert!(stderr(&no_paths).contains("requires explicit files or directories"));

    tmp.write("info.md", "Bare https://example.com.\n");
    assert_eq!(
        run(&tmp.path, &["--check", "info.md"], None).status.code(),
        Some(0)
    );
    assert_eq!(
        run(&tmp.path, &["--check", "--strict", "info.md"], None)
            .status
            .code(),
        Some(1)
    );
}

// 原案例：CLI reports invalid UTF-8 and NUL bytes as I/O errors
#[test]
fn reports_invalid_utf8_and_nul_bytes_as_io_errors() {
    let tmp = TempDir::new("io-errors");
    tmp.write("invalid.md", [0xff, 0xfe, 0x23, 0x20]);
    let invalid = run(&tmp.path, &["--check", "invalid.md"], None);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(stderr(&invalid).to_lowercase().contains("utf-8"));

    tmp.write("nul.md", "# Title\0\n");
    let nul = run(&tmp.path, &["--check", "nul.md"], None);
    assert_eq!(nul.status.code(), Some(2));
    assert!(stderr(&nul).contains("NUL"));
}

// 原案例：does not claim success when some files cannot be read
#[test]
fn does_not_claim_success_when_some_files_cannot_be_read() {
    let tmp = TempDir::new("partial");
    tmp.write("good.md", "# Title\n");
    tmp.write("bad.md", [0xff, 0xfe]);
    let result = run(&tmp.path, &["--check", "good.md", "bad.md"], None);
    assert_eq!(result.status.code(), Some(2));
    let out = stdout(&result);
    assert!(out.contains("Some files could not be read"));
    assert!(!out.contains("No Markdown issues found"));
}

// 原案例：writes through a symlink without replacing it
#[cfg(unix)]
#[test]
fn writes_through_a_symlink_without_replacing_it() {
    let tmp = TempDir::new("symlink");
    let target = tmp.write("doc.md", "#Title\n");
    let link = tmp.join("link.md");
    std::os::unix::fs::symlink("doc.md", &link).unwrap();
    let result = run(&tmp.path, &["--write", "link.md"], None);
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Title\n");
}

/// 原子寫入不留下暫存檔，並保留原檔權限。
#[cfg(unix)]
#[test]
fn atomic_write_keeps_permissions_and_leaves_no_temp_files() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new("atomic");
    let file = tmp.write("doc.md", "#Title\n");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let result = run(&tmp.path, &["--write", "doc.md"], None);
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let names: Vec<_> = std::fs::read_dir(&tmp.path)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["doc.md".to_string()]);
}

/// 寫入失敗（目錄不可寫）時回報 I/O 錯誤、結束碼 2，原檔不變。
#[cfg(unix)]
#[test]
fn write_failure_is_an_io_error() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new("readonly-dir");
    let file = tmp.write("ro/doc.md", "#Title\n");
    let dir = tmp.join("ro");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    // root 不受權限限制，略過。
    if std::fs::write(dir.join("probe"), "x").is_ok() {
        return;
    }
    let result = run(&tmp.path, &["--write", "ro/doc.md"], None);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(result.status.code(), Some(2));
    let err = stderr(&result);
    assert!(
        err.starts_with("Error: cannot write ro/doc.md: EACCES: permission denied, open '"),
        "{err}"
    );
    assert!(err.trim_end().ends_with(".tmp'"), "{err}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "#Title\n");
    // 寫入失敗的檔案不列入結果，所以沒有任何報告，只有 I/O 錯誤。
    assert_eq!(stdout(&result), "");
}

/// stdout 被提早關閉（例如接到 `head`）時不 panic。
#[cfg(unix)]
#[test]
fn closed_stdout_does_not_panic() {
    use std::process::{Command, Stdio};
    let tmp = TempDir::new("epipe");
    let mut child = Command::new(common::bin())
        .arg("--list-rules")
        .current_dir(&tmp.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(!stderr(&output).contains("panicked"));
}
