# 架構

這份文件說明 `markdown-fixer` 的整體結構：crate 怎麼切、一份文件從輸入到輸出經過哪些步驟、哪些區域受到保護、欄號怎麼算，以及為什麼刻意不用 Markdown parser。

Rust 版的每一個行為都以 JS oracle（`reference/js/markdown-fixer.mjs` v1.1.0）為準；唯一的例外是 Rust 專有的渲染保護（見〈渲染保護〉），關閉後就與 oracle 逐位元組相同。下面提到的 JS 函式名稱都指向 oracle，行號是 v1.0.0 時的位置，升版後可能偏移幾行；對應的 Rust 函式與模組請查 [porting-map.md](porting-map.md)。

## Crate 結構

| 路徑 | 類型 | 內容 |
| --- | --- | --- |
| `crates/markdown-fixer` | library（`rlib`） | 純函式庫：規則目錄、`diagnose`、`fix_document`、`detect`。不做 I/O |
| `crates/markdown-fixer-cli` | binary | 執行檔 `markdown-fixer`：參數解析、目錄掃描、讀寫檔、文字與 JSON 輸出 |
| `crates/markdown-fixer-ffi` | `cdylib`、`staticlib` | C ABI，以 UTF-8 位元組與 JSON 字串交換資料 |

相依方向是單向的：CLI 與 FFI 都只依賴函式庫，函式庫不依賴任何一方。

```text
markdown-fixer-cli ──┐
                     ├──> markdown-fixer（核心，無 I/O）
markdown-fixer-ffi ──┘
dedent-paste（外部專案）──> markdown-fixer
```

repo 裡其他與架構相關的目錄：

- `reference/js/`：oracle 與它的 42 個測試，唯讀。
- `fixtures/corpus/`、`fixtures/golden/`：差異測試的語料與 oracle 產生的預期輸出。
- `scripts/`：需要 Node 的 parity 工具，不在 `cargo test` 裡執行。

## 函式庫內部模組

實際檔案以 `crates/markdown-fixer/src/` 為準，JS 與 Rust 的逐函式對照在 [porting-map.md](porting-map.md)。

| 模組 | 對應的 JS | 職責 |
| --- | --- | --- |
| `rules` | `RULES`、`makeProblem`、`createEnabled` | 規則目錄、問題建構、規則啟用判斷 |
| `scan` | `scanDocument` | 逐行標記 frontmatter、fence、HTML、縮排 code、數學區塊、參考定義 |
| `mask` | `buildMask`、`splitMasked` | 在一行之內標出不能動的區段 |
| `line_fix` | `fixHeading`、`fixBlockquote`、`fixListMarker`、`fixEmphasis`、`fixNbsp` | 單行修正 |
| `structure` | `compute*Structure` 等 | 需要看多行的診斷 |
| `cjk_emphasis` | `analyzeCjkEmphasis` 等 | MDX042 的段落級 delimiter 配對 |
| `blank_lines` | `computeBlankLine*`、`applyBlankLine*` | 空行的診斷與修正 |
| `table` | `splitTableCells`、`findTables` 等 | 表格偵測 |
| `diagnose`、`fix` | `diagnose`、`fixDocument` | 兩條主要流程 |
| `detect` | 無（Rust 版新增） | Markdown 偵測，見 [detect.md](detect.md) |
| `guard` | 無（Rust 版新增） | 渲染保護：以 `pulldown-cmark` 渲染比對，擋下會意外改變渲染的修正。需要 `render-guard` feature，見〈渲染保護〉 |
| `jsstr` | 無 | JS 字串語意：UTF-16 字串型別、`\s`、`\w`、`trim()` 等 |
| `jsregex` | 無 | 只涵蓋 oracle 用到的語法子集的 JS 正規表示式引擎 |
| `unicode_tables` | 無 | 由 Node 產生的 `\p{P}`、`\p{S}` 範圍表 |
| `parity_probe` | 無 | 只在測試中編譯，把 scan／mask、`fix_document`、`diagnose` 的結果輸出成文字與 oracle 比對 |
| `regex_hazards` | 無 | 只在測試中編譯，用病態輸入檢查每個正規表示式的比對時間，另含 1 MB 效能量測 |

## 前處理：normalizeForAnalysis

兩條流程都先做同樣的正規化（JS `normalizeForAnalysis`，第 131 行）：

1. 去掉開頭所有的 U+FEFF。
2. 把 `\r\n` 與單獨的 `\r` 換成 `\n`。

之後一律以 `split('\n')` 分行。JS 的 `split` 會保留最後一個空字串，所以 `"a\n"` 分成 `["a", ""]` 兩行。Rust 必須用 `split('\n')`，不能用 `lines()`。

## scanDocument：逐行的區塊標記

`scanDocument`（第 300 行）從上往下掃一次，替每一行產生一筆 line info，並收集結構錯誤。這是整個工具判斷「哪些行可以動」的依據。

line info 的主要欄位：

| 欄位 | 意思 |
| --- | --- |
| `blank` | 只有空格與 tab |
| `frontmatter` | 檔首 `---` 到結尾 `---`／`...` 之間（含兩端） |
| `fence`、`fenceOpen`、`fenceClose`、`fenceContent` | fenced code 的開頭、結尾與內容行 |
| `fenceInfo`、`fenceChar`、`fenceLen` | fence 的 info string、符號、長度 |
| `htmlBlock`、`htmlStart`、`htmlKind` | raw HTML 區塊與 HTML comment（CommonMark type 1 到 7 的近似） |
| `indentedCode` | 縮排 4 欄以上的 code block（tab 以 4 欄計） |
| `mathBlock`、`mathOpen`、`mathClose` | `$$` 數學區塊 |
| `definition` | 參考定義 `[label]: url` |

每一行依序判斷，先成立的優先：

1. 已標成 frontmatter 的行直接跳過。
2. 在 fence 裡：標為 fence 內容，遇到相同符號、長度足夠的結尾 fence 就關閉。
3. 在 HTML 區塊裡：標為 HTML，遇到結尾條件（例如 `-->`、type 6／7 的空行）就關閉。
4. 在數學區塊裡：標為數學，遇到含 `$$` 的行就關閉；blockquote 裡的數學區塊若 blockquote 先結束，回報 MDX085。
5. 空行：維持或結束縮排 code。
6. 縮排 code 的開始或延續；緊接在段落後面的縮排行視為段落延續，不是 code。
7. `$$` 數學區塊開頭（可以在 blockquote、清單、task-list 內）。
8. fenced code 開頭，包括 blockquote 裡的 fence。backtick fence 的 info string 含 backtick 時回報 MDX053，不開 fence。
9. HTML 區塊開頭。type 7 只有在檔首或前一行是空行時才成立。
10. 參考定義。

掃描結束時，還沒關閉的 fence 回報 MDX051，數學區塊回報 MDX085，HTML comment 回報 MDX083。frontmatter 沒有關閉的 MDX081 在掃描一開始就判斷。

一行如果有 `frontmatter`、`fenceContent`、`htmlBlock`、`indentedCode`、`mathBlock`、`definition` 任一個標記，就是**保護行**（JS `isProtectedLine`）。大多數規則看到保護行就跳過。

## 遮罩：行內的保護區段

保護行以外的行，還有一部分內容不能動，例如 inline code 與 URL。`buildMask`（第 544 行）替一行的每個 UTF-16 code unit 產生一個布林值，`true` 表示受保護。標記順序如下：

1. 反斜線跳脫：`\` 與後面一個字元。
2. inline code：成對、長度相同的 backtick run。
3. 數學式：`$$...$$`，以及 `$...$`（開頭 `$` 後面不能是空白或 `$`，結尾 `$` 前面不能是空白或反斜線）。
4. 連結目的地：`](` 到對應的 `)`，處理巢狀括號、角括號與引號（選項 `maskLinkDestinations`，預設開）。
5. 參考標籤：`][label]`（選項 `maskReferenceLabels`，預設開）。
6. 連結文字：`[text](` 與 `[text][label]`（選項 `maskLinkText`，預設關，只有 MDX070 打開）。
7. 角括號內容：`<...>`，最長 500 字元，涵蓋 autolink 與 inline HTML。
8. 裸 URL 與 email（選項 `maskBareUrls`，預設開）。

`splitMasked` 再把一行切成「受保護」與「不受保護」交替的區段，規則只在不受保護的區段上做比對與替換。

不同規則用不同的遮罩選項。例如 MDX040 用 `maskBareUrls: false`，所以 URL 裡的 `*` 仍會參與判斷；MDX073 關掉連結目的地與參考標籤的遮罩，才看得到 `[text][label]`。移植時每一個呼叫點的選項都要照抄。

## diagnose 的流程

`diagnose(text, options)`（第 1992 行）只檢查、不修改輸入，回傳問題清單。

1. **編碼檢查**：原文開頭是 BOM 回報 MDX001；原文含 `\r` 回報 MDX002（行號是第一個 `\r` 所在的行）。
2. **掃描**：對正規化後的文字跑 `scanDocument`，收集 MDX051、MDX053、MDX081、MDX083、MDX085。
3. **單行修正（只取問題）**：每一個非保護行依序經過 `fixHeading` → `fixBlockquote` → `fixListMarker` → `fixEmphasis` → `fixNbsp`（JS `transformLine`）。每一步收到的是前一步的輸出，所以後面步驟的欄號是相對於已修正的行。
4. **原文的空白檢查**：MDX004 行尾空白、URL 內的 MDX008、MDX009 零寬字元、MDX003／MDX005／MDX006 空行。這些看的是原本的行，不是步驟 3 的輸出。
5. **重新掃描**：把步驟 3 修正後的行接起來，再跑一次 `scanDocument`。
6. **結構檢查**：在修正後的文字上依序跑標題（MDX014 到 MDX019）、清單（MDX031、MDX035、MDX032）、表格（MDX061、MDX063、MDX060）、參考連結（MDX074、MDX071、MDX075、MDX072、MDX073）、MDX080、MDX084、MDX042、空行（MDX013、MDX052、MDX062、MDX082）、MDX090。
7. **其他逐行檢查**：MDX007、MDX070、MDX050、MDX041。
8. **過濾、去重、排序**：濾掉停用的規則，以 `規則:行:欄:訊息` 去重，再依行號、欄號、規則 ID 穩定排序。

步驟 6 看的是「單行修正之後」的文字，這是刻意的：例如 `#Title` 缺空格時，標題結構檢查仍然能把它當成標題。

排序用的是 JS `localeCompare` 比較規則 ID；ID 全是 `MDX` 加三位數字，結果和逐位元比較相同。同一行同一欄、同一條規則的兩筆問題，維持產生時的先後順序。

## fix_document 的流程

`fix_document(text, options)`（JS `fixDocument`，第 2101 行）回傳 `text`、`changed`、`refused`、`reason`。

```text
原文
 │  記下原本的行尾（CRLF 優先，其次 CR）與 BOM
 │  normalizeForAnalysis
 ▼
初次掃描 ──有阻擋問題──────────────> 拒絕（回傳原文）
 │       ──移除檔首空行會冒出 frontmatter──> 拒絕
 ▼
修正迴圈（最多 6 輪，某一輪輸出沒變就停）
 │  每輪開頭重新掃描，再檢查一次阻擋問題與 frontmatter 陷阱
 │  1. 單行修正（transformLine）
 │  2. MDX042 收尾端標點搬移（applyCjkEmphasisFixes）
 │  3. MDX060 表格分隔列（applyTableDelimiterFixes）
 │  4. MDX031 清單 bullet（fixListStyle）
 │  5. MDX013／052／062／082 補空行（applyBlankLineFixes）
 │  6. MDX004 行尾空白（applyTrailingWhitespace）
 │  7. MDX003／005／006 空行正規化（applyBlankLineNormalization）
 ▼
迴圈後再檢查 frontmatter 陷阱 ─────────> 拒絕
 │  與正規化後的原文相同且編碼沒變 ──> 回傳原文，changed = false
 │  停用 MDX001 時把 BOM 加回去
 │  停用 MDX002 時把 \n 換回原本的行尾
 ▼
修正後的文字
```

步驟 2 到 7 每一步都會自己重新跑 `scanDocument`，因為前一步可能改變了區塊結構（例如補了空行之後，原本的段落延續變成獨立的縮排 code）。

多輪迴圈的理由是修正之間會互相影響：補空行可能讓下一輪的清單或表格判斷不同。6 輪是 oracle 的上限，Rust 必須一樣，不能改成「跑到收斂為止」。

## 安全契約與拒絕修正

下列情況 `fix_document` 會回傳原文、`changed = false`、`refused = true`，`reason` 是英文原因：

| 情況 | `reason` |
| --- | --- |
| MDX081 frontmatter 未關閉 | 該問題的訊息 |
| MDX083 HTML comment 未關閉 | 該問題的訊息 |
| MDX053 fence info 不合法 | 該問題的訊息 |
| MDX085 數學區塊未關閉 | 該問題的訊息 |
| 原文沒有 frontmatter，修正後卻出現 | 固定的 frontmatter 陷阱訊息 |

阻擋判斷直接看 `scanDocument` 回傳的問題，不經過規則啟用判斷，所以 `--ignore-rule MDX083` 不能放行。frontmatter 陷阱要檢查三次：迴圈前、每一輪開頭、迴圈結束後。

MDX051（fence 未關閉）是 error，但不阻擋：fence 以後的所有行都會被當成 code 保護，本來就不會被改。

其他安全保證：

- 只改 Markdown 語法，不改措辭、不重排段落。
- 不發明 alt text、連結目的地、標題層級或表格內容。
- 剛好兩個行尾空格視為 hard break，保留。
- report-only 規則不會被套用。

## 欄號模型

問題的 `column` 從 1 起算，單位是 **UTF-16 code unit**，也就是 JS 字串索引加 1。

| 字元 | UTF-8 bytes | `char` 數 | UTF-16 欄寬 |
| --- | --- | --- | --- |
| `a` | 1 | 1 | 1 |
| `中` | 3 | 1 | 1 |
| `😀` | 4 | 1 | 2 |

Rust 版的做法是**內部直接用 UTF-16**：`jsstr` 模組把每一行轉成 `Vec<u16>`（`JsString`），掃描、遮罩、正規表示式比對都以 code unit 為單位，只在公開 API 的邊界與 `String` 互轉。這樣索引、遮罩陣列長度、`text[i]` 取到的「字元」都和 JS 完全一樣（emoji 會被拆成兩個 surrogate），欄號天生就是 UTF-16，不需要另外換算，也不會出現 byte offset 落在字元中間的問題。

這個單位剛好是 LSP 的預設，以後做 LSP server 可以直接用。

MDX090 的行長是例外：它以 Unicode code point 計算（JS `Array.from(line).length`），CJK 與 emoji 都算 1。

## 正規表示式語意

oracle 的規則大多是 JS 正規表示式，移植時要以 JS 語意為準：

- `\s` 包含 NBSP（U+00A0）、U+FEFF、U+3000 等 Unicode 空白。Rust `char::is_whitespace` 不含 U+FEFF。
- `\w` 與 `\b` 只看 ASCII 英數字與底線。
- `\p{P}`、`\p{S}`（MDX040、MDX042 的 flanking 判斷）需要 Unicode general category。
- JS `trim()` 會去掉 U+FEFF；Rust `trim()` 不會。
- `toLowerCase()`（參考標籤正規化）與 Rust 的 Unicode 小寫轉換在少數字元上不同。
- oracle 大量使用 lookbehind、lookahead 與惰性量詞，回溯的優先順序會影響比對結果。

Rust `regex` crate 不支援 lookaround、以 UTF-8 運作、`\w` 預設是 Unicode，逐條改寫成手寫掃描又容易漏掉回溯細節。所以函式庫內建一個小型的回溯式引擎 `jsregex`：

- 只支援 oracle 用到的語法子集：字元類別、錨點、`\b`、群組、選擇、貪婪與惰性量詞、lookahead、固定寬度的 lookbehind、ASCII 的 `i` 旗標。
- 以 UTF-16 code unit 比對，`\s`、`\w`、`\d` 採 JS 定義。
- 用明確的堆疊回溯，不遞迴，長行不會 stack overflow。
- 有步數預算（約 4096 + 64 ×（長度 + 1））。預算用完後改用記憶化回溯：記住已經失敗過的（指令, 位置）組合，跨起點共用。因為不支援 backreference，記憶化不會改變結果，最壞情況是「指令數 × 長度」，不會指數爆炸。
- 以樣式的第一個字元預先過濾起點。
- 不支援 backreference、named group，以及可以比對到空字串的無上限迴圈（編譯時回報錯誤）。

好處是 JS 的正規表示式原文幾乎可以直接搬過來（寫成 `re!(r"...")`，不要改寫成 Rust `regex` 語法），比對優先順序與 JS 相同。`\p{P}`、`\p{S}` 則由 `unicode_tables` 的範圍表判斷，表格是從 Node 產生的，不要手改。

常見的差異來源整理在 [debugging.md](debugging.md#常見的差異來源)。

## 為什麼不用 Markdown parser

用 `pulldown-cmark` 或 `comrak` 解析出 AST 再檢查，看起來更「正確」，但這個專案刻意不這麼做：

1. **相容性是第一目標**。oracle 是行導向的啟發式掃描，它對 lazy continuation、HTML 區塊、巢狀結構的判斷與完整 parser 不同。換成 parser，連要報哪些問題都會不一樣，逐位元組一致（PRD G1）就不可能達成。
2. **修正需要保留原文**。這個工具只改語法、其他位元組原封不動。從 AST 重新輸出 Markdown 會改掉縮排、跳脫、清單編號等使用者沒要求改的地方。
3. **壞掉的文件也要能處理**。工具的對象就是語法有錯的文件，啟發式掃描在結構錯誤時比較好控制，也容易做「看不懂就拒絕」的保守判斷。

代價是一些已知限制，例如跨行強調、巢狀 blockquote 裡的清單不完整解析。這些記錄在 [rules.md](rules.md#已知限制)。

渲染保護（見下一節）確實用了 `pulldown-cmark`，但只用來**驗證**修正有沒有改變渲染，不用來產生修正。修正的內容永遠來自行導向的掃描；parser 只決定某個修正要不要套用。

## 渲染保護

PRD〈6.9〉要求自動修正不能意外改變渲染結果。行導向掃描與真正的 CommonMark 解析在邊界情況會不一致（例如 fence 屬於哪個容器），這些不一致會讓修正改到渲染。渲染保護在 `fix_document` 的最後一關，用 `pulldown-cmark` 實際渲染比對，把這類修正擋下。程式在 `crates/markdown-fixer/src/guard.rs`，`Options::render_guard` 控制開關，預設開啟。

### 規則分類

| 類別 | 規則 | 要求 |
| --- | --- | --- |
| 編碼 | MDX001、MDX002 | 比對前先去掉 BOM、統一行尾，所以永遠不會改變渲染，一律照常套用 |
| 版面 | MDX003～006、011、012、013、020、052、060、082 | 不得改變渲染 |
| 修復 | MDX008、010、030、040、042、062 | 目的就是改變渲染，但只能改被修正的區塊與相鄰的一個區塊 |

MDX031 在 oracle v1.1.0 改成只回報，不在任何一類。

### 流程

```text
fix::fix_document（與 oracle 相同）
 │  拒絕修正或沒有改變 ─────────────────────> 原樣回傳
 │  render(全部修正) == render(原文) ───────> 原樣回傳（便宜路徑）
 ▼
修復類檢查：只套用修復類 → rep
 │  1. 行 diff（原文 ↔ rep）切成 hunk
 │  2. 改到程式碼、raw HTML、frontmatter 的 hunk ─────> 還原
 │  3. 只含行內修復（MDX008、040、042）的 hunk 改變區塊結構 ─> 還原
 │  4. 其他頂層區塊的 HTML 或行範圍改變 ───> 還原最近的 hunk，最多 8 輪
 ▼
版面類檢查：render(全部修正) 必須等於 render(rep)
 │  相等 ───────────────────────────────────> 回傳全部修正
 │  先在差異區域的摘錄上，逐條試「rep ＋ 規則 L」
 │  摘錄不可用時，在整份文件上逐條試
 │  去掉肇事規則後在整份文件上再驗證一次
 │  驗證失敗 ─────────────────> 只套用修復類（所有觸發的版面類規則都不套用）
 ▼
FixResult（skipped 列出沒套用的修正）
```

每一層都以「驗證，失敗就退一步」收尾：全部修正 → 去掉肇事規則 → 只套用（部分）修復 → 原文。原文永遠安全，所以以 pulldown-cmark 渲染時，結果不會有未預期的改變（與 GitHub 渲染器的差異見〈已知限制〉）。工作量有上限：版面類每條規則最多多跑一次完整修正，修復類最多 8 輪渲染。

### 渲染與比對

- **渲染選項**：表格、刪除線、任務清單、註腳、GFM blockquote 標籤（`> [!NOTE]`）、數學式。開數學式是因為 fixer 本來就把 `$…$` 與 `$$` 區塊當保護區域，不開的話數學式裡的 `*`、`_` 會被當成強調，產生與修正無關的誤報。
- **frontmatter**：不開 pulldown-cmark 的 YAML metadata block，因為它在文件中間也會觸發，GitHub 只認檔首。檔首 frontmatter 改寫成同樣行數的 fenced code，內容逐字比對。
- **看得見的差異**：比對前先去掉讀者看不到的差異。一般內文的連續空白摺成一個空格；程式碼區塊與 raw HTML 區塊只去掉行尾空白與區塊結尾的空行，區塊內部的空行數照原樣比對。pulldown-cmark 會把 HTML 區塊裡 tab 展開剩下的欄位另外送成文字事件，比對時整個 HTML 區塊合成一段，不拆開。硬換行、NBSP、程式碼內容與所有標籤照原樣比對。
- **頂層區塊**：深度 0 的事件範圍。每個區塊單獨渲染成 HTML，並記下來源的行範圍。

### 行 diff

先去掉共同的開頭與結尾，中段用編輯距離上限 256 的 Myers。超過上限時改用貪婪重新對齊（視窗 64 行），對齊一定成功：

1. 優先找下一行也相同的共同行。
2. 只差一行、而且下一行相同時，當成單行替換。
3. 視窗內完全找不到共同行時（例如長清單、長表格、長段落的每一行都被修復），沿對角線逐行配對。遇到以下任一情況就停下，交回視窗搜尋：兩邊的行再度相同；兩邊錯開 1～4 行就能對齊（例如長串修復後緊接著 MDX062 補的空行）；或已經配對滿一個視窗（64 行）。不能一路配對到檔尾，否則後段整個變成一個 hunk，區域性檢查就失效了。相鄰的 hunk 會合併，與 Myers 的結果一樣。

任何共同子序列都是合法的對齊，品質只影響 hunk 大小（hunk 越大檢查越寬鬆）。修復類規則多半不改行數，逐行配對就是精確的對齊。

### 已知限制

- 版面類以「規則」為單位擋下：一個 MDX004 修正會改變渲染，整份文件的 MDX004 修正都不套用。`skipped` 會列出該規則的每一個問題行。PRD〈6.9〉就是這樣規定的；以區域為單位還原是可能的改進。
- 渲染器是 pulldown-cmark，不是 GitHub 的 cmark-gfm，guard 只能保證「在 pulldown-cmark 上」渲染不變。兩者不一致時，結果有兩種方向：
  - **少修**：pulldown-cmark 看得出差異、GitHub 看不出，guard 擋下一個其實無害的修正。例如行尾「空格、tab、空格」在 pulldown-cmark 是硬換行；ATX 結尾 `#` 後面有 tab 時，pulldown-cmark 不算結尾序列。
  - **放過**：GitHub 看得出差異、pulldown-cmark 看不出，guard 接受了會改變 GitHub 渲染的修正，`skipped` 也是空的。已知的例子是只有空白的縮排行：在空的清單項目標記之後，或在註腳的續行裡，GitHub 把它當成非空行，pulldown-cmark 依規格當成空行；MDX004 把它清成空行時 guard 看不出差異。
  - 已知的「放過」案例由 oracle 端修根因，清單見 [parity.md](parity.md)。
- 稽核結果與剩下的 oracle 疑似問題見 [parity.md](parity.md)。

## 相依政策

函式庫的核心引擎**零執行期相依**。`--no-default-features` 建置時完全沒有相依。目前的相依：

| 套件 | 範圍 | 理由 |
| --- | --- | --- |
| `pulldown-cmark` | 選用 feature `render-guard`（預設開啟），`default-features = false`，只開 `html` | 渲染保護需要一個 CommonMark／GFM 渲染器來比對修正前後。它是純 Rust、沒有 I/O、沒有執行緒、沒有 C 相依，連帶只多了 `pulldown-cmark-escape`、`bitflags`、`memchr`、`unicase`。核心的修正與診斷完全不用它 |
| `serde` | 選用 feature `serde` | 讓公開型別可以序列化，CLI 的 `--json` 與 FFI 會開啟 |
| `proptest` | dev-dependency | property test：不 panic、冪等性 |
| `serde_json` | dev-dependency | 讀取 golden 檔 |
| `pulldown-cmark` | dev-dependency | `tests/guard.rs` 用自己的渲染比對檢查 guard，不呼叫函式庫內部 |

新增相依前先問能不能不加。真的需要時：

- 在這份文件說明理由。
- 不加入會帶進 I/O、執行緒池或 C 相依的套件。

刻意不用 `regex` crate，原因見〈正規表示式語意〉。`jsregex` 的記憶化回溯保證最壞情況是多項式時間；新增樣式時仍要把它加進 `regex_hazards` 的病態輸入測試。

CLI 與 FFI crate 可以有自己的相依（例如 FFI 用 `serde_json` 解析 options），但不能讓函式庫因此多出相依。

## 錯誤處理與不 panic 政策

- 函式庫的公開函式對任何輸入都回傳結果，不 panic。非法結構、極長行、深層巢狀、混合行尾、控制字元都必須能處理。
- 不對來自輸入的值用 `unwrap()`、`expect()`。字串切片前先確認在字元邊界上。
- 唯一會失敗的公開 API 是 `Options::validate`，回傳 `OptionsError`。`diagnose` 與 `fix_document` 不驗證選項；未知的規則 ID 只是不會比對到任何規則。
- 函式庫不做 I/O、不讀環境變數、不印東西、不依賴時間或語系，相同輸入一定得到相同輸出。
- `unsafe` 只能出現在 `crates/markdown-fixer-ffi`，每一處都要有 `// SAFETY:` 註解。FFI 邊界用 `catch_unwind`，不讓 panic 跨過 C ABI。

這些保證由 property test 與差異模糊測試驗證，見 [testing.md](testing.md)。

## 效能

PRD〈7〉的目標：1 MB 的 Markdown，`fix_document` 在 Apple M 系列上低於 200 ms；剪貼簿常見的 20 KB 以下片段，整個 CLI 執行低於 20 ms；release 版 CLI 小於 3 MB。

實測結果（release build，Apple M5 Max）：

| 項目 | 結果 |
| --- | --- |
| 約 1.16 MB 的文件，`fix_document` | 約 136 ms |
| 同一份文件，`diagnose` | 約 65 ms（105 個問題） |
| 15 KB 片段，`markdown-fixer --stdin --write`（含行程啟動） | 中位數約 6.5 ms |
| release 版 CLI 執行檔（macOS aarch64） | 約 1.1 MB（含渲染保護；不含時約 0.70 MB） |

1 MB 的量測可以自己重跑：

```sh
cargo test --release -p markdown-fixer --lib -- --ignored one_megabyte --nocapture
```

### 渲染保護的成本

渲染保護在便宜路徑上只多兩次渲染。修正改變渲染時（只要有修復類修正就會），多一次「只套用修復類」的修正與一次 `diagnose`；有肇事的版面類規則時，再加上摘錄上的逐條修正與整份文件的一次驗證。

實測（release build，Apple M5 Max，機器同時有其他編譯工作，load average 約 6）：

| 1 MB 文件 | oracle 的 `fix_document` | 開啟渲染保護 |
| --- | --- | --- |
| 以 `fixtures/corpus` 拼成，含修復類修正與幾個會被擋下的 U4 回歸案例 | 約 115 ms | 約 380 ms |
| 同上，檔尾再加一個會被擋下的 MDX004 | 約 115 ms | 約 480 ms |
| 只有版面類修正、渲染不變的段落與清單 | 約 430 ms | 約 460 ms |
| 每 4 行就有一個修復類修正（`#標題`、`-item` 等重複） | 約 570 ms | 約 1.5 s |
| 每個區塊都有會被擋下的 MDX004（摘錄超過半份文件，改在整份文件上逐條找） | 約 480 ms | 約 3 s |

最後兩列是刻意製造的最壞情況：成本是幾次完整修正加上診斷，仍然是線性的。量測可以自己重跑：

```sh
cargo test --release -p markdown-fixer --lib -- --ignored guard_one_megabyte --nocapture
```

加入 pulldown-cmark 後，release 版 CLI 執行檔約 1.1 MB（macOS aarch64），仍在 3 MB 以內。

### 修正迴圈

修正迴圈每一輪都會重新掃描好幾次，最壞情況是 6 輪乘上每輪的掃描次數。每次掃描在一般輸入上是線性的，實作時要避免在迴圈裡做「對每一行重新組整份文字」這類二次方運算。

oracle 本身有幾處在病態輸入上不是線性的。Rust 的輸出照 oracle，但能做到線性的地方就換成線性演算法：

| 位置 | oracle | Rust |
| --- | --- | --- |
| MDX042 的 delimiter 配對（`matchEmphasisRuns`） | 平方時間：每個 closer 往回逐一找 opener，`a* ` 重複 4 萬次（117 KB）約 0.9 秒，輸入加倍時間約變 4 倍 | 攤銷 O(n)：`cjk_emphasis::leftover_runs` 以 6 個 stack 加差分陣列實作，1 MB 的 `a* ` 整個 CLI `--stdin --write` 約 0.3 秒 |
| MDX042 的修正判斷與套用（`analyzeCjkEmphasis`、`applyCjkEmphasisFixes`） | 平方時間：每個候選 closer 重掃全部 leftovers 與 runs，每個 transform 重新組合整個段落；`**甲：**乙 ` 重複 8,000 次（112 KB）的 `fixDocument` 約 0.7 秒，v1.0.0 在 16,000 次（224 KB）時會拋出 `RangeError`，v1.1.0 已修正（見 [parity.md](parity.md#oracle-疑似問題)） | 每段 O(n log n)：事先算好每個位置的合格 opener、以 run 索引判斷中間有沒有 run、二分搜尋行號，套用時在整份文件上原地改寫。重複 32,000 次（448 KB）的 `fix_document` 約 52 ms（release），舊版約 5.4 秒 |
| 深層引言的參考定義 regex | 指數級回溯，40 層 `>` 會卡住 | `jsregex` 的記憶化回溯，200 層以內毫秒級 |
| 一行裡大量 `[](` 的連結目的地遮罩（`buildMask`） | 平方時間 | 照 oracle，仍是平方時間：10k 字元的病態行，`diagnose` 約 30 ms |

演算法換掉的地方都保留一份直接移植的舊版，只在測試中編譯，用來比對兩者結果相同。oracle 的這些行為記錄在 [parity.md](parity.md#oracle-疑似問題)。

`regex_hazards` 測試以約 340 種病態輸入檢查每個正規表示式與遮罩，時間上限只在 release build 斷言，避免慢速的 debug CI 誤報。
