# Rule reference

這份檔案是 [`../SKILL.md`](../SKILL.md) 的規則目錄。`SKILL.md` 說明流程與安全界線；這裡說明每一條規則偵測什麼、能不能自動修、以及什麼情況要留給人判斷。

## 怎麼讀這份目錄

- **Severity**：`error` 會讓預設檢查失敗；`warning` 也會；`info` 只有在 `--strict` 時才讓檢查失敗。
- **Fixable**：`yes` 表示 `--write` 會處理；`no` 表示只報告。報告為 fixable 的項目仍可能因為結構錯誤而拒絕寫入。
- **Blocking**：`MDX081`、`MDX083`、`MDX053`、`MDX085`、`MDX086` 會讓整個檔案保持原樣，並回報原因。先人工修好再跑；blocking 規則不受 `--ignore-rule` 影響。
- **markdownlint 對應**：近似對應，不是等價。兩邊的規則邊界與預設值不同。

## 規則總表

| ID | Severity | Fixable | 檢查內容 |
| --- | --- | --- | --- |
| MDX001 | warning | yes | 檔案開頭的 UTF-8 BOM |
| MDX002 | warning | yes | CRLF 或 CR 行尾 |
| MDX003 | warning | yes | 檔尾是否剛好一個換行 |
| MDX004 | warning | yes | 行尾空白；兩個空格視為 hard break |
| MDX005 | warning | yes | 多個連續空行 |
| MDX006 | warning | 部分 | 檔首空行；後面那行以 U+FEFF 開頭時只報告 |
| MDX007 | warning | no | 文章或縮排中的 tab |
| MDX008 | warning | 部分 | NBSP；URL 內的 NBSP 只報告 |
| MDX009 | warning | no | 零寬字元與檔中 BOM |
| MDX010 | warning | yes | `#` 後缺空格 |
| MDX011 | warning | yes | `#` 後多個空格 |
| MDX012 | warning | yes | ATX 標題尾端多餘的 closing hashes |
| MDX013 | warning | yes | 標題與相鄰段落之間缺空行 |
| MDX014 | info | no | 標題有縮排 |
| MDX015 | warning | no | 標題層級跳級 |
| MDX016 | warning | no | 空標題 |
| MDX017 | info | no | 標題尾端句號、冒號等 |
| MDX018 | info | no | 第一個標題不是 H1 |
| MDX019 | info | no | 多個 H1 |
| MDX020 | warning | yes | blockquote 的 `>` 後空格不一致 |
| MDX030 | warning | yes | 清單標記與內容之間缺空格 |
| MDX031 | warning | no | 同一層級的清單 bullet 不一致（換 bullet 就是新清單，只報告） |
| MDX032 | warning | no | 清單與相鄰段落之間缺空行 |
| MDX035 | warning | no | 空清單項目 |
| MDX040 | warning | yes | 強調符號內側多餘空白 |
| MDX041 | warning | no | 可能未配對的 `**`、`__`、`~~` |
| MDX042 | warning | 部分 | CJK／全形標點旁配對不成立的 `**`／`*`；收尾端與成對引號可自動修 |
| MDX050 | info | no | fenced code 缺語言 |
| MDX051 | error | no | 未關閉的 fenced code |
| MDX052 | warning | yes | 頂層 fenced code 與相鄰 block 之間缺空行 |
| MDX053 | error | no | 非法 fence info（backtick fence 內含 backtick） |
| MDX060 | warning | yes | 表格分隔列每格少於三個 `-` |
| MDX061 | error | no | 表格 header、分隔列、body 欄數不一致 |
| MDX062 | warning | yes | 頂層表格與相鄰 block 之間缺空行 |
| MDX063 | warning | no | 表格 cell 內的 code/URL 含未轉義的 `\|` |
| MDX070 | info | no | 裸 URL |
| MDX071 | warning | no | 空連結或空目標 |
| MDX072 | info | no | `]` 與 `(` 之間有空白 |
| MDX073 | warning | no | 缺少 reference definition |
| MDX074 | warning | no | 重複的 reference definition |
| MDX075 | warning | no | 圖片缺 alt text |
| MDX080 | warning | no | `---`/`===` 被讀成 Setext 標題 |
| MDX081 | error | no | 未關閉的 YAML frontmatter |
| MDX082 | warning | yes | frontmatter 後缺空行 |
| MDX083 | error | no | 未關閉的 HTML comment |
| MDX084 | warning | no | 全形標點被用在 Markdown 語法位置 |
| MDX085 | error | no | 未關閉的 `$$` 數學區塊 |
| MDX086 | error | no | 引言巢狀深度超過 `--max-quote-depth`（資源上限） |
| MDX090 | info | no | 超過指定行長 |

## 可自動修正的規則與範例

### MDX001：UTF-8 BOM

檔案第一個字元是 U+FEFF 時移除。BOM 常來自 Windows 工具或某些網頁貼上。

```text
﻿# Title
```

修正後：

```markdown
# Title
```

### MDX002：CRLF / CR

把 `\r\n` 與單獨的 `\r` 統一成 `\n`。停用這條規則時，若沒有其他修正，檔案維持原樣（含混合行尾）；若有其他修正，混合行尾會以 CRLF 優先、其次 CR 統整。

### MDX003：檔尾換行

- 沒有結尾換行：補一個。
- 檔尾有多個空行：收成一個。
- 空檔案：不動。

```markdown
text


```

修正後：

```markdown
text
```

### MDX004：行尾空白

移除一般行尾空白，但保留**剛好兩個空格**，因為那是 Markdown 的 hard line break。fenced code、frontmatter、HTML block、indented code 與多行 `$$` 數學區塊內不處理。

```markdown
這行要換行  
下一行
```

`這行要換行` 後面的兩個空格會被保留。三個以上的空格會收成兩個；一個空格會移除。

空的清單項目標記之後（`- `、`1.`，引言裡的 `> - ` 也算）以及註腳定義裡，有縮排的只含空白行不清空，MDX005 也不把它併進前後的空行：GitHub（cmark-gfm）把這種行當成內容，`- \n  \n  x` 在 GitHub 是一個內容為 `x` 的清單項目，清空後會變成空項目加一個段落。

### MDX005 / MDX006：空行

- MDX005：兩個以上連續空行收成一個。
- MDX006：移除檔案開頭的空行，讓第一個 block 從第 1 行開始。空行後面那一行以 U+FEFF 開頭時不修，只報告（fixable 為 `false`）：移除空行會讓 U+FEFF 落在檔首、被當成 BOM 去掉，那一行的意義也會改變。請先刪掉 U+FEFF。

```markdown

# Title


text
```

修正後：

```markdown
# Title

text
```

### MDX008：NBSP

把 code、數學式、URL 以外的 U+00A0 換成普通空格。Word、Outlook 與網頁貼上很常帶入 NBSP。

URL 內的 NBSP 會保留並回報，因為直接換成普通空格會讓 URL 變成兩個 token。請人工改成 `%20` 或修正連結。

NBSP 是整行（或引言、清單項目）唯一的內容時也不換：換成普通空格會讓那一行變成空行，結束段落或清空清單項目。

緊接在清單標記後的 NBSP（例如 `*<NBSP>文字`、`1.<NBSP>文字`）只報告（fixable 為 `false`）：NBSP 對清單標記不算空白，所以那一行是段落，換成普通空格會讓它變成清單項目。真的要清單就人工改成普通空格。

### MDX010 / MDX011 / MDX012：ATX 標題

```markdown
#Title
##  Sub  ##
```

修正後：

```markdown
# Title
## Sub
```

- MDX010：`#` 後沒有空白。
- MDX011：`#` 後有兩個以上空白。
- MDX012：移除結尾的 `###`，因為它們容易被誤數，且不影響渲染。

`#hashtag` 在 CommonMark 不是標題。這個工具無法分辨「壞掉的標題」與「刻意以 hashtag 開頭的段落」，所以 MDX010 的修正要看 diff。

### MDX013：標題與段落之間的空行

只在標題緊鄰**段落**時補空行。標題後面直接接清單或 code fence 時不動，避免把緊湊的大綱排版拆開。

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

### MDX020：Blockquote

把 `>` 之間與之後的空格統一成一個；保留縮排 4 個空白以上的 code block，以及以 tab 分隔的內容。

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

`>` 後面的空白有時是內容的一部分，這些情況不修：

- 兩到三個空格：可能是引言裡清單項目的延續（例如 `>   延續段落`、`>   > 巢狀引言`），移除後內容會跑出清單項目。
- fence 前的空格（`>  ```）：決定程式碼內容要扣掉多少縮排。
- 清單標記前的空格（`>  - item`）：決定項目內容的欄位，影響後面哪些行屬於這個項目。

整串 `>  >  >` 會在同一輪修完，所以修正是冪等的。

### MDX030：清單標記後的空格

```markdown
-item
1.item
```

修正後：

```markdown
- item
1. item
```

為了避免誤改，以下情況不修：

- `--flag`、`-1`、`+1`：可能是命令列參數或數字。
- `1.a`、`1.5`：可能是版本或章節編號。
- `*item *x*`：不確定 `*` 是清單還是強調。

`-item` 與 `- item` 的界線仍然只能靠 diff 判斷；連字號開頭的英文詞可能被誤認。

### MDX040：強調符號內側空白

修正 `**`、`__` 的兩側空白，以及單一 `*`、`_` 的**非對稱**空白。行首（引言標記與外層清單標記之後）當作清單標記的 `*` 不是強調符號，`* 說明*` 不會被改成 `*說明*`，否則清單項目會變成段落。

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

對稱的單一星號/底線（`* spaced *`、`_ spaced _`）不修，避免把 `2 * 3 * 4` 這類文字改成強調。`~~ strike ~~` 目前只由 MDX041 報告，不自動修。

配對時會用 CommonMark 的 flanking 規則（Unicode 標點分類）判斷哪個 delimiter 能當開頭、哪個能當結尾：開頭的 delimiter 必須在移除內側空白後是 left-flanking，而且在原文裡不能已經像是前一個強調的結尾（結尾端同理鏡像判斷）。所以「`。** E＋F…驗證。**`」這種一行兩個粗體、剛好相接的寫法不會被誤認成一組內側有空白的粗體；相對的，前面緊鄰文字或標點的 `**  粗體**` 不會被自動修，因為那個 `**` 可能是別的結構的一部分。跨行的強調不配對。

### MDX042：CJK 標點旁的強調配對

在每個段落內模擬 CommonMark 的 delimiter 配對（不是數單行奇偶），找出最後沒配對的 `**`／`*`；只有 delimiter 鄰接 CJK 或全形標點時才回報，英文 `**Note:**text` 這類維持 CommonMark 行為、不回報。行內 code、fenced code、數學式、URL 與 `\*` 跳脫都不算 delimiter。單獨一個沒配對的 `*`（例如 `claude-*` 這種字面星號）不會被當成壞掉的強調。

可自動修正的安全情況有兩種，都是把全形標點移到 `**` 外，意思不變。

收尾端形如 `**X：**Y` 時改成 `**X**：Y`，把全形句讀標點（`、。，：；！？…` 等，不含括號引號）移到收尾 `**` 外：

```markdown
**審查執行：**2026-11-04
**Spec 來源：**本次工作
```

修正後：

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

其他開頭端的問題只報告、不自動修，因為單邊的引號括號要怎麼搬需要人工判斷：

```markdown
這是**「引號**文字
這是**引號」**文字
```

`**狀態：待修正**（2026-…）`、`標籤：**內容**` 這種能正常配對的寫法不回報。

### MDX060：表格分隔列寬度

每個分隔 cell 至少三個 `-`，保留 `:` 對齊。

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

只會處理真正的分隔列（所有 cell 都是 `-` 與 `:` 的組合），不會把 `foo | -- | bar` 這種文章行當成表格。

表頭與分隔列的儲存格數必須相同才處理，而且以 GFM 的規則計算：只有 `\|` 不分割儲存格，code span 裡的 `|` 也會分割。數量不同時 GFM 不把它當成表格，只是段落，修正分隔列會改變文字內容。

### MDX052：Fenced code 前後的空行

頂層 fence 與相鄰 block 之間補一個空行。引言裡的 fence 補帶有相同深度標記的空行（`>`、`> >`…），不補完全空白的行，因為完全空白的行會結束引言、把它拆成兩個：

```markdown
> text
> ```js
> x
> ```
> more
```

修正後：

```markdown
> text
>
> ```js
> x
> ```
>
> more
```

相鄰的行已經是引言空行（`>`）時不回報。引言標記後還有縮排的 fence（例如 `>   ```js`）可能屬於清單項目，補任何空行都會讓清單變成 loose list，所以不處理。

### MDX082：Frontmatter 後的空行

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

## 只報告、需要人工判斷的規則

### 標題與結構

- **MDX014**：標題有 1–3 個空白縮排。清單內的標題可能是刻意的；先確認不是意外縮排。
- **MDX015**：H2 直接跳到 H4。改成下一個層級，或確認這是刻意的分段。
- **MDX016**：`##` 或 `##   ` 沒有文字。刪掉或補標題。
- **MDX017**：標題以 `.`、`:`、`?`、`。`、`：` 等結尾。標題通常不需要句末標點。
- **MDX018**：第一個標題不是 H1。簡短文件、從章節開始的文件可能刻意如此。
- **MDX019**：多個 H1。Changelog、任務日誌、範例文件可能刻意重複。
- **MDX080**：段落後面的 `---` 或 `===` 會變成 Setext 標題，不是水平線。要水平線就在上面加空行，或用 `***`。
- **MDX081**：`---` 開始的 frontmatter 找不到 `---` 或 `...` 結尾。工具會拒絕寫入。
- **MDX083**：`<!--` 找不到 `-->`。工具會拒絕寫入。
- **MDX084**：`＃`、`＊`、`＋`、`－`、`［］（）` 被用在標題、清單或連結的語法位置。只改語法位置，不會把中文段落裡的全形標點全部改成半形。
- **MDX085**：行首、blockquote、清單或 task-list 內容的 `$$` 數學區塊找不到結尾。工具會拒絕寫入，避免把後面的 Markdown 當成數學內容，或反過來破壞公式。
- **MDX086**：引言（`>`）巢狀深度超過上限（`--max-quote-depth`，預設 32，`0` 表示不限制）。深度依 CommonMark 的引言標記計算：每一層是至多三個空格、`>`、再一個可有可無的空格或 tab。fenced code、HTML 區塊、數學區塊與 frontmatter 裡的行不算。預設（`--on-limit error`）在每一個超過上限的行回報這個 error，並拒絕寫入整份檔案；blocking，不受 `--ignore-rule` 影響。`--on-limit ignore` 時不回報，超過上限的行改當成不透明的行：不修改、不診斷，也不當成 reference definition。

### 清單

- **MDX032**：清單直接貼在段落前後。CommonMark 可能把它讀成 lazy continuation；請補空行。這一條不自動修，因為補空行可能改變 loose/tight list 的語意。
- **MDX031**：同一個縮排層級的清單混用 `-`、`*`、`+`。CommonMark 裡換一種 bullet 就會開始新的清單，所以 `- a` 接 `* b` 本來就是兩個清單；統一 bullet 會把它們合併成一個（中間有空行時還會變成 loose list），因此 v1.1.0 起只報告。是同一個清單就人工統一 bullet；是兩個刻意分開的清單，就用其他內容隔開或 `--ignore-rule MDX031`。
- **MDX035**：`-`、`*`、`1.` 後面沒有內容。刪掉或補內容。

### 強調與 code fence

- **MDX041**：同一行有奇數個 `**`、`__` 或 `~~`。可能少了一個結尾，也可能是刻意顯示符號。
- **MDX042（只報告的部分）**：開頭端配對不成立的 `**`／`*`（例如 `這是**「引號**文字`），以及單一 `*`（例如 `*標籤：*內容`）。收尾端 `**X：**Y` 與成對引號括住的 `**「X」**` 會自動修；其他情況請人工決定是搬動標點、補 delimiter，還是改成 code。
- **MDX050**：fence 沒有語言。知道語言時補上，例如 ` ```js `。
- **MDX051**：fence 沒有關閉。這是 error；先補結尾 fence，再重跑。
- **MDX053**：backtick fence 的 info string 含 backtick，例如 ` ```js`bad `。這是 error，工具會拒絕寫入。

### 表格

- **MDX061**：header、分隔列、body 的 cell 數不一致。這是 error；通常是漏了 `|` 或多打了 `|`。
- **MDX062**：表格直接貼在段落或清單旁。只有行首無縮排的頂層表格會自動補空行；縮排表格不處理，以免改變清單結構。
- **MDX063**：表格 cell 裡的 code span 或 URL 含 `|`。GFM 會先把 row 切開；code 內寫 `\|`，URL 內用 `%7C`。這一條不自動修，避免改到 code 或 URL 內容。

### 連結與圖片

- **MDX070**：裸 URL。要不要改成 `<https://...>` 或 `[text](url)` 取決於上下文；預設只給 info。
- **MDX071**：`[](url)`、`[text]()`、`![](img)` 等空文字或空目標。
- **MDX072**：`[text] (url)`。CommonMark 不會把它解析成連結；如果本來要連結，移除 `]` 與 `(` 之間的空格。
- **MDX073**：`[text][label]` 找不到 `[label]: url`。補 definition，或改成 inline link。
- **MDX074**：同一個 label 定義兩次。CommonMark 使用第一個定義；請刪掉重複的。
- **MDX075**：`![](img)` 沒有 alt text。裝飾性圖片可以刻意留空，但要確認。

### 其他

- **MDX007**：文章或縮排中的 tab。tab 的顯示寬度依環境而異；用空白取代。code span 與 URL 內的 tab 會跳過。
- **MDX009**：U+200B（zero-width space）、U+2060（word joiner）、U+FEFF（mid-file BOM）。可能是網頁貼上或編輯器留下的；確認不是刻意用來控制換行。
- **MDX090**：超過 `--max-line-length` 的行。以 Unicode code point 計算，CJK 算 1；表格行不計入。

## markdownlint 近似對應

| 這個工具 | markdownlint |
| --- | --- |
| MDX003 | MD047 |
| MDX004 | MD009 |
| MDX005 | MD012 |
| MDX007 | MD010 |
| MDX010 | MD018 |
| MDX011 | MD019 |
| MDX012 | MD020 / MD021（部分） |
| MDX013 | MD022（部分） |
| MDX014 | MD023 |
| MDX015 | MD001 |
| MDX016 | 無直接對應 |
| MDX017 | MD026 |
| MDX018 | MD041（部分） |
| MDX019 | MD025（部分） |
| MDX020 | MD027 |
| MDX030 | MD030 |
| MDX031 | MD004（部分，只含 bullet） |
| MDX032 | MD032 |
| MDX035 | 無直接對應 |
| MDX040 | MD037 |
| MDX041 | 無直接對應 |
| MDX042 | 無直接對應 |
| MDX050 | MD040 |
| MDX051 | 無直接對應 |
| MDX052 | MD031 |
| MDX060 | 無直接對應 |
| MDX061 | MD056 |
| MDX062 | MD058 |
| MDX063 | MD056（部分） |
| MDX070 | MD034 |
| MDX071 | MD042 |
| MDX072 | MD011（部分） |
| MDX073 | MD052 |
| MDX074 | 無直接對應 |
| MDX075 | MD045 |
| MDX080 | MD003（部分） |
| MDX085 | 無直接對應 |
| MDX086 | 無直接對應 |
| MDX090 | MD013 |

## CJK 與貼上來源的常見問題

- **Word / Outlook**：NBSP、彎引號、全形標點、清單階層失落。這個工具只處理 Markdown 語法與 NBSP，不改彎引號或文章語氣。
- **網頁**：零寬字元、NBSP、BOM、CRLF、把 `<p>` 貼成空白行。先跑 `--check` 看 MDX001/002/005/008/009。
- **Excel**：表格欄數、cell 內換行、pipe。表格相關規則會先檢查 MDX061，再處理 MDX060/063。
- **VS Code**：fenced code 的語言與縮排。MDX050 只給 info，因為不是每個 code block 都需要語言。
- **CJK 排版**：中文沒有單字間空格，不要為了「排版」自動在漢字之間加空白。行長以 code point 計算，不代表實際顯示寬度。全形 `＃＊＋－［］（）` 只在 Markdown 語法位置會被 MDX084 點出。
- **全形標點與強調**：`**標籤：**內容` 這種「收尾 `**` 前面是全形標點、後面直接接文字」在 CommonMark 會顯示字面 `**`，由 MDX042 點出；`**X：**Y` 可自動改成 `**X**：Y`，成對引號括住的 `**「X」**` 可自動改成 `「**X**」`。`**標籤：**` 後面是空白或行尾時能正常收尾，不會被點出。`**Note:**text`（半形冒號）維持 CommonMark 行為，不回報。

## 安全邊界

- 這個工具是行導向的啟發式檢查器，不是 CommonMark parser。
- Frontmatter 必須從檔案第一個位元組開始（可有 BOM）。`---` 前有空行時，語意上是 thematic break；若移除空行會讓第一行（忽略 BOM）變成 `---`，即使那行可能只是 thematic break，工具也會拒絕寫入，請人工確認。
- HTML block 採 type 1–7 的近似判斷。引言或清單標記後開始的多行 HTML 區塊（comment、`<pre>`、`<script>`、`<style>`、`<?…?>`、CDATA 等以結束標記收尾的區塊）也受保護，一直到結束標記；沒有關閉的 comment 會拒絕寫入（MDX083）。type 6/7 持續到下一個空行，因此 `</div>` 後沒有空行時，後面的 Markdown 會被視為 raw HTML。
- 縮排 4 個空白欄位（tab 以 4 欄計）以上的表格、blockquote 內的表格，可能不會被偵測；清單內縮排 1–3 個空格的表格會偵測，但 MDX062 不處理。
- 縮排段落延續（lazy continuation）與 indented code 的邊界用啟發式判斷；4 欄以上的行若緊接段落，會視為段落延續。
- 跨行的 link/emphasis、巢狀 blockquote 內的標題與清單、複雜 YAML 不做完整解析。
- Fence 的開關以行判斷，不追蹤容器。清單項目或引言裡開的 fence，若被容器外的 fence 標記「關閉」，工具會把之後的行當成一般文字；CommonMark 則是容器結束時 fence 也結束，容器外的標記開了新的 fence，之後的行會渲染成程式碼。
- 修正不得改變渲染結果。在引言裡補空行時帶上相同的 `>` 前綴；無法保持結構時（例如引言裡清單項目內的 fence）就不修。
- 引言巢狀超過 `--max-quote-depth` 的行依 `--on-limit` 回報 MDX086 或當成不透明的行，避免病態輸入耗盡資源。
- 多行數學區塊以行首、blockquote、清單或 task-list 內容開頭的 `$$` 開始，之後任一行含 `$$` 結束；若區塊內容本身含 `$$` 會提前結束。行首 `$$` 一律視為數學區塊開頭；若只是文字，請改寫或加反斜線。跨行單一 `$...$` 不支援；` ```math ` 仍由 fenced code 保護。更深的混合巢狀（例如清單內 blockquote 再清單）可能無法精確配對，會保守地回報 MDX085；`\[...\]`、`\begin{...}` 等其他數學語法也不特別保護。
- `MDX010`、`MDX030` 有本質上的歧義；`--write` 後一定要看 diff。
- `MDX002` 停用時，若沒有其他修正，檔案維持原樣（含混合行尾）；若有其他修正，混合行尾會以 CRLF 優先、其次 CR 統整。
- URL 內的 NBSP 只報告、不自動改。
