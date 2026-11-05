//! 重播 `tests/cli-golden/cases/*.json`：每個案例由 `scripts/gen-cli-golden.mjs`
//! 以 oracle CLI 產生，內含輸入檔、參數與預期的 stdout、stderr、結束碼、執行後檔案。
//! 這裡在暫存目錄重建同樣的輸入，執行 Rust 版 `markdown-fixer` 並逐位元組比對。
//!
//! `copyDir` 項目（例如整個 `fixtures/corpus`）不內嵌在 golden 檔，重播時從 repo
//! 複製；golden 記錄目錄指紋，內容改變時本測試失敗並要求重新產生 golden。
//! `copyDir` 底下的檔案在執行後以 FNV-1a 雜湊比對。
//!
//! 比對前的正規化：暫存目錄路徑換成 `<TMP>`、版本字串換成 `<VERSION>`、
//! 移除 `--help` 裡 Rust 版專有旗標的區塊。
//!
//! 每個案例跑兩次：
//!
//! 1. 在參數最前面加上 `--no-render-guard`：渲染保護關閉，行為必須與 oracle 逐位元組相同。
//!    `--no-render-guard` 放在最前面，不會被 `--rule` 之類需要值的旗標吃掉，也不影響
//!    後面參數的錯誤順序。
//! 2. 原樣執行（渲染保護開啟，也就是預設模式）：除非輸出裡出現被擋下的修正
//!    （文字的 `Fix not applied`、`--quiet` 時的摘要行 `Render guard skipped`、JSON 的
//!    `skippedFixes`），否則也必須與 golden 相同。
//!    出現時只計數，不比對內容（那是刻意差異，見 `docs/parity.md`）。
//!
//! 只跑部分案例：`MDFIX_CLI_GOLDEN=<名稱片段> cargo test -p markdown-fixer-cli --test cli_golden`。
//!
//! 案例用到 symlink 與 Unix 權限，golden 也是在 Unix 上產生，所以整個檔案只在 Unix 編譯。

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

use common::{TempDir, decode_hex, run};

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct SetupEntry {
    path: String,
    kind: Kind,
    mode: Option<u32>,
}

enum Kind {
    Dir,
    Symlink(String),
    File(Vec<u8>),
    CopyDir { source: String, fingerprint: String },
}

/// FNV-1a 64，與 `scripts/gen-cli-golden.mjs` 相同。
fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// 目錄指紋：每個檔案以「相對路徑 + NUL + 內容」計算 FNV-1a 後相加，與順序無關。
fn fingerprint_dir(dir: &Path) -> String {
    fn walk(dir: &Path, rel: &str, sum: &mut u64) {
        for entry in fs::read_dir(dir).expect("read_dir").flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel_path = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let file_type = entry.file_type().expect("file_type");
            if file_type.is_dir() {
                walk(&entry.path(), &rel_path, sum);
            } else if file_type.is_file() {
                let seed = fnv1a(format!("{rel_path}\0").as_bytes(), FNV_OFFSET);
                let hash = fnv1a(&fs::read(entry.path()).expect("read"), seed);
                *sum = sum.wrapping_add(hash);
            }
        }
    }
    let mut sum = 0;
    walk(dir, "", &mut sum);
    format!("{sum:016x}")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir copy");
    for entry in fs::read_dir(from).expect("read_dir").flatten() {
        let target = to.join(entry.file_name());
        let file_type = entry.file_type().expect("file_type");
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target);
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// `copyDir` 底下的檔案只比對雜湊；以 `fnv:<hex>` 字串代表內容。
fn hashed(bytes: &[u8]) -> Vec<u8> {
    format!("fnv:{:016x}", fnv1a(bytes, FNV_OFFSET)).into_bytes()
}

fn repo_root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn content_of(value: &Value) -> Vec<u8> {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        text.as_bytes().to_vec()
    } else if let Some(hex) = value.get("hex").and_then(Value::as_str) {
        decode_hex(hex)
    } else if let Some(fnv) = value.get("fnv").and_then(Value::as_str) {
        format!("fnv:{fnv}").into_bytes()
    } else {
        panic!("entry has neither text nor hex: {value}")
    }
}

fn parse_setup(value: &Value) -> Vec<SetupEntry> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let path = item["path"].as_str().expect("path").to_string();
                    let kind = if let Some(source) = item.get("copyDir").and_then(Value::as_str) {
                        Kind::CopyDir {
                            source: source.to_string(),
                            fingerprint: item["fingerprint"]
                                .as_str()
                                .expect("fingerprint")
                                .to_string(),
                        }
                    } else if item.get("dir").and_then(Value::as_bool) == Some(true) {
                        Kind::Dir
                    } else if let Some(target) = item.get("symlink").and_then(Value::as_str) {
                        Kind::Symlink(target.to_string())
                    } else {
                        Kind::File(content_of(item))
                    };
                    let mode = item
                        .get("mode")
                        .and_then(Value::as_str)
                        .map(|m| u32::from_str_radix(m, 8).expect("octal mode"));
                    SetupEntry { path, kind, mode }
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
}

#[cfg(unix)]
fn get_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).expect("stat").permissions().mode() & 0o777
}

#[cfg(unix)]
fn apply_setup(root: &Path, entries: &[SetupEntry]) {
    for entry in entries {
        let target = root.join(&entry.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("mkdir parent");
        }
        match &entry.kind {
            Kind::Dir => fs::create_dir_all(&target).expect("mkdir"),
            Kind::Symlink(link) => std::os::unix::fs::symlink(link, &target).expect("symlink"),
            Kind::File(bytes) => fs::write(&target, bytes).expect("write"),
            Kind::CopyDir { source, .. } => copy_dir(&repo_root().join(source), &target),
        }
    }
    for entry in entries {
        if let Some(mode) = entry.mode {
            set_mode(&root.join(&entry.path), mode);
        }
    }
}

/// 執行後的檔案樹：路徑 → (內容, 權限)。只記錄 setup 指定過權限的檔案的權限。
#[cfg(unix)]
fn collect_files(root: &Path, entries: &[SetupEntry]) -> BTreeMap<String, (Vec<u8>, Option<u32>)> {
    let copied: Vec<String> = entries
        .iter()
        .filter(|entry| matches!(entry.kind, Kind::CopyDir { .. }))
        .map(|entry| format!("{}/", entry.path))
        .collect();
    let mut modes = BTreeMap::new();
    for entry in entries {
        if entry.mode.is_some() {
            let target = root.join(&entry.path);
            modes.insert(entry.path.clone(), get_mode(&target));
            set_mode(
                &target,
                if matches!(entry.kind, Kind::Dir) {
                    0o755
                } else {
                    0o644
                },
            );
        }
    }
    let mut files = BTreeMap::new();
    fn walk(
        dir: &Path,
        rel: &str,
        copied: &[String],
        modes: &BTreeMap<String, u32>,
        files: &mut BTreeMap<String, (Vec<u8>, Option<u32>)>,
    ) {
        for entry in fs::read_dir(dir).expect("read_dir").flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel_path = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let file_type = entry.file_type().expect("file_type");
            if file_type.is_dir() {
                walk(&entry.path(), &rel_path, copied, modes, files);
            } else if file_type.is_file() {
                let mut bytes = fs::read(entry.path()).expect("read");
                if copied.iter().any(|prefix| rel_path.starts_with(prefix)) {
                    bytes = hashed(&bytes);
                }
                let mode = modes.get(&rel_path).copied();
                files.insert(rel_path, (bytes, mode));
            }
        }
    }
    walk(root, "", &copied, &modes, &mut files);
    files
}

fn normalize(text: &str, tmp: &TempDir) -> String {
    let mut out = text.replace(&*tmp.path.to_string_lossy(), "<TMP>");
    out = out.replace(&*tmp.raw.to_string_lossy(), "<TMP>");
    out = out.replace(
        &format!("markdown-fixer {VERSION}\n"),
        "markdown-fixer <VERSION>\n",
    );
    out = out.replace(
        &format!("\"version\": \"{VERSION}\""),
        "\"version\": \"<VERSION>\"",
    );
    strip_rust_only_help(&out)
}

/// 移除 `--help` 中 Rust 版專有旗標的區塊（含其後的空行）。
fn strip_rust_only_help(text: &str) -> String {
    let marker = "\nRust-only options";
    let Some(start) = text.find(marker) else {
        return text.to_string();
    };
    let block_start = start + 1;
    match text[block_start..].find("\n\n") {
        Some(end) => format!("{}{}", &text[..block_start], &text[block_start + end + 2..]),
        None => text.to_string(),
    }
}

fn first_difference(expected: &str, actual: &str) -> String {
    for (i, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("line {}:\n    expected: {e:?}\n    actual:   {a:?}", i + 1);
        }
    }
    format!(
        "line counts differ: expected {} lines, actual {} lines\n    expected: {expected:?}\n    actual:   {actual:?}",
        expected.lines().count(),
        actual.lines().count()
    )
}

#[cfg(unix)]
fn running_as_root(tmp: &TempDir) -> bool {
    // 建一個 000 的目錄，讀得到就代表是 root（權限檢查不生效）。
    let probe = tmp.path.join(".root-probe");
    fs::create_dir_all(&probe).expect("mkdir probe");
    set_mode(&probe, 0o000);
    let readable = fs::read_dir(&probe).is_ok();
    set_mode(&probe, 0o755);
    let _ = fs::remove_dir(&probe);
    readable
}

/// 一次重播的結果。
#[derive(Debug, PartialEq, Eq)]
enum Replay {
    Matched,
    /// 需要非 root 權限，但目前以 root 執行。
    Skipped,
    /// 渲染保護擋下了修正，輸出刻意與 oracle 不同。
    Guarded,
}

/// 輸出裡是否有渲染保護擋下的修正。
fn mentions_skipped_fixes(output: &std::process::Output) -> bool {
    [&output.stdout, &output.stderr].iter().any(|bytes| {
        let text = String::from_utf8_lossy(bytes);
        text.contains("Fix not applied")
            || text.contains("Render guard skipped")
            || text.contains("\"skippedFixes\"")
    })
}

#[cfg(unix)]
fn replay(case: &Value, guard_off: bool) -> Result<Replay, String> {
    let name = case["name"].as_str().expect("name");
    let mut args: Vec<&str> = case["args"]
        .as_array()
        .expect("args")
        .iter()
        .map(|a| a.as_str().expect("arg"))
        .collect();
    if guard_off {
        args.insert(0, "--no-render-guard");
    }
    let tmp = TempDir::new(name);
    if case["nonRoot"].as_bool() == Some(true) && running_as_root(&tmp) {
        return Ok(Replay::Skipped);
    }
    let setup = parse_setup(&case["setup"]);
    for entry in &setup {
        if let Kind::CopyDir {
            source,
            fingerprint,
        } = &entry.kind
        {
            let actual = fingerprint_dir(&repo_root().join(source));
            if &actual != fingerprint {
                return Err(format!(
                    "[{name}] {source} changed since the golden file was generated \
                     (fingerprint {fingerprint} -> {actual}); run `node scripts/gen-cli-golden.mjs`"
                ));
            }
        }
    }
    apply_setup(&tmp.path, &setup);
    let cwd = tmp.path.join(case["cwd"].as_str().unwrap_or(""));
    fs::create_dir_all(&cwd).expect("mkdir cwd");

    let output = run(&cwd, &args, None);
    if !guard_off && mentions_skipped_fixes(&output) {
        return Ok(Replay::Guarded);
    }
    let expected = &case["expected"];
    let mut problems = Vec::new();

    let status = output.status.code().map(i64::from);
    if status != expected["status"].as_i64() {
        problems.push(format!(
            "exit status: expected {}, actual {status:?}",
            expected["status"]
        ));
    }
    for (stream, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
        let actual = normalize(&String::from_utf8_lossy(bytes), &tmp);
        let wanted = expected[stream].as_str().expect("stream");
        if actual != wanted {
            problems.push(format!(
                "{stream} differs at {}",
                first_difference(wanted, &actual)
            ));
        }
    }

    let actual_files = collect_files(&tmp.path, &setup);
    let mut expected_files = BTreeMap::new();
    for file in expected["files"].as_array().expect("files") {
        let mode = file
            .get("mode")
            .and_then(Value::as_str)
            .map(|m| u32::from_str_radix(m, 8).expect("octal mode"));
        expected_files.insert(
            file["path"].as_str().expect("path").to_string(),
            (content_of(file), mode),
        );
    }
    if actual_files != expected_files {
        for (path, wanted) in &expected_files {
            match actual_files.get(path) {
                None => problems.push(format!("file {path} missing after run")),
                Some(actual) if actual != wanted => problems.push(format!(
                    "file {path} differs: expected {:?} (mode {:?}), actual {:?} (mode {:?})",
                    String::from_utf8_lossy(&wanted.0),
                    wanted.1,
                    String::from_utf8_lossy(&actual.0),
                    actual.1
                )),
                _ => {}
            }
        }
        for path in actual_files.keys() {
            if !expected_files.contains_key(path) {
                problems.push(format!("unexpected file {path} after run"));
            }
        }
    }
    // symlink 必須維持是 symlink（oracle 經由 realpath 寫入目標檔）。
    for entry in &setup {
        if let Kind::Symlink(target) = &entry.kind {
            match fs::read_link(tmp.path.join(&entry.path)) {
                Ok(link) if link == Path::new(target) => {}
                other => problems.push(format!("symlink {} changed: {other:?}", entry.path)),
            }
        }
    }

    if problems.is_empty() {
        Ok(Replay::Matched)
    } else {
        Err(format!(
            "[{name}] args={args:?}\n  {}",
            problems.join("\n  ")
        ))
    }
}

#[cfg(unix)]
#[test]
fn cli_matches_oracle_golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli-golden/cases");
    let filter = std::env::var("MDFIX_CLI_GOLDEN").unwrap_or_default();
    let mut names: Vec<_> = fs::read_dir(&dir)
        .expect("golden dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    names.sort();
    assert!(names.len() >= 40, "expected at least 40 golden cases");

    let mut failures = Vec::new();
    let (mut passed, mut skipped, mut guarded) = (0, 0, 0);
    for path in &names {
        let case: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read golden")).expect("json");
        if !case["name"].as_str().unwrap_or("").contains(&filter) {
            continue;
        }
        for guard_off in [true, false] {
            match replay(&case, guard_off) {
                Ok(Replay::Matched) => passed += 1,
                Ok(Replay::Skipped) => skipped += 1,
                Ok(Replay::Guarded) => guarded += 1,
                Err(message) => failures.push(format!(
                    "{}{message}",
                    if guard_off { "" } else { "(render guard on) " }
                )),
            }
        }
    }
    eprintln!(
        "CLI golden: {passed} passed, {} failed, {skipped} skipped, {guarded} differ only by render-guard skips",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} CLI golden case(s) differ from the oracle:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn strip_rust_only_help_block() {
    let help = "a\n  -h, --help\n\nRust-only options (x):\n  --stdin  y\n\nExit codes:\n";
    assert_eq!(
        strip_rust_only_help(help),
        "a\n  -h, --help\n\nExit codes:\n"
    );
    assert_eq!(strip_rust_only_help("no block\n"), "no block\n");
}
