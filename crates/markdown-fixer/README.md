# markdown-fixer

安全、行導向的 Markdown 語法檢查與自動修正函式庫。這是 [`markdown-autocorrect`](https://github.com/doggy8088/markdown-fixer) skill 裡 `markdown-fixer.mjs` 的 Rust 移植版，在同樣的輸入與選項下（`render_guard: false`），輸出與 JS 參考實作逐位元組相同。

- 渲染保護（`render-guard` feature 與 `Options::render_guard`，預設開啟）：以 [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark) 渲染修正前後的文件比對，會意外改變渲染的修正不套用，列在 `FixResult::skipped`。`default-features = false` 時沒有任何相依。

- 不做 I/O、不讀環境變數、不 panic，結果具決定性。
- 只修 Markdown 語法，不改措辭；fenced code、inline code、frontmatter、數學式、URL、raw HTML 都受保護。
- 欄號以 UTF-16 code unit 計算，與 JavaScript 和 LSP 相同。

```rust
use markdown_fixer::{Options, diagnose, fix_document};

let text = "#標題\n\n-項目\n**說明：**內容\n";
let options = Options::default();

for problem in diagnose(text, &options) {
    println!("{}:{} {} {}", problem.line, problem.column, problem.rule, problem.message);
}

let result = fix_document(text, &options);
assert_eq!(result.text, "# 標題\n\n- 項目\n**說明**：內容\n");
```

命令列工具請安裝 [`markdown-fixer-cli`](https://crates.io/crates/markdown-fixer-cli)。完整文件與規則說明見 [GitHub repo](https://github.com/doggy8088/markdown-fixer)。

授權：MIT。
