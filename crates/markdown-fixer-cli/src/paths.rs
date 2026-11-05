//! 路徑展開與顯示，對應 oracle 的 `expandPaths` 與 Node `path` 模組的
//! `resolve`、`join`、`relative`。
//!
//! 重點：
//!
//! - `path.resolve`、`path.join` 都是純字面運算（lexical），不解析 symlink，
//!   所以這裡不使用 `fs::canonicalize`。
//! - 頂層輸入用 `fs.statSync`（會跟隨 symlink），任何副檔名都接受；目錄裡的
//!   項目用 `Dirent`（不跟隨 symlink），只收 `.md`／`.markdown`，symlink 一律略過。
//! - libuv 的 `scandir` 以 `strcmp` 排序目錄項目，錯誤訊息依這個順序出現。
//! - 最後去重，再依 UTF-16 code unit 排序（JS `Array.prototype.sort()`）。

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::jsstr::utf16_cmp;
use crate::node_err;

/// 目錄掃描時略過的名稱（oracle 的 `DEFAULT_EXCLUDES`）。
pub const DEFAULT_EXCLUDES: [&str; 10] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    "coverage",
    "test-results",
    "playwright-report",
    "blob-report",
    ".e2e-tmp",
];

/// `expandPaths` 的結果。
#[derive(Debug, Default)]
pub struct Expanded {
    /// 去重並排序後的絕對路徑。
    pub files: Vec<PathBuf>,
    /// 錯誤訊息（不含 `Error: ` 前綴），依走訪順序。
    pub errors: Vec<String>,
}

/// 展開檔案與目錄參數。`cwd` 是 `process.cwd()`。
pub fn expand_paths(inputs: &[OsString], cwd: &Path) -> Expanded {
    let mut expanded = Expanded::default();
    for input in inputs {
        visit(Path::new(input), cwd, &mut expanded);
    }
    let mut seen = HashSet::new();
    let mut keyed: Vec<(String, PathBuf)> = Vec::new();
    for file in expanded.files.drain(..) {
        let key = file.to_string_lossy().into_owned();
        if seen.insert(key.clone()) {
            keyed.push((key, file));
        }
    }
    keyed.sort_by(|a, b| utf16_cmp(&a.0, &b.0));
    expanded.files = keyed.into_iter().map(|(_, file)| file).collect();
    expanded
}

fn visit(target: &Path, cwd: &Path, expanded: &mut Expanded) {
    let shown = target.to_string_lossy();
    let metadata = match fs::metadata(target) {
        Ok(metadata) => metadata,
        Err(error) => {
            expanded.errors.push(format!(
                "Cannot access {shown}: {}",
                node_err::format(&error, "stat", &shown)
            ));
            return;
        }
    };
    if metadata.is_file() {
        expanded.files.push(resolve(cwd, target));
        return;
    }
    if !metadata.is_dir() {
        return;
    }
    let read_error = |error: std::io::Error| {
        format!(
            "Cannot read directory {shown}: {}",
            node_err::format(&error, "scandir", &shown)
        )
    };
    let mut entries = Vec::new();
    match fs::read_dir(target) {
        Ok(iter) => {
            for entry in iter {
                match entry {
                    Ok(entry) => entries.push(entry),
                    Err(error) => {
                        expanded.errors.push(read_error(error));
                        return;
                    }
                }
            }
        }
        Err(error) => {
            expanded.errors.push(read_error(error));
            return;
        }
    }
    entries.sort_by(|a, b| {
        a.file_name()
            .as_encoded_bytes()
            .cmp(b.file_name().as_encoded_bytes())
    });
    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if DEFAULT_EXCLUDES.contains(&name_str.as_ref()) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let child = join(target, Path::new(&name));
        if file_type.is_dir() {
            visit(&child, cwd, expanded);
            continue;
        }
        if file_type.is_file() && has_markdown_extension(&name_str) {
            expanded.files.push(resolve(cwd, &child));
        }
    }
}

/// `/\.(?:md|markdown)$/i`：只比較 ASCII 大小寫。
pub fn has_markdown_extension(name: &str) -> bool {
    let bytes = name.as_bytes();
    let ends_with = |suffix: &[u8]| {
        bytes.len() >= suffix.len()
            && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    };
    ends_with(b".md") || ends_with(b".markdown")
}

/// Node `path.normalize` 的字面正規化：去掉 `.`、合併 `..`、去掉多餘分隔符號。
/// 空結果回傳 `.`。
pub fn normalize(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => parts.push(component),
            },
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return PathBuf::from(".");
    }
    parts.iter().collect()
}

/// Node `path.join(a, b)`。
pub fn join(base: &Path, name: &Path) -> PathBuf {
    normalize(&base.join(name))
}

/// Node `path.resolve(target)`（以 `cwd` 為基準）。
pub fn resolve(cwd: &Path, target: &Path) -> PathBuf {
    normalize(&cwd.join(target))
}

/// Node `path.relative(from, to)`；兩者都應是 `resolve` 過的絕對路徑。
pub fn relative(from: &Path, to: &Path) -> PathBuf {
    let from_parts: Vec<Component<'_>> = from.components().collect();
    let to_parts: Vec<Component<'_>> = to.components().collect();
    if let (Some(Component::Prefix(a)), Some(Component::Prefix(b))) =
        (from_parts.first(), to_parts.first())
    {
        if !same_component(a.as_os_str(), b.as_os_str()) {
            return to.to_path_buf();
        }
    }
    let common = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| same_component(a.as_os_str(), b.as_os_str()))
        .count();
    let mut out = PathBuf::new();
    for _ in common..from_parts.len() {
        out.push("..");
    }
    for part in &to_parts[common..] {
        out.push(part.as_os_str());
    }
    out
}

#[cfg(windows)]
fn same_component(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    // Node 在 win32 上以 toLowerCase() 比較。
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

#[cfg(not(windows))]
fn same_component(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    a == b
}

/// 顯示路徑：`path.relative(process.cwd(), file) || file`。
pub fn display(cwd: &Path, file: &Path) -> String {
    let rel = relative(cwd, file);
    if rel.as_os_str().is_empty() {
        file.to_string_lossy().into_owned()
    } else {
        rel.to_string_lossy().into_owned()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn normalize_is_lexical() {
        assert_eq!(normalize(&p("./docs")), p("docs"));
        assert_eq!(normalize(&p("a/../b")), p("b"));
        assert_eq!(normalize(&p("a/..")), p("."));
        assert_eq!(normalize(&p("../a/./b/")), p("../a/b"));
        assert_eq!(normalize(&p("/../x//y")), p("/x/y"));
        assert_eq!(normalize(&p("")), p("."));
    }

    #[test]
    fn join_matches_node() {
        assert_eq!(join(&p("."), &p("d")), p("d"));
        assert_eq!(join(&p("docs/"), &p("a.md")), p("docs/a.md"));
        assert_eq!(join(&p("/tmp/../x"), &p("y")), p("/x/y"));
    }

    #[test]
    fn resolve_and_relative() {
        let cwd = p("/home/u/repo");
        assert_eq!(resolve(&cwd, &p("docs/../a.md")), p("/home/u/repo/a.md"));
        assert_eq!(resolve(&cwd, &p("/etc/x.md")), p("/etc/x.md"));
        assert_eq!(resolve(&cwd, &p("./")), p("/home/u/repo"));
        assert_eq!(display(&cwd, &p("/home/u/repo/docs/a.md")), "docs/a.md");
        assert_eq!(display(&cwd, &p("/home/u/a.md")), "../a.md");
        assert_eq!(display(&cwd, &p("/home/u/repo2/a.md")), "../repo2/a.md");
        assert_eq!(display(&cwd, &p("/etc/x.md")), "../../../etc/x.md");
        assert_eq!(display(&p("/"), &p("/x.md")), "x.md");
        assert_eq!(display(&cwd, &cwd), "/home/u/repo");
    }

    #[test]
    fn markdown_extension_is_ascii_case_insensitive() {
        assert!(has_markdown_extension("README.md"));
        assert!(has_markdown_extension("README.MD"));
        assert!(has_markdown_extension("notes.Markdown"));
        assert!(has_markdown_extension(".md"));
        assert!(!has_markdown_extension("page.mdx"));
        assert!(!has_markdown_extension("md"));
        assert!(!has_markdown_extension("notes.txt"));
    }
}
