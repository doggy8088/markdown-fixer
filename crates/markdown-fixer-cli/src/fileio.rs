//! 讀寫 Markdown 檔，對應 oracle 的 `readMarkdownFile` 與 `writeFileAtomic`。

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::node_err;

/// 檔案含 NUL 位元組時的錯誤訊息。
pub const NUL_MESSAGE: &str = "file contains NUL bytes; refusing to treat it as text";
/// 非法 UTF-8 時的錯誤訊息（Node `TextDecoder` 的 `fatal` 錯誤）。
pub const UTF8_MESSAGE: &str = "The encoded data was not valid for encoding utf-8";

/// 先檢查 NUL，再以嚴格 UTF-8 解碼；保留 BOM（oracle 用 `ignoreBOM: true`）。
pub fn decode_markdown(bytes: Vec<u8>) -> Result<String, String> {
    if bytes.contains(&0) {
        return Err(NUL_MESSAGE.to_string());
    }
    String::from_utf8(bytes).map_err(|_| UTF8_MESSAGE.to_string())
}

/// 讀取檔案；錯誤訊息與 oracle 相同（`file` 是絕對路徑）。
pub fn read_markdown_file(file: &Path) -> Result<String, String> {
    let bytes = fs::read(file)
        .map_err(|error| node_err::format(&error, "open", &file.to_string_lossy()))?;
    decode_markdown(bytes)
}

/// 原子寫入：解析 symlink 找到真正的檔案，寫入同目錄的暫存檔，套用原檔權限，
/// 再 rename 蓋過原檔。失敗時盡量刪除暫存檔。
pub fn write_file_atomic(file: &Path, contents: &str) -> Result<(), String> {
    let shown = file.to_string_lossy();
    let target =
        fs::canonicalize(file).map_err(|error| node_err::format(&error, "realpath", &shown))?;
    let target_str = target.to_string_lossy().into_owned();
    let metadata =
        fs::metadata(&target).map_err(|error| node_err::format(&error, "stat", &target_str))?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_str = format!("{target_str}.{}.{millis}.tmp", std::process::id());
    let mut temp = target.clone().into_os_string();
    temp.push(format!(".{}.{millis}.tmp", std::process::id()));
    let temp = Path::new(&temp);

    let result = fs::write(temp, contents)
        .map_err(|error| node_err::format(&error, "open", &temp_str))
        .and_then(|()| {
            fs::set_permissions(temp, metadata.permissions())
                .map_err(|error| node_err::format(&error, "chmod", &temp_str))
        })
        .and_then(|()| {
            fs::rename(temp, &target)
                .map_err(|error| node_err::format_rename(&error, &temp_str, &target_str))
        });
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_checks_nul_before_utf8() {
        assert_eq!(decode_markdown(vec![0xff, 0x00]).unwrap_err(), NUL_MESSAGE);
        assert_eq!(decode_markdown(vec![0xff, 0xfe]).unwrap_err(), UTF8_MESSAGE);
        assert_eq!(
            decode_markdown("\u{FEFF}# T\n".as_bytes().to_vec()).unwrap(),
            "\u{FEFF}# T\n"
        );
    }
}
