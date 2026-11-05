# 與 oracle 的一致性（parity）

這份文件說明 Rust 版與 JS 參考實作（oracle，`reference/js/markdown-fixer.mjs` v1.0.0）一致是什麼意思、golden 檔怎麼產生、oracle 升版怎麼同步，並記錄刻意差異與 oracle 的疑似問題。測試怎麼跑見 [testing.md](testing.md)。

## 一致性的定義

**函式庫層級**：同一份輸入文字與同一組選項下，`diagnose` 回傳的問題清單與 JS `diagnose` 完全相同，包括元素順序與每個欄位（`rule`、`name`、`severity`、`fixable`、`line`、`column`、`message`）；`fix_document` 回傳的 `text`、`changed`、`refused`、`reason` 與 JS `fixDocument` 完全相同。`column` 的單位是 UTF-16 code unit。

**CLI 層級**：旗標、結束碼、文字輸出、`--json` 結構與 oracle CLI 相同，只有下面〈刻意差異〉列出的項目例外。CLI 的 golden 由 `scripts/gen-cli-golden.mjs` 產生，細節見 [cli.md](cli.md)。

兩個容易誤會的地方：

- **問題會排序**：oracle 的 `dedupeProblems`（`markdown-fixer.mjs` 第 1931 行）先以 `rule:line:column:message` 去重，再以行號、欄號、規則 ID 做穩定排序（規則 ID 用 `localeCompare`，但 ID 都是 ASCII，結果與一般字串比較相同）。同一行同一欄同一規則的多個問題保留產生順序。
- **規則數是 48 條**：oracle 的 `RULES` 共 48 條，其中 20 條可自動修正。測試一律以 `fixtures/golden/rules.json` 為準，不寫死數量。

## Golden 檔怎麼產生

`scripts/gen-golden.mjs` 直接 import oracle 的 `diagnose` 與 `fixDocument`，對 `fixtures/corpus/` 下每一份 `.md`／`.markdown` 文件跑五組選項：

| 名稱 | 選項 | 用途 |
| --- | --- | --- |
| `default` | `{}` | 預設行為 |
| `snippet` | `{"ignoreRules":["MDX003"]}` | 剪貼簿片段（不補檔尾換行） |
| `line-length-80` | `{"maxLineLength":80}` | 啟用 MDX090 |
| `keep-encoding` | `{"ignoreRules":["MDX001","MDX002"]}` | 保留 BOM 與行尾 |
| `headings-only` | `{"rules":["MDX010","MDX011","MDX012","MDX013"]}` | 只啟用部分規則 |

讀檔方式與 oracle CLI 的 `readMarkdownFile` 相同：含 NUL 位元組就拒絕、以嚴格 UTF-8 解碼、保留 BOM。輸出寫到 `fixtures/golden/<相同相對路徑>.json`，格式如下：

```json
{
  "source": "rules/MDX010/trigger.md",
  "referenceVersion": "1.0.0",
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

`parity.sh` 與 `fuzz-parity.mjs` 透過 `cargo run --example dump --features serde` 取得 Rust 結果，比對的是函式庫層級的 `diagnose` 與 `fixDocument`，不是 CLI 輸出。

## 目前狀態

2026-11-05 的結果：

- 語料 192 份文件、4,893 行，五組選項共 960 個案例，與 oracle 全部一致。
- `scripts/fuzz-parity.mjs --count 10000 --seed 20261105`：1 萬份隨機文件、五組選項共 5 萬個案例，沒有差異。
- oracle 的冪等性：約 6 萬組（文件, 選項）中，唯一不冪等的情況是下面〈oracle 疑似問題〉第 7 項。

## Oracle 升版流程

1. 把新版的 `markdown-fixer.mjs` 與 `markdown-fixer.test.mjs` 複製到 `reference/js/`。來源是 `~/.agents/skills/markdown-autocorrect/`，只複製、不修改來源。
2. 若新版附了新的測試，移植到 `crates/markdown-fixer/tests/js_ported.rs`。
3. 執行 `node scripts/gen-golden.mjs`，逐一檢查 `fixtures/golden/` 的 diff，確認每個變化都能用 oracle 的 changelog 解釋。
4. 更新 `crates/markdown-fixer/src/lib.rs` 的 `REFERENCE_VERSION`。`golden.rs` 會檢查每個 golden 的 `referenceVersion` 與它相同。
5. 跑 `cargo test --workspace --all-features`，修 Rust 直到全部通過。再跑 `scripts/parity.sh fixtures/corpus` 與 `node scripts/fuzz-parity.mjs --count 10000`。
6. 回頭檢查本文件的〈oracle 疑似問題〉：已在 oracle 修正的項目移除，對應的 regression 語料保留（golden 會反映新行為）。

## 刻意差異

除了以下項目，Rust 版的行為必須與 oracle 相同（PRD〈5.4〉）。

1. `--version` 印出 `markdown-fixer <crate 版本>`，`--json` 輸出的 `version` 欄位也是 crate 版本。
2. `--help` 的 `Usage:` 與 `Examples:` 行使用執行檔名稱 `markdown-fixer`，不是 `node markdown-fixer.mjs`。
3. `--help` 多一個 `Rust-only options` 區塊，列出 oracle 沒有的旗標。
4. Rust 版專有旗標 `--stdin`、`--exit-zero`、`--detect`：
   - `--stdin --write --json` 的 JSON 報告輸出到 stderr，stdout 只放修正後的完整文字。
   - `--stdin --write --quiet` 遇到拒絕修正時，仍在 stderr 印出 `<stdin>: Not modified: <原因>`，stdout 輸出原文。
   - `--detect` 只讀標準輸入，同時給路徑是用法錯誤；`--detect` 也遵守 `--exit-zero`。

函式庫層級沒有刻意差異。效能上 Rust 不得有 oracle 的指數級回溯（見下面第 1 項），但輸出必須相同。

## Oracle 疑似問題

這些是 oracle 看起來不合理的行為。Rust 照 oracle 做，每一項都有 regression 語料固定目前的行為；要不要修正原 skill 由擁有者決定。修正後依〈Oracle 升版流程〉同步。

### 1. reference definition 的 regex 會指數級回溯

- **位置**：`scanDocument`（第 515 行）與 `computeReferenceStructure`（第 1331 行）的 `/^(?: {0,3}>[ \t]?)* {0,3}\[[^\]\n]+\]:.../`。
- **重現**：`'> '.repeat(30) + 'q'` 這種沒有 `[` 的深層引言行。空白可以被 `[ \t]?` 吃掉，也可以被下一輪的 ` {0,3}` 吃掉，失敗時要試遍所有組合：深度 24 約 31 ms，每多一層約加倍。`'>'.repeat(40) + ' q'` 會讓 `fixDocument` 停住，因為 `fixBlockquote` 先把它改成 `> > > …`，下一輪掃描就觸發回溯。
- **影響**：只影響執行時間，不影響結果。
- **Rust**：輸出照 oracle，但掃描必須是線性時間（PRD〈7〉）。`properties.rs` 的 `deep_blockquotes_finish_in_time` 以 200 層引言設時間上限。語料 `edge/deep-nesting.md` 刻意只用 12 層，讓 oracle 產生 golden 時不會卡住。

### 2. MDX052 把引言裡的 fence 當成頂層，插入的空行會拆開引言

- **重現**：`regressions/oracle-mdx052-quoted-fence.md`

  ```md
  # T

  > text
  > ```js
  > x
  > ```
  > more
  ```

- **oracle 行為**：`isTopLevelLine` 只檢查第一個字元不是空白，`> ```js` 被當成頂層 fence，回報兩個 MDX052，修正時在前後插入完全空白的行，一個引言變成三個。
- **預期**：引言裡的 fence 應該插入 `>` 空行，或不處理。

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

### 7. 移除檔首空行後露出的 BOM 留在檔首，修正因此不冪等

- **重現**：`regressions/oracle-bom-exposed-by-leading-blanks.md`，內容是兩個空行，接著一行以 U+FEFF 開頭的 `# T`。
- **oracle 行為**：MDX001 只在修正前移除檔首 BOM；MDX006 移除檔首空行後，原本在第 3 行行首的 U+FEFF 變成檔首，輸出以 BOM 開頭。再修一次時 MDX001 又把它移除，所以 `fixDocument` 在這種輸入上不冪等。`fuzz-parity.mjs --idempotence` 找到的不冪等案例全部縮小成這個形狀。
- **Rust**：照 oracle。`properties.rs` 的冪等與編碼 property 排除這個情況（`bom_exposed_by_leading_blank_removal`）。
