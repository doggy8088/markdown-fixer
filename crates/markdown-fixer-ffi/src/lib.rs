//! markdown-fixer 的 C ABI。規格見 PRD.md〈6.5 C ABI 共用函式庫〉，使用說明見 `docs/ffi.md`。
//!
//! 這個 crate 產出 `cdylib`（`.dylib`／`.so`／`.dll`）與 `staticlib`，C 標頭檔是手寫的
//! `include/markdown_fixer.h`。所有資料以 UTF-8 位元組與 JSON 字串交換，讓 ABI 不受
//! Rust 型別變動影響：
//!
//! - 輸入文字以「指標＋長度」傳入，不要求 NUL 結尾，可以包含 NUL 位元組。
//! - 選項以 NUL 結尾的 JSON 字串傳入，`NULL` 表示全部預設。
//! - 回傳值一律是 NUL 結尾的 UTF-8 JSON 物件，由呼叫端以 [`mdfix_string_free`] 釋放；
//!   失敗時回傳 `{"error":"..."}`，永遠不回傳 `NULL`。
//!
//! 每個進入點都以 [`std::panic::catch_unwind`] 包住，panic 不會跨過 FFI 邊界，而是轉成
//! `{"error":"internal error: ..."}`。這只在 `panic = "unwind"`（預設）時有效；以
//! `panic = "abort"` 建置時 panic 會直接結束行程。
//!
//! 所有函式都沒有全域狀態，可以在多個執行緒同時呼叫。

use std::any::Any;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use markdown_fixer::{Options, Problem, Rule};
use serde::{Deserialize, Serialize};

/// 以 NUL 結尾的版本字串，給 [`mdfix_version`] 回傳。
static VERSION_CSTR: &CStr =
    match CStr::from_bytes_with_nul(concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes()) {
        Ok(version) => version,
        Err(_) => c"unknown",
    };

/// `options_json` 的格式，鍵名與 JS `diagnose(text, options)` 的 options 相同。
///
/// 未知的鍵視為錯誤（JS 會默默忽略），避免 `ignore_rules` 之類的拼字錯誤讓設定失效。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FfiOptions {
    /// 只啟用這些規則；`null` 或省略表示全部啟用，空陣列表示全部停用（與 JS 相同）。
    rules: Option<Vec<String>>,
    /// 停用這些規則。
    ignore_rules: Option<Vec<String>>,
    /// MDX090 行長上限；`null`、省略或 0 表示停用 MDX090。必須是非負整數。
    max_line_length: Option<usize>,
}

impl From<FfiOptions> for Options {
    fn from(value: FfiOptions) -> Self {
        Options {
            rules: value.rules,
            ignore_rules: value.ignore_rules,
            max_line_length: value.max_line_length.unwrap_or(0),
            ..Options::default()
        }
    }
}

/// 把結果或錯誤包成 JSON 字串，並攔下 `f` 裡的 panic。
fn guarded<F>(f: F) -> *mut c_char
where
    F: FnOnce() -> Result<String, String>,
{
    let json = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(json)) => json,
        Ok(Err(message)) => error_json(&message),
        Err(payload) => error_json(&format!("internal error: {}", panic_message(&*payload))),
    };
    into_raw_c_string(json)
}

/// 從 panic payload 取出訊息文字。
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        message
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message
    } else {
        "unknown panic"
    }
}

/// 產生 `{"error":"..."}`，以 serde_json 跳脫訊息中的引號與控制字元。
fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// `mdfix_diagnose` 的輸出外殼。
///
/// 刻意用結構而不用 `serde_json::json!`：`json!` 會先轉成 `serde_json::Value`，在沒有
/// `preserve_order` feature 時鍵會按字母排序，破壞與 JS 相同的欄位順序。
#[derive(Serialize)]
struct ProblemsOut<'a> {
    problems: &'a [Problem],
}

/// `mdfix_rules_json` 的輸出外殼，理由同 [`ProblemsOut`]。
#[derive(Serialize)]
struct RulesOut<'a> {
    rules: &'a [Rule],
}

fn problems_json(problems: &[Problem]) -> Result<String, String> {
    to_json(&ProblemsOut { problems })
}

fn rules_json(rules: &[Rule]) -> Result<String, String> {
    to_json(&RulesOut { rules })
}

/// 序列化成 JSON 字串。
fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("cannot serialize result: {error}"))
}

/// 把 JSON 字串轉成交給 C 端擁有的指標。
///
/// serde_json 會把字串裡的 NUL 跳脫成 `\u0000`，所以 `CString::new` 理論上不會失敗；
/// 萬一失敗仍回傳錯誤 JSON，維持「永遠不回傳 NULL」的契約。
fn into_raw_c_string(json: String) -> *mut c_char {
    CString::new(json)
        .unwrap_or_else(|_| CString::from(c"{\"error\":\"internal error: output contained NUL\"}"))
        .into_raw()
}

/// 讀取「指標＋長度」形式的 UTF-8 文字。
///
/// # Safety
///
/// `text` 不是 `NULL` 時，必須指向至少 `len` 個可讀位元組，且在呼叫期間不被修改。
unsafe fn read_text<'a>(text: *const c_char, len: usize) -> Result<&'a str, String> {
    if text.is_null() {
        return if len == 0 {
            Ok("")
        } else {
            Err(format!("text is NULL but len is {len}"))
        };
    }
    if len > isize::MAX as usize {
        return Err(format!("len is too large: {len}"));
    }
    // SAFETY: `text` 不是 NULL，且呼叫端保證它指向至少 `len` 個可讀、呼叫期間不變的
    // 位元組；`len` 不超過 `isize::MAX`，符合 `from_raw_parts` 的前提。
    let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), len) };
    std::str::from_utf8(bytes).map_err(|error| format!("text is not valid UTF-8: {error}"))
}

/// 讀取並驗證 `options_json`；`NULL` 表示全部預設。
///
/// # Safety
///
/// `options_json` 不是 `NULL` 時，必須指向 NUL 結尾的字串，且在呼叫期間不被修改。
unsafe fn read_options(options_json: *const c_char) -> Result<Options, String> {
    if options_json.is_null() {
        return Ok(Options::default());
    }
    // SAFETY: `options_json` 不是 NULL，且呼叫端保證它是呼叫期間有效的 NUL 結尾字串。
    let raw = unsafe { CStr::from_ptr(options_json) };
    let json = raw
        .to_str()
        .map_err(|error| format!("options_json is not valid UTF-8: {error}"))?;
    let parsed: Option<FfiOptions> =
        serde_json::from_str(json).map_err(|error| format!("invalid options JSON: {error}"))?;
    let options = Options::from(parsed.unwrap_or_default());
    options.validate().map_err(|error| error.to_string())?;
    Ok(options)
}

/// 檢查 Markdown，回傳 `{"problems":[...]}`，內容與順序和 JS `diagnose(text, options)` 相同。
///
/// 每個問題的欄位依序為 `rule`、`name`、`severity`、`fixable`、`line`、`column`、
/// `message`；`column` 的單位是 UTF-16 code unit。失敗時回傳 `{"error":"..."}`。
///
/// # Safety
///
/// - `text` 為 `NULL` 時 `len` 必須是 0；否則 `text` 必須指向至少 `len` 個可讀位元組。
/// - `options_json` 必須是 `NULL` 或 NUL 結尾的字串。
/// - 回傳值必須以 [`mdfix_string_free`] 釋放，且只能釋放一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mdfix_diagnose(
    text: *const c_char,
    len: usize,
    options_json: *const c_char,
) -> *mut c_char {
    guarded(|| {
        // SAFETY: 指標的前提由本函式的呼叫端保證（見 `# Safety`）。
        let text = unsafe { read_text(text, len) }?;
        // SAFETY: 同上。
        let options = unsafe { read_options(options_json) }?;
        let problems = markdown_fixer::diagnose(text, &options);
        problems_json(&problems)
    })
}

/// 套用安全修正，回傳 `{"text":...,"changed":...,"refused":...,"reason":...}`，
/// 與 JS `fixDocument(text, options)` 相同；未拒絕時 `reason` 是空字串。
///
/// 失敗時回傳 `{"error":"..."}`。
///
/// # Safety
///
/// 與 [`mdfix_diagnose`] 相同。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mdfix_fix(
    text: *const c_char,
    len: usize,
    options_json: *const c_char,
) -> *mut c_char {
    guarded(|| {
        // SAFETY: 指標的前提由本函式的呼叫端保證（見 `# Safety`）。
        let text = unsafe { read_text(text, len) }?;
        // SAFETY: 同上。
        let options = unsafe { read_options(options_json) }?;
        to_json(&markdown_fixer::fix_document(text, &options))
    })
}

/// 判斷文字是否像 Markdown，回傳 `{"is_markdown":...,"score":...,"signals":[...]}`。
///
/// 失敗時回傳 `{"error":"..."}`。
///
/// # Safety
///
/// - `text` 為 `NULL` 時 `len` 必須是 0；否則 `text` 必須指向至少 `len` 個可讀位元組。
/// - 回傳值必須以 [`mdfix_string_free`] 釋放，且只能釋放一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mdfix_detect(text: *const c_char, len: usize) -> *mut c_char {
    guarded(|| {
        // SAFETY: 指標的前提由本函式的呼叫端保證（見 `# Safety`）。
        let text = unsafe { read_text(text, len) }?;
        to_json(&markdown_fixer::detect::looks_like_markdown(text))
    })
}

/// 回傳規則目錄 `{"rules":[...]}`，順序與 JS `RULES` 相同。
///
/// 每條規則的欄位依序為 `id`、`name`、`category`、`severity`、`fixable`、`summary`。
/// 回傳值必須以 [`mdfix_string_free`] 釋放。
#[unsafe(no_mangle)]
pub extern "C" fn mdfix_rules_json() -> *mut c_char {
    guarded(|| rules_json(markdown_fixer::rules()))
}

/// 回傳函式庫版本（例如 `"0.1.0"`）。
///
/// 回傳的是靜態字串，在整個行程期間有效，**不可**釋放。
#[unsafe(no_mangle)]
pub extern "C" fn mdfix_version() -> *const c_char {
    VERSION_CSTR.as_ptr()
}

/// 釋放本函式庫回傳的字串。傳入 `NULL` 時什麼都不做。
///
/// # Safety
///
/// `s` 必須是 `NULL`，或是 [`mdfix_diagnose`]、[`mdfix_fix`]、[`mdfix_detect`]、
/// [`mdfix_rules_json`] 回傳且尚未釋放的指標。不可傳入 [`mdfix_version`] 的回傳值。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mdfix_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: 呼叫端保證 `s` 來自本函式庫的 `CString::into_raw` 且尚未釋放，
    // 因此可以交回 `CString::from_raw` 取得所有權並釋放。
    drop(unsafe { CString::from_raw(s) });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 取回 JSON 字串並釋放指標。
    fn take(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        // SAFETY: `ptr` 來自 `guarded`，是有效的 NUL 結尾字串。
        let json = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_owned();
        // SAFETY: `ptr` 來自 `guarded` 且只釋放這一次。
        unsafe { mdfix_string_free(ptr) };
        json
    }

    #[test]
    fn guarded_turns_str_panic_into_error_json() {
        let json = take(guarded(|| panic!("boom")));
        assert_eq!(json, r#"{"error":"internal error: boom"}"#);
    }

    #[test]
    fn guarded_turns_string_panic_into_error_json() {
        let detail = String::from("formatted \"boom\"");
        let json = take(guarded(move || std::panic::panic_any(detail)));
        assert_eq!(json, r#"{"error":"internal error: formatted \"boom\""}"#);
    }

    #[test]
    fn guarded_handles_non_string_panic_payload() {
        let json = take(guarded(|| std::panic::panic_any(42_u32)));
        assert_eq!(json, r#"{"error":"internal error: unknown panic"}"#);
    }

    #[test]
    fn guarded_passes_through_ok_and_err() {
        assert_eq!(take(guarded(|| Ok("{\"a\":1}".into()))), r#"{"a":1}"#);
        assert_eq!(
            take(guarded(|| Err("bad \"input\"\n".into()))),
            r#"{"error":"bad \"input\"\n"}"#
        );
    }

    #[test]
    fn error_json_escapes_control_characters_and_nul() {
        let json = error_json("a\0b\u{1}");
        assert_eq!(json, r#"{"error":"a\u0000b\u0001"}"#);
        // 可以放進 CString：沒有未跳脫的 NUL。
        assert!(CString::new(json).is_ok());
    }

    #[test]
    fn problems_json_keeps_js_field_order() {
        let problems = [Problem {
            rule: "MDX010",
            name: "atx-missing-space",
            severity: markdown_fixer::Severity::Warning,
            fixable: true,
            line: 1,
            column: 2,
            message: "msg \"q\"".to_owned(),
        }];
        assert_eq!(
            problems_json(&problems).unwrap(),
            r#"{"problems":[{"rule":"MDX010","name":"atx-missing-space","severity":"warning","fixable":true,"line":1,"column":2,"message":"msg \"q\""}]}"#
        );
        assert_eq!(problems_json(&[]).unwrap(), r#"{"problems":[]}"#);
    }

    #[test]
    fn rules_json_keeps_js_field_order() {
        let rules = [Rule {
            id: "MDX001",
            name: "bom",
            category: "encoding",
            severity: markdown_fixer::Severity::Warning,
            fixable: true,
            summary: "Remove a UTF-8 BOM from the start of the file.",
        }];
        assert_eq!(
            rules_json(&rules).unwrap(),
            r#"{"rules":[{"id":"MDX001","name":"bom","category":"encoding","severity":"warning","fixable":true,"summary":"Remove a UTF-8 BOM from the start of the file."}]}"#
        );
    }

    #[test]
    fn ffi_options_maps_to_library_options() {
        let parsed: FfiOptions =
            serde_json::from_str(r#"{"rules":["MDX001"],"ignoreRules":[],"maxLineLength":80}"#)
                .unwrap();
        let options = Options::from(parsed);
        assert_eq!(options.rules, Some(vec!["MDX001".to_owned()]));
        // 空陣列要保持 Some(vec![])，與 JS「空陣列仍為 truthy」一致。
        assert_eq!(options.ignore_rules, Some(vec![]));
        assert_eq!(options.max_line_length, 80);

        let parsed: FfiOptions =
            serde_json::from_str(r#"{"rules":null,"maxLineLength":null}"#).unwrap();
        assert_eq!(Options::from(parsed), Options::default());
    }

    #[test]
    fn ffi_options_rejects_unknown_keys_and_bad_values() {
        assert!(serde_json::from_str::<FfiOptions>(r#"{"ignore_rules":[]}"#).is_err());
        assert!(serde_json::from_str::<FfiOptions>(r#"{"maxLineLength":-1}"#).is_err());
        assert!(serde_json::from_str::<FfiOptions>(r#"{"maxLineLength":1.5}"#).is_err());
        assert!(serde_json::from_str::<FfiOptions>(r#"{"rules":"MDX001"}"#).is_err());
    }
}
