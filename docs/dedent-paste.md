# dedent-paste 整合

[`dedent-paste`](https://github.com/doggy8088/dedent-paste) 是一個剪貼簿工具：按下熱鍵（macOS 預設 `左 Option+V`），它會移除剪貼簿文字共同的縮排、整理終端機提示符的折行，再貼上。這份文件說明它怎麼使用 `markdown-fixer` 函式庫，在貼上前順便把 Markdown 正規化。規格在 PRD〈6.8〉。

> **狀態**：規劃中。下面描述的是 PRD 規定的行為；整合完成後，以 dedent-paste repo 的 `README.md` 與 `DEVELOPMENT.md` 為準。

## 處理流程

```text
讀剪貼簿
  │
  ▼
dedent_text（既有：去共同縮排、去行尾空白、展開提示符）
  │
  ▼
Markdown 整合關閉？ ──是──> 寫回剪貼簿（與整合前完全相同）
  │否
  ▼
detect::looks_like_markdown
  │不是 Markdown ─────────> 寫回 dedent 之後的文字
  │是
  ▼
fix_document（停用 MDX003）
  │refused ───────────────> 寫回 dedent 之後的文字
  │
  ▼
寫回修正後的文字 → 貼上
```

重點：

- **順序固定**：先 dedent，再偵測，再修正。dedent 會移除共同縮排，偵測與修正看到的是已經對齊行首的文字；不先 dedent 的話，整段縮排 4 格的 Markdown 會被當成縮排 code。
- **只用函式庫**：dedent-paste 依賴 `markdown-fixer` crate，不呼叫 CLI，不需要另外安裝執行檔。
- **停用 MDX003**：dedent-paste 對檔尾換行已有自己的規則（單行文字去掉結尾換行、多行文字保留）。修正時停用 MDX003，避免 `markdown-fixer` 補上或刪掉檔尾換行。
- **非 Markdown 的輸出不變**：偵測判定不是 Markdown 時，輸出與整合前逐位元組相同。這是硬性要求，既有測試必須全部維持通過。

偵測的規則與「寧可漏判、不要誤判」的取捨見 [detect.md](detect.md)。

## 拒絕修正時的回退

`fix_document` 遇到結構錯誤（MDX081、MDX083、MDX053、MDX085）或 frontmatter 陷阱時會回傳 `refused = true` 與原文。dedent-paste 把這當成正常情況：

- 寫回 dedent 之後的文字，照常貼上。
- 不顯示錯誤、不改變結束碼。
- log 記一筆「拒絕修正」，不記內容。

使用者按熱鍵的期待是「貼上」，不能因為 Markdown 有結構問題就什麼都不貼。

## 開關

預設開啟。關閉方式與既有的 `--no-paste`／`DEDENT_PASTE_NO_PASTE` 同一種風格：

| 方式 | 寫法 | 說明 |
| --- | --- | --- |
| CLI 旗標 | `dedent-paste --no-markdown` | 這一次執行不做 Markdown 偵測與修正 |
| 環境變數 | `DEDENT_PASTE_MARKDOWN=0` | 長期關閉 |

兩者同時存在時旗標優先。在 Karabiner-Elements 的規則裡加上 `--no-markdown`，就能讓某個熱鍵只做 dedent。

## Log

dedent-paste 原本就只記錄大小，不記錄剪貼簿內容。整合後，每次執行多記下列資訊：

- 是否判定為 Markdown。
- 命中的偵測訊號名稱（`Detection::signals`，例如 `atx-heading,list`）。
- 是否修正、是否被拒絕。
- 修正前後的大小（bytes、字元數、行數）。

不記錄：剪貼簿文字、問題訊息裡引用的內容、拒絕原因以外的任何文件片段。

檢視 log：

```sh
dedent-paste logs -n 20
```

## 與 markdown-fixer 熱鍵的差別

`markdown-fixer` 自己也提供剪貼簿熱鍵範例（預設 `左 Option+Shift+V`，見 [clipboard.md](clipboard.md)）。兩者的差別：

| | dedent-paste 整合 | markdown-fixer 熱鍵 |
| --- | --- | --- |
| 預設熱鍵 | `左 Option+V` | `左 Option+Shift+V` |
| 去縮排與提示符展開 | 有 | 沒有 |
| 是否先偵測 | 是，不像 Markdown 就不修 | 否，一律當成 Markdown 修正 |
| 呼叫方式 | 函式庫 | `markdown-fixer --stdin --write` |

剪貼簿內容不確定是什麼時用 dedent-paste；確定是 Markdown、想強制修正時用 `markdown-fixer` 熱鍵。

## 已知行為

- **hard break 已經被 dedent 移除**：dedent 會移除每一行的行尾空白，所以兩個空格的 hard line break 在 `markdown-fixer` 看到之前就不見了。這是 dedent-paste 既有的行為，整合不會改變它。
- **圖片轉文字**：剪貼簿只有圖片時，dedent-paste 會走 Gemini 圖片轉文字的路徑。PRD 只規定文字路徑的整合，圖片轉出的文字不經過 Markdown 修正。
- **相依方式**：開發期以 path 相依指向這個 repo，dedent-paste 的 CI 與 release 在改成 git 或 crates.io 相依之前會失敗（PRD〈11〉Q1）。

## 測試

dedent-paste 端要涵蓋的測試（PRD〈6.8〉）：

- Markdown 被正規化，例如 `#標題` 變成 `# 標題`。
- 程式碼與 log 不被誤判，輸出與整合前相同。
- 提示符展開後的 Markdown 也會被正規化。
- 拒絕修正時回退到 dedent 之後的文字。
- `--no-markdown` 與 `DEDENT_PASTE_MARKDOWN=0` 都能關閉整合，且旗標優先。
