# dedent-paste 整合

[`dedent-paste`](https://github.com/doggy8088/dedent-paste) 是一個剪貼簿工具：按下熱鍵（macOS 預設 `左 Option+V`），它會移除剪貼簿文字共同的縮排、整理終端機提示符的折行，再貼上。從這次整合開始，它會在 dedent 之後偵測內容是不是 Markdown，是的話再用 `markdown-fixer` 函式庫套用安全修正。規格在 PRD〈6.8〉。

這份文件說明整合的行為。dedent-paste 端的實作在它的 `src/lib.rs`（`normalize_text`、`resolve_paste_settings`）與 `src/main.rs`，使用者文件以 dedent-paste repo 的 `README.md` 與 `DEVELOPMENT.md` 為準。

## 處理流程

文字路徑的核心是 `dedent_paste::normalize_text(input, markdown)`，回傳 `NormalizeOutcome`：

```text
讀剪貼簿
  │
  ▼
dedent_text（既有：去共同縮排、去行尾空白、展開提示符）
  │
  ▼
Markdown 整合關閉？ ──是──────────────> 寫回 dedent 之後的文字
  │否
  ▼
detect::looks_like_markdown
  │不是 Markdown ─────────────────────> 寫回 dedent 之後的文字
  │是
  ▼
拿掉結尾的換行，只修正本文
fix_document（ignore_rules = MDX002、MDX003）
  │refused ───────────────────────────> 寫回 dedent 之後的文字
  │
  ▼
把 dedent 結果原本的結尾換行接回去 → 寫回剪貼簿 → 貼上
```

重點：

- **順序固定**：先 dedent，再偵測，再修正。dedent 會移除共同縮排，偵測與修正看到的是已經對齊行首的文字；不先 dedent 的話，整段縮排 4 格的 Markdown 會被當成縮排 code。
- **只用函式庫**：dedent-paste 依賴 `markdown-fixer` crate，不呼叫 CLI，不需要另外安裝執行檔。
- **停用 MDX002 與 MDX003**：
  - MDX003 會替沒有檔尾換行的片段補上換行。
  - MDX002 會把 Windows 剪貼簿的 CRLF 改成 LF。停用後，`fix_document` 會以原文的行尾輸出（見 [rules.md](rules.md#mdx002-line-endings)）。
- **結尾換行不變**：修正前先拿掉結尾的換行，修正後再接回 dedent 結果原本的結尾換行。dedent-paste 對結尾換行有自己的規則（單行文字去掉結尾換行、多行文字保留），空行相關的規則不能改到它。
- **非 Markdown 的輸出不變**：偵測判定不是 Markdown 時，輸出就是 `dedent_text` 的結果，與整合前逐位元組相同。

偵測的規則與「寧可漏判、不要誤判」的取捨見 [detect.md](detect.md)。

## NormalizeOutcome

| 欄位 | 意思 |
| --- | --- |
| `text` | 要寫回剪貼簿的文字 |
| `markdown_detected` | dedent 之後的文字被判定為 Markdown |
| `signals` | 命中的偵測訊號名稱（只有名稱，不含內容） |
| `markdown_fixed` | `markdown-fixer` 改動了 dedent 之後的文字 |
| `refused` | `markdown-fixer` 拒絕修正，改用 dedent 之後的文字 |

## 拒絕修正時的回退

`fix_document` 遇到結構錯誤（MDX081、MDX083、MDX053、MDX085）或 frontmatter 陷阱時會回傳 `refused = true`。dedent-paste 把這當成正常情況：

- 寫回 dedent 之後的文字，照常貼上。
- 不顯示錯誤、不改變結束碼。
- log 記為 `detected, fix refused`。

使用者按熱鍵的期待是「貼上」，不能因為 Markdown 有結構問題就什麼都不貼。

## 開關

預設開啟。關閉方式與既有的 `--no-paste`／`DEDENT_PASTE_NO_PASTE` 同一種風格：

| 方式 | 寫法 | 說明 |
| --- | --- | --- |
| CLI 旗標 | `dedent-paste --no-markdown` | 只做 dedent，不偵測也不修正 |
| 環境變數 | `DEDENT_PASTE_MARKDOWN=0` | 值是 `0`、`false`、`no`、`off`（不分大小寫，前後空白忽略）時關閉 |

兩者任一個表示關閉就關閉。環境變數是其他值（例如 `1`）或空字串時維持開啟。`--no-markdown` 可以和 `-n`／`--no-paste`、`--paste-delay-ms` 一起使用。

在 Karabiner-Elements 的規則裡加上 `--no-markdown`，就能讓某個熱鍵只做 dedent。

## Log

dedent-paste 原本就只記錄大小，不記錄剪貼簿內容。整合後，每次處理文字的 log 行最後多一段 Markdown 摘要，例如：

```text
text dedented and pasted; before: 120 bytes, 98 chars, 6 lines; after: 122 bytes, 100 chars, 6 lines; markdown: detected, fixed (signals: atx-heading, list)
```

摘要可能是：

| 摘要 | 意思 |
| --- | --- |
| `markdown: disabled` | 被 `--no-markdown` 或 `DEDENT_PASTE_MARKDOWN` 關閉 |
| `markdown: not detected (signals: …)` | 不像 Markdown，輸出就是 dedent 的結果 |
| `markdown: detected, fixed (signals: …)` | 有修正 |
| `markdown: detected, unchanged (signals: …)` | 是 Markdown，但沒有需要修正的地方 |
| `markdown: detected, fix refused (signals: …)` | 拒絕修正，輸出 dedent 的結果 |

沒有命中任何訊號時寫 `signals: none`。log 不記錄剪貼簿文字，也不記錄問題訊息或拒絕原因。

檢視最近的 log：

```sh
dedent-paste logs -n 20
```

## 與 markdown-fixer 熱鍵的差別

`markdown-fixer` 自己也提供剪貼簿熱鍵範例（預設 `左 Option+Shift+V`，見 [clipboard.md](clipboard.md)）。兩者的差別：

| 項目 | dedent-paste 整合 | markdown-fixer 熱鍵 |
| --- | --- | --- |
| 預設熱鍵 | `左 Option+V` | `左 Option+Shift+V` |
| 去縮排與提示符展開 | 有 | 沒有 |
| 是否先偵測 | 是，不像 Markdown 就不修 | 否，一律當成 Markdown 修正 |
| 呼叫方式 | 函式庫 | `markdown-fixer --stdin --write` |

剪貼簿內容不確定是什麼時用 dedent-paste；確定是 Markdown、想強制修正時用 `markdown-fixer` 熱鍵。

## 已知行為

- **hard break 已經被 dedent 移除**：dedent 會移除每一行的行尾空白，所以兩個空格的 hard line break 在 `markdown-fixer` 看到之前就不見了。這是 dedent-paste 既有的行為，整合不會改變它。
- **圖片轉文字不經過修正**：剪貼簿只有圖片時，dedent-paste 走 Gemini 圖片轉文字的路徑，轉出的文字不呼叫 `normalize_text`。
- **相依方式**：dedent-paste 以 crates.io 上的 `markdown-fixer = "0.1"` 相依。要在 dedent-paste 裡試這個 repo 尚未發佈的變更，在本機用 `[patch.crates-io]` 暫時指向 checkout，不要 commit path 相依。

## 測試

dedent-paste 端的測試涵蓋（PRD〈6.8〉）：

- Markdown 被正規化，例如 `#標題` 變成 `# 標題`、`**重點：**說明` 變成 `**重點**：說明`、混用的 bullet 統一成 `-`。
- 程式碼與 log 不被誤判，輸出與整合前相同。
- 提示符展開後的 Markdown 也會被正規化。
- 拒絕修正時回退到 dedent 之後的文字。
- 結尾換行與 CRLF 行尾維持 dedent 之後的樣子。
- `--no-markdown` 與 `DEDENT_PASTE_MARKDOWN` 都能關閉整合。
- log 摘要只有訊號名稱，不含內容。
