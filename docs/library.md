# 函式庫使用說明

`markdown-fixer` crate 是純函式庫：輸入字串、輸出問題清單或修正後的字串，不讀檔、不讀環境變數、不印東西。CLI、C ABI 與 `dedent-paste` 都是建立在它上面。

公開 API 的簽章是 crate 之間的契約，定義在 PRD〈6.3〉。改動時要同步更新 `PRD.md` 與這份文件。

## 加入相依

crates.io 版本還沒發佈（名稱待確認，見 PRD〈11〉Q2）。目前請用 path 或 git 相依：

```toml
[dependencies]
# 本機開發
markdown-fixer = { path = "../markdown-syntax-fixer/crates/markdown-fixer" }

# 或指定 git commit
# markdown-fixer = { git = "https://github.com/doggy8088/markdown-syntax-fixer", rev = "<commit>" }
```

需要把結果序列化成 JSON 時，開啟 `serde` feature：

```toml
markdown-fixer = { path = "../markdown-syntax-fixer/crates/markdown-fixer", features = ["serde"] }
```

MSRV 是 Rust 1.85（edition 2024）。

## API 一覽

| 項目 | 說明 |
| --- | --- |
| `VERSION` | crate 版本 |
| `REFERENCE_VERSION` | 對齊的 oracle 版本，目前是 `"1.0.0"` |
| `rules()` | 完整規則目錄，順序與 oracle 相同 |
| `rule(id)` | 依 ID 查規則，找不到回傳 `None` |
| `diagnose(text, &options)` | 檢查文字，回傳 `Vec<Problem>` |
| `fix_document(text, &options)` | 套用安全修正，回傳 `FixResult` |
| `Options::validate()` | 確認選項裡的規則 ID 都存在 |
| `detect::looks_like_markdown(text)` | 判斷一段文字像不像 Markdown，見 [detect.md](detect.md) |
| `detect::MAX_LINES`、`detect::MAX_INLINE_BYTES` | 偵測最多看的行數（2,000）與每行行內掃描的位元組數（4,096） |

主要型別：

| 型別 | 欄位 |
| --- | --- |
| `Severity` | `Error`、`Warning`、`Info`；`as_str()` 回傳 `"error"` 等小寫字串 |
| `Rule` | `id`、`name`、`category`、`severity`、`fixable`、`summary` |
| `Problem` | `rule`、`name`、`severity`、`fixable`、`line`、`column`、`message` |
| `Options` | `rules`、`ignore_rules`、`max_line_length` |
| `FixResult` | `text`、`changed`、`refused`、`reason` |
| `detect::Detection` | `is_markdown`、`score`、`signals` |

## 檢查文字

```rust
use markdown_fixer::{Options, Severity, diagnose};

fn report(text: &str) {
    let problems = diagnose(text, &Options::default());
    for p in &problems {
        println!(
            "{}:{} {} {} {}",
            p.line,
            p.column,
            p.severity.as_str(),
            p.rule,
            p.message
        );
    }
    let failing = problems
        .iter()
        .any(|p| matches!(p.severity, Severity::Error | Severity::Warning));
    println!("failing = {failing}");
}
```

注意事項：

- `line` 從 1 起算。CRLF 與 CR 先正規化成 LF 再分行，所以行號和編輯器看到的一致。
- `column` 從 1 起算，單位是 **UTF-16 code unit**。`中` 算 1，`😀` 算 2。要對應到 Rust 字串的 byte offset，請自己換算；要對應到 LSP 的 `Position.character`，可以直接用 `column - 1`。
- 問題已經去重，並依行號、欄號、規則 ID 排序，順序與 oracle 相同。
- `message` 是英文，與 oracle 逐字相同。

## 套用修正

```rust
use markdown_fixer::{Options, fix_document};

fn normalize(text: &str) -> String {
    let result = fix_document(text, &Options::default());
    if result.refused {
        // 結構錯誤（MDX081、MDX083、MDX053、MDX085）或 frontmatter 陷阱：
        // result.text 是原文，result.reason 是英文原因。
        eprintln!("not fixed: {}", result.reason);
    }
    result.text
}
```

`FixResult` 的四個欄位：

| 欄位 | 意思 |
| --- | --- |
| `text` | 修正後的文字；拒絕修正或沒有變更時就是原文 |
| `changed` | `text` 和輸入是否不同 |
| `refused` | 是否因為結構錯誤或 frontmatter 陷阱而拒絕修正 |
| `reason` | 拒絕的原因；沒有拒絕時是空字串 |

`refused` 為 `true` 時，`text` 一定等於輸入，呼叫端可以放心直接使用。只有 report-only 問題（例如 MDX015）的文件，`changed` 是 `false`、`refused` 也是 `false`。

## 選項

```rust
use markdown_fixer::Options;

fn options_examples() {
    // 全部規則（MDX090 預設停用）
    let all = Options::default();

    // 只啟用標題相關的規則
    let headings = Options {
        rules: Some(vec!["MDX010".into(), "MDX011".into(), "MDX012".into()]),
        ..Default::default()
    };

    // 停用某些規則
    let relaxed = Options {
        ignore_rules: Some(vec!["MDX031".into(), "MDX032".into()]),
        ..Default::default()
    };

    // 啟用 MDX090，行長上限 100
    let wrapped = Options {
        max_line_length: 100,
        ..Default::default()
    };

    let _ = (all, headings, relaxed, wrapped);
}
```

選項的語意與 JS `createEnabled` 相同：

- `rules` 是 `Some` 時，只有列出的規則會啟用。
- `ignore_rules` 裡的規則一律停用；同時出現在兩邊時以停用為準。
- `max_line_length` 為 0 時 MDX090 停用，即使 `rules` 指定了 MDX090。CLI 的「`--rule MDX090` 沒給行長就預設 120」是參數解析層的邏輯，函式庫不會自動補。
- 阻擋規則（MDX081、MDX083、MDX053、MDX085）即使被停用，`fix_document` 仍然會拒絕修正，只是 `diagnose` 不回報它們。

`diagnose` 與 `fix_document` 不驗證規則 ID。來自使用者的設定請先呼叫 `validate`：

```rust
use markdown_fixer::{Options, OptionsError};

fn parse_ignore(list: &str) -> Result<Options, OptionsError> {
    let options = Options {
        ignore_rules: Some(list.split(',').map(|s| s.trim().to_string()).collect()),
        ..Default::default()
    };
    options.validate()?;
    Ok(options)
}
```

`OptionsError::UnknownRule` 的 `Display` 是 `Unknown rule: <id>`，與 CLI 的錯誤訊息相同。

## 規則目錄

```rust
use markdown_fixer::{rule, rules};

fn list_fixable() {
    for r in rules().iter().filter(|r| r.fixable) {
        println!("{} {} — {}", r.id, r.name, r.summary);
    }
    if let Some(r) = rule("MDX042") {
        println!("{} is {}", r.id, r.severity.as_str());
    }
}
```

每條規則的中文說明在 [rules.md](rules.md)。注意 `Rule::fixable` 是規則的預設值，個別 `Problem::fixable` 可能不同：URL 裡的 NBSP（MDX008）與開頭端的 MDX042 問題會以 `fixable: false` 回報。

## 處理片段：剪貼簿與使用者輸入

剪貼簿、聊天訊息、LLM 輸出常常是不完整的 Markdown 片段：沒有檔尾換行、沒有 H1、只有幾行清單。直接套用預設選項會有兩個問題：

1. MDX003 會補上檔尾換行，貼上時多出一行。
2. MDX018 等文件層級的規則會回報對片段沒有意義的問題。

修正片段時至少停用 MDX003：

```rust
use markdown_fixer::{Options, detect, fix_document};

fn fix_snippet(text: &str) -> String {
    // 先確認這段文字像 Markdown，避免把程式碼或 log 的空行合併掉
    if !detect::looks_like_markdown(text).is_markdown {
        return text.to_string();
    }
    let options = Options {
        ignore_rules: Some(vec!["MDX003".into()]),
        ..Default::default()
    };
    fix_document(text, &options).text
}
```

只顯示問題給使用者看的話，可以再加上 `MDX018`、`MDX019` 等文件結構規則。`dedent-paste` 的整合方式見 [dedent-paste.md](dedent-paste.md)。

## serde feature

開啟 `serde` feature 後：

| 型別 | 衍生 |
| --- | --- |
| `Severity` | `Serialize`、`Deserialize`，序列化成小寫字串 |
| `Rule`、`Problem`、`FixResult`、`detect::Detection` | `Serialize` |
| `Options` | 無 |

`Problem` 序列化後的鍵名與 oracle `--json` 的 problem 物件相同：

```rust
// 需要 features = ["serde"] 與 serde_json 相依
fn to_json(text: &str) -> serde_json::Result<String> {
    let problems = markdown_fixer::diagnose(text, &markdown_fixer::Options::default());
    serde_json::to_string_pretty(&problems)
}
```

`FixResult` 序列化時 `reason` 是空字串而不是省略。CLI 的 `--json` 會在 `reason` 為空時省略這個欄位，那是 CLI 的輸出格式，不是函式庫的。

`Options` 沒有 serde 支援，因為它的 JSON 表示法（`{"rules":[...],"ignoreRules":[...],"maxLineLength":0}`，鍵名與 JS 相同）屬於 FFI 的契約，由 `markdown-fixer-ffi` 自己解析，見 [ffi.md](ffi.md)。

## 執行緒安全

- 所有公開函式都是純函式，只讀取 `&str` 與 `&Options`，沒有全域可變狀態。
- `Problem`、`FixResult`、`Options`、`Detection` 都是 `Send + Sync`，可以在執行緒之間傳遞。
- 同一個 `Options` 可以同時給多個執行緒使用。

要平行處理大量檔案，在呼叫端切分工作即可，例如用 `std::thread::scope` 或 `rayon` 的 `par_iter`。函式庫本身不開執行緒。

## 效能

- 處理時間與輸入長度大致成線性。目標是 1 MB 的 Markdown 在 Apple M 系列上 200 ms 以內修正完畢。
- `fix_document` 最多跑 6 輪修正，每輪都會重新掃描文件，所以比 `diagnose` 慢幾倍。只需要知道「有沒有問題」時用 `diagnose`。
- CLI 對每個檔案會呼叫 `diagnose` 兩次、`fix_document` 一次。嵌入時可以依需求只呼叫需要的部分。
- 呼叫端只需要一份規則目錄時，`rules()` 回傳的是 `&'static` 切片，不會配置記憶體。

## 嵌入應用程式的建議

- **只修正明確要修的東西**。編輯器的「存檔時修正」可以用 `rules` 指定幾條可修正規則，避免使用者意外看到大量 diff。
- **處理 `refused`**。拒絕修正不是錯誤，請顯示 `reason`，讓使用者手動修好結構。
- **沒變就不要寫**。`changed` 為 `false` 時不需要寫檔或寫回剪貼簿，可以省下不必要的 I/O，也不會改到檔案的修改時間。
- **不要依賴訊息文字做判斷**。用 `rule` ID 判斷問題種類；訊息是給人看的，日後可能提供中文版（PRD〈12〉）。
- **LSP 整合**。`line - 1` 與 `column - 1` 就是 LSP 的 `Position`（UTF-16），不用另外換算。
