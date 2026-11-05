# markdown-fixer

安全、行導向的 Markdown 語法檢查與自動修正工具。它把常見的 Markdown 錯誤分成兩類：可以安全自動修正的格式問題（標題空格、清單標記、表格分隔列、空行、行尾空白、CRLF、BOM、NBSP、CJK 標點旁的粗體），以及必須由人判斷的結構或語意問題（標題跳級、未關閉的 fence、表格欄數、缺少的參考定義）。

`markdown-fixer` 是 `markdown-autocorrect` skill 裡 `markdown-fixer.mjs` 的 Rust 移植版，提供三種用法：

- **CLI**：單一執行檔 `markdown-fixer`，不需要 Node.js。
- **Rust 函式庫**：`markdown_fixer::fix_document(&text, &options)`，不做 I/O，可以嵌入任何應用程式。
- **C ABI**：`cdylib`／`staticlib` 與 C 標頭檔，給 Swift、C#、Go、Python 等語言呼叫。

> **狀態**：v0.1，實作中。下文標示「規劃中」的項目還沒完成。進度以 [`PRD.md`](PRD.md)〈10. 驗收標準〉為準。

## 為什麼要有 Rust 版

原本的 Node 腳本已經在日常寫作、AI agent 產出文件與 CI 裡證明有用，但有三個限制：

| 限制 | Node 版 | Rust 版 |
| --- | --- | --- |
| 執行環境 | 需要 Node.js 20 以上 | 單一執行檔（macOS aarch64 release 版約 1.1 MB），沒有執行期相依 |
| 嵌入其他程式 | 只能當 CLI 呼叫 | Rust 函式庫與 C ABI |
| 啟動時間 | 約數十毫秒 | 15 KB 片段從啟動到輸出約 6.5 ms，適合「按熱鍵就修正剪貼簿」的即時情境 |

行為上的契約是與 Node 版一致：同樣的輸入與選項，Rust 版回報的問題（順序、行號、欄號、訊息）與修正後的文字都要和 Node 版逐位元組相同。唯一的例外是 Rust 版預設開啟的**渲染保護**：修正後實際以 CommonMark／GFM 渲染比對，會意外改變渲染結果的修正不套用，並列在輸出裡；加上 `--no-render-guard`（函式庫是 `render_guard: false`）就與 Node 版完全相同。JS 版本是行為的唯一標準（oracle），以 golden 語料與差異模糊測試驗證；目前的一致性狀態、刻意差異與已知問題見 [docs/parity.md](docs/parity.md)。

## 安裝

### 從原始碼安裝

需要 Rust 1.85 以上：

```sh
git clone https://github.com/doggy8088/markdown-fixer.git
cd markdown-fixer
cargo install --path crates/markdown-fixer-cli
```

### 預先編譯的執行檔（規劃中）

GitHub Releases 會提供 macOS（x86_64、aarch64）、Linux（x86_64、aarch64，glibc 與 musl）、Windows（x86_64、aarch64）的執行檔，以及 shell／PowerShell 安裝程式與 checksum。Homebrew tap 與 npm 包裝列為後續工作。發佈流程見 [docs/release.md](docs/release.md)。

### crates.io（規劃中）

```sh
cargo install markdown-fixer-cli
```

crates.io 上的名稱發佈前還要確認。

## 30 秒上手

```sh
# 檢查：只報告，不寫檔（預設模式）
markdown-fixer --check README.md docs/

# 修正：只寫入明確指定的檔案
markdown-fixer --write README.md docs/guide.md

# 看修改了什麼
git diff -- README.md docs/guide.md
```

`--check` 的輸出長這樣：

```text
README.md
  1:2        warning MDX010  Add a space after the heading marker: "# " rather than "#". (fixable)
  5:1        error   MDX051  Unclosed backtick code fence.

2 problem(s) in 1 file(s): 1 fixable, 1 error(s), 1 warning(s), 0 info.
```

結束碼：`0` 沒有失敗的問題；`1` 還有 warning 或 error（加 `--strict` 時 info 也算），或有檔案被拒絕修正；`2` 用法、I/O 或解析錯誤。

## CLI 範例

```sh
# 只檢查某幾條規則
markdown-fixer --check --rule MDX010,MDX011 README.md

# 忽略某條規則
markdown-fixer --check --ignore-rule MDX032 docs/

# CI 用的 JSON 輸出
markdown-fixer --check --json docs/

# info 也算失敗
markdown-fixer --check --strict docs/

# 啟用行長檢查
markdown-fixer --check --max-line-length 100 docs/

# 列出所有規則
markdown-fixer --list-rules
```

Rust 版另外提供 oracle 沒有的旗標：

```sh
# 從標準輸入讀，修正後的完整文字輸出到標準輸出
pbpaste | markdown-fixer --stdin --write --quiet --ignore-rule MDX003

# 判斷一段文字像不像 Markdown
pbpaste | markdown-fixer --stdin --detect

# 關閉渲染保護，套用所有安全修正（輸出與 Node 版完全相同）
markdown-fixer --write --no-render-guard README.md
```

`--write` 一定要明確指定檔案或目錄，不帶路徑會直接拒絕，避免意外改寫整個工作目錄。完整的旗標說明見 [docs/cli.md](docs/cli.md)。

## 函式庫範例

```rust
use markdown_fixer::{Options, diagnose, fix_document};

fn main() {
    let text = "#Title\n\n-item\n";
    let options = Options::default();

    for p in diagnose(text, &options) {
        println!("{}:{} {} {}", p.line, p.column, p.rule, p.message);
    }

    let result = fix_document(text, &options);
    if result.refused {
        eprintln!("not fixed: {}", result.reason);
    }
    assert_eq!(result.text, "# Title\n\n- item\n");
}
```

選項、序列化、片段處理與執行緒安全見 [docs/library.md](docs/library.md)；C ABI 見 [docs/ffi.md](docs/ffi.md)。

## 剪貼簿熱鍵

macOS 上可以搭配 Karabiner-Elements，按 `左 Option+Shift+V` 把剪貼簿裡的 Markdown 修正後直接貼上。設定檔與輔助腳本在 [`examples/macos/`](examples/macos/)，安裝與疑難排解（含 Linux 與 Windows 的對應寫法）見 [docs/clipboard.md](docs/clipboard.md)。

[dedent-paste](https://github.com/doggy8088/dedent-paste) 也整合了這個函式庫：按原本的 `左 Option+V`，若剪貼簿是 Markdown 就順便正規化（目前在 dedent-paste 的開發版，以 path 相依連到這個 repo，尚未發佈），見 [docs/dedent-paste.md](docs/dedent-paste.md)。

## 安全契約

- 只改 Markdown 語法，不改意思、措辭或段落排版。
- 不動 fenced code、inline code、frontmatter、數學式、URL、raw HTML、HTML comment 與連結目的地。剛好兩個行尾空格視為刻意的換行，會保留。
- 不發明 alt text、連結目標、標題層級或表格內容。只報告的規則不會被自動套用。
- 遇到未關閉的 frontmatter（MDX081）、未關閉的 HTML comment（MDX083）、不合法的 fence（MDX053）、未關閉的 `$$` 數學區塊（MDX085）、超過深度上限的巢狀引言（MDX086），或修正後會讓檔首的 `---` 變成 YAML frontmatter 時，整份文件保持原樣並回報原因。這些阻擋規則即使用 `--ignore-rule` 停用也照樣阻擋。
- 渲染保護（預設開啟）：修正不能意外改變渲染結果。排版類修正（空行、行尾空白、引言空白等）不得改變渲染；語法修復（`#標題`、`-item`、`** x **` 等）只能改變被修正的區塊，也不能改到程式碼。做不到的修正不套用，在輸出中列為 `skipped`。

這是行導向的啟發式檢查器，不是完整的 CommonMark parser；渲染保護只用 parser 來驗證，不用來產生修正。`MDX010`（`#hashtag`）、`MDX030`（以連字號開頭的英文詞）有本質上的歧義，`--write` 之後一定要看 diff。原因與已知限制見 [docs/architecture.md](docs/architecture.md) 與 [docs/rules.md](docs/rules.md)。

## 文件

| 文件 | 內容 |
| --- | --- |
| [docs/README.md](docs/README.md) | 文件索引 |
| [docs/rules.md](docs/rules.md) | 49 條規則的完整說明與範例 |
| [docs/cli.md](docs/cli.md) | CLI 旗標、輸出格式、結束碼 |
| [docs/library.md](docs/library.md) | Rust 函式庫 API |
| [docs/ffi.md](docs/ffi.md) | C ABI |
| [docs/architecture.md](docs/architecture.md) | 架構與處理流程 |
| [docs/contributing.md](docs/contributing.md) | 貢獻指南 |
| [PRD.md](PRD.md) | 產品需求文件 |

## 授權

[MIT](LICENSE)
