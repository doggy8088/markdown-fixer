---
name: markdown-autocorrect
description: 檢查、整理或自動校正 Markdown 的常見語法錯誤：標題空格、清單標記、強調符號內距、表格分隔列、空行、行尾空白、CRLF/BOM、全形標點、NBSP、參考連結、數學區塊與 CJK 排版。Use when asked to lint, clean, normalize, or fix Markdown; when Markdown renders differently than intended; before committing documentation; or when a paste conversion produces broken headings, lists, tables, links, or math blocks. 觸發詞：Markdown 校正、Markdown 整理、Markdown 檢查、修 Markdown、fix markdown、lint markdown、normalize markdown、math block. **必須由使用者明確調用才使用：Claude Code 以 `/markdown-autocorrect`、Codex CLI 以 `$markdown-autocorrect`；不要自動觸發。**
disable-model-invocation: true
---

# Markdown Autocorrect

> **調用方式（明確調用）：** 本 skill 只在使用者明確調用時執行——Claude Code 用 `/markdown-autocorrect`，Codex CLI 用 `$markdown-autocorrect`；未被點名時不要自動套用。

用一個零依賴的 Node 腳本，把 Markdown 的常見語法錯誤分成兩類：可自動修正的格式問題，以及必須人工判斷的結構或語意問題。流程是先檢查、再只寫入明確指定的檔案、看過 diff、最後重跑確認。

## 快速開始

```bash
# 檢查（預設模式；不帶路徑時掃描目前目錄）
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check README.md docs/

# 只寫入明確指定的檔案
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --write README.md docs/guide.md

# 只看某幾條規則
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check --rule MDX010,MDX011 README.md

# 忽略某條規則
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check --ignore-rule MDX032 docs/

# CI 用的 JSON
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check --json docs/

# 放寬引言巢狀深度上限（預設 32；0 表示不限制），或把過深的行當成不透明的行略過
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check --max-quote-depth 64 docs/
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --write --on-limit ignore notes.md

# 列出規則
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --list-rules
```

- 需要 Node.js 20 以上（建議 22+），沒有 npm 依賴。
- `--check` 是預設模式，不會寫檔。
- `--write` 必須明確指定路徑；若給目錄，會處理該目錄下所有掃描到的 Markdown。先跑 `--check` 看清單；不帶路徑會直接拒絕，避免意外重寫整個工作樹。
- 結束碼：`0` 沒有需要處理的問題；`1` 仍有 warning/error（加 `--strict` 時 info 也算）；`2` 用法、I/O 或解析錯誤。
- `--rule MDX090` 會自動啟用 120 字元行長；也可用 `--max-line-length N` 指定。
- `--json` 的 `changed` 表示這次真的寫入，`wouldChange` 表示檔案需要變更；check 模式的 `changed` 一定是 `false`。
- `--max-quote-depth N`（或 `--max-quote-depth=N`）設定引言（`>`）巢狀深度上限，預設 32，`0` 表示不限制；只接受非負整數。
- `--on-limit error|ignore` 決定超過上限時怎麼做。`error`（預設）在該行回報 MDX086，整份檔案拒絕寫入；`ignore` 把超過上限的行當成不透明的行：不修改、不診斷，也不當成 reference definition。
- 函式庫 API 對應的選項是 `maxQuoteDepth` 與 `onLimit`。

## 工作流程

1. **界定範圍。** 列出這次要處理的 Markdown 檔案；排除 `vendor/`、`node_modules/`、產生的檔案與 golden fixtures。
   **完成條件：** 有一份明確的檔案清單，知道哪些檔案不會被碰。

2. **先檢查，不要先寫。** 對清單執行 `--check`，記錄 rule ID、行號與 severity。
   **完成條件：** 能分辨哪些是 fixable、哪些是 report-only、哪些是結構錯誤。

3. **判斷風險。** 如果出現 MDX081（未關閉 frontmatter）、MDX083（未關閉 HTML comment）、MDX053（非法 fence info）、MDX085（未關閉 `$$` 數學區塊）或 MDX086（引言巢狀超過上限），先人工修好結構，或對 MDX086 明確調整 `--max-quote-depth`／`--on-limit`，不要期待 `--write` 處理。
   **完成條件：** 沒有會讓工具拒絕寫入的結構錯誤。

4. **只寫入明確檔案。** 用 `--write <files>`，一次一個範圍。不要用 `--write .`。
   **完成條件：** 指令結束，且沒有寫到清單以外的檔案。

5. **看 diff。** 執行 `git diff -- <files>`，逐段確認。特別注意標題、清單邊界、表格欄數與 CJK 行長。
   **完成條件：** 每一個 diff 都能說明為什麼要改；不想要的變更已用 `--ignore-rule` 或人工回退排除。

6. **人工處理 report-only 項目。** 依 `reference/rules.md` 的說明修正，或用 `--ignore-rule` 明確接受。
   **完成條件：** 每個剩下的 warning/error 都有處置理由。

7. **重跑驗證。** 再跑一次 `--check`；目標是 0 warning/0 error，或只剩已記錄理由的 info。
   **完成條件：** 指令結束碼為 `0`，或剩下的項目已明確說明。

8. **依專案規範收尾。** 若這是要提交的變更，照 `AGENTS.md` 更新版號、跑對應測試。
   **完成條件：** 專案要求的檢查與版號更新都完成。

## 安全契約

- 只改 Markdown 語法，不改意思、不改措辭、不重新排版段落。
- 不動 fenced code、inline code、frontmatter、數學式、URL、raw HTML、連結目的地。兩個行尾空格視為刻意換行，會保留。
- 不發明 image alt text、連結目標、標題層級或表格內容。
- 不確定的地方留給人判斷；report-only 規則不會被 `--write` 套用。
- 修正不得改變渲染結果：在引言或清單裡補空行時帶上相同的容器前綴（例如 `>`），無法保持結構時就不修。
- `MDX081`、`MDX083`、`MDX053`、`MDX085`、`MDX086`，以及「修正後會把 `---` 變成 frontmatter」的情況，會讓檔案保持原樣並回報原因；這是刻意的安全拒絕，不是 bug。blocking 規則即使被 `--ignore-rule` 點名，也不會放行寫入。
- 這個工具是行導向的啟發式檢查器，不是完整 CommonMark parser。跨行的 inline 結構、MDX/JSX、複雜 YAML 不會被完整解析。

## 可自動修正的規則

| Rule | 內容 |
| --- | --- |
| MDX001 | 移除檔案開頭的 UTF-8 BOM |
| MDX002 | 把 CRLF/CR 統一成 LF |
| MDX003 | 檔尾保留剛好一個換行 |
| MDX004 | 移除行尾空白；保留兩個空格的 hard break |
| MDX005 | 多個連續空行縮成一個 |
| MDX006 | 移除檔首空行；空行後的那一行以 U+FEFF 開頭時不修（否則它會變成 BOM），只報告 |
| MDX008 | 把 code/URL 以外的 NBSP 換成普通空格；NBSP 是整行或清單項目唯一的內容時不換；緊接在清單標記後的 NBSP（`*<NBSP>文字`）只報告 |
| MDX010 | `#Title` → `# Title` |
| MDX011 | 標題標記後只留一個空格 |
| MDX012 | 移除 ATX 標題尾端的 `###` |
| MDX013 | 標題與相鄰段落之間補空行 |
| MDX020 | blockquote 的 `>` 之間與之後只留一個空格；只移除一般內容前多出的一個空格，保留可能是清單延續的兩到三個空格、fence 與清單標記前的空格，以及縮排 code |
| MDX030 | `-item` → `- item`、`1.item` → `1. item` |
| MDX040 | 移除 `**`、`__`、`*`、`_` 內側多餘空白（單一 `*`/`_` 只修非對稱情況；開頭 delimiter 前面緊鄰文字或標點時不修；行首當作清單標記的 `*` 不算） |
| MDX042 | 部分：CJK／全形標點旁配對不成立的強調，收尾端 `**X：**Y` → `**X**：Y`；開頭端問題只報告 |
| MDX052 | 頂層 fenced code 與相鄰 block 之間補空行；引言裡的 fence 補 `>`（依深度 `> >`…）空行，標記後還有縮排的 fence（可能在清單項目裡）不處理 |
| MDX060 | 表格分隔列每格至少三個 `-`；表頭與分隔列的儲存格數（以 GFM 規則分割，code span 裡的直線符號也算）不同時不是表格，不處理 |
| MDX062 | 頂層表格與相鄰 block 之間補空行 |
| MDX082 | frontmatter 後補一個空行 |

這些修正仍要看 diff。`MDX010` 無法分辨 `#hashtag` 與壞掉的標題；`MDX030` 無法分辨 `-item` 與以連字號開頭的英文字。

## 只報告、不自動修正的規則

- **標題與結構：** MDX014 縮排標題、MDX015 跳級、MDX016 空標題、MDX017 標題尾端標點、MDX018 第一個標題不是 H1、MDX019 多個 H1、MDX080 Setext 歧義、MDX081 未關閉 frontmatter、MDX083 未關閉 HTML comment、MDX084 全形 Markdown 語法、MDX085 未關閉的 `$$` 數學區塊。
- **清單與強調：** MDX031 同一層清單混用 bullet（CommonMark 裡換一種 bullet 就是新的清單，統一後會合併成一個清單，所以 v1.1.0 起只報告）、MDX032 清單與段落之間缺空行、MDX035 空清單項目、MDX041 可能未配對的強調符號、MDX042 只報告的部分（開頭端配對不成立的 `**`／`*`，或單一 `*`；收尾端 `**X：**Y` 會自動修）。
- **Code fence：** MDX050 缺少語言、MDX051 未關閉 fence、MDX053 非法 fence info。
- **表格：** MDX061 欄數不一致、MDX063 表格 cell 內的 code/URL 含未轉義的 `|`。
- **連結與圖片：** MDX070 裸 URL、MDX071 空連結、MDX072 `] (` 的空格、MDX073 缺少 reference definition、MDX074 重複定義、MDX075 圖片缺 alt。
- **其他：** MDX007 tab、MDX009 零寬字元、MDX086 引言巢狀超過 `--max-quote-depth`（blocking）、MDX090 行長。

完整說明、before/after 範例、markdownlint 近似對應與 CJK 注意事項在 [`reference/rules.md`](reference/rules.md)。

## 常見操作

```bash
# 只修標題相關
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --write \
  --rule MDX010,MDX011,MDX012,MDX013 README.md

# 先看某個目錄有多少問題，不要寫入
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check docs/

# 把 info 也視為失敗，適合嚴格 CI
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check --strict docs/

# 只看前幾行問題
node .agents/skills/markdown-autocorrect/scripts/markdown-fixer.mjs --check docs/ | head -40
```

## Paste-to-Markdown 專案注意事項

- 這個 skill 處理的是**專案裡的 Markdown 文件**，不是 app 的貼上轉換邏輯。執行期轉換在 `assets/clipboard2markdown.js` 與 `assets/to-markdown.js`；要改貼上行為，必須改 app 邏輯並補 `e2e/` 測試，不能只跑這支腳本。
- `tests/tcN-perfect.md` 是 golden fixtures，代表 app 的預期輸出。除非轉換行為刻意改變，否則不要對它們跑 `--write`；若真的改了轉換行為，連同 fixture 與對應的 E2E 期望一起更新。
- 不要在這個 repo 執行 `--write .`。它會掃到 `AGENTS.md`、`tasks/todo.md` 與 golden fixtures，產生不必要的 diff。
- 任何要提交的變更都要照 `AGENTS.md`：`index.html` 的 footer 與 JSON-LD `softwareVersion` 同步 bump patch 版號；只有改到 JS/CSS 時才需要更新 cache-busting query string。
- 這支腳本自己的測試：
  ```bash
  node --test .agents/skills/markdown-autocorrect/scripts/markdown-fixer.test.mjs
  ```

## 已知限制

- Frontmatter 必須從檔案第一個位元組開始（可有 BOM）。`---` 前面若有空行，語意上是 thematic break；若移除空行會讓第一行（忽略 BOM）變成 `---`，即使那行可能只是 thematic break，工具也會拒絕寫入，請人工確認。
- HTML block 採 CommonMark type 1–7 的近似判斷。引言或清單標記後開始的多行 HTML 區塊（comment、`<pre>`、`<script>`、`<style>`、`<?…?>`、CDATA 等以結束標記收尾的區塊）也受保護，一直到結束標記；沒有關閉的 comment 會拒絕寫入（MDX083）。type 6/7 會持續到下一個空行，所以 `</div>` 後面沒有空行時，後面的 Markdown 會被視為 raw HTML 而不處理。
- 縮排 4 個空白欄位（tab 以 4 欄計）以上的表格、blockquote 內的表格，可能不會被偵測；清單內縮排 1–3 個空格的表格會偵測，但 MDX062 不處理。
- 縮排段落延續（lazy continuation）與 indented code 的邊界用啟發式判斷；4 欄以上的行若緊接段落，會視為段落延續。
- 以 `---` 開頭且後面有第二條 `---` 的檔案一律視為 frontmatter；若其實是 Setext 標題或水平線，先人工確認。若移除檔首空行會讓後面的 `---` 變成 frontmatter，`--write` 會拒絕寫入。
- 目錄掃描預設跳過 `.git`、`node_modules`、`vendor`、`coverage`、`test-results`、`playwright-report`、`blob-report`、`.e2e-tmp`；其他隱藏目錄（例如 `.github`、`.agents`）仍會掃描。
- 行長以 Unicode code point 計算，CJK 算 1，不是終端顯示寬度；表格行不計入 MDX090。
- 跨行的 link/emphasis、巢狀 blockquote 內的標題與清單、複雜 YAML 不做完整解析。
- Fence 的開關以行判斷，不追蹤容器：清單項目或引言裡開的 fence，若被容器外的 fence 標記「關閉」，工具會把之後的行當成一般文字，但 CommonMark 會把它們渲染成程式碼。遇到這種情況請先把 fence 放回同一個容器。
- 多行數學區塊以行首、blockquote、清單或 task-list 內容開頭的 `$$` 開始，之後任一行含 `$$` 結束；若區塊內容本身含 `$$` 會提前結束。行首 `$$` 一律視為數學區塊開頭；若只是文字，請改寫或加反斜線。跨行單一 `$...$` 不支援；` ```math ` 仍由 fenced code 保護。更深的混合巢狀（例如清單內 blockquote 再清單）可能無法精確配對，會保守地回報 MDX085；`\[...\]`、`\begin{...}` 等其他數學語法也不特別保護。
- `MDX002` 停用時，若沒有其他修正，檔案維持原樣（含混合行尾）；若有其他修正，行尾會以 CRLF 優先、其次 CR 統整。
- URL 內的 NBSP 只報告、不自動改；請人工換成 `%20` 或修正 URL。
