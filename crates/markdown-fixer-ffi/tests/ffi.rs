//! 從 Rust 端呼叫 `extern "C"` 進入點，驗證 JSON 契約、錯誤處理與記憶體管理。
//!
//! 預期輸出的 JSON 是以 oracle（`reference/js/markdown-fixer.mjs`）的
//! `JSON.stringify(diagnose(...))`／`JSON.stringify(fixDocument(...))` 產生後貼上的。

use std::ffi::{CStr, CString, c_char};

use markdown_fixer_ffi::{
    mdfix_detect, mdfix_diagnose, mdfix_fix, mdfix_rules_json, mdfix_string_free, mdfix_version,
};

/// 複製回傳的 JSON 字串並釋放指標。
fn take(ptr: *mut c_char) -> String {
    assert!(!ptr.is_null(), "FFI 函式不應回傳 NULL");
    // SAFETY: `ptr` 由本函式庫回傳，是有效的 NUL 結尾字串。
    let json = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .expect("回傳值必須是 UTF-8")
        .to_owned();
    // SAFETY: `ptr` 由本函式庫回傳，且只釋放這一次。
    unsafe { mdfix_string_free(ptr) };
    json
}

fn diagnose_bytes(text: &[u8], options: Option<&str>) -> String {
    let options = options.map(|o| CString::new(o).unwrap());
    let options_ptr = options.as_ref().map_or(std::ptr::null(), |o| o.as_ptr());
    // SAFETY: `text` 是有效的切片；`options_ptr` 是 NULL 或存活中的 CString。
    take(unsafe { mdfix_diagnose(text.as_ptr().cast(), text.len(), options_ptr) })
}

fn fix_bytes(text: &[u8], options: Option<&str>) -> String {
    let options = options.map(|o| CString::new(o).unwrap());
    let options_ptr = options.as_ref().map_or(std::ptr::null(), |o| o.as_ptr());
    // SAFETY: 同 `diagnose_bytes`。
    take(unsafe { mdfix_fix(text.as_ptr().cast(), text.len(), options_ptr) })
}

fn detect_bytes(text: &[u8]) -> String {
    // SAFETY: `text` 是有效的切片。
    take(unsafe { mdfix_detect(text.as_ptr().cast(), text.len()) })
}

fn assert_error_containing(json: &str, needle: &str) {
    let value: serde_json::Value = serde_json::from_str(json).expect("必須是合法 JSON");
    let object = value.as_object().expect("必須是 JSON 物件");
    assert_eq!(object.len(), 1, "錯誤物件只能有 error 鍵：{json}");
    let message = object["error"].as_str().expect("error 必須是字串");
    assert!(
        message.contains(needle),
        "錯誤訊息 {message:?} 應包含 {needle:?}"
    );
}

// ---- 不依賴核心函式庫的測試 ----

#[test]
fn version_is_static_and_matches_crate() {
    let ptr = mdfix_version();
    assert!(!ptr.is_null());
    // SAFETY: `mdfix_version` 回傳靜態的 NUL 結尾字串。
    let version = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
    assert_eq!(version, markdown_fixer::VERSION);
    // 每次呼叫回傳同一個靜態指標。
    assert_eq!(ptr, mdfix_version());
}

#[test]
fn string_free_accepts_null() {
    // SAFETY: 傳入 NULL 是明文允許的空操作。
    unsafe { mdfix_string_free(std::ptr::null_mut()) };
}

#[test]
fn invalid_utf8_text_is_an_error() {
    let bad = b"# ok\n\xff\xfe broken\n";
    assert_error_containing(&diagnose_bytes(bad, None), "text is not valid UTF-8");
    assert_error_containing(&fix_bytes(bad, None), "text is not valid UTF-8");
    assert_error_containing(&detect_bytes(bad), "text is not valid UTF-8");
}

#[test]
fn truncated_multibyte_sequence_is_an_error() {
    // 「中」的 UTF-8 是 E4 B8 AD；截斷成兩個位元組。
    let text = "中".as_bytes();
    assert_error_containing(&diagnose_bytes(&text[..2], None), "not valid UTF-8");
}

#[test]
fn null_text_with_nonzero_len_is_an_error() {
    // SAFETY: 本函式庫保證在 text 為 NULL 時不解參考，直接回傳錯誤。
    let json = take(unsafe { mdfix_diagnose(std::ptr::null(), 3, std::ptr::null()) });
    assert_error_containing(&json, "text is NULL but len is 3");
    // SAFETY: 同上。
    let json = take(unsafe { mdfix_fix(std::ptr::null(), 1, std::ptr::null()) });
    assert_error_containing(&json, "text is NULL");
    // SAFETY: 同上。
    let json = take(unsafe { mdfix_detect(std::ptr::null(), 1) });
    assert_error_containing(&json, "text is NULL");
}

#[test]
fn malformed_options_json_is_an_error() {
    for options in ["", "not json", "{", "[1]", "42", r#"{"rules":"MDX001"}"#] {
        assert_error_containing(
            &diagnose_bytes(b"# Title\n", Some(options)),
            "invalid options JSON",
        );
        assert_error_containing(
            &fix_bytes(b"# Title\n", Some(options)),
            "invalid options JSON",
        );
    }
}

#[test]
fn unknown_option_key_is_an_error() {
    let json = diagnose_bytes(b"# Title\n", Some(r#"{"ignore_rules":["MDX003"]}"#));
    assert_error_containing(&json, "unknown field `ignore_rules`");
}

#[test]
fn negative_or_fractional_max_line_length_is_an_error() {
    assert_error_containing(
        &diagnose_bytes(b"# Title\n", Some(r#"{"maxLineLength":-1}"#)),
        "invalid options JSON",
    );
    assert_error_containing(
        &diagnose_bytes(b"# Title\n", Some(r#"{"maxLineLength":80.5}"#)),
        "invalid options JSON",
    );
}

#[test]
fn options_json_with_invalid_utf8_is_an_error() {
    let options = CString::new(b"{\"rules\":[\"\xff\"]}".to_vec()).unwrap();
    let text = b"# Title\n";
    // SAFETY: `text` 是有效切片，`options` 是存活中的 NUL 結尾字串。
    let json = take(unsafe { mdfix_diagnose(text.as_ptr().cast(), text.len(), options.as_ptr()) });
    assert_error_containing(&json, "options_json is not valid UTF-8");
}

#[test]
fn error_results_can_be_freed_repeatedly_without_leaking_state() {
    // 反覆配置與釋放，搭配 sanitizer 或 leaks 工具時可以看出問題。
    for _ in 0..1000 {
        assert_error_containing(&diagnose_bytes(b"\xff", None), "not valid UTF-8");
    }
}

// ---- 需要核心函式庫完成後才能通過的測試 ----

const NBSP_DOC: &str = "#Title\n\n中文 text\u{a0}here\n";

#[test]
#[ignore = "等核心函式庫完成"]
fn rules_json_matches_library_catalog() {
    let json = take(mdfix_rules_json());
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let rules = value["rules"].as_array().expect("rules 必須是陣列");
    assert_eq!(rules.len(), 48);
    assert_eq!(rules.iter().filter(|r| r["fixable"] == true).count(), 20);
    // 直接比對原始字串：重新序列化解析後的 Value 會把鍵排序，驗不出欄位順序。
    assert!(
        json.starts_with(
            r#"{"rules":[{"id":"MDX001","name":"bom","category":"encoding","severity":"warning","fixable":true,"summary":"Remove a UTF-8 BOM from the start of the file."},"#
        ),
        "{json}"
    );
    assert_eq!(rules[47]["id"], "MDX090");
}

#[test]
#[ignore = "等核心函式庫完成"]
fn diagnose_matches_oracle_json() {
    let expected = concat!(
        r##"{"problems":[{"rule":"MDX010","name":"atx-missing-space","severity":"warning","fixable":true,"line":1,"column":2,"message":"Add a space after the heading marker: \"# \" rather than \"#\"."},"##,
        r#"{"rule":"MDX008","name":"non-breaking-space","severity":"warning","fixable":true,"line":3,"column":8,"message":"Replace non-breaking spaces (U+00A0) with normal spaces; they usually come from Word or a web page paste."}]}"#
    );
    assert_eq!(diagnose_bytes(NBSP_DOC.as_bytes(), None), expected);
    // `null` 與 `{}` 都等於預設選項。
    assert_eq!(diagnose_bytes(NBSP_DOC.as_bytes(), Some("null")), expected);
    assert_eq!(diagnose_bytes(NBSP_DOC.as_bytes(), Some("{}")), expected);
}

#[test]
#[ignore = "等核心函式庫完成"]
fn diagnose_honours_options() {
    let json = diagnose_bytes(NBSP_DOC.as_bytes(), Some(r#"{"ignoreRules":["MDX008"]}"#));
    assert_eq!(
        json,
        r##"{"problems":[{"rule":"MDX010","name":"atx-missing-space","severity":"warning","fixable":true,"line":1,"column":2,"message":"Add a space after the heading marker: \"# \" rather than \"#\"."}]}"##
    );
    // 空陣列：JS 視為 truthy，所有規則都不啟用。
    assert_eq!(
        diagnose_bytes(NBSP_DOC.as_bytes(), Some(r#"{"rules":[]}"#)),
        r#"{"problems":[]}"#
    );
    let long = format!("# T\n\n{}\n", "x".repeat(30));
    assert_eq!(
        diagnose_bytes(long.as_bytes(), Some(r#"{"maxLineLength":20}"#)),
        r#"{"problems":[{"rule":"MDX090","name":"line-length","severity":"info","fixable":false,"line":3,"column":21,"message":"Line is 30 characters; limit is 20."}]}"#
    );
}

#[test]
#[ignore = "等核心函式庫完成"]
fn unknown_rule_is_an_error() {
    let json = diagnose_bytes(b"# Title\n", Some(r#"{"rules":["MDX999"]}"#));
    assert_eq!(json, r#"{"error":"Unknown rule: MDX999"}"#);
    let json = fix_bytes(b"# Title\n", Some(r#"{"ignoreRules":["nope"]}"#));
    assert_eq!(json, r#"{"error":"Unknown rule: nope"}"#);
}

#[test]
#[ignore = "等核心函式庫完成"]
fn fix_matches_oracle_json() {
    assert_eq!(
        fix_bytes(NBSP_DOC.as_bytes(), None),
        r##"{"text":"# Title\n\n中文 text here\n","changed":true,"refused":false,"reason":""}"##
    );
    assert_eq!(
        fix_bytes(NBSP_DOC.as_bytes(), Some(r#"{"ignoreRules":["MDX008"]}"#)),
        "{\"text\":\"# Title\\n\\n中文 text\u{a0}here\\n\",\"changed\":true,\"refused\":false,\"reason\":\"\"}"
    );
    assert_eq!(
        fix_bytes(b"---\na: 1\n", None),
        r#"{"text":"---\na: 1\n","changed":false,"refused":true,"reason":"Unclosed YAML frontmatter: no closing --- or ... line was found."}"#
    );
}

#[test]
#[ignore = "等核心函式庫完成"]
fn empty_and_null_text_are_valid_input() {
    // SAFETY: text 為 NULL 且 len 為 0，代表空字串。
    let json = take(unsafe { mdfix_diagnose(std::ptr::null(), 0, std::ptr::null()) });
    assert_eq!(json, r#"{"problems":[]}"#);
    // SAFETY: 同上。
    let json = take(unsafe { mdfix_fix(std::ptr::null(), 0, std::ptr::null()) });
    assert_eq!(
        json,
        r#"{"text":"","changed":false,"refused":false,"reason":""}"#
    );
    assert_eq!(diagnose_bytes(b"", None), r#"{"problems":[]}"#);
}

#[test]
#[ignore = "等核心函式庫完成"]
fn interior_nul_round_trips_through_json() {
    // 長度制的輸入可以含 NUL；輸出以 \u0000 跳脫，仍是合法的 NUL 結尾字串。
    let json = fix_bytes(b"a\0b\n", None);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["text"], "a\0b\n");
    assert!(json.contains("\\u0000"));
}

#[test]
#[ignore = "等核心函式庫完成"]
fn detect_returns_detection_object() {
    let json = detect_bytes("# 標題\n\n- 一\n- 二\n\n```sh\necho hi\n```\n".as_bytes());
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["is_markdown"], true, "{json}");
    assert!(value["score"].as_i64().unwrap() >= 3);
    assert!(
        value["signals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "fenced-code")
    );

    let json = detect_bytes(b"#!/bin/sh\necho hello\n");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["is_markdown"], false, "{json}");
}

#[test]
#[ignore = "等核心函式庫完成"]
fn concurrent_calls_are_independent() {
    let handles: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..50 {
                    let json = diagnose_bytes(NBSP_DOC.as_bytes(), None);
                    assert!(json.starts_with(r#"{"problems":[{"rule":"MDX010""#));
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
