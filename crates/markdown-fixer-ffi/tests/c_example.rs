//! 以系統的 C 編譯器編譯 `tests/c/smoke.c` 與 `examples/c/main.c`，分別連結 staticlib
//! 與 cdylib 後執行。找不到 C 編譯器或在 Windows 上時略過（印出原因，不算失敗）。
//!
//! 編譯器取自 `CC` 環境變數，預設為 `cc`。

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// 找本 crate 的 `staticlib`／`cdylib`。
///
/// Cargo 1.100 起把測試執行檔與函式庫分別放在同個套件下的不同建置單元。
/// 舊版放在 profile 目錄或其 `deps/`。同一版面下可能有多份產物，取修改時間最新的一份。
fn find_lib(file_name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("找不到測試執行檔路徑");
    let output = exe.parent().expect("測試執行檔應位於建置輸出目錄");
    let parent = output.parent().unwrap_or(output);
    let dirs = if output.ends_with("out") {
        let package = parent
            .parent()
            .filter(|path| path.ends_with(env!("CARGO_PKG_NAME")))
            .expect("測試執行檔應位於本套件的 Cargo 建置單元");
        package
            .read_dir()
            .expect("無法讀取 Cargo 建置單元")
            .filter_map(Result::ok)
            .map(|unit| unit.path().join("out"))
            .collect::<Vec<_>>()
    } else {
        vec![parent.to_owned(), output.to_owned()]
    };
    dirs.iter()
        .map(|dir| dir.join(file_name))
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
        .unwrap_or_else(|| panic!("在 {dirs:?} 找不到 {file_name}；請確認 cargo test 已建置 C ABI"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compiler() -> Option<String> {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_owned());
    let available = Command::new(&cc)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if available {
        Some(cc)
    } else {
        eprintln!("略過：找不到 C 編譯器 {cc:?}");
        None
    }
}

/// 連結 Rust staticlib 時需要的系統函式庫（`rustc --print native-static-libs`）。
fn native_static_libs() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["-lSystem", "-lc", "-lm"]
    } else {
        &[
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc",
        ]
    }
}

#[derive(Clone, Copy, Debug)]
enum Link {
    Static,
    Dynamic,
}

/// 編譯 `source`，回傳執行檔路徑；找不到編譯器或函式庫時回傳 `None`。
fn build(source: &Path, name: &str, link: Link) -> Option<PathBuf> {
    let cc = compiler()?;
    let dylib_name = if cfg!(target_os = "macos") {
        "libmarkdown_fixer_ffi.dylib"
    } else {
        "libmarkdown_fixer_ffi.so"
    };

    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{link:?}"));
    let mut command = Command::new(cc);
    command
        .args(["-std=c99", "-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-I")
        .arg(repo_root().join("crates/markdown-fixer-ffi/include"))
        .arg(source)
        .arg("-o")
        .arg(&out);
    match link {
        Link::Static => {
            command
                .arg(find_lib("libmarkdown_fixer_ffi.a"))
                .args(native_static_libs());
        }
        Link::Dynamic => {
            let dylib = find_lib(dylib_name);
            let lib_dir = dylib.parent().unwrap_or(Path::new("."));
            command
                .arg("-L")
                .arg(lib_dir)
                .arg("-lmarkdown_fixer_ffi")
                .arg(format!("-Wl,-rpath,{}", lib_dir.display()));
        }
    }
    let output = command.output().expect("無法執行 C 編譯器");
    assert!(
        output.status.success(),
        "C 編譯失敗（{link:?}）：\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(out)
}

fn run(exe: &Path, args: &[&str]) -> String {
    let output = Command::new(exe)
        .args(args)
        .output()
        .expect("無法執行 C 程式");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{} 結束碼 {:?}\nstdout:\n{stdout}\nstderr:\n{}",
        exe.display(),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

#[test]
fn c_smoke_test_static_and_dynamic() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/smoke.c");
    for link in [Link::Static, Link::Dynamic] {
        let Some(exe) = build(&source, "mdfix-smoke", link) else {
            return;
        };
        let stdout = run(&exe, &[env!("CARGO_PKG_VERSION")]);
        assert_eq!(stdout, format!("ok {}\n", env!("CARGO_PKG_VERSION")));
    }
}

#[test]
fn c_example_compiles_against_header() {
    let source = repo_root().join("examples/c/main.c");
    for link in [Link::Static, Link::Dynamic] {
        if build(&source, "mdfix-example-build", link).is_none() {
            return;
        }
    }
}

#[test]
fn c_example_runs() {
    let source = repo_root().join("examples/c/main.c");
    for link in [Link::Static, Link::Dynamic] {
        let Some(exe) = build(&source, "mdfix-example", link) else {
            return;
        };
        let stdout = run(&exe, &[]);
        assert!(stdout.starts_with(&format!("markdown-fixer {}\n", env!("CARGO_PKG_VERSION"))));
        assert!(
            stdout.contains("fix: {\"text\":\"# Title\\n\\nSome text here.\\n\""),
            "{stdout}"
        );
        assert!(
            stdout.contains("unknown rule: {\"error\":\"Unknown rule: MDX999\"}"),
            "{stdout}"
        );
    }
}
