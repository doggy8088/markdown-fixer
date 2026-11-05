# 規則參考

這份文件說明 `markdown-fixer` 的每一條規則：偵測什麼、能不能自動修正、什麼時候可以忽略，以及 CJK 文件要注意的地方。內容改寫自原 skill 的 [`reference/rules.md`](../reference/rules.md)，並逐條對照 oracle（`reference/js/markdown-fixer.mjs` v1.1.0）的原始碼與實際輸出校正過。

規則目錄共 **49 條**，其中 19 條可自動修正（v1.1.0 新增 MDX086，MDX031 改為只報告）。ID、名稱、分類、嚴重度、是否可修正與英文摘要都和 oracle 的 `RULES` 陣列完全相同，`markdown-fixer --list-rules` 印出的就是這份目錄。

> 本文所有「修正前」範例都放在 fenced code block 裡，所以這份文件本身可以通過檢查。範例裡的「修正後」是把「修正前」交給 oracle 的 `fixDocument`（只啟用該條規則）實際得到的輸出，不是手寫的。

## 怎麼讀這份文件

- **嚴重度**：`error` 與 `warning` 會讓檢查失敗（結束碼 1）；`info` 只有加 `--strict` 才算失敗。
- **自動修正**：「是」表示 `--write` 與 `fix_document` 會處理；「否」表示只報告。個別問題可以覆寫這個值，例如 URL 內的 NBSP 會以 `fixable: false` 回報，即使 MDX008 本身是可修正規則。
- **阻擋（blocking）**：MDX081、MDX083、MDX053、MDX085、MDX086 出現時，整份文件拒絕修正，`fix_document` 回傳原文、`refused = true` 與原因。這五條即使列在 `--ignore-rule` 也照樣阻擋。
- **不改變渲染**：修正以不改變 CommonMark／GFM 的渲染結果為原則（PRD〈6.9〉）。v1.1.0 修掉了 oracle 會改變渲染的修正（見各規則與 [parity.md](parity.md)）；Rust 版另有執行期的渲染保護（`render_guard`），擋下剩下的少數情況。
- **欄號**：從 1 起算，單位是 UTF-16 code unit（與 JS 字串索引相同），emoji 等 BMP 以外的字元算 2。
- **markdownlint 對應**：只是近似，兩邊的規則邊界與預設值不同。完整對照表在文末。

## 規則總表

| ID | 名稱 | 分類 | 嚴重度 | 自動修正 | 檢查內容 |
| --- | --- | --- | --- | --- | --- |
| MDX001 | `bom` | encoding | warning | 是 | 檔案開頭的 UTF-8 BOM |
| MDX002 | `line-endings` | encoding | warning | 是 | CRLF 或 CR 行尾 |
| MDX003 | `final-newline` | whitespace | warning | 是 | 檔尾是否剛好一個換行 |
| MDX004 | `trailing-whitespace` | whitespace | warning | 是 | 行尾空白；兩個空格視為 hard break |
| MDX005 | `multiple-blank-lines` | whitespace | warning | 是 | 連續多個空行 |
| MDX006 | `leading-blank-lines` | whitespace | warning | 是 | 檔首空行；後面那行以 U+FEFF 開頭時只報告 |
| MDX007 | `hard-tab` | whitespace | warning | 否 | 內文或縮排中的 tab |
| MDX008 | `non-breaking-space` | whitespace | warning | 部分 | NBSP；URL 內的只報告 |
| MDX009 | `zero-width-character` | encoding | warning | 否 | 零寬空白、word joiner、檔中 BOM |
| MDX010 | `atx-missing-space` | heading | warning | 是 | `#` 後缺空格 |
| MDX011 | `atx-multiple-space` | heading | warning | 是 | `#` 後多個空格 |
| MDX012 | `atx-closing-hashes` | heading | warning | 是 | ATX 標題尾端的 `#` |
| MDX013 | `heading-blank-lines` | heading | warning | 是 | 標題與相鄰段落之間缺空行 |
| MDX014 | `heading-indent` | heading | info | 否 | 標題有縮排 |
| MDX015 | `heading-increment` | heading | warning | 否 | 標題層級跳級 |
| MDX016 | `empty-heading` | heading | warning | 否 | 空標題 |
| MDX017 | `heading-trailing-punctuation` | heading | info | 否 | 標題結尾有句讀標點 |
| MDX018 | `first-heading-h1` | heading | info | 否 | 第一個標題不是 H1 |
| MDX019 | `multiple-h1` | heading | info | 否 | 多個 H1 |
| MDX020 | `blockquote-space` | blockquote | warning | 是 | `>` 後的空格不一致 |
| MDX030 | `list-marker-space` | list | warning | 是 | 清單標記與內容之間缺空格 |
| MDX031 | `list-marker-style` | list | warning | 否 | 同一層清單的 bullet 不一致（只報告） |
| MDX032 | `list-blank-lines` | list | warning | 否 | 清單與相鄰段落之間缺空行 |
| MDX035 | `empty-list-item` | list | warning | 否 | 空清單項目 |
| MDX040 | `emphasis-inner-space` | emphasis | warning | 是 | 強調符號內側多餘空白 |
| MDX041 | `unmatched-emphasis` | emphasis | warning | 否 | 同一行 `**`、`__`、`~~` 數量為奇數 |
| MDX042 | `cjk-emphasis-unpaired` | emphasis | warning | 部分 | CJK／全形標點旁無法配對的 `**`、`*` |
| MDX050 | `fence-language` | fence | info | 否 | fenced code 沒有標語言 |
| MDX051 | `fence-unclosed` | fence | error | 否 | fenced code 沒有關閉 |
| MDX052 | `fence-blank-lines` | fence | warning | 是 | 頂層 fenced code 與相鄰區塊之間缺空行 |
| MDX053 | `fence-invalid` | fence | error | 否 | backtick fence 的 info string 含 backtick（阻擋） |
| MDX060 | `table-delimiter-width` | table | warning | 是 | 表格分隔列某格少於三個 `-` |
| MDX061 | `table-column-count` | table | error | 否 | 表格各列的欄數不一致 |
| MDX062 | `table-blank-lines` | table | warning | 是 | 頂層表格與相鄰區塊之間缺空行 |
| MDX063 | `table-pipe-in-code` | table | warning | 否 | 表格 cell 內 code 或 URL 含未跳脫的 `\|` |
| MDX070 | `bare-url` | link | info | 否 | 裸 URL |
| MDX071 | `empty-link` | link | warning | 否 | 連結文字或目的地是空的 |
| MDX072 | `link-space-before-destination` | link | info | 否 | `]` 與 `(` 之間有空白 |
| MDX073 | `reference-missing-definition` | link | warning | 否 | 參考連結找不到定義 |
| MDX074 | `reference-duplicate-definition` | link | warning | 否 | 參考定義重複 |
| MDX075 | `image-alt` | link | warning | 否 | 圖片沒有 alt text |
| MDX080 | `setext-ambiguity` | structure | warning | 否 | 段落下一行的 `---`／`===` 變成 Setext 標題 |
| MDX081 | `frontmatter-unclosed` | structure | error | 否 | YAML frontmatter 沒有關閉（阻擋） |
| MDX082 | `frontmatter-blank-line` | structure | warning | 是 | frontmatter 後缺空行 |
| MDX083 | `html-comment-unclosed` | structure | error | 否 | HTML comment 沒有關閉（阻擋） |
| MDX084 | `fullwidth-markdown-syntax` | structure | warning | 否 | 用全形標點寫 Markdown 語法 |
| MDX085 | `math-unclosed` | structure | error | 否 | `$$` 數學區塊沒有關閉（阻擋） |
| MDX086 | `resource-limit` | structure | error | 否 | 引言巢狀深度超過上限（阻擋） |
| MDX090 | `line-length` | style | info | 否 | 超過指定行長（預設停用） |

## 保護區域

除非規則另有說明，下列內容一律不檢查也不修改。這是[安全契約](architecture.md#安全契約與拒絕修正)的一部分。

- **整行保護**：frontmatter、fenced code 的內容行、raw HTML 區塊與 HTML comment、縮排 code block、`$$` 數學區塊、參考定義行（`[label]: url`）。
- **行內保護**：反斜線跳脫、inline code、`$...$` 與 `$$...$$`、連結目的地 `](...)`、參考標籤 `][...]`、角括號 autolink 與 inline HTML `<...>`、裸 URL 與 email。

## 編碼與空白

### MDX001 `bom`

- 嚴重度：warning
- 自動修正：是

檔案第一個字元是 U+FEFF 時回報並移除。BOM 常來自 Windows 工具或網頁貼上，會讓某些解析器把第一行的 `#` 或 `---` 當成普通文字。檔案中間的 U+FEFF 由 MDX009 處理。

停用 MDX001 時，`fix_document` 會把原本的 BOM 加回輸出開頭。

### MDX002 `line-endings`

- 嚴重度：warning
- 自動修正：是

檔案含 `\r` 時回報一次，行號是第一個 `\r` 所在的行。修正會把 `\r\n` 與單獨的 `\r` 都換成 `\n`。

停用 MDX002 時：若沒有其他修正，檔案維持原樣（包括混合行尾）；若有其他修正，輸出的行尾一律換成原文出現過的行尾，CRLF 優先，其次 CR。

### MDX003 `final-newline`

- 嚴重度：warning
- 自動修正：是

檔案結尾沒有換行時補一個；結尾有多餘空行時收成一個換行。空檔案不處理。

```markdown
text


```

修正後：

```markdown
text
```

剪貼簿片段通常沒有檔尾換行，而且呼叫端不希望多出一個換行。這種情境請停用這條規則（CLI 加 `--ignore-rule MDX003`，函式庫在 `ignore_rules` 放 `"MDX003"`）。

### MDX004 `trailing-whitespace`

- 嚴重度：warning
- 自動修正：是

移除行尾的空格與 tab，但保留**剛好兩個空格**，因為那是 Markdown 的 hard line break。

| 行尾的空白 | 修正結果 |
| --- | --- |
| 剛好兩個空格 | 保留，不回報 |
| 以兩個以上空格結尾，例如三個空格、tab 加兩個空格 | 收成兩個空格 |
| 其他，例如一個空格、tab、空格加 tab | 全部移除 |
| 整行只有空白 | 變成空行 |

frontmatter、fenced code 內容、HTML 區塊、縮排 code、`$$` 數學區塊不處理。參考定義行會處理。

空的清單項目標記之後（`- `、`1.`，引言裡的 `> - ` 也算）以及註腳定義裡，有縮排的只含空白行不清空，MDX005 也不把它併進前後的空行：GitHub（cmark-gfm）把這種行當成內容，`- \n  \n  x` 在 GitHub 是一個內容為 `x` 的清單項目，清空後會變成空項目加一個段落。MDX003、MDX006 同樣不移除這種行（v1.1.0）。

### MDX005 `multiple-blank-lines`

- 嚴重度：warning
- 自動修正：是

兩個以上連續空行收成一個。fenced code、HTML 區塊等保護區域裡的空行不算。檔尾的多餘空行歸 MDX003 處理，檔首的歸 MDX006。

### MDX006 `leading-blank-lines`

- 嚴重度：warning
- 自動修正：是

移除檔案開頭的空行。

有一個例外：移除空行後，第一行如果變成 `---`，就會被 frontmatter 解析器當成 YAML 的開頭。這時 `fix_document` 會拒絕修正並回傳固定原因（frontmatter 陷阱）：

```text
A fix would turn a leading --- into YAML frontmatter. Fix the first lines manually; use *** if the line was meant as a horizontal rule.
```

如果那行 `---` 本來是水平線，改用 `***`；如果本來就是 frontmatter，把前面的空行刪掉。

另一個例外（v1.1.0）：空行後面那一行以 U+FEFF 開頭時不修，問題以 `fixable: false` 回報。移除空行會讓 U+FEFF 落在檔首，被當成 BOM 去掉，那一行的意義也跟著改變（例如 `\uFEFF# T` 從段落變成標題）；v1.0.0 因此也不冪等。請先刪掉那個 U+FEFF。

### MDX007 `hard-tab`

- 嚴重度：warning
- 自動修正：否

內文或縮排中出現 tab 時回報，每行只報第一個。inline code、連結目的地等保護區段裡的 tab 不算。tab 的顯示寬度隨環境而異，請改用空格。

### MDX008 `non-breaking-space`

- 嚴重度：warning
- 自動修正：部分

把保護區段以外的 NBSP（U+00A0）換成普通空格。回報以不受保護的區段為單位：被 inline code、連結目的地等隔開的兩段文字各自回報一次。Word、Outlook 與網頁貼上很常帶進 NBSP。

URL 裡的 NBSP 只回報（`fixable: false`）：直接換成空格會把 URL 拆成兩段。請人工改成 `%20` 或修正連結。

NBSP 是整行（或引言、清單項目）唯一的內容時，不修也不回報（v1.1.0）：換成普通空格會讓那一行變成空行，結束段落或清空清單項目。

緊接在清單標記後的 NBSP（`*<NBSP>文字`、`1.<NBSP>文字`，引言與外層清單標記之後也算）只回報（`fixable: false`，v1.1.0）：NBSP 對清單標記不算空白，所以那一行是段落，換成普通空格會變成清單項目。同一行其他的 NBSP 照常替換。

### MDX009 `zero-width-character`

- 嚴重度：warning
- 自動修正：否

偵測 U+200B（zero-width space）、U+2060（word joiner）與檔案中間的 U+FEFF，每行報第一個。U+200C、U+200D（ZWNJ、ZWJ）不在檢查範圍，所以 emoji 組合序列不會被誤報。確認不是刻意用來控制斷行後再刪除。

## 標題

### MDX010 `atx-missing-space`

- 嚴重度：warning
- 自動修正：是

`#` 到 `######` 後面直接接文字時補一個空格。

```markdown
#Title
###Section
```

修正後：

```markdown
# Title
### Section
```

`#hashtag` 在 CommonMark 不是標題，但這條規則分不出「壞掉的標題」與「刻意以 hashtag 開頭的段落」，修正後一定要看 diff。七個以上的 `#` 不處理。

### MDX011 `atx-multiple-space`

- 嚴重度：warning
- 自動修正：是

`#` 後面有兩個以上空白（空格或 tab）時收成一個。

```markdown
#   Title
```

修正後：

```markdown
# Title
```

### MDX012 `atx-closing-hashes`

- 嚴重度：warning
- 自動修正：是

移除 ATX 標題結尾、前面有空白的 `#`。這些 closing hashes 不影響渲染，卻容易數錯。

```markdown
## Section ##
```

修正後：

```markdown
## Section
```

`# C#` 這種前面沒有空白的 `#` 不會被移除。

### MDX013 `heading-blank-lines`

- 嚴重度：warning
- 自動修正：是

沒有縮排的 ATX 標題直接接在段落前面或後面時，補一個空行。前後兩側分開回報，所以同一個標題可能出現兩筆。

```markdown
intro
## Section
text
```

修正後：

```markdown
intro

## Section

text
```

只有相鄰的是**段落**才補。標題後面直接接清單、code fence、blockquote、以 `|` 開頭的表格列或另一個標題時不動，以免拆開緊湊的大綱排版。

### MDX014 `heading-indent`

- 嚴重度：info
- 自動修正：否

標題前面有 1 到 3 個空格。在清單項目裡縮排可能是刻意的；在頂層通常是意外。縮排的標題不列入 MDX015、MDX018、MDX019 的大綱計算。

### MDX015 `heading-increment`

- 嚴重度：warning
- 自動修正：否

標題層級一次往下跳超過一級，例如 H1 直接到 H3。往上跳（H4 回到 H2）不算。只看沒有縮排的 ATX 標題。

```markdown
# Title

### Skipped
```

### MDX016 `empty-heading`

- 嚴重度：warning
- 自動修正：否

`#` 到 `######` 後面沒有文字（只有空白也算）。刪掉或補上標題。

### MDX017 `heading-trailing-punctuation`

- 嚴重度：info
- 自動修正：否

標題結尾是 `.`、`:`、`;`、`!`、`?`、`。`、`：`、`；`、`！`、`？` 之一。問句標題（例如 FAQ）可以忽略。

```markdown
## 注意事項：
```

### MDX018 `first-heading-h1`

- 嚴重度：info
- 自動修正：否

文件裡第一個（沒有縮排的）ATX 標題不是 H1。Setext 標題（`===`、`---` 底線）不列入計算。從章節開始的片段或 include 檔可以忽略。

### MDX019 `multiple-h1`

- 嚴重度：info
- 自動修正：否

第二個以後的每個 H1 各回報一次。CHANGELOG、工作日誌、範例集常常刻意用多個 H1。

## Blockquote

### MDX020 `blockquote-space`

- 嚴重度：warning
- 自動修正：是

把每個 `>` 後面的空格統一成一個，巢狀的 `>>` 寫成 `> >`。

```markdown
>quote
>>nested
>  > one extra space
>  text
>     code
```

修正後：

```markdown
> quote
> > nested
> > one extra space
> text
>     code
```

`>` 後面有 4 個以上空格時視為 blockquote 裡的縮排 code，保留不動；`>` 後面接 tab 時也不處理。

v1.1.0 起，`>` 與它自己的空格之後的空白，只有兩種情況會移除：後面是一般內容時多出的**一個**空格，以及後面沒有內容時的至多三個空格。這些空白是內容的一部分，保留不動：

- 兩到三個空格：可能是引言裡清單項目的延續（`>   延續段落`、`>   > 巢狀引言`），移除後內容會跑出清單項目。
- fence 前的空格（`>  ```）：決定程式碼內容要扣掉多少縮排。
- 清單標記前的空格（`>  - item`）：決定項目內容的欄位，影響後面哪些行屬於這個項目。

一整串 `>  >  >` 在同一輪修完；v1.0.0 每一輪只合併一層，超過 6 層時 `fix_document` 不冪等。

## 清單

### MDX030 `list-marker-space`

- 嚴重度：warning
- 自動修正：是

清單標記與內容之間缺空格。

```markdown
-item
1.item
```

修正後：

```markdown
- item
1. item
```

為了減少誤判，以下情況不修：

- `--flag`、`++`：標記後面是同一個符號。
- `-1`、`+1`：標記後面是數字。
- `1.5`：數字標記後面還是數字。
- `1.a`：數字標記後面只有一個英文字母（像章節或選項編號）。
- `*item *x*`：`*` 後面的內容還有未跳脫的 `*`，分不出是清單還是強調。
- `---`、`***` 等水平線。

`-item` 也可能是以連字號開頭的英文詞，`1.ab` 會被改成 `1. ab`。修正後要看 diff。

### MDX031 `list-marker-style`

- 嚴重度：warning
- 自動修正：否（v1.1.0 起）

同一個縮排層級的 bullet 與這一串的第一個 bullet 不同時回報。只看 `-`、`*`、`+`，不看有序清單的 `.` 與 `)`。

```markdown
- a
* b
+ c
```

CommonMark 裡換一種 bullet 就會開始新的清單，所以上面本來就是三個清單。v1.0.0 會把 bullet 統一成 `-`，結果三個清單合併成一個；中間有空行時還會從 tight 變成 loose。這個修正不可能不改變渲染，所以 v1.1.0 改為只報告。是同一個清單就人工統一 bullet；是刻意分開的清單，就用其他內容隔開，或 `--ignore-rule MDX031`。

這一串的範圍在遇到沒有縮排的非清單內容、水平線，或頂層的 HTML 區塊、frontmatter 等保護行時結束；空行不會結束這一串。

### MDX032 `list-blank-lines`

- 嚴重度：warning
- 自動修正：否

清單直接接在段落後面，或清單最後一項後面直接接段落。後者在 CommonMark 會被讀成最後一項的 lazy continuation。

```markdown
intro
- a
- b
after
```

請在清單前後補空行。這條規則不自動修，因為補空行可能改變 tight／loose list 的語意。

### MDX035 `empty-list-item`

- 嚴重度：warning
- 自動修正：否

`-`、`*`、`+`、`1.`、`1)` 後面沒有內容。刪掉或補上內容。

## 強調

### MDX040 `emphasis-inner-space`

- 嚴重度：warning
- 自動修正：是

移除強調符號內側多餘的空白。`**` 與 `__` 兩側都修；單一 `*`、`_` 只修**不對稱**的情況（一側貼著內容、另一側有空白）。

```markdown
**  bold  **
**bold **
* italic*
*italic *
```

修正後：

```markdown
**bold**
**bold**
*italic*
*italic*
```

不修的情況：

- 對稱的單一符號，例如 `2 * 3 * 4` 或 `_ spaced _`，以免把算式變成斜體。
- `~~ strike ~~`：只由 MDX041 回報。
- 開頭的 delimiter 前面緊貼文字或標點，例如 `文字**  粗體**`。那個 `**` 可能是前一個強調的結尾。
- 行首（引言標記與外層清單標記之後）當作清單標記的 `*`（v1.1.0）。`* 說明*` 是清單項目，改成 `*說明*` 會變成段落。
- 跨行的強調。

配對時套用 CommonMark 的 flanking 規則（以 Unicode 標點分類判斷）：開頭的 delimiter 移除空白後必須是 left-flanking，而且在原文裡不能已經像是前一段強調的結尾；結尾端反過來判斷。所以一行裡兩段相鄰的粗體不會被誤配成一組。

### MDX041 `unmatched-emphasis`

- 嚴重度：warning
- 自動修正：否

保護區段以外，同一行的 `**`、`__` 或 `~~` 數量是奇數時回報（欄號固定是 1）。單一 `*`、`_` 不計；`__` 前後緊貼英數字（例如 `snake__case`）也不計。

```markdown
this **bold never closes
~~gone
```

可能是少了結尾，也可能是刻意顯示符號。後者請改用 `\*\*` 跳脫或放進 inline code。跨行的強調也會觸發這條規則，因為檢查是逐行做的。

### MDX042 `cjk-emphasis-unpaired`

- 嚴重度：warning
- 自動修正：部分

在每個段落內模擬 CommonMark 的 delimiter 配對，找出最後配不成對、而且緊鄰 CJK 字元或全形標點的 `*` run。常見的成因是全形標點讓收尾的 `**` 無法當作 right-flanking：

```markdown
**審查執行：**2026-11-04
**Spec 來源：**本次工作
```

GitHub 會把上面兩行的 `**` 原樣顯示出來。可以安全自動修正的有兩種，都是把全形標點移到 `**` 外，意思不變。

收尾端形如 `**X：**Y` 時改成 `**X**：Y`，把全形句讀標點（`、。，．：；！？…‥`，不含括號與引號）移到粗體外面：

```markdown
**審查執行**：2026-11-04
**Spec 來源**：本次工作
```

成對的全形引號括號包住整段強調內容時（`**「X」**`）改成 `「**X**」`，兩個引號一起移到 `**` 外。只處理成對的引號括號（「」『』《》〈〉【】〔〕（）），而且引號內的內容要緊貼 `**`、在同一行、中間沒有其他強調符號：

```markdown
這是**「引號」**文字
```

修正後：

```markdown
這是「**引號**」文字
```

其他開頭端的問題只回報（`fixable: false`），因為單邊的引號括號要怎麼搬需要人工判斷：

```markdown
這是**「引號**文字
這是**引號」**文字
```

不回報的情況：

- 能正常配對的寫法，例如 `**狀態：待修正**（2026-11-05）`、`標籤：**內容**`。
- 收尾 `**` 後面是空白或行尾，例如 `**標籤：**` 單獨一行。
- 半形標點，例如 `**Note:**text`，維持 CommonMark 原本的行為。
- 單獨一個配不成對的 `*`，例如 `claude-*` 這種字面星號。
- inline code、fenced code、數學式、URL 與 `\*` 跳脫裡的星號。

每個問題的 `**` 會回報兩筆：開頭與收尾各一筆，欄號分別指向兩個 delimiter。

## Fenced code

### MDX050 `fence-language`

- 嚴重度：info
- 自動修正：否

fenced code 開頭沒有 info string。知道語言時補上，例如 `js`；純文字或終端機輸出可以寫 `text`。

~~~markdown
```
code
```
~~~

### MDX051 `fence-unclosed`

- 嚴重度：error
- 自動修正：否

fence 一直到檔尾都沒有關閉。這條規則**不是**阻擋規則，但開頭以後的每一行都會被當成 code 內容保護起來，實際上不會被修正。先補上結尾 fence 再重跑。

~~~markdown
```js
code
~~~

### MDX052 `fence-blank-lines`

- 嚴重度：warning
- 自動修正：是

行首沒有縮排的 fenced code 直接貼著前後的非空行時，補一個空行。

~~~markdown
text
```js
code
```
more
~~~

修正後：

~~~markdown
text

```js
code
```

more
~~~

清單裡縮排的 fence 不處理。

blockquote 裡的 fence 補帶有相同深度標記的空行（`>`、`> >`…，v1.1.0）。v1.0.0 補的是完全空白的行，會結束 blockquote、把它拆成兩個。

~~~markdown
> text
> ```js
> x
> ```
> more
~~~

修正後：

~~~markdown
> text
>
> ```js
> x
> ```
>
> more
~~~

- 相鄰的行已經是引言空行（`>`、`> >`）時不回報。
- 前一行是 lazy continuation（沒有 `>` 的段落延續）時，補的 `>` 讓它留在同一個 blockquote 裡。
- 引言標記後還有縮排的 fence（`>   ```js`）可能屬於清單項目，補任何空行都會讓清單變成 loose list，所以不處理。

### MDX053 `fence-invalid`

- 嚴重度：error
- 自動修正：否（阻擋）

backtick fence 的 info string 裡又出現 backtick，例如 ```` ```js`bad ````。CommonMark 不會把它當成 fence 開頭，後面的內容會整段被誤讀。`fix_document` 會拒絕修正整份文件。

## 表格

表格偵測是近似的：表頭列含 `|`、下一行是分隔列（每格都是 `-` 與 `:` 的組合）就算一個表格，之後連續含 `|` 的非空行都算表格內容。blockquote 裡的表格與縮排 4 欄以上的表格不偵測。

### MDX060 `table-delimiter-width`

- 嚴重度：warning
- 自動修正：是

分隔列某一格只有一或兩個 `-` 時，補到三個，保留 `:` 對齊。一列只回報第一格。

```markdown
| a | b |
| - | --: |
| 1 | 2 |
```

修正後：

```markdown
| a | b |
| --- | ---: |
| 1 | 2 |
```

只處理真正的分隔列，不會把 `foo | -- | bar` 這種內文當成表格。

表頭與分隔列的儲存格數必須相同才回報與修正（v1.1.0），而且以 GFM 的規則計算：只有 `\|` 不分割儲存格，code span 裡的 `|` 也會分割。數量不同時 GFM 不把它當成表格，只是段落，修正分隔列等於改了段落文字。

~~~markdown
| a | `x|y` |
|:--|--:|
~~~

上面在 GFM 裡表頭有 3 格、分隔列 2 格，所以不是表格，MDX060 不處理。

### MDX061 `table-column-count`

- 嚴重度：error
- 自動修正：否

表頭與分隔列的格數不同，或某一列的格數和分隔列不同。通常是漏了或多了一個 `|`。

```markdown
| a | b |
| --- | --- |
| 1 | 2 | 3 |
```

### MDX062 `table-blank-lines`

- 嚴重度：warning
- 自動修正：是

沒有縮排的表格直接貼著前後的非空行時，補一個空行。清單內縮排的表格不處理，以免改變清單結構。

### MDX063 `table-pipe-in-code`

- 嚴重度：warning
- 自動修正：否

表格 cell 裡的 inline code 或 URL 含有未跳脫的 `|`。GFM 會先依 `|` 切 cell，再處理 inline code，所以這個 `|` 仍然會把 cell 切開。

```markdown
| cmd | note |
| --- | --- |
| `a | b` | x |
```

code 裡寫成 `\|`，URL 裡寫成 `%7C`。這條規則不自動修，避免改到 code 或 URL 的內容。

## 連結與圖片

### MDX070 `bare-url`

- 嚴重度：info
- 自動修正：否

保護區段以外、不在連結文字裡的 `http://`、`https://`、`ftp://` 或 `www.` 開頭的網址，每個各回報一次。要不要改成 `<https://...>` 或 `[文字](網址)` 取決於上下文，預設只給 info。

### MDX071 `empty-link`

- 嚴重度：warning
- 自動修正：否

`[](url)`、`[text]()`、`![](img)` 這類文字或目的地是空的連結。每個不受保護的區段只報第一個，所以同一行被 inline code 隔開的兩個空連結會回報兩筆。

### MDX072 `link-space-before-destination`

- 嚴重度：info
- 自動修正：否

`[text] (url)`：CommonMark 不會把它解析成連結。如果本來就要連結，刪掉 `]` 與 `(` 之間的空白。

### MDX073 `reference-missing-definition`

- 嚴重度：warning
- 自動修正：否

完整參考連結 `[text][label]` 與 collapsed 參考 `[text][]` 找不到對應的 `[label]: url`。shortcut 參考 `[label]` 不檢查，因為分不出它與一般中括號文字。

標籤比對前會先去掉前後空白、把連續空白收成一個空格、轉成小寫，所以 `[Foo]` 與 `[foo]` 視為同一個標籤。

### MDX074 `reference-duplicate-definition`

- 嚴重度：warning
- 自動修正：否

同一個標籤定義兩次以上，第二個以後各回報一次。CommonMark 使用第一個定義，後面的不會生效。

### MDX075 `image-alt`

- 嚴重度：warning
- 自動修正：否

`![](img)` 或 `![ ](img)` 沒有 alt text。裝飾性圖片可以刻意留空，但要確認。這種寫法同時也會觸發 MDX071。

## 結構

### MDX080 `setext-ambiguity`

- 嚴重度：warning
- 自動修正：否

段落下一行是 `---` 或 `===` 時，CommonMark 會把它讀成 Setext 標題的底線，段落變成標題，而不是一條水平線。

```markdown
paragraph
---
```

要水平線就在上面補空行，或改用 `***`；要標題就改成 ATX 標題。前一行是表格、blockquote、清單、標題或參考定義時不回報。

### MDX081 `frontmatter-unclosed`

- 嚴重度：error
- 自動修正：否（阻擋）

檔案第一行是 `---`，之後找不到 `---` 或 `...` 結尾，而且第 2 到 12 行裡第一個非空行看起來像 YAML（`key:`）。

```markdown
---
title: x

# Title
```

如果第一個非空行不像 YAML，`---` 會被當成一般的水平線，不回報這條規則。

### MDX082 `frontmatter-blank-line`

- 嚴重度：warning
- 自動修正：是

frontmatter 結尾後面直接接內容時補一個空行。

```markdown
---
title: x
---
# Title
```

修正後：

```markdown
---
title: x
---

# Title
```

### MDX083 `html-comment-unclosed`

- 嚴重度：error
- 自動修正：否（阻擋）

行首（最多 3 個空格縮排）的 `<!--` 一直到檔尾都找不到 `-->`。整份文件後面都會被當成註解，所以拒絕修正。行中間的 `<!--` 不在這條規則的範圍。

### MDX084 `fullwidth-markdown-syntax`

- 嚴重度：warning
- 自動修正：否

全形標點被用在 Markdown 語法的位置：

- 行首的 `＃` 當標題標記。
- 行首的 `＊`、`＋`、`－` 後接空白，當清單標記。
- 全形中括號 `［…］` 後面緊接全形小括號 `（…）`（前面可以有 `！`），當連結或圖片。

```markdown
＃ 標題

－ 項目
```

只看語法位置，不會把中文段落裡的全形標點全部改成半形。注意這條規則是對整行比對，inline code 裡的全形連結寫法也會被回報；在文件裡舉例時，請把範例放進 fenced code block。

### MDX085 `math-unclosed`

- 嚴重度：error
- 自動修正：否（阻擋）

`$$` 數學區塊找不到結尾。區塊從行首、blockquote、清單或 task-list 內容開頭的 `$$` 開始，之後任何一行含 `$$` 就結束。blockquote 裡的區塊若在找到結尾前 blockquote 先結束，也會回報。

```markdown
$$
x = 1
```

行首的 `$$` 一律視為數學區塊開頭；如果只是文字，請改寫或加反斜線。

### MDX086 `resource-limit`

- 嚴重度：error
- 自動修正：否（阻擋）
- 新增於 v1.1.0

引言（`>`）巢狀深度超過上限時回報，避免病態輸入耗盡資源。上限由 `max_quote_depth`（JS `maxQuoteDepth`，CLI `--max-quote-depth N`）設定，預設 32，0 表示不限制。

- **深度怎麼算**：與 CommonMark 的引言標記相同，每一層是至多三個空格、`>`、再一個可有可無的空格或 tab，取能連續吃到的最多層數。`>  > x` 是 2 層，`>     > x` 是 1 層（第二個 `>` 是縮排 code）。
- **哪些行不算**：fenced code、HTML 區塊、數學區塊與 frontmatter 裡的行。
- **`on_limit = error`**（預設，CLI `--on-limit error`）：每一個超過上限的行回報一個 MDX086，欄號 1，`fix_document` 拒絕修正整份文件，原因是第一個 MDX086 的訊息。即使 `--ignore-rule MDX086` 也照樣拒絕。
- **`on_limit = ignore`**（CLI `--on-limit ignore`）：不回報。超過上限的行當成不透明的保護行：不是任何區塊的開頭，不當成 reference definition，不修改，也不被其他規則診斷。

兩種模式下，超過上限的行都不會被其他規則診斷或修正，差別只在要不要回報 MDX086 並拒絕修正。

```text
Blockquote nesting depth 33 exceeds the limit of 32. Reduce the nesting, raise --max-quote-depth (0 = no limit), or use --on-limit ignore to leave such lines untouched.
```

## 風格

### MDX090 `line-length`

- 嚴重度：info
- 自動修正：否

預設停用。用 `--max-line-length N` 啟用；`--rule MDX090` 沒有指定行長時預設 120。函式庫的 `Options::max_line_length` 為 0 時停用。

行長以 Unicode code point 計算，CJK 字元算 1，不是終端機顯示寬度。表格行與保護行不計。欄號固定是「上限 + 1」。

## markdownlint 近似對應

| markdown-fixer | markdownlint |
| --- | --- |
| MDX003 | MD047 |
| MDX004 | MD009 |
| MDX005 | MD012 |
| MDX007 | MD010 |
| MDX010 | MD018 |
| MDX011 | MD019 |
| MDX012 | MD020／MD021（部分） |
| MDX013 | MD022（部分） |
| MDX014 | MD023 |
| MDX015 | MD001 |
| MDX017 | MD026 |
| MDX018 | MD041（部分） |
| MDX019 | MD025（部分） |
| MDX020 | MD027 |
| MDX030 | MD030 |
| MDX031 | MD004（部分，只有 bullet） |
| MDX032 | MD032 |
| MDX040 | MD037 |
| MDX050 | MD040 |
| MDX052 | MD031 |
| MDX061 | MD056 |
| MDX062 | MD058 |
| MDX063 | MD056（部分） |
| MDX070 | MD034 |
| MDX071 | MD042 |
| MDX072 | MD011（部分） |
| MDX073 | MD052 |
| MDX075 | MD045 |
| MDX080 | MD003（部分） |
| MDX090 | MD013 |

沒有列出的規則（MDX001、MDX002、MDX006、MDX008、MDX009、MDX016、MDX035、MDX041、MDX042、MDX051、MDX053、MDX060、MDX074、MDX081 到 MDX086）在 markdownlint 沒有直接對應。

## CJK 與貼上來源的注意事項

- **Word／Outlook**：NBSP、彎引號、全形標點、清單階層遺失。這個工具只處理 Markdown 語法與 NBSP，不改引號或文字。
- **網頁**：零寬字元、NBSP、BOM、CRLF。先跑 `--check` 看 MDX001、MDX002、MDX005、MDX008、MDX009。
- **Excel**：表格欄數、cell 內換行、`|`。先處理 MDX061，再看 MDX060 與 MDX063。
- **CJK 排版**：中文沒有字間空格，工具不會為了「排版」在漢字之間加空白。行長以 code point 計算，不代表實際顯示寬度。
- **全形標點與粗體**：AI 產出的文件很常出現 `**標籤：**內容` 這種寫法，在 GitHub 上會顯示字面的 `**`。MDX042 會把它改成 `**標籤**：內容`。這個 repo 的寫作規範也要求冒號放在粗體外。
- **全形語法符號**：`＃`、`＊`、`＋`、`－`、`［］（）` 只在 Markdown 語法位置才由 MDX084 回報。

## 已知限制

這個工具是行導向的啟發式檢查器，不是 CommonMark parser。原因與取捨見 [architecture.md](architecture.md#為什麼不用-markdown-parser)。

- frontmatter 必須從檔案第一個位元組開始（前面可以有 BOM）。以 `---` 開頭、後面還有第二條 `---` 的檔案一律視為 frontmatter。
- HTML 區塊採 CommonMark type 1 到 7 的近似判斷。引言或清單標記後開始的多行 HTML 區塊（comment、`<pre>`、`<script>`、`<style>`、`<?…?>`、CDATA 等以結束標記收尾的區塊）也受保護，一直到結束標記；沒有關閉的 comment 以 MDX083 拒絕修正（v1.1.0）。type 6、7 一直延續到下一個空行，所以 `</div>` 後面沒有空行時，後面的 Markdown 會被當成 raw HTML 而不處理。
- 縮排段落延續（lazy continuation）與縮排 code 的邊界用啟發式判斷；4 欄以上的行如果緊接在段落後面，視為段落延續。
- 跨行的連結與強調、巢狀 blockquote 裡的標題與清單、複雜的 YAML 都不做完整解析。
- fence 的開關以行判斷，不追蹤容器。清單項目或引言裡開的 fence，若被容器外的 fence 標記「關閉」，工具會把之後的行當成一般文字；CommonMark 則是容器結束時 fence 也結束，容器外的標記開了新的 fence，之後的行渲染成程式碼。Rust 版的渲染保護會擋下這種情況下的修正（見 [parity.md](parity.md)）。
- `$$` 區塊內容本身含 `$$` 會提前結束；跨行的單一 `$...$` 不支援；`\[...\]`、`\begin{...}` 不特別保護。` ```math ` 由 fenced code 保護。
- `MDX010`、`MDX030` 有本質上的歧義，`--write` 後一定要看 diff。
