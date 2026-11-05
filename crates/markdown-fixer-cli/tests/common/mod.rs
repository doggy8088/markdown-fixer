//! 整合測試共用的小工具：暫存目錄、執行 CLI、hex 解碼。

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// CLI 執行檔路徑。
pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_markdown-fixer")
}

/// 測試結束時自動刪除的暫存目錄（`canonicalize` 過，macOS 上是 `/private/var/...`）。
pub struct TempDir {
    pub path: PathBuf,
    pub raw: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> TempDir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let raw = std::env::temp_dir().join(format!(
            "mdfix-cli-test-{}-{n}-{}",
            std::process::id(),
            label.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        ));
        let _ = std::fs::remove_dir_all(&raw);
        std::fs::create_dir_all(&raw).expect("create temp dir");
        let path = std::fs::canonicalize(&raw).expect("canonicalize temp dir");
        TempDir { path, raw }
    }

    pub fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.path.join(rel)
    }

    pub fn write(&self, rel: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let file = self.join(rel);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&file, contents).expect("write file");
        file
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        restore_permissions(&self.path);
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
fn restore_permissions(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            restore_permissions(&path);
        } else if file_type.is_file() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
        }
    }
}

#[cfg(not(unix))]
fn restore_permissions(_dir: &Path) {}

/// 在 `cwd` 執行 CLI，可選擇送入標準輸入。
pub fn run(cwd: &Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut child = Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn markdown-fixer");
    if let Some(input) = stdin {
        let mut pipe = child.stdin.take().expect("stdin pipe");
        // 子行程可能沒讀標準輸入就結束（例如用法錯誤），這時寫入會得到 BrokenPipe。
        if let Err(error) = pipe.write_all(input) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe,
                "write stdin: {error}"
            );
        }
    }
    child.wait_with_output().expect("wait markdown-fixer")
}

pub fn decode_hex(hex: &str) -> Vec<u8> {
    let digits: Vec<u8> = hex
        .bytes()
        .map(|b| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => panic!("bad hex digit {b}"),
        })
        .collect();
    digits
        .chunks(2)
        .map(|pair| pair[0] << 4 | pair[1])
        .collect()
}
