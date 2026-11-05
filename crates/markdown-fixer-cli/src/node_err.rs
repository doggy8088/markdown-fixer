//! 把 `std::io::Error` 格式化成 Node.js `fs` 錯誤的 `error.message`。
//!
//! oracle 直接把 Node 的錯誤訊息印出來，例如
//! `ENOENT: no such file or directory, stat 'missing.md'`。格式是
//! `<code>: <libuv 訊息>, <syscall> '<path>'`；rename 則是
//! `<code>: <訊息>, rename '<src>' -> '<dest>'`。訊息文字取自 libuv 的 `uv_strerror`。

use std::io;

/// 依 errno 找出 Node 的錯誤代碼與 libuv 訊息。
fn code_and_message(error: &io::Error) -> Option<(&'static str, &'static str)> {
    if let Some(found) = error.raw_os_error().and_then(from_errno) {
        return Some(found);
    }
    use io::ErrorKind::*;
    Some(match error.kind() {
        NotFound => ("ENOENT", "no such file or directory"),
        PermissionDenied => ("EACCES", "permission denied"),
        AlreadyExists => ("EEXIST", "file already exists"),
        NotADirectory => ("ENOTDIR", "not a directory"),
        IsADirectory => ("EISDIR", "illegal operation on a directory"),
        ReadOnlyFilesystem => ("EROFS", "read-only file system"),
        StorageFull => ("ENOSPC", "no space left on device"),
        CrossesDevices => ("EXDEV", "cross-device link not permitted"),
        ResourceBusy => ("EBUSY", "resource busy or locked"),
        InvalidInput => ("EINVAL", "invalid argument"),
        _ => return None,
    })
}

#[cfg(unix)]
fn from_errno(errno: i32) -> Option<(&'static str, &'static str)> {
    // 前 30 個 errno 在 Linux 與 macOS 相同。
    let common = match errno {
        1 => Some(("EPERM", "operation not permitted")),
        2 => Some(("ENOENT", "no such file or directory")),
        5 => Some(("EIO", "i/o error")),
        6 => Some(("ENXIO", "no such device or address")),
        9 => Some(("EBADF", "bad file descriptor")),
        12 => Some(("ENOMEM", "not enough memory")),
        13 => Some(("EACCES", "permission denied")),
        16 => Some(("EBUSY", "resource busy or locked")),
        17 => Some(("EEXIST", "file already exists")),
        18 => Some(("EXDEV", "cross-device link not permitted")),
        20 => Some(("ENOTDIR", "not a directory")),
        21 => Some(("EISDIR", "illegal operation on a directory")),
        22 => Some(("EINVAL", "invalid argument")),
        23 => Some(("ENFILE", "file table overflow")),
        24 => Some(("EMFILE", "too many open files")),
        26 => Some(("ETXTBSY", "text file is busy")),
        27 => Some(("EFBIG", "file too large")),
        28 => Some(("ENOSPC", "no space left on device")),
        30 => Some(("EROFS", "read-only file system")),
        _ => None,
    };
    common.or_else(|| platform_errno(errno))
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
fn platform_errno(errno: i32) -> Option<(&'static str, &'static str)> {
    match errno {
        35 => Some(("EAGAIN", "resource temporarily unavailable")),
        62 => Some(("ELOOP", "too many symbolic links encountered")),
        63 => Some(("ENAMETOOLONG", "name too long")),
        _ => None,
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))
))]
fn platform_errno(errno: i32) -> Option<(&'static str, &'static str)> {
    match errno {
        11 => Some(("EAGAIN", "resource temporarily unavailable")),
        36 => Some(("ENAMETOOLONG", "name too long")),
        40 => Some(("ELOOP", "too many symbolic links encountered")),
        _ => None,
    }
}

/// Windows 的路徑名稱錯誤照 libuv `uv_translate_sys_error` 對應。
/// `ErrorKind::InvalidFilename` 要到 Rust 1.87 才穩定，MSRV 1.85 只能看原始錯誤碼。
#[cfg(windows)]
fn from_errno(code: i32) -> Option<(&'static str, &'static str)> {
    match code {
        // ERROR_INVALID_NAME、ERROR_BAD_PATHNAME
        123 | 161 => Some(("ENOENT", "no such file or directory")),
        // ERROR_FILENAME_EXCED_RANGE
        206 => Some(("ENAMETOOLONG", "name too long")),
        _ => None,
    }
}

#[cfg(not(any(unix, windows)))]
fn from_errno(_errno: i32) -> Option<(&'static str, &'static str)> {
    None
}

/// `<code>: <訊息>, <syscall> '<path>'`；遇到無法對應的錯誤時退回 Rust 的訊息。
pub fn format(error: &io::Error, syscall: &str, path: &str) -> String {
    match code_and_message(error) {
        Some((code, message)) => format!("{code}: {message}, {syscall} '{path}'"),
        None => format!("{error}, {syscall} '{path}'"),
    }
}

/// `rename` 的錯誤訊息：`<code>: <訊息>, rename '<src>' -> '<dest>'`。
pub fn format_rename(error: &io::Error, from: &str, to: &str) -> String {
    match code_and_message(error) {
        Some((code, message)) => format!("{code}: {message}, rename '{from}' -> '{to}'"),
        None => format!("{error}, rename '{from}' -> '{to}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_node() {
        let error = io::Error::from(io::ErrorKind::NotFound);
        assert_eq!(
            format(&error, "stat", "missing.md"),
            "ENOENT: no such file or directory, stat 'missing.md'"
        );
        let error = io::Error::from(io::ErrorKind::PermissionDenied);
        assert_eq!(
            format_rename(&error, "a.tmp", "a.md"),
            "EACCES: permission denied, rename 'a.tmp' -> 'a.md'"
        );
    }

    #[cfg(unix)]
    #[test]
    fn uses_raw_errno() {
        let error = io::Error::from_raw_os_error(20);
        assert_eq!(
            format(&error, "stat", "a.md/"),
            "ENOTDIR: not a directory, stat 'a.md/'"
        );
        let error = io::Error::from_raw_os_error(1);
        assert_eq!(
            format(&error, "open", "x"),
            "EPERM: operation not permitted, open 'x'"
        );
    }

    #[cfg(windows)]
    #[test]
    fn maps_windows_path_errors_like_libuv() {
        let error = io::Error::from_raw_os_error(206);
        assert_eq!(
            format(&error, "open", "x"),
            "ENAMETOOLONG: name too long, open 'x'"
        );
        let error = io::Error::from_raw_os_error(123);
        assert_eq!(
            format(&error, "stat", "a?b"),
            "ENOENT: no such file or directory, stat 'a?b'"
        );
    }
}
