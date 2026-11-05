# C ABI 共用函式庫

`crates/markdown-fixer-ffi` 把函式庫包成 C ABI，讓 C、Swift、C#、Go、Python 等語言呼叫。規格見 PRD〈6.5〉。

- **產物**：`cdylib`（`.dylib`／`.so`／`.dll`）與 `staticlib`（`.a`／`.lib`）。
- **標頭檔**：`crates/markdown-fixer-ffi/include/markdown_fixer.h`，手寫，不依賴 cbindgen。
- **資料格式**：文字以 UTF-8 位元組傳入，結果以 NUL 結尾的 UTF-8 JSON 傳回。ABI 只用到 `char *` 與 `size_t`，Rust 型別改變不會破壞 ABI。

## 建置

```sh
make ffi
# 等同於
cargo build -p markdown-fixer-ffi --release --locked
```

產物在 `target/release/`：

| 平台 | 共用函式庫 | 靜態函式庫 |
| --- | --- | --- |
| macOS | `libmarkdown_fixer_ffi.dylib` | `libmarkdown_fixer_ffi.a` |
| Linux（glibc） | `libmarkdown_fixer_ffi.so` | `libmarkdown_fixer_ffi.a` |
| Linux（musl） | 不提供（見下方說明） | `libmarkdown_fixer_ffi.a` |
| Windows（MSVC） | `markdown_fixer_ffi.dll`＋匯入程式庫 `markdown_fixer_ffi.dll.lib` | `markdown_fixer_ffi.lib` |

musl 目標預設 `crt-static`，rustc 不會產生 `cdylib`，所以 musl 版只提供靜態函式庫。

GitHub Release 的 `markdown-fixer-ffi-<版本>-<target>` 壓縮檔裡有 `include/markdown_fixer.h` 與 `lib/` 下的函式庫，見 [release.md](release.md)。

## API

```c
char *mdfix_diagnose(const char *text, size_t len, const char *options_json);
char *mdfix_fix(const char *text, size_t len, const char *options_json);
char *mdfix_detect(const char *text, size_t len);
char *mdfix_rules_json(void);
const char *mdfix_version(void);
void mdfix_string_free(char *s);
```

| 函式 | 成功時回傳 | 對應的 Rust API |
| --- | --- | --- |
| `mdfix_diagnose` | `{"problems":[...]}` | `diagnose` |
| `mdfix_fix` | `{"text":"...","changed":true,"refused":false,"reason":""}` | `fix_document` |
| `mdfix_detect` | `{"is_markdown":true,"score":5,"signals":[...]}` | `detect::looks_like_markdown` |
| `mdfix_rules_json` | `{"rules":[...]}` | `rules` |
| `mdfix_version` | `"0.1.0"`（靜態字串） | `VERSION` |

`problems` 每個元素的欄位依序是 `rule`、`name`、`severity`、`fixable`、`line`、`column`、`message`，與 JS `diagnose` 和 CLI 的 `--json` 相同。`line`、`column` 從 1 起算，`column` 的單位是 UTF-16 code unit（與 JS、LSP 相同）。

`rules` 每個元素的欄位依序是 `id`、`name`、`category`、`severity`、`fixable`、`summary`。

`mdfix_detect` 的鍵名照 Rust `Detection` 的 serde 輸出（`is_markdown`），與 CLI 的 `--detect --json` 一致。

`mdfix_fix` 的渲染保護（預設開啟，見 [library.md](library.md#渲染保護)）擋下修正時，結果多一個 `skipped` 鍵；沒有被擋下的修正時省略，JSON 形狀與 JS `fixDocument` 相同：

```json
{"text":"a \t \nb\n","changed":false,"refused":false,"reason":"","skipped":[{"rule":"MDX004","line":1,"reason":"fixes for this layout rule would change how the document renders, so none were applied in this file"}]}
```

`line` 是原文的行號（從 1 起算），`reason` 是英文說明。

## 輸入

### 文字

`text` 與 `len` 是一段 UTF-8 位元組：

- 不需要 NUL 結尾，可以含 NUL 位元組（輸出時以 `\u0000` 跳脫）。
- `text` 為 `NULL` 且 `len` 為 0 時代表空字串；`text` 為 `NULL` 但 `len` 不為 0 是錯誤。
- 不是合法 UTF-8 時回傳錯誤，不會做有損轉換。

### 選項

`options_json` 是 NUL 結尾的 JSON 字串，`NULL` 表示全部預設。鍵名與 JS 的 `options` 相同：

```json
{"rules":["MDX010","MDX090"],"ignoreRules":["MDX003"],"maxLineLength":100,"maxQuoteDepth":32,"onLimit":"error","renderGuard":true}
```

| 鍵 | 型別 | 意思 |
| --- | --- | --- |
| `rules` | 字串陣列或 `null` | 只啟用這些規則。`null` 或省略表示全部啟用；空陣列 `[]` 表示全部停用（與 JS 相同） |
| `ignoreRules` | 字串陣列或 `null` | 停用這些規則 |
| `maxLineLength` | 非負整數或 `null` | MDX090 的行長上限；0、`null` 或省略表示停用 MDX090 |
| `maxQuoteDepth` | 非負整數或 `null` | 引言巢狀深度上限；`null` 或省略表示預設 32，0 表示不限制 |
| `onLimit` | `"error"`、`"ignore"` 或 `null` | 超過上限時的處理方式。`"error"`（預設）回報 MDX086 並拒絕修正；`"ignore"` 讓超過上限的行不修改、不診斷。大小寫有別 |
| `renderGuard` | 布林值或 `null` | 渲染保護（Rust 專有），預設 `true`。`false` 時 `mdfix_fix` 的結果與 JS 逐位元組相同 |

與 CLI 的差異：

- CLI 在 `--rule` 指定 MDX090 但沒給 `--max-line-length` 時預設 120。這是 CLI 參數解析層的邏輯，C ABI 照函式庫的行為，不會自動補 120。要啟用 MDX090 請明確給 `maxLineLength`。
- 未知的鍵是錯誤（JS 會默默忽略），避免 `ignore_rules` 這類拼字錯誤讓設定無聲失效。
- 未知的規則 ID 是錯誤，訊息與 CLI 相同：`Unknown rule: MDX999`。

## 錯誤處理

任何錯誤都回傳只有一個鍵的物件 `{"error":"訊息"}`，函式永遠不回傳 `NULL`。呼叫端先檢查有沒有 `error` 鍵即可。

| 情況 | 訊息開頭 |
| --- | --- |
| `text` 為 `NULL` 但 `len` 不為 0 | `text is NULL but len is` |
| 文字不是合法 UTF-8 | `text is not valid UTF-8:` |
| `options_json` 不是合法 UTF-8 | `options_json is not valid UTF-8:` |
| `options_json` 不是合法 JSON、型別錯誤、有未知的鍵 | `invalid options JSON:` |
| 未知的規則 ID | `Unknown rule:` |
| 函式庫內部 panic | `internal error:` |

每個進入點都以 `catch_unwind` 包住，panic 不會跨過 FFI 邊界。這要求以預設的 `panic = "unwind"` 建置；如果你在自己的 profile 設了 `panic = "abort"`，panic 會直接結束行程。

函式庫不安裝 panic hook（那是整個行程共用的狀態，屬於宿主程式），所以發生 panic 時 Rust 預設的 hook 仍會在 stderr 印一行訊息。

## 記憶體管理

- `mdfix_diagnose`、`mdfix_fix`、`mdfix_detect`、`mdfix_rules_json` 回傳的字串由 Rust 配置，必須用 `mdfix_string_free` 釋放，且只能釋放一次。不可以用 `free()`。
- `mdfix_string_free(NULL)` 什麼都不做。
- `mdfix_version` 回傳靜態字串，整個行程期間有效，**不可**釋放。
- 輸入的 `text` 與 `options_json` 只在呼叫期間被讀取，函式不會保留指標。

## 執行緒

所有函式都沒有全域狀態，可以在多個執行緒同時呼叫。

## C 範例

`examples/c/main.c` 依序呼叫每個 API 並示範錯誤處理：

```sh
make ffi-example
```

手動編譯：

```sh
cargo build -p markdown-fixer-ffi --release
# macOS
cc -std=c99 -Wall -I crates/markdown-fixer-ffi/include examples/c/main.c \
   target/release/libmarkdown_fixer_ffi.a -o target/mdfix-example
# Linux（glibc）
cc -std=c99 -Wall -I crates/markdown-fixer-ffi/include examples/c/main.c \
   target/release/libmarkdown_fixer_ffi.a -lpthread -ldl -lm -o target/mdfix-example
./target/mdfix-example
```

靜態連結時需要的系統函式庫可以用 `cargo rustc -p markdown-fixer-ffi --lib --crate-type staticlib -- --print native-static-libs` 查詢。

動態連結時加上 `-L target/release -lmarkdown_fixer_ffi -Wl,-rpath,$PWD/target/release`。在 macOS 上，本機 `cargo build` 產生的 dylib install name 是建置時的絕對路徑；GitHub Release 裡的 dylib 已經用 `install_name_tool -id @rpath/libmarkdown_fixer_ffi.dylib` 改過，連結時要設定 rpath 指向你放置 dylib 的目錄。

## 其他語言

### Python（ctypes）

```python
import ctypes, json

lib = ctypes.CDLL("target/release/libmarkdown_fixer_ffi.dylib")  # Linux 用 .so
lib.mdfix_fix.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_char_p]
lib.mdfix_fix.restype = ctypes.c_void_p          # 保留原始指標，之後才能釋放
lib.mdfix_string_free.argtypes = [ctypes.c_void_p]

def fix(text: str, **options) -> dict:
    data = text.encode("utf-8")
    opts = json.dumps(options).encode("utf-8") if options else None
    ptr = lib.mdfix_fix(data, len(data), opts)
    try:
        result = json.loads(ctypes.string_at(ptr).decode("utf-8"))
    finally:
        lib.mdfix_string_free(ptr)
    if "error" in result:
        raise ValueError(result["error"])
    return result

print(fix("#Title\n", ignoreRules=["MDX003"])["text"])
```

`restype` 要設成 `c_void_p` 而不是 `c_char_p`：`c_char_p` 會自動轉成 `bytes`，原始指標就遺失了，無法交回 `mdfix_string_free`。

### Swift

把標頭檔放進 bridging header 或 module map，連結 `libmarkdown_fixer_ffi.a`：

```swift
let input = Array("#Title\n".utf8)
let ptr = input.withUnsafeBufferPointer { buf in
    buf.baseAddress!.withMemoryRebound(to: CChar.self, capacity: buf.count) {
        mdfix_fix($0, buf.count, nil)
    }
}
defer { mdfix_string_free(ptr) }
let json = String(cString: ptr!)
```

## 測試

```sh
cargo test -p markdown-fixer-ffi
```

- `src/lib.rs` 的單元測試：`catch_unwind` 的 panic 轉換、錯誤 JSON 的跳脫、選項解析。
- `tests/ffi.rs`：從 Rust 呼叫 `extern "C"` 函式，與 oracle 產生的 JSON 逐字比對，並測試非法 UTF-8、`NULL`、非法選項等錯誤路徑。
- `tests/c_example.rs`：以系統 C 編譯器（`CC` 環境變數，預設 `cc`）用 `-std=c99 -Wall -Wextra -Werror -pedantic` 編譯 `tests/c/smoke.c` 與 `examples/c/main.c`，分別靜態與動態連結後執行。找不到編譯器時印出原因並略過；Windows 上不執行。
