# markdown-fixer（Rust 版）產品需求文件

- **狀態**：v0.1 草案，實作中
- **擁有者**：Will 保哥
- **最後更新**：2026-11-05
- **參考實作**：`reference/js/markdown-fixer.mjs`（JS v1.1.0，來源為 [doggy8088/skills](https://github.com/doggy8088/skills) 的 `skills/markdown-autocorrect/`）

## 1. 背景

`markdown-autocorrect` 是一個 Claude Code／Codex skill，核心是一支零依賴的 Node 腳本 `markdown-fixer.mjs`。它把 Markdown 的常見語法錯誤分成兩類：可以安全自動修正的格式問題，以及必須人工判斷的結構或語意問題。這支腳本已經在日常寫作、AI agent 產出文件與 CI 裡證明有用，但有三個限制：

1. 執行時需要 Node.js 20 以上，不能在沒有 Node 的機器、容器或嵌入式環境使用。
2. 只能當 CLI 呼叫，Rust 寫的應用程式（例如 `dedent-paste`、`mhtml`）無法直接把它當函式庫使用。
3. 啟動成本約數十毫秒，不適合「按一個熱鍵就修正剪貼簿」這種要求即時回應的情境。

本專案把它移植成 Rust：一個純函式庫 crate、一個 CLI、一個 C ABI 共用函式庫，並把能力整合進剪貼簿熱鍵與 `dedent-paste`。

## 2. 目標與非目標

### 2.1 目標

- **G1 行為一致**：在同一份輸入與同一組選項下，Rust 版的 `diagnose` 結果（順序、欄位、訊息）與 `fix_document` 輸出文字，和 JS 參考實作逐位元組相同。
- **G2 跨平台**：提供 macOS（x86_64、aarch64）、Linux（x86_64、aarch64，glibc 與 musl）、Windows（x86_64、aarch64）的單一執行檔，不需要任何執行環境。
- **G3 可嵌入**：Rust 應用程式以 `markdown-fixer` crate 直接呼叫；其他語言透過 C ABI（`cdylib`／`staticlib`）呼叫。
- **G4 剪貼簿工作流程**：提供 Karabiner-Elements 設定範例，按熱鍵就把剪貼簿裡的 Markdown 正規化後貼上。
- **G5 dedent-paste 整合**：`dedent-paste` 在既有的去縮排流程後，自動偵測內容是否為 Markdown，若是則輸出正規化後的 Markdown。
- **G6 品質**：以接近真實的 Markdown 語料做差異測試（differential testing），涵蓋全部 48 條規則與主要使用情境。

### 2.2 非目標

- 不實作完整的 CommonMark／GFM parser，也不改用 `pulldown-cmark` 之類的 parser 取代行導向啟發式掃描。行為差異會破壞 G1。
- 不做段落重排、措辭修改、語意判斷（例如自動補 alt text、猜標題層級）。
- 第一版不提供 LSP、編輯器外掛、設定檔（`.markdown-fixer.toml`）與規則訊息的中文化。這些列在〈12. 後續規劃〉。
- 不修改 `~/.agents/skills/markdown-autocorrect/` 裡的原始 skill。它是 oracle，只讀。

## 3. 名詞

| 名詞 | 意思 |
| --- | --- |
| Oracle、參考實作 | `reference/js/markdown-fixer.mjs`，行為的唯一標準 |
| Parity、一致性 | Rust 與 oracle 在同一輸入下產生相同輸出 |
| Fixable 規則 | `--write` 會自動修正的規則 |
| Report-only 規則 | 只回報、不自動修正的規則 |
| Blocking 規則 | MDX081、MDX083、MDX053、MDX085；出現時整份文件拒絕修正 |
| Frontmatter 陷阱 | 修正後會讓檔首的 `---` 變成 YAML frontmatter 的情況；一律拒絕修正 |
| 保護區域 | fenced code、inline code、frontmatter、數學式、URL、raw HTML、HTML comment、連結目的地 |
| Golden 檔 | 由 oracle 產生、提交進 repo 的預期輸出 |
| 片段（snippet） | 剪貼簿裡不完整的 Markdown，例如沒有檔尾換行、沒有 H1 |

## 4. 使用者與情境

| 編號 | 使用者 | 情境 |
| --- | --- | --- |
| U1 | 寫文件的人 | `markdown-fixer --check docs/` 找問題，`--write` 修正，再看 `git diff` |
| U2 | CI 維護者 | 在沒有 Node 的 runner 上跑 `markdown-fixer --check --json`，依結束碼決定成敗 |
| U3 | AI agent | 產生文件後跑 `--check`，依 rule ID 與行號自我修正；`AGENTS.md` 的完成條件要求 0 error／warning |
| U4 | Rust 應用程式開發者 | `markdown_fixer::fix_document(&text, &options)` 正規化使用者輸入或 LLM 輸出 |
| U5 | 其他語言開發者 | 透過 C ABI 從 Swift、C#、Go、Python 呼叫 |
| U6 | macOS 使用者 | 按 Karabiner 熱鍵，把剪貼簿裡的 Markdown 修正後直接貼上 |
| U7 | dedent-paste 使用者 | 按原本的 `左 Option+V`，若剪貼簿是 Markdown 就順便正規化 |

## 5. 相容性契約（與 oracle 的一致性）

這一節是整個專案最重要的規格。任何與 oracle 不一致的行為都是 bug，除非記錄在 `docs/parity.md` 的「刻意差異」清單並有理由。

### 5.1 Library 層級

- `diagnose(text, options)` 回傳的 `Vec<Problem>` 必須與 JS `diagnose(text, options)` 的陣列相同：元素順序、`rule`、`name`、`severity`、`fixable`、`line`、`column`、`message` 都一樣。`dedupeProblems` 先以 `rule:line:column:message` 去重，再以 line → column → `rule.localeCompare` 做穩定排序；三者都相同時保留各檢查函式的呼叫順序。Rust 必須使用穩定排序（`sort_by`），並照 JS 的呼叫順序產生問題，才能在同位置、同規則的多筆問題上一致。規則 ID 都是 `MDX` 加三位數字的 ASCII 字串，`localeCompare` 與 byte 比較結果相同。
- `fix_document(text, options)` 回傳的 `text`、`changed`、`refused`、`reason` 必須與 JS `fixDocument` 相同。
- **行號**：從 1 起算，以 `\n` 分行（先做與 JS `normalizeForAnalysis` 相同的正規化）。
- **欄號**：從 1 起算，單位是 JS 字串索引，也就是 UTF-16 code unit。BMP 以外的字元（例如 emoji）算 2。Rust 內部可以用 byte offset 運算，但輸出前必須換算成 UTF-16 欄號。這也剛好是 LSP 的預設單位。
- **行長（MDX090）**：以 Unicode code point 計算，CJK 算 1，不是終端顯示寬度；表格行不計。
- **正規表示式語意**：JS 的 `\s`、`\w`、`\b`、`\p{P}`、`\p{S}`、`toLowerCase()` 等在 Rust 必須逐一對照。`\s` 在 JS 包含 NBSP、U+FEFF 與 Unicode 空白；`\w` 只有 ASCII。移植時以 JS 語意為準，不以 Rust `regex` 預設為準。
- **修正迴圈**：最多 6 輪，輸出不再變動就停止，與 JS 相同。

### 5.2 CLI 層級

- 所有 JS CLI 既有的旗標、結束碼、文字輸出格式、`--json` 結構，都必須與 JS 相同。比較時忽略 `version` 欄位（Rust 版輸出自己的 crate 版本）。
- `--json` 使用 2 個空格縮排，欄位順序與 JS 相同；`reason` 在空字串時省略（JS 的 `undefined`）。非 ASCII 字元不跳脫。
- 目錄掃描結果去重後依 JS `Array.prototype.sort()` 排序，也就是依 UTF-16 code unit 字典序，不是 Rust `str` 的 byte 字典序。兩者對 BMP 以外字元的結果不同，必須照 JS。
- 顯示路徑是相對於目前工作目錄的路徑（JS `path.relative(process.cwd(), file)`）。

### 5.3 語言

- 規則訊息、CLI 輸出（`--help`、摘要行、錯誤訊息）保持英文，與 oracle 逐字相同，避免破壞 parity 與既有的 CI 解析腳本。
- 所有文件（`PRD.md`、`AGENTS.md`、`docs/`、`README.md`、程式碼中的模組層級說明）以臺灣正體中文撰寫。

### 5.4 刻意差異

第一版只允許以下差異，每一項都要在 `docs/parity.md` 記錄：

1. `--version` 輸出 `markdown-fixer <crate 版本>`，JSON 的 `version` 欄位亦同。
2. 新增的旗標（`--stdin`、`--exit-zero`、`--detect` 等，見〈6.4〉）是 Rust 版專有，oracle 沒有。
3. 用法說明（`--help`）的 `Usage:` 與 `Examples:` 行把 `node markdown-fixer.mjs` 換成 `markdown-fixer`。

## 6. 功能需求

### 6.1 規則目錄

規則目錄共 49 條（19 條 fixable；v0.1 新增 MDX086，MDX031 改為只報告），必須與 JS `RULES` 陣列完全相同（ID、name、category、severity、fixable、summary、順序）。下表是摘要；英文 summary 原文以 `reference/js/markdown-fixer.mjs` 為準，中文說明見 `docs/rules.md`。

| Rule | Name | Category | Severity | Fixable |
| --- | --- | --- | --- | --- |
| MDX001 | `bom` | encoding | warning | yes |
| MDX002 | `line-endings` | encoding | warning | yes |
| MDX003 | `final-newline` | whitespace | warning | yes |
| MDX004 | `trailing-whitespace` | whitespace | warning | yes |
| MDX005 | `multiple-blank-lines` | whitespace | warning | yes |
| MDX006 | `leading-blank-lines` | whitespace | warning | yes |
| MDX007 | `hard-tab` | whitespace | warning | no |
| MDX008 | `non-breaking-space` | whitespace | warning | yes |
| MDX009 | `zero-width-character` | encoding | warning | no |
| MDX010 | `atx-missing-space` | heading | warning | yes |
| MDX011 | `atx-multiple-space` | heading | warning | yes |
| MDX012 | `atx-closing-hashes` | heading | warning | yes |
| MDX013 | `heading-blank-lines` | heading | warning | yes |
| MDX014 | `heading-indent` | heading | info | no |
| MDX015 | `heading-increment` | heading | warning | no |
| MDX016 | `empty-heading` | heading | warning | no |
| MDX017 | `heading-trailing-punctuation` | heading | info | no |
| MDX018 | `first-heading-h1` | heading | info | no |
| MDX019 | `multiple-h1` | heading | info | no |
| MDX020 | `blockquote-space` | blockquote | warning | yes |
| MDX030 | `list-marker-space` | list | warning | yes |
| MDX031 | `list-marker-style` | list | warning | no |
| MDX032 | `list-blank-lines` | list | warning | no |
| MDX035 | `empty-list-item` | list | warning | no |
| MDX040 | `emphasis-inner-space` | emphasis | warning | yes |
| MDX041 | `unmatched-emphasis` | emphasis | warning | no |
| MDX042 | `cjk-emphasis-unpaired` | emphasis | warning | yes |
| MDX050 | `fence-language` | fence | info | no |
| MDX051 | `fence-unclosed` | fence | error | no |
| MDX052 | `fence-blank-lines` | fence | warning | yes |
| MDX053 | `fence-invalid` | fence | error | no |
| MDX060 | `table-delimiter-width` | table | warning | yes |
| MDX061 | `table-column-count` | table | error | no |
| MDX062 | `table-blank-lines` | table | warning | yes |
| MDX063 | `table-pipe-in-code` | table | warning | no |
| MDX070 | `bare-url` | link | info | no |
| MDX071 | `empty-link` | link | warning | no |
| MDX072 | `link-space-before-destination` | link | info | no |
| MDX073 | `reference-missing-definition` | link | warning | no |
| MDX074 | `reference-duplicate-definition` | link | warning | no |
| MDX075 | `image-alt` | link | warning | no |
| MDX080 | `setext-ambiguity` | structure | warning | no |
| MDX081 | `frontmatter-unclosed` | structure | error | no |
| MDX082 | `frontmatter-blank-line` | structure | warning | yes |
| MDX083 | `html-comment-unclosed` | structure | error | no |
| MDX084 | `fullwidth-markdown-syntax` | structure | warning | no |
| MDX085 | `math-unclosed` | structure | error | no |
| MDX086 | `resource-limit` | structure | error | no |
| MDX090 | `line-length` | style | info | no |

個別問題可以覆寫 severity 或 fixable（例如 MDX008 在 URL 內是 report-only，MDX042 開頭端只報告），覆寫邏輯與 JS `makeProblem(..., overrides)` 相同。

### 6.2 安全契約

以下行為不可退讓，任何違反都視為嚴重 bug：

1. 只改 Markdown 語法，不改意思、措辭、段落排版。
2. 不動保護區域：fenced code、inline code、frontmatter、數學式（`$...$`、`$$` 區塊）、URL、raw HTML、HTML comment、連結目的地。兩個行尾空格視為刻意的 hard break，保留。
3. 不發明 alt text、連結目標、標題層級或表格內容。
4. 出現 blocking 規則（MDX081、MDX083、MDX053、MDX085）時，`fix_document` 回傳原文、`refused = true` 與原因。blocking 規則即使被 `ignore_rules` 點名也照樣阻擋。
5. Frontmatter 陷阱：原文沒有 frontmatter，但修正（例如移除檔首空行）後會出現 frontmatter，一律拒絕修正並回傳固定訊息 `FRONTMATTER_TRAP`。
6. 停用 MDX001 時保留 BOM；停用 MDX002 時以原文的行尾（CRLF 優先、其次 CR）還原。
7. 函式庫不得 panic。任何輸入（含非法結構、極長行、深層巢狀、混合行尾、控制字元）都要回傳結果。測試以 property test 與 fuzz 驗證。
8. 函式庫不做 I/O，不讀環境變數，不依賴目前時間或語系，結果具決定性（deterministic）。

### 6.3 Rust 函式庫 API（`crates/markdown-fixer`）

公開 API 以 `crates/markdown-fixer/src/lib.rs` 的骨架為契約：

```rust
pub const VERSION: &str;            // crate 版本
pub const REFERENCE_VERSION: &str;  // 對齊的 oracle 版本，目前 "1.0.0"

pub enum Severity { Error, Warning, Info }
pub struct Rule { id, name, category, severity, fixable, summary }
pub struct Problem { rule, name, severity, fixable, line, column, message }
pub struct Options { rules: Option<Vec<String>>, ignore_rules: Option<Vec<String>>, max_line_length: usize }
pub struct FixResult { text: String, changed: bool, refused: bool, reason: String }

pub fn rules() -> &'static [Rule];
pub fn rule(id: &str) -> Option<&'static Rule>;
pub fn diagnose(text: &str, options: &Options) -> Vec<Problem>;
pub fn fix_document(text: &str, options: &Options) -> FixResult;
impl Options { pub fn validate(&self) -> Result<(), OptionsError>; }

pub mod detect {
    pub struct Detection { is_markdown: bool, score: i32, signals: Vec<&'static str> }
    pub fn looks_like_markdown(text: &str) -> Detection;
}
```

要求：

- 核心規則引擎零依賴。預設啟用的 `render-guard` feature 加入 `pulldown-cmark`（純 Rust，`default-features = false`）作為渲染保護用的 CommonMark/GFM parser；以 `default-features = false` 關閉後函式庫完全零依賴（此時 `render_guard` 沒有作用）。`serde` 是選用 feature，開啟後 `Severity`、`Rule`、`Problem`、`FixResult`、`Detection` 可序列化。不得加入會拉進 I/O、執行緒池或 C 相依的套件。
- `Options` 的 `max_line_length` 為 0 時停用 MDX090，行為與 JS `createEnabled` 相同。在 `rules` 指定了 MDX090 但沒給行長時預設 120，這個邏輯屬於 CLI 解析層（與 JS `parseArgs` 相同），函式庫保持原樣。
- 公開型別要有 rustdoc（臺灣正體中文），`cargo doc` 無警告。
- 內部模組建議切分：`scan`（`scanDocument` 與 line info）、`mask`（`buildMask`、`splitMasked`）、`line_fix`（標題、blockquote、清單、強調、NBSP 的單行修正）、`structure`（各 `compute*Structure` 診斷）、`cjk_emphasis`、`blank_lines`、`table`、`diagnose`、`fix`、`detect`、`jsstr`（JS 字串語意與 UTF-16 code unit 處理）、`jsregex`（JS 語意的 regex 引擎）。JS 函式與 Rust 函式的對照表維護在 `docs/porting-map.md`。

### 6.4 CLI（`crates/markdown-fixer-cli`，執行檔名稱 `markdown-fixer`）

#### 6.4.1 與 oracle 相同的旗標

| 旗標 | 行為 |
| --- | --- |
| `--check` | 只報告，不寫檔（預設） |
| `--write` | 原地套用安全修正；必須明確指定路徑，不帶路徑時以結束碼 2 拒絕 |
| `--json` | 輸出機器可讀 JSON |
| `--quiet` | 不列出每個檔案的問題 |
| `--strict` | info 也算失敗 |
| `--rule <ids>`、`--rule=<ids>` | 只啟用這些規則，可重複，逗號分隔；指定 MDX090 時預設行長 120 |
| `--ignore-rule <ids>`、`--ignore-rule=<ids>` | 停用這些規則 |
| `--max-line-length N`、`--max-line-length=N` | 啟用 MDX090 |
| `--list-rules` | 列出規則目錄 |
| `--version`、`-v` | 印出版本 |
| `--help`、`-h` | 印出說明 |
| `--` | 之後的參數都是路徑 |

- 參數解析照 JS 的手寫迴圈，不使用 `clap`（避免 `clap` 的錯誤訊息格式與旗標合併規則和 oracle 不同）。未知旗標輸出 `Error: Unknown option: <arg>` 與 `Run with --help for usage.`，結束碼 2。
- 目錄掃描跳過 `.git`、`.hg`、`.svn`、`node_modules`、`vendor`、`coverage`、`test-results`、`playwright-report`、`blob-report`、`.e2e-tmp`；副檔名判斷與 JS `expandPaths` 相同。
- 寫檔採原子寫入（寫入暫存檔再 rename），保留原檔權限，與 JS `writeFileAtomic` 相同。
- **結束碼**：`0` 沒有失敗的問題；`1` 仍有 warning／error（`--strict` 時含 info）或有拒絕修正的檔案；`2` 用法、I/O 或解析錯誤。

#### 6.4.2 Rust 版新增的旗標

| 旗標 | 行為 |
| --- | --- |
| `--stdin` | 從標準輸入讀取 Markdown，不能與路徑同時使用。顯示名稱為 `<stdin>` |
| `--stdin --check` | 問題清單輸出到 stdout，格式與檔案模式相同 |
| `--stdin --write` | 修正後的**完整文字**輸出到 stdout；報告與摘要輸出到 stderr（`--quiet` 時不輸出）。拒絕修正時 stdout 輸出原文，stderr 印出原因 |
| `--exit-zero` | 除了用法或 I/O 錯誤（結束碼 2），一律以 0 結束。給剪貼簿腳本與 `set -o pipefail` 的管線使用 |
| `--detect` | 只做 Markdown 偵測，輸出 `markdown` 或 `not-markdown`（加 `--json` 時輸出 `Detection`），結束碼 0 表示是 Markdown、1 表示不是 |

`--stdin --write` 的硬性要求：

- stdout 一定是一份完整文件，不論修正、未變更或拒絕修正。下游的 `pbcopy` 絕不能拿到空字串或半份內容。
- 標準輸入不是合法 UTF-8 時，結束碼 2，stdout 不輸出任何東西。呼叫端要保留原剪貼簿。
- 不得改寫輸入原本沒有的檔尾換行以外的內容；剪貼簿情境應搭配 `--ignore-rule MDX003`，讓「原本沒有檔尾換行」的片段維持原樣。

### 6.5 C ABI 共用函式庫（`crates/markdown-fixer-ffi`）

- 產出 `cdylib`（`.dylib`／`.so`／`.dll`）與 `staticlib`，附 C 標頭檔 `crates/markdown-fixer-ffi/include/markdown_fixer.h`。
- API 以 UTF-8 位元組與 JSON 字串交換資料，保持 ABI 穩定：

```c
/* 回傳 NUL 結尾的 UTF-8 JSON；呼叫端必須用 mdfix_string_free 釋放。 */
char *mdfix_diagnose(const char *text, size_t len, const char *options_json);
char *mdfix_fix(const char *text, size_t len, const char *options_json);
char *mdfix_detect(const char *text, size_t len);
char *mdfix_rules_json(void);
const char *mdfix_version(void);          /* 靜態字串，不用釋放 */
void mdfix_string_free(char *s);
```

- `options_json` 可為 `NULL`（全部預設），格式為 `{"rules":[...],"ignoreRules":[...],"maxLineLength":0}`，鍵名與 JS options 相同。
- 任何錯誤（非法 UTF-8、非法 JSON、未知規則）都回傳 `{"error":"..."}`，不得讓 panic 跨過 FFI 邊界（`catch_unwind`）。
- 附一個以 C 撰寫並在測試中編譯執行的範例（`examples/c/`），以及 Rust 端的 FFI 單元測試。

### 6.6 Markdown 偵測（`detect::looks_like_markdown`）

用途：剪貼簿或 `dedent-paste` 的輸入可能是程式碼、log、純文字或 Markdown。只有在判定為 Markdown 時才套用修正，誤判成 Markdown 的代價（例如把 Python 腳本的多個空行合併）遠大於漏判。所以偵測必須**保守**：寧可漏判，不要誤判。

正向訊號（每種訊號只計一次分數，數字為建議權重，實作可調整但必須有測試保護）：

| 訊號 | 條件 | 權重 |
| --- | --- | --- |
| `fenced-code` | 成對的 ```` ``` ```` 或 `~~~` fence | 3 |
| `table` | 表頭列後接合法的分隔列（`\| --- \|`） | 3 |
| `atx-heading` | `#`～`######` 後接空白與文字，且不像程式碼註解 | 2 |
| `link` | `[文字](目的地)` 或 `![alt](src)` | 2 |
| `list` | 連續兩行以上 `- `、`* `、`+ `、`1. ` 開頭 | 1 |
| `task-list` | `- [ ]`、`- [x]` | 2 |
| `emphasis` | 成對的 `**粗體**` 或 `` `inline code` `` | 1 |
| `blockquote` | `> ` 開頭 | 1 |
| `reference-definition` | `[label]: url` | 2 |
| `frontmatter` | 檔首 `---` 到下一個 `---` | 2 |
| `syntax-error` | 常見的 Markdown 筆誤，例如 `#標題`、`**標籤：**內容`、`-item` | 2 |

反向訊號（任一成立即扣分或直接判定為非 Markdown）：

- 以 `#!` 開頭的 shebang、`#include`、`#define`、`#pragma`、`#region` 等前置處理指令。
- 大量行以 `;`、`{`、`}` 結尾，或出現 `def `、`fn `、`function `、`class `、`import `、`package ` 等程式碼關鍵字開頭的行，而且沒有 fenced code 包住。
- 看起來像 log 或 stack trace（大量時間戳記、`at ` 開頭的堆疊行）。
- 看起來像 JSON、YAML、TOML、XML 整份文件（第一個非空白字元是 `{`、`[`、`<`，或大多數行是 `key: value`／`key = value`），或含 YAML block scalar（`key: |`）。
- 連續的 `# ` 註解區塊（shell、Python、YAML 註解）與 Python 三引號字串。

權重與否決條件的實際值以 `crates/markdown-fixer/src/detect.rs` 為準，說明見 `docs/detect.md`。

判定規則：分數至少 3，且至少命中兩種不同訊號，或命中一個權重 3 的訊號；同時反向訊號沒有成立。單行文字除非命中語法錯誤訊號且沒有反向訊號，否則不判定為 Markdown。

偵測結果的 `signals` 是穩定字串，供 log 與除錯使用（不得包含剪貼簿內容本身）。

### 6.7 剪貼簿熱鍵（Karabiner-Elements）

- 範例放在 `examples/macos/`：一份 Karabiner complex modification JSON（`markdown-fixer-paste.json`）與一支輔助腳本（`markdown-fix-paste.sh`）。預設熱鍵為 `左 Option+Shift+V`，不與 `dedent-paste` 的 `左 Option+V` 衝突。
- 腳本必須：
  1. 匯出 `LANG`／`LC_ALL=en_US.UTF-8`，否則 Karabiner 啟動的 shell 沒有語系，`pbpaste`／`pbcopy` 會把 CJK 轉壞。
  2. 剪貼簿沒有文字（例如只有圖片）時什麼都不改，直接結束，不得把剪貼簿清空。
  3. 呼叫 `markdown-fixer --stdin --write --quiet --exit-zero --ignore-rule MDX003`；執行失敗（結束碼 2 或輸出為空）時保留原剪貼簿。
  4. 寫回剪貼簿後，等使用者放開修飾鍵再送出 `Cmd+V`，避免送出 `Cmd+Option+Shift+V`。
  5. 不在 log 裡寫入剪貼簿內容。
- 文件 `docs/clipboard.md` 說明安裝、改熱鍵、疑難排解（輔助使用權限、語系、找不到執行檔），並附 Linux（`wl-paste`／`xclip`）與 Windows（AutoHotkey）的對應寫法。

### 6.8 dedent-paste 整合（[doggy8088/dedent-paste](https://github.com/doggy8088/dedent-paste)）

- 新增相依 `markdown-fixer`，只用函式庫 crate。
- 流程順序：讀剪貼簿 → 既有 `dedent_text`（含提示符展開）→ `detect::looks_like_markdown` → 若為 Markdown，呼叫 `fix_document`（停用 MDX002 與 MDX003：保留 Windows 剪貼簿的 CRLF 與 dedent 既有的檔尾換行行為）→ 寫回剪貼簿 → 貼上。
- 修正被拒絕（`refused`）時，輸出 dedent 之後的文字，不視為錯誤。
- 新增關閉開關：CLI 旗標 `--no-markdown` 與環境變數 `DEDENT_PASTE_MARKDOWN=0`（旗標優先），風格與既有的 `--no-paste`／`DEDENT_PASTE_NO_PASTE` 一致。
- Log 只記錄是否偵測為 Markdown、命中的訊號名稱、修正前後的大小，不記錄內容。
- 既有測試必須全部維持通過；非 Markdown 輸入的輸出必須逐位元組不變。新增的測試涵蓋：Markdown 被正規化、程式碼與 log 不被誤判、提示符展開後的 Markdown、拒絕修正時的回退、關閉開關。
- 更新 `README.md`、`DEVELOPMENT.md`、`AGENTS.md`、`CHANGELOG.md`（未發佈區段）。不自動 bump 版號、不 commit、不 push，交由擁有者決定。

### 6.9 渲染保護（v0.1 新增，Rust 專有）

**目標**：自動修正不能意外改變渲染結果。這一節優先於「與 oracle 逐位元組相同」；兩者衝突時，以不改變渲染為準，差異記錄在 `docs/parity.md`〈刻意差異〉。

做法分兩層：

1. **根因修正**（JS 與 Rust 一起改，oracle 升到 v1.1.0）：已知會改變渲染的修正直接修掉，例如 MDX052 在引言（`>`）或清單裡插入完全空白的行，把一個引言拆成兩個。這類修正改為「插入帶有相同容器前綴的空行」或「不在容器內插入」，以不改變渲染為原則。
2. **執行期保護**（`render_guard`，預設開啟，需要 `render-guard` feature，以 `pulldown-cmark` 渲染）：
   - 每條 fixable 規則屬於兩類之一。**版面類**（layout）：MDX001～006、011、012、013、020、052、060、082，本身不應改變渲染。MDX031 在 v1.1.0 改為只報告：不同的項目符號在 CommonMark 裡本來就是兩個清單，統一符號一定會合併清單，沒有不改變渲染的修法。**修復類**（repair）：MDX008、010、030、040、042、062，目的就是讓原本壞掉的語法正確渲染。
   - 版面類檢查：`render(全部修正)` 必須等於 `render(只套用修復類)`。不相等時，逐條找出造成差異的版面類規則，不套用它們。
   - 修復類檢查：修復類只能改變「被修正的行所在的頂層區塊」以及與它相鄰的一個區塊；其他頂層區塊的 HTML 與行範圍必須完全相同。違反時把該區域的修復還原。
   - 被擋下的修正記錄在 `FixResult::skipped`（規則、行號、原因），CLI 在文字輸出與 `--json`（`skippedFixes`，只在非空時出現）中顯示；對應的問題仍會出現在診斷中，結束碼照常計算。
   - 修正沒有改變渲染時（最常見的情況），只多兩次 parse 的成本。
   - `render_guard = false`（CLI `--no-render-guard`）時行為與 oracle 逐位元組相同，parity 測試使用這個模式。
3. **稽核**：以 render guard 的比對邏輯掃描語料、fuzz 與真實文件，列出 oracle 所有會改變渲染的修正，每一項都要有根因修正或被 guard 擋下的測試。

### 6.10 資源上限（v0.1 新增，JS 與 Rust 一起實作）

- `max_quote_depth`（JS `maxQuoteDepth`，CLI `--max-quote-depth N`）：引言巢狀深度上限，預設 32，0 表示不限制。
- `on_limit`（JS `onLimit`，CLI `--on-limit error|ignore`）：超過上限時的處理方式。
  - `error`（預設）：在該行回報新規則 **MDX086** `resource-limit`（structure、error、不可修正、blocking），`fix_document` 拒絕修正整份文件。CLI 結束碼依一般規則為 1。
  - `ignore`：超過上限的行視為不透明的保護行，不當成 reference definition、不修改、不診斷。
  - 兩種模式下，超過上限的行都視為不透明的保護行；`error` 模式另外回報 MDX086 並拒絕修正。
- reference definition 的偵測改為先手動掃描引言前綴、再比對其餘部分，消除 oracle v1.0.0 的指數級回溯；結果必須與舊 regex 相同（有等價測試）。
- 同時修正 oracle v1.0.0 單行超過約 12 萬 code unit 時 `mask.push(...)` 拋出 `RangeError` 的問題。

## 7. 非功能需求

| 項目 | 要求 |
| --- | --- |
| 效能 | 1 MB 的 Markdown，`fix_document` 在 Apple M 系列上 < 200 ms；剪貼簿典型片段（< 20 KB）整體 CLI 執行 < 20 ms |
| 複雜度 | 對輸入長度維持近線性；不得有會被惡意輸入觸發的指數級回溯 |
| 二進位大小 | release 版 CLI < 3 MB（strip、LTO） |
| MSRV | Rust 1.85（edition 2024） |
| 平台 | 見〈2.1 G2〉；CI 至少在 Linux、macOS、Windows 跑完整測試 |
| 安全 | 不連網、不執行外部程式（CLI 只讀寫使用者指定的檔案）；`unsafe` 只允許出現在 FFI crate |
| 可觀察性 | CLI 的錯誤訊息要能讓人或 agent 直接行動；函式庫不印任何東西 |

## 8. 測試策略

測試是這個專案的主要交付物之一。完成定義見〈10. 驗收標準〉。

1. **移植 JS 單元測試**：`reference/js/markdown-fixer.test.mjs` 的 42 個案例全部改寫成 Rust 測試，案例名稱保持可追溯（在測試名稱或註解標註原案例名稱）。
2. **Golden 語料差異測試**：`fixtures/corpus/` 收錄接近真實的 Markdown 文件，由 `scripts/gen-golden.mjs` 以 oracle 產生 `fixtures/golden/` 下的預期輸出（`diagnose` JSON、`fixDocument` 結果、數組代表性選項組合）。Rust 測試讀取 golden 檔比對，CI 不需要 Node。語料至少涵蓋：
   - 中英混排技術文件、README、CHANGELOG、API 文件、教學文章、部落格文章、會議記錄、規格書。
   - AI 產出的文件（大量粗體標籤、`**標籤：**內容`、巢狀清單、表格）。
   - 從網頁或聊天工具複製的片段（NBSP、零寬字元、全形符號、CRLF、BOM、行尾空白）。
   - 有 frontmatter 的文件（Hugo、Jekyll、Docusaurus）、含數學式的筆記、含 raw HTML 與 HTML comment 的文件、含 MDX／JSX 的文件。
   - 每一條規則至少一個觸發案例與一個「看起來像但不該觸發」的反例。
   - 每一種 blocking 狀況與 frontmatter 陷阱。
3. **即時差異檢查**：`scripts/parity.sh`（或 `.mjs`）在有 Node 的環境下，對任意檔案或目錄同時跑 oracle 與 Rust，逐一比對 JSON 與修正後文字，列出第一個差異。也用來檢查 repo 外的真實文件（例如 `~/projects/*/README.md`），找出語料沒涵蓋的差異。
4. **差異模糊測試**：`scripts/fuzz-parity.mjs` 以 Markdown 片段文法隨機組合文件（含 CJK、emoji、混合行尾、邊界情況），批次比對 oracle 與 Rust，把找到的差異縮小後存進 `fixtures/corpus/regressions/`。
5. **Property test**：不 panic、`fix_document` 冪等（再修一次不變；若 oracle 本身不冪等，以 oracle 為準並記錄）、拒絕修正時輸出等於輸入、停用 MDX001／MDX002 時 BOM 與行尾保留。
6. **CLI 整合測試**：以暫存目錄驗證旗標、結束碼、文字與 JSON 輸出、`--write` 原子寫入、`--stdin` 各模式、目錄掃描排除規則、與 oracle CLI 輸出的 golden 比對。
7. **FFI 測試**：Rust 端呼叫 `extern "C"` 函式；C 範例在 macOS／Linux CI 編譯執行。
8. **偵測測試**：Markdown 正例與程式碼、log、JSON、YAML、shell script、Python、純文字的反例各至少 10 個，確保誤判率可控。
9. **品質關卡**：`cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --workspace --all-features`、`cargo doc --no-deps` 無警告。

## 9. 發佈與散佈

- crates.io：`markdown-fixer`（函式庫）與 `markdown-fixer-cli`（執行檔）。名稱在發佈前再確認可用。
- GitHub Releases：以 cargo-dist 或與 `mhtml` 相同的 release workflow 產出各平台執行檔、shell／PowerShell installer 與 checksum。
- Homebrew tap 與 npm 包裝（下載原生執行檔）列為後續工作，參考 `mhtml` 與 `dedent-paste` 的做法。
- 版本號採 SemVer；`REFERENCE_VERSION` 與 oracle 版本同步，oracle 升版時要重新產生 golden 檔並更新 `docs/parity.md`。

## 10. 驗收標準

- [ ] 48 條規則的目錄與 oracle 完全相同（有測試）。
- [ ] 42 個移植自 JS 的測試全部通過。
- [ ] Golden 語料至少 40 份文件、總計至少 3,000 行，全部與 oracle 一致。
- [ ] 差異模糊測試連續跑 10,000 份隨機文件，沒有未記錄的差異。
- [ ] CLI 在同一批語料上，`--check --json`、`--write` 後的檔案內容、文字輸出與 oracle 一致（忽略 `version`）。
- [ ] `--stdin` 三種模式、`--exit-zero`、`--detect` 有整合測試。
- [ ] C ABI 標頭檔、範例與測試完成。
- [ ] Karabiner 範例與 `docs/clipboard.md` 完成，腳本有 `shellcheck` 等級的防呆（空剪貼簿、非 UTF-8、執行檔不存在）。
- [ ] dedent-paste 整合完成，既有測試與新增測試全部通過，`cargo fmt --check`、`cargo clippy` 無警告。
- [ ] 本專案所有 Markdown 文件以 `markdown-fixer --check` 檢查為 0 error／warning，並以 `mhtml <file> /tmp/x.html --no-config --check` 確認沒有未配對強調警告。
- [ ] 至少一輪獨立 code review，所有實質問題已修正或記錄不修的理由。

## 11. 待決事項

| 編號 | 問題 | 目前的暫定做法 |
| --- | --- | --- |
| Q1 | dedent-paste 要用什麼方式取得 `markdown-fixer` crate：path、git 或 crates.io？ | 開發期先用 path 相依（`../markdown-syntax-fixer/crates/markdown-fixer`）。這會讓 dedent-paste 的 CI 與 release 失敗，發佈前必須改成 git（含 `rev`）或 crates.io 相依 |
| Q2 | crates.io 名稱 `markdown-fixer` 是否可用？ | 發佈前確認；不可用時改名為 `mdfix` 並同步文件 |
| Q3 | 規則訊息要不要提供中文？ | 第一版保持英文；後續以 `--lang zh-TW` 選項提供，不改變預設輸出 |
| Q4 | oracle 升版時的同步流程 | 重新複製 `reference/js/`、重跑 `scripts/gen-golden.mjs`、逐一檢查 golden diff，流程記錄在 `docs/parity.md` |

## 12. 後續規劃

- 設定檔（`.markdown-fixer.toml`）與每個目錄的規則覆寫。
- LSP server 與 VS Code 擴充套件（欄號已經是 UTF-16，可以直接對應 LSP）。
- 規則訊息中文化（`--lang zh-TW`）。
- WebAssembly 版本，讓瀏覽器端的 paste-to-markdown 應用程式直接使用。
- 反向回饋：在 Rust 版發現的 oracle bug，回報並修正原始 skill，再同步 golden 檔。
