# 與 oracle 的一致性（parity）

這份文件說明 Rust 版與 JS 參考實作（oracle，`reference/js/markdown-fixer.mjs` v1.1.0）一致是什麼意思、golden 檔怎麼產生、oracle 升版怎麼同步，並記錄刻意差異與 oracle 的疑似問題。測試怎麼跑見 [testing.md](testing.md)。

## 一致性的定義

**函式庫層級**：同一份輸入文字與同一組選項下，`diagnose` 回傳的問題清單與 JS `diagnose` 完全相同，包括元素順序與每個欄位（`rule`、`name`、`severity`、`fixable`、`line`、`column`、`message`）；`fix_document` 回傳的 `text`、`changed`、`refused`、`reason` 與 JS `fixDocument` 完全相同。`column` 的單位是 UTF-16 code unit。比對時 Rust 一律以 `render_guard = false` 執行（PRD〈6.9〉）；渲染保護開啟時，只有被它擋下的修正（`FixResult::skipped`）會與 oracle 不同。

**CLI 層級**：旗標、結束碼、文字輸出、`--json` 結構與 oracle CLI 相同，只有下面〈刻意差異〉列出的項目例外。CLI 的 golden 由 `scripts/gen-cli-golden.mjs` 產生，細節見 [cli.md](cli.md)。`cli_golden.rs` 把每個案例重播兩次：

- 在參數最前面加上 `--no-render-guard`，輸出必須與 golden 逐位元組相同。
- 照原樣（渲染保護開啟），輸出只能在渲染保護擋下修正時不同；文字輸出裡要看得到 `Fix not applied`、`--quiet` 時的 `Render guard skipped ...` 摘要行，或 `--json` 的 `skippedFixes`。

兩個容易誤會的地方：

- **問題會排序**：oracle 的 `dedupeProblems`（`markdown-fixer.mjs` 第 2108 行）先以 `rule:line:column:message` 去重，再以行號、欄號、規則 ID 做穩定排序（規則 ID 用 `localeCompare`，但 ID 都是 ASCII，結果與一般字串比較相同）。同一行同一欄同一規則的多個問題保留產生順序。
- **規則數是 49 條**：oracle v1.1.0 的 `RULES` 共 49 條，其中 19 條可自動修正（v1.0.0 是 48 條、20 條；v1.1.0 新增 MDX086，MDX031 改為只報告）。測試一律以 `fixtures/golden/rules.json` 為準，不寫死數量。

## Golden 檔怎麼產生

`scripts/gen-golden.mjs` 直接 import oracle 的 `diagnose` 與 `fixDocument`，對 `fixtures/corpus/` 下每一份 `.md`／`.markdown` 文件跑六組選項：

| 名稱 | 選項 | 用途 |
| --- | --- | --- |
| `default` | `{}` | 預設行為 |
| `snippet` | `{"ignoreRules":["MDX003"]}` | 剪貼簿片段（不補檔尾換行） |
| `line-length-80` | `{"maxLineLength":80}` | 啟用 MDX090 |
| `keep-encoding` | `{"ignoreRules":["MDX001","MDX002"]}` | 保留 BOM 與行尾 |
| `headings-only` | `{"rules":["MDX010","MDX011","MDX012","MDX013"]}` | 只啟用部分規則 |
| `quote-depth-3-ignore` | `{"maxQuoteDepth":3,"onLimit":"ignore"}` | 資源上限的 `ignore` 模式（v1.1.0） |

讀檔方式與 oracle CLI 的 `readMarkdownFile` 相同：含 NUL 位元組就拒絕、以嚴格 UTF-8 解碼、保留 BOM。輸出寫到 `fixtures/golden/<相同相對路徑>.json`，格式如下：

```json
{
  "source": "rules/MDX010/trigger.md",
  "referenceVersion": "1.1.0",
  "cases": [
    {
      "name": "default",
      "options": {},
      "diagnose": [{ "rule": "MDX010", "name": "atx-missing-space", "severity": "warning", "fixable": true, "line": 1, "column": 2, "message": "..." }],
      "fix": { "text": "...", "changed": true, "refused": false, "reason": "" }
    }
  ]
}
```

- 鍵的順序固定，2 個空格縮排，結尾一個換行。
- NBSP、零寬字元、U+FEFF、U+3000、U+2028 等不可見字元寫成 `\uXXXX`，讓 `git diff` 看得出差異；JSON 語意不變。
- `fixtures/golden/rules.json` 是 oracle 的規則目錄，Rust 測試用它比對 `rules()`。
- 產生 golden 時也做語料自我檢查：`rules/<ID>/trigger.md` 必須回報該規則、`negative.md` 不可回報（MDX090 用 `line-length-80`，其餘用 `default`）；blocking 規則的 trigger 與 `edge/blocking-*`、`edge/frontmatter-trap-*` 必須被拒絕修正；每條規則至少在語料某處被回報一次。檢查失敗時結束碼為 1。
- 最後會連帶執行 `scripts/gen-cli-golden.mjs`（CLI 的 golden 記錄了語料的指紋），可用 `--no-cli` 跳過。

```sh
node scripts/gen-golden.mjs            # 重新產生
node scripts/gen-golden.mjs --check    # 只檢查：golden 過期、缺少或多餘時結束碼 1
```

`fixtures/golden/` 不可以手改。語料或 oracle 改變時重新產生，並逐一檢查 `git diff`。

## 驗證工具

| 工具 | 需要 Node | 用途 |
| --- | --- | --- |
| `crates/markdown-fixer/tests/golden.rs` | 否 | 以 golden 檔比對 Rust，CI 的主要防線 |
| `scripts/parity.sh <檔案或目錄>...` | 是 | 對任意檔案即時比對 oracle 與 Rust，列出每個檔案的第一個差異 |
| `scripts/fuzz-parity.mjs` | 是 | 以文法隨機產生文件，批次比對、縮小差異並存成 regression |

`parity.sh` 與 `fuzz-parity.mjs` 透過 `cargo run --example dump --features serde` 取得 Rust 結果，比對的是函式庫層級的 `diagnose` 與 `fixDocument`，不是 CLI 輸出。`dump` 以 `render_guard = false` 執行，並認得 `maxQuoteDepth`、`onLimit` 選項。

## 目前狀態

2026-11-05 的結果（oracle v1.1.0）：

- 語料 203 份文件、4,569 行，六組選項共 1,218 個案例，`golden.rs` 與 `scripts/parity.sh fixtures/corpus` 都與 oracle 全部一致。
- `scripts/fuzz-parity.mjs`：種子 1101 的 3,000 份與種子 20261105 的 1 萬份隨機文件（六組選項共 7.8 萬個案例），沒有差異。v1.1.0 的 fuzz 文法偶爾產生超過 32 層的引言，涵蓋 MDX086。
- oracle 的冪等性：`fuzz-parity.mjs --idempotence` 以種子 1101、20261105、20261106、7 各 5,000 份文件（約 9.9 萬組非拒絕的（文件, 選項）），沒有不冪等的情況。v1.0.0 的兩個不冪等來源（露出的 BOM、`>  >` 每輪只合併一層）都已修正。
- 渲染稽核（guard，第 2 輪，在 MDX008 的 NBSP 修正之前執行）：語料 201 份與 `~/projects` 底下 2,412 份真實文件，渲染保護沒有擋下任何修正，所有渲染改變都是修復類規則的預期修復。剩下的只出現在 fuzz，見〈oracle 疑似問題〉。

## Oracle 升版流程

oracle 的原始碼在 GitHub `doggy8088/skills` repo 的 `skills/markdown-autocorrect/`（本機工作樹 `~/projects/skills/skills/markdown-autocorrect/`）。安裝到 `~/.agents/skills/markdown-autocorrect/` 的那一份由 lead 在審查後更新，這個 repo 不直接修改它。

1. 在 skills repo 修改 `scripts/markdown-fixer.mjs`（含 `VERSION`）、`scripts/markdown-fixer.test.mjs`、`SKILL.md` 與 `reference/rules.md`，跑 `node --test scripts/markdown-fixer.test.mjs`。oracle 必須維持零相依的 Node 腳本。
2. 把 `markdown-fixer.mjs` 與 `markdown-fixer.test.mjs` 複製到 `reference/js/`，`SKILL.md` 與 `reference/rules.md` 複製到 `reference/`。只複製，兩邊不可以分歧。
3. 若新版附了新的測試，移植到 `crates/markdown-fixer/tests/js_ported.rs`（CLI 測試移植到 CLI crate）。
4. 執行 `node scripts/gen-golden.mjs`，逐一檢查 `fixtures/golden/` 的 diff（濾掉 `referenceVersion` 那一行），確認每個變化都能用 oracle 的變更解釋。
5. 更新 `crates/markdown-fixer/src/lib.rs` 的 `REFERENCE_VERSION`。`golden.rs` 會檢查每個 golden 的 `referenceVersion` 與它相同。
6. 跑 `cargo test --workspace --all-features`，修 Rust 直到全部通過。再跑 `scripts/parity.sh fixtures/corpus` 與 `node scripts/fuzz-parity.mjs --count 10000`。
7. 回頭檢查本文件的〈oracle 疑似問題〉：已在 oracle 修正的項目移到〈已在 v1.1.0 修正〉，對應的 regression 語料保留（golden 會反映新行為）。

## 刻意差異

除了以下項目，Rust 版的行為必須與 oracle 相同（PRD〈5.4〉）。

1. `--version` 印出 `markdown-fixer <crate 版本>`，`--json` 輸出的 `version` 欄位也是 crate 版本。
2. `--help` 的 `Usage:` 與 `Examples:` 行使用執行檔名稱 `markdown-fixer`，不是 `node markdown-fixer.mjs`。
3. `--help` 多一個 `Rust-only options` 區塊，列出 oracle 沒有的旗標。
4. Rust 版專有旗標 `--stdin`、`--exit-zero`、`--detect`：
   - `--stdin --write --json` 的 JSON 報告輸出到 stderr，stdout 只放修正後的完整文字。
   - `--stdin --write --quiet` 遇到拒絕修正時，仍在 stderr 印出 `<stdin>: Not modified: <原因>`，stdout 輸出原文。
   - `--detect` 只讀標準輸入，同時給路徑是用法錯誤；`--detect` 也遵守 `--exit-zero`。
5. 渲染保護（PRD〈6.9〉）：`render_guard` 預設開啟，CLI 以 `--no-render-guard` 關閉。開啟時，會意外改變渲染結果的版面類修正，以及超出範圍的修復類修正不套用，記錄在 `FixResult::skipped` 與 CLI 的 `skippedFixes`；對應的問題仍出現在診斷中。關閉時與 oracle 逐位元組相同。

函式庫層級在 `render_guard = false` 時沒有刻意差異。效能上 Rust 不得有 oracle 的平方或指數時間（見〈oracle 疑似問題〉），但輸出必須相同。

## v1.1.0 的變更

oracle 從 v1.0.0 升到 v1.1.0 時，與 Rust 一起修正了下列行為。每一項都有 regression 語料，golden 已反映新行為。

| 項目 | 變更 | 語料 |
| --- | --- | --- |
| 資源上限（PRD〈6.10〉） | 新選項 `maxQuoteDepth`（預設 32，0 不限制）與 `onLimit`（`error`／`ignore`），新規則 MDX086（blocking）；CLI `--max-quote-depth`、`--on-limit` | `rules/MDX086/`、`edge/blocking-quote-depth.md`、`regressions/quote-depth-ignore.md` |
| 定義行掃描 | 指數級回溯的 regex 改成線性的 `matchDefinition`，結果相同（等價測試見 [porting-map.md](porting-map.md)） | `regressions/definition-deep-quotes-30.md` |
| 很長的行與很多問題 | `mask.push(...)` 與 `issues.push(...)` 展開大陣列會拋出 `RangeError`，改成迴圈 | JS 測試（輸入太大，不放語料） |
| MDX052 | 引言裡的 fence 補 `>` 空行；標記後有縮排的 fence 不處理；相鄰的引言空行算已分隔 | `regressions/oracle-mdx052-quoted-fence.md`、`regressions/mdx052-quoted-fence-*.md` |
| MDX031 | 只報告，不再修正（換 bullet 就是新清單，統一後必然合併清單） | `regressions/mdx031-adjacent-lists.md` |
| MDX040 | 清單標記的 `*` 不當成強調符號 | `regressions/mdx040-list-marker-star.md` |
| MDX020 | 只移除一般內容前多的一個空格，或空內容前至多三個空格；一次處理整串 `>  >` | `regressions/mdx020-significant-gaps.md` |
| HTML 區塊 | raw 標籤名稱在行尾（`<iframe`）也算區塊開頭 | `regressions/html-raw-tag-at-eol.md` |
| MDX060 | 表頭與分隔列的 GFM 儲存格數不同時不處理 | `regressions/mdx060-gfm-not-a-table.md` |
| MDX006 | 空行後那一行以 U+FEFF 開頭時不修，只報告 | `regressions/oracle-bom-exposed-by-leading-blanks.md` |
| MDX008 | NBSP 是整行或清單項目唯一的內容時不換 | `regressions/mdx008-nbsp-only-content.md` |

除了資源上限與 `RangeError`，這些都來自渲染稽核：v1.0.0 的修正會改變 CommonMark／GFM 的渲染結果。

### 已在 v1.1.0 修正的疑似問題

v1.0.0 時記錄的第 1、2、7、12 項已在 v1.1.0 修正：

- **定義行 regex 的指數級回溯**（原第 1 項）：改成線性掃描。`'>'.repeat(40) + ' q'` 不再讓 `fixDocument` 停住；預設選項下 33 層以上的引言改由 MDX086 處理。
- **MDX052 把引言拆成兩個**（原第 2 項）：改補 `>` 空行。
- **露出的 BOM 讓修正不冪等**（原第 7 項）：MDX006 不再移除那些空行。`properties.rs` 的冪等 property 已不再排除這個情況。
- **很長的一行拋出 `RangeError`**（原第 12 項）：改成迴圈。同類的 `issues.push(...)` 在一份有 15 萬個問題的文件上也會拋出，一起修正。

## Oracle 疑似問題

這些是 oracle v1.1.0 看起來仍不合理的行為。Rust 照 oracle 做，每一項都有 regression 語料或測試固定目前的行為；要不要修正原 skill 由擁有者決定。修正後依〈Oracle 升版流程〉同步。編號沿用 v1.0.0，已修正的項目移到上一節。

### 3. `## ##` 不回報 MDX016

- **重現**：`regressions/oracle-mdx016-hashes-only.md`（`## ##`）。
- **oracle 行為**：空標題的 regex 要求井號後面只有空白；第二個 regex 去掉結尾井號時要求井號前有空白，但標題文字本身就是 `##`，所以文字不是空的。`fixHeading` 的結尾井號 regex 也不成立，MDX012 同樣不回報。
- **CommonMark**：`## ##` 是空的 H2。

### 4. MDX084 對整行比對，inline code 裡的全形語法也會回報

- **重現**：`regressions/oracle-mdx084-inline-code.md`，內容是 inline code 包住的全形方括號與全形括號。
- **oracle 行為**：`computeFullwidthSyntax` 直接對整行跑 regex，沒有用 `buildMask` 排除 code。
- **預期**：inline code 裡的內容不回報。

### 5. Setext 標題不算進標題結構

- **重現**：`regressions/oracle-mdx018-setext-h1.md`

  ```md
  Title
  =====

  ## Sub
  ```

- **oracle 行為**：`computeHeadingStructure` 只看 ATX 標題，所以 `## Sub` 被當成第一個標題而回報 MDX018；MDX015、MDX019 也看不到 Setext 標題。另外 `=====` 行會回報 MDX080，訊息說它「renders as a heading instead of a horizontal rule」，但 `===` 本來就不是分隔線。
- **預期**：Setext H1／H2 算進標題結構；MDX080 只針對 `---`。

### 6. `1.a 是…` 這類章節編號被當成缺空白的清單

- **重現**：`regressions/oracle-mdx030-section-number.md`（`1.a 是章節編號。`）。
- **oracle 行為**：`fixListMarker` 只排除剛好一個英文字母的 `1.a`，後面有其他文字時回報 MDX030，修正成 `1. a 是章節編號。`，變成有序清單。
- **預期**：這類輸入語意不明，報告即可，不應自動修正。

### 8. 同一段落有多個 MDX042 修正時，以原始偏移量逐一套用（潛在問題）

- **位置**：`applyCjkEmphasisFixes`（第 1805 行）。core 回報。
- **oracle 行為**：`analyzeCjkEmphasis` 以原始段落的偏移量產生每個 transform，`applyCjkEmphasisFixes` 卻逐一套用在已經被前一個 transform 改過的段落上。
- **實際影響**：目前無法觸發。每個 transform 只是把一段標點從 `**` 內側搬到外側，長度不變，而且後一個 transform 的位置都在前一個的 closer 之後。以結構化產生的 39,985 個「同一段落有多個 transform」的段落比對「逐一套用」與「依原始偏移量一次套用」，結果全部相同。
- **風險**：日後若加入會改變長度的修正，就會套用到錯誤的位置。
- **重現**：`regressions/oracle-mdx042-multiple-transforms.md` 固定目前的多 transform 行為。
- **Rust**：結果照 oracle，但不逐一重組段落，而是在整份文件上原地改寫每個 transform 的範圍（理由見 [porting-map.md](porting-map.md)）。原地改寫的前提不成立時，退回與 oracle 相同的逐一套用，所以日後即使加入會改變長度的修正，Rust 仍與 oracle 一致。

### 9. `analyzeCjkEmphasis` 的 `run !== opener` 永遠成立（不影響結果）

- **位置**：第 1766 行。core 回報。
- **oracle 行為**：`matchEmphasisRuns` 回傳的 leftovers 是 `{ ...run }` 的複本，與 `runs` 裡的原物件比較永遠不相等，所以排除 opener 與 closer 的條件形同虛設。
- **實際影響**：沒有。後面的位置條件（`run.start >= opener.end && run.end <= closer.start`）本來就排除了 opener 與 closer 本身。Rust 照位置條件實作即可。

### 10. `matchEmphasisRuns` 在大量 closer 上是平方時間

- **位置**：`matchEmphasisRuns`（第 1642 到 1675 行），由 `analyzeCjkEmphasis` 呼叫（MDX042）。review 回報。
- **oracle 行為**：每遇到一個能當 closer 的 run，就從它往回逐一檢查前面所有的 run 找 opener。只有 closer、沒有可用 opener 的段落（例如 `a* ` 重複很多次，每個 `*` 前面是文字、後面是空白，只能收尾）每次都要掃到段落開頭，總共是 O(n²)。
- **重現**：`'a* '.repeat(n)` 單一段落，只啟用 MDX042。實測 n = 10,000（29 KB）約 60 ms、20,000（59 KB）約 230 ms、40,000（117 KB）約 900 ms，輸入加倍時間約變 4 倍；1 MB 會花上數十秒。
- **影響**：只影響執行時間，不影響結果。剪貼簿或 CI 遇到這種輸入時，Node 版會明顯停頓。
- **Rust**：輸出照 oracle，演算法改成攤銷 O(n)（`cjk_emphasis::leftover_runs`，細節見 [porting-map.md](porting-map.md)）。舊的逐一反向掃描保留為測試專用的 `leftover_runs_naive`，以 20,000 組隨機 run 序列驗證兩者結果相同。1 MB 的 `a* ` 整個 CLI `--stdin --write` 約 0.3 秒。

### 11. `analyzeCjkEmphasis` 與 `applyCjkEmphasisFixes` 在大量可修正組合上是平方時間

- **位置**：`analyzeCjkEmphasis`（第 1677 到 1793 行）與 `applyCjkEmphasisFixes`（第 1805 到 1822 行），MDX042。review 回報。
- **oracle 行為**：每個候選 closer 都重掃全部 leftovers 找 opener，再用 `runs.some(...)` 掃全部 run 找中間的 delimiter；`locate` 與 `lineOffsetOf` 反向線性找行號。套用時每個 transform 都重新 join、改寫、split 整個段落。
- **重現**：`'**甲：**乙 '.repeat(n) + '\n'`，預設選項。oracle 的 `fixDocument` 在 n = 4,000（56 KB）約 0.2 秒、8,000（112 KB）約 0.7 秒；v1.0.0 在 n = 16,000 時改為拋出 `RangeError`，v1.1.0 已修正（見〈已在 v1.1.0 修正的疑似問題〉），但仍是平方時間。
- **影響**：只影響執行時間，不影響結果。
- **Rust**：輸出照 oracle，分析每段 O(n log n)、套用 O(n)（細節見 [porting-map.md](porting-map.md)）。舊版直接移植保留在測試專用的 `cjk_emphasis::naive`，`linear_equivalence_tests` 以 30,000 份隨機文件比對分析結果與修正輸出。n = 32,000（448 KB）的 `fix_document` 從約 5.4 秒降到約 52 ms（release）。

### 13. fence 的開關不追蹤容器

- **來源**：渲染稽核 U4（guard）。fuzz 數百次，語料與真實文件沒有出現。
- **oracle 行為**：fence 以行判斷開關。清單項目或引言裡開的 fence，被容器外的 fence 標記「關閉」：

  ````md
  - a
    ```
  x
    ```
  ** y **
  ````

  oracle 認為 fence 在第 4 行關閉，把 `** y **` 修成 `**y**`。
- **CommonMark**：fence 不能 lazy continuation；`x` 讓清單與第一個 fence 結束，第 4 行開了新的 fence，一直到檔尾，`** y **` 是程式碼。引言版本（`> ```js` 被引言外的 ` >>> ``` ` 或 ` ``` ` 「關閉」）同理。
- **為什麼不在 v1.1.0 修**：要讓 fence 跟著容器結束，掃描器得追蹤清單項目與引言的範圍，等於做一個 CommonMark 區塊解析器，超出行導向掃描的設計（[architecture.md](architecture.md)）。
- **Rust**：照 oracle。渲染保護會擋下這種情況下的版面類修正；修復類修正由 guard 的「修復不得改到渲染成程式碼或 raw HTML 的行」檢查擋下。

### 14. 表頭以 U+FEFF 開頭時儲存格數算錯

- **來源**：渲染稽核 N2（guard），只在 fuzz 出現。
- **oracle 行為**：`splitTableCells` 與 `gfmCellCount` 都先 `trim()`，JS 的 `trim()` 會去掉 U+FEFF，所以 `\uFEFF| a | b |` 被當成以 `|` 開頭。GFM 不把 U+FEFF 當空白，第一格是 U+FEFF，儲存格數多一格，整塊不是表格。
- **影響**：MDX060 會修正一個 GFM 不認為是表格的分隔列。
- **Rust**：照 oracle；渲染保護擋下。優先度低。

### 15. pulldown-cmark 與 CommonMark 規格的差異（不是 oracle 的問題）

渲染稽核 R1～R3：行尾空白含 tab 時 pulldown-cmark 當成 hard break（MDX004）、ATX 結尾 `#` 之後有 tab（MDX004／MDX011／MDX012）、空的 YAML frontmatter（MDX082）。依 CommonMark 規格這些修正不改變渲染，但 pulldown-cmark 的渲染會變，渲染保護因此保守地擋下。oracle 不需要修改。

### 16. MDX062 補的空行可能改變渲染（屬於修復類，保留）

- **來源**：strict 以 pulldown-cmark 檢查容器裡的空行插入時發現。
- **前一行是 lazy continuation**：`> para` 或 `- item` 後面直接接表格時，GFM 把表頭與分隔列當成引言或清單項目段落的延續文字，不是表格。MDX062 在表格前補空行後，它變成容器外的表格。
- **表格後面接沒有 `|` 的行**：GFM 把 `text` 當成表格的最後一列，MDX062 補空行後它變成表格後的段落。
- **為什麼保留**：PRD〈6.9〉把 MDX062 歸為修復類，目的就是讓作者想要的表格與段落正確分開；這些改變只發生在被修正的表格與相鄰的一個區塊。渲染保護以修復類的規則檢查，只在影響超出範圍時還原。
