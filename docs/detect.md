# Markdown 偵測

`detect::looks_like_markdown` 判斷一段文字「看起來像不像 Markdown」。它是 Rust 版新增的功能，oracle 沒有對應的函式，規格在 PRD〈6.6〉。

## 為什麼需要偵測

`markdown-fixer` 的 CLI 處理的是使用者指定的 `.md` 檔，不需要猜。剪貼簿熱鍵與 `dedent-paste` 就不一樣了：剪貼簿裡可能是 Python、shell script、JSON、log、純文字，也可能是 Markdown。只有判定為 Markdown 時才套用修正。

兩種錯誤的代價差很多：

| 錯誤 | 後果 |
| --- | --- |
| 漏判：Markdown 被當成純文字 | 這次沒有被修正，原樣貼上。使用者可以手動修 |
| 誤判：程式碼被當成 Markdown | 程式碼被「修正」：Python 的多個空行被合併、`#include` 後面被補空格、`-flag` 變成 `- flag` |

所以偵測刻意**保守**：寧可漏判，不要誤判。

## API

```rust
use markdown_fixer::detect::{Detection, looks_like_markdown};

fn check(text: &str) {
    let d: Detection = looks_like_markdown(text);
    if d.is_markdown {
        println!("markdown, score {}, signals {:?}", d.score, d.signals);
    }
}
```

| 欄位 | 意思 |
| --- | --- |
| `is_markdown` | 最後的判定 |
| `score` | 命中的正向訊號權重總和 |
| `signals` | 命中的訊號名稱，穩定字串，可以寫進 log |

`signals` 只有訊號名稱，**不包含**輸入內容，所以記錄它不會洩漏剪貼簿資料。開啟 `serde` feature 時 `Detection` 可以序列化。

CLI 的對應旗標是 `--detect`：輸出 `markdown` 或 `not-markdown`，結束碼 0 表示是 Markdown、1 表示不是；加 `--json` 時輸出整個 `Detection`。詳見 [cli.md](cli.md)。

## 處理範圍

- 開頭的 BOM 會先去掉，每行結尾的 `\r` 也會去掉，所以 CRLF 不影響判斷。
- 只看前 2,000 行（`detect::MAX_LINES`）。超過的部分不影響結果。
- 連結、強調、inline code 等行內掃描，每行最多看前 4,096 個位元組（`detect::MAX_INLINE_BYTES`），整份文字合計最多 256 KB。工作量與輸入長度成線性。
- 全部是空白的輸入一律判定為非 Markdown，分數 0。

掃描前會先找出檔首的 frontmatter 與**成對**的 fenced code。fence 裡的內容完全不參與正向或反向訊號的判斷，所以 Markdown 文件裡附一段 Python 範例不會被當成程式碼。沒有關閉的 fence 不算訊號，它後面的行照常檢查，避免程式碼裡出現一個 ```` ``` ```` 就躲過程式碼判斷。

## 正向訊號

每一種訊號只計一次分數，不會因為有十個標題就加十次。`signals` 依下表的順序列出命中的訊號，不重複。

| 訊號 | 條件 | 權重 |
| --- | --- | --- |
| `fenced-code` | 至少一段成對的 ```` ``` ```` 或 `~~~` fence | 3 |
| `table` | 含 `\|` 的表頭列，下一行是格數相同的分隔列 | 3 |
| `atx-heading` | `#` 到 `######` 後接空白與文字；文字不像程式碼，而且下一行是空行、檔尾或另一個 Markdown 區塊 | 2 |
| `link` | `[文字](目的地)` 或 `![alt](src)`，目的地看起來像網址或路徑 | 2 |
| `list` | 連續兩行以上的清單項目（`- `、`* `、`+ `、`1. `、`1) `） | 1 |
| `task-list` | 清單項目以 `[ ]`、`[x]`、`[X]` 開頭 | 2 |
| `emphasis` | 成對的 `**粗體**`，或成對且內容不是空白的 inline code | 1 |
| `blockquote` | `>` 開頭（排除 Python REPL 的 `>>>`） | 1 |
| `reference-definition` | `[label]: url`，或註腳定義 `[^1]: 文字` | 2 |
| `frontmatter` | 檔首 `---` 到下一個 `---`，而且後面還有內容 | 2 |
| `syntax-error` | 常見的 CJK Markdown 筆誤，見下方 | 2 |

`syntax-error` 涵蓋三種寫法：

- `#標題`：`#` 後面直接接 CJK 字元。和 `atx-heading` 一樣，下一行必須是空行、檔尾或另一個 Markdown 區塊才算，否則 `#安裝套件` 這種緊貼指令的 shell 註解會被誤認。
- `-項目`、`+項目`：清單符號後面直接接 CJK 字元。ASCII 的 `-item` 不算，因為它太常是命令列參數。
- `**標籤：**內容`：收尾的 `**` 前面是全形標點，後面直接接文字。

`atx-heading` 的「不像程式碼」指標題文字不以 `;`、`{`、`}`、`(`、`)`、`\` 結尾，也不含 ` = `、`()`、`=>`、`&&`、`||`。

> `syntax-error` 的權重是 2（PRD〈6.6〉最初建議 1，已依實作更新）。`**標籤：**內容` 這種兩、三行的 AI 輸出片段只會命中 `emphasis` 與 `syntax-error`，權重 1 時永遠到不了門檻。提高權重的風險由上面 `#標題` 的區塊條件抵銷。

## 反向訊號

反向訊號只影響 `is_markdown`，不會出現在 `signals` 裡。fence 與 frontmatter 內的行不檢查；標題、清單、blockquote、參考定義、表格列與 `#` 開頭的行不做程式碼特徵檢查。

**一出現就否決**的訊號：

- 第一個非空白行是 `#!` 開頭的 shebang。
- 前置處理指令：`#include`、`#define`、`#pragma`、`#region`、`#endregion`、`#if`、`#ifdef`、`#ifndef`、`#elif`、`#else`、`#endif`、`#undef`、`#import`、`#error`、`#warning`、`#line`。
- unified diff 的標頭：`diff --git`、`@@ -… @@`、`--- a/`、`+++ b/`、`index <hash>..`、`new file mode`、`deleted file mode`。
- stack trace 或 panic 的標記：`Traceback (most recent call last)`、`Exception in thread`、`Caused by:`、`Uncaught`、`thread '…' panicked at`、`goroutine <數字>`。
- Python 或 TOML 多行字串的界線：以 `"""` 或 `'''` 開頭或結尾的行。fence 外的 Markdown 不會出現這種行。
- `#` 註解區塊：中間沒有空行的連續三行以上 `#` 開頭的行，或連續兩行而其中一行是單獨的 `#` 或 `#foo`（`#` 後直接接 ASCII 字元）。Markdown 很少連續放三個標題，也不會有空標題。`#[...]` 與 `#![...]`（Rust attribute）不算在內。
- YAML 的 block scalar：不在清單裡、值是 `|`、`>`、`|-`、`>-`、`|+`、`>+` 的 `key:` 行。
- 只有程式碼才有的宣告：Python 的 `def`、`class`，各語言的函式宣告，`const`／`let`／`var` 宣告，`export`，Java／C# 的存取修飾詞，C 風格的函式宣告。
- 第一個內容行是 `<?xml` 或 `<!doctype`，或整份以 JSON 陣列開頭（README 開頭的 badge 連結不算）。

**依比例成立**的訊號（分母是 fence 與 frontmatter 以外的非空白行）：

| 訊號 | 門檻 |
| --- | --- |
| 堆疊框架行（`at foo (file.js:1:2)`、`File "x.py", line 3`） | 2 行以上 |
| 弱程式碼特徵（`//`、`/*`、`$ `、`>>> `、裝飾器、常見 shell 指令、SQL 與 Dockerfile 關鍵字、以 tab 開頭的非清單行等） | 20% 以上 |
| log 行（行首是時間戳記或 `INFO`、`ERROR` 等記錄層級） | 30% 以上 |
| YAML 的 `key: value`（至少一行不在清單裡，至少 2 行） | 扣掉 `#` 開頭的行後 60% 以上 |
| TOML／INI／`.env` 的 `key = value`、`[section]`（至少 2 行） | 扣掉 `#` 開頭的行後 60% 以上 |
| 第一個內容行以 `{` 開頭 | 直接成立 |
| 第一個內容行以 `<` 開頭，且 `<` 開頭的行佔一半以上 | 直接成立 |

## 判定規則

同時滿足下列條件才判定為 Markdown：

1. 沒有任何反向訊號成立。
2. 只有一行非空白內容時：必須命中 `syntax-error`。
3. 兩行以上時：分數至少 3，而且至少命中兩種訊號，或命中一個權重 3 的訊號（`fenced-code`、`table`）。

一行 `# 標題` 可能是 shell 註解，一行 `- item` 可能是 diff；但 `**標籤：**內容` 這種筆誤幾乎只會出現在 Markdown，所以單行只認 `syntax-error`。

下表是實際執行 `looks_like_markdown` 的結果：

| 輸入 | `signals` | `score` | 結果 |
| --- | --- | --- | --- |
| H2 標題、空行、兩行清單 | `atx-heading`、`list` | 3 | Markdown |
| 只有一段 fenced code | `fenced-code` | 3 | Markdown |
| 一個表頭、分隔列、一列資料 | `table` | 3 | Markdown |
| 兩行 `**狀態：**完成` 這類 AI 輸出 | `emphasis`、`syntax-error` | 3 | Markdown |
| 單行 `#標題` | `syntax-error` | 2 | Markdown |
| 單行 `# Title` | `atx-heading` | 2 | 不是（單行） |
| 只有兩行清單 | `list` | 1 | 不是（分數不足） |
| 單行 `-item` | 無 | 0 | 不是 |
| `#!/bin/bash` 開頭，後面有標題與清單 | `atx-heading`、`list` | 3 | 不是（shebang） |
| `# install deps` 緊接 `npm install` | 無 | 0 | 不是 |
| Python 腳本（`import os`、`def main():`） | 無 | 0 | 不是 |
| YAML 設定檔 | 無 | 0 | 不是 |
| 帶時間戳記的 log | 無 | 0 | 不是 |
| `{"a": 1}` | 無 | 0 | 不是 |
| H1 標題加一段 Python fenced code | `fenced-code`、`atx-heading` | 5 | Markdown |
| 連續三行 `#` 開頭的註解，後面接兩行清單 | `atx-heading`、`list` | 3 | 不是（`#` 註解區塊） |
| `import Tabs from '@theme/Tabs';` 開頭的 MDX 文件 | `atx-heading`、`list` | 3 | 不是（程式碼宣告） |

注意後面幾個例子：正向訊號照樣計分，但只要反向訊號成立，`is_markdown` 就是 `false`。

## 調整與測試

偵測是啟發式的，一定會有判斷錯的輸入。調整時照這個流程：

1. **先寫測試**。把判斷錯的輸入加進 `crates/markdown-fixer/src/detect.rs` 的 `tests` 模組，正例放 Markdown，反例放程式碼、log、JSON、YAML、shell script、Python、純文字。PRD 要求每一類至少 10 個。權重與門檻以這個檔案為準，調整時同步更新模組文件與這份文件。
2. **優先加反向訊號，不要降低門檻**。誤判比漏判嚴重，遇到誤判時找出能排除它的特徵；遇到漏判時先確認提高分數不會讓既有的反例變成誤判。
3. **跑全部偵測測試**。任何反例從「不是 Markdown」變成「是 Markdown」都要停下來檢查。
4. **訊號名稱是穩定字串**。新增訊號可以，改名或刪除會影響依賴 log 的使用者，要記在變更說明裡。

```sh
CARGO_TARGET_DIR=target/<你的名字> cargo test -p markdown-fixer detect
```

要快速試一段文字，可以用 CLI：

```sh
pbpaste | markdown-fixer --stdin --detect --json
```

## 偵測不負責的事

- 已知的取捨：Docusaurus 或 MDX 文件裡的 `import X from '...'` 會被當成程式碼宣告，整份判定為非 Markdown。
- 偵測不保證文件「正確」，只判斷像不像 Markdown。是否要修正、修正結果，由 `fix_document` 決定。
- 偵測不處理 MDX、AsciiDoc、reStructuredText 等其他格式，它們可能被判成 Markdown 或不是，都不保證。
- 判定為 Markdown 之後，`fix_document` 仍可能因為結構錯誤拒絕修正。呼叫端要處理 `refused`，見 [library.md](library.md#套用修正)。
