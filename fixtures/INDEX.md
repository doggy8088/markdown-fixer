# 測試語料索引

`fixtures/corpus/` 是 golden 差異測試的語料，`fixtures/golden/` 是由 `scripts/gen-golden.mjs` 以 oracle 產生的預期輸出（不可手改）。這份索引本身不在語料目錄裡，不參與比對。測試怎麼跑見 [docs/testing.md](../docs/testing.md)，golden 格式見 [docs/parity.md](../docs/parity.md)。

- **規模**：209 份文件、4,642 行（2026-11-06）。
- **位元組**：`fixtures/.gitattributes` 設定 `* -text`，git 不會改寫 CRLF、CR 與 BOM。含 BOM、NBSP、零寬字元、CRLF 或沒有檔尾換行的檔案以 Node 寫入，不要用會自動正規化的編輯器修改。
- **自我檢查**：`gen-golden.mjs` 會確認 `rules/<ID>/trigger.md` 回報該規則、`negative.md` 不回報，`edge/blocking-*` 與 `edge/frontmatter-trap-*` 被拒絕修正，且 48 條規則都至少被回報一次。

## real：真實文件

從使用者自己的公開專案複製（只複製、未修改），用來確認真實文件上的行為一致。

| 檔案 | 來源 | 內容 |
| --- | --- | --- |
| `real/mhtml/README.md` | `~/projects/mhtml/README.md` | 中文 README，表格、指令範例、徽章 |
| `real/mhtml/README.en.md` | `~/projects/mhtml/README.en.md` | 英文 README |
| `real/mhtml/example.md` | `~/projects/mhtml/examples/example.md` | Markdown 功能展示：數學式、表格、程式碼 |
| `real/mhtml/screenshot.md` | `~/projects/mhtml/examples/screenshot.md` | 截圖用的短文件 |
| `real/dedent-paste/README.md` | `~/projects/dedent-paste/README.md` | 長篇中文 README，安裝、環境變數表、多平台說明 |
| `real/dedent-paste/CHANGELOG.md` | `~/projects/dedent-paste/CHANGELOG.md` | Keep a Changelog 格式的中文變更紀錄 |
| `real/dedent-paste/DEVELOPMENT.md` | `~/projects/dedent-paste/DEVELOPMENT.md` | 開發說明，大量程式碼區塊 |
| `real/dedent-paste/AGENTS.md` | `~/projects/dedent-paste/AGENTS.md` | AI agent 指引 |
| `real/skills/*-SKILL.md` | `~/.agents/skills/<名稱>/SKILL.md` | 2 份作者本人的 skill 說明（markdown-autocorrect、query-taiwan-holiday），含 frontmatter 與英文長文；第三方 skill 因授權考量不收錄 |

## synthetic：接近真實的手寫文件

依常見寫作情境撰寫，刻意放入該情境常見的錯誤。

| 檔案 | 情境 | 主要涵蓋 |
| --- | --- | --- |
| `synthetic/api-reference-zh.md` | 中文 API 文件 | 結尾井號、表格欄數不符與短分隔、code span 裡的管線、重複的 reference 定義、`**標籤：**內容` |
| `synthetic/changelog-keepachangelog.md` | 英文 CHANGELOG | 多個 H1、清單符號混用、缺空白、空清單項目、reference 連結 |
| `synthetic/tutorial-rust-cli.md` | 教學文章 | fence 緊接段落、沒有語言的 fence、縮排程式碼、引言缺空白、標題缺空白 |
| `synthetic/blog-hugo-frontmatter.md` | Hugo 部落格 | YAML frontmatter 後缺空行、shortcode、`<!--more-->` |
| `synthetic/blog-jekyll-liquid.md` | Jekyll 部落格 | Liquid 標籤、frontmatter、Setext 歧義的 `---` |
| `synthetic/docusaurus-mdx.md` | Docusaurus MDX | import／export、JSX 元件、admonition、`<details>` |
| `synthetic/storybook-mdx.markdown` | Storybook MDX（`.markdown` 副檔名） | JSX、MDX 註解、強調內側空白 |
| `synthetic/meeting-notes-outlook.md` | 從 Outlook 複製的會議紀錄（CRLF） | 標籤式中繼資料、NBSP、任務清單、空項目、行尾空白、多餘空行 |
| `synthetic/spec-prd-ai-generated.md` | AI 產出的規格書 | 大量 `**標籤：**內容`、巢狀清單、標題結尾冒號、表格 |
| `synthetic/ai-chat-answer.md` | 聊天機器人回答 | 從 H3 開始、清單裡的 fence、表格緊接標題 |
| `synthetic/code-review-report-ai.md` | AI code review 報告 | emoji、標籤、清單缺空白、標題裡的粗體 |
| `synthetic/math-notes.md` | 數學筆記 | `$$` 區塊（含引言與清單裡）、行內數學、金額 `$` |
| `synthetic/obsidian-note.md` | Obsidian 筆記 | wikilink、callout、`#tag` 行、`%%` 註解、嵌入圖片 |
| `synthetic/readme-en-library.md` | 英文函式庫 README | 置中 HTML、徽章、空 alt 圖片、`] (` 連結、未定義的 reference |
| `synthetic/html-heavy-readme.md` | 大量 raw HTML 的 README | HTML comment、`<details>`、`<table>`、`<pre>`、`<style>` |
| `synthetic/web-copy-paste.md` | 從網頁複製（BOM、CRLF） | 全形 `＃`、`－`、全形連結、NBSP（含 URL 內）、零寬空白、U+3000、行尾空白 |
| `synthetic/troubleshooting-faq.md` | FAQ | 標題結尾問號、引言缺空白、表格欄數不符、空清單項目 |
| `synthetic/config-guide.md` | 設定檔指南 | TOML／YAML／Makefile fence、tab、縮排程式碼 |
| `synthetic/marp-slides.md` | Marp 投影片 | 以 `---` 分頁造成的 Setext 歧義、指令註解 |
| `synthetic/release-notes-zh.md` | 中文版本說明 | 清單符號混用、缺空白、強調內側空白、`#1188` |
| `synthetic/onboarding-mixed-lang.md` | 中英混排 onboarding | 任務清單、fence 緊接段落、表格、`[runbook][]` |
| `synthetic/weekly-report.md` | 週報 | 檔首空行、表格前後缺空行、引言缺空白 |

## snippets：剪貼簿片段

模擬從聊天工具、終端機、Excel 複製出來的不完整片段，多數沒有檔尾換行。

| 檔案 | 內容 |
| --- | --- |
| `snippets/chat-reply-no-newline.md` | 聊天回覆，`**結論：**`，沒有檔尾換行 |
| `snippets/slack-copy-nbsp.md` | 從 Slack 複製的 NBSP（含 URL 後），沒有檔尾換行 |
| `snippets/terminal-output.md` | 終端機輸出，大量行尾空白 |
| `snippets/table-from-excel.md` | Excel 複製的 tab 分隔表格 |
| `snippets/crlf-from-windows.md` | CRLF、清單缺空白、沒有檔尾換行 |
| `snippets/bom-notepad.md` | 記事本存檔的 BOM 與 CRLF |
| `snippets/cr-only-old-mac.md` | 只用 CR 換行 |
| `snippets/single-line-heading.md` | 只有一行 `#快速筆記` |
| `snippets/gpt-bullets.md` | 用「•」當清單符號的 AI 回答 |
| `snippets/code-only.md` | 只有一個 fence，沒有檔尾換行 |
| `snippets/leading-blank-lines.md` | 開頭三個空行 |
| `snippets/mixed-emphasis.md` | 強調內側空白與 `**截止：**` |
| `snippets/url-only.md` | 只有一個 URL |
| `snippets/quote-reply.md` | 引言缺空白的回覆 |
| `snippets/frontmatter-lookalike.md` | 空行後的 `---`（frontmatter 陷阱） |

## rules：逐條規則的觸發與反例

48 條規則各有一個目錄 `rules/<ID>/`：`trigger.md` 是會回報該規則的最小但接近真實的案例，`negative.md` 是看起來像、但不該回報該規則的案例。blocking 規則的 trigger 會被拒絕修正。MDX090 以 `line-length-80` 選項檢查，其餘以預設選項檢查。

| 規則 | `trigger.md` | `negative.md` |
| --- | --- | --- |
| MDX001 | 檔首 BOM | inline code 與 fence 裡提到 BOM，檔首沒有 |
| MDX002 | Outlook 複製的 CRLF 會議紀錄 | 只用 LF，inline code 裡提到 CRLF |
| MDX003 | 沒有檔尾換行 | 以 fence 結尾、剛好一個換行 |
| MDX004 | 一個與三個行尾空白、tab 結尾、空白行 | 兩個空白的硬換行、fence 裡的行尾空白 |
| MDX005 | 段落間三個空行 | fence 裡的連續空行 |
| MDX006 | 檔首兩個空行 | fence 開頭的空行 |
| MDX007 | 散文與清單裡的 tab | Makefile fence 與 inline code 裡的 tab |
| MDX008 | 從 Word 複製的 NBSP | code 裡的 NBSP、全形空白 U+3000 |
| MDX009 | 零寬空白、檔案中段的 BOM | ZWJ 表情符號、ZWNJ、code 裡的零寬空白 |
| MDX010 | 井號後缺空白 | `C#`、跳脫的井號、七個井號、`#include`、縮排程式碼 |
| MDX011 | 兩個空白、tab 間隔 | 一個空白、inline code |
| MDX012 | 結尾井號 | `C#`、`Issue#42`、`F#` 結尾 |
| MDX013 | 標題前後緊接段落 | 標題後接清單、引言與清單裡的標題 |
| MDX014 | 意外縮排的標題 | 清單後四個空白（縮排程式碼） |
| MDX015 | H1 跳到 H3 | 依序遞增、往回跳、fence 裡的標題 |
| MDX016 | 只有井號、井號後只有空白 | 有內容的標題、inline code 與跳脫的井號 |
| MDX017 | 結尾問號、冒號、句點 | 版本號、`C++`、引號、`Q&A` 結尾 |
| MDX018 | 從 H2 開始 | frontmatter 後先有縮排標題再有 H1 |
| MDX019 | 兩個 H1 | fence 裡的 H1、Setext H1 |
| MDX020 | 缺空白、巢狀缺空白、兩個空白 | 正常引言、引言裡的縮排程式碼 |
| MDX030 | `-`、`+`、`*`、`1.`、`2)` 後缺空白 | `--help`、`-1`、`+1`、`1.5`、`*強調*`、`1.a`、`-->` |
| MDX031 | 同一清單混用三種符號 | 巢狀層用不同符號、被段落隔開的兩份清單 |
| MDX032 | 清單緊接段落前後 | 有空行、清單緊接標題 |
| MDX035 | 空項目 `- ` 與 `1.` | 任務清單、分隔線、fence 裡的空項目 |
| MDX040 | 粗體、底線、斜體內側空白 | 對稱的單一星號、算式、code、連結目的地 |
| MDX041 | 未配對的 `**` 與 `~~` | 已配對、code 裡、`snake__case__name` |
| MDX042 | `**標籤：**內容`、開頭側的 `**「引號」**` | 已修正的寫法、ASCII 冒號、code、`claude-*` |
| MDX050 | 沒有語言的 backtick 與 tilde fence | 有語言的 fence |
| MDX051 | 未關閉的 fence | 較長的外層 fence、不同字元的 fence |
| MDX052 | fence 緊接段落前後 | 有空行、清單裡的 fence |
| MDX053 | info string 含反引號（拒絕修正） | tilde fence 的 info string 含反引號、`title=` 屬性 |
| MDX060 | 一個與兩個連字號的分隔儲存格 | 三個連字號、散文裡的 `--` |
| MDX061 | 列的儲存格數不符 | 跳脫的管線、code span 裡的管線 |
| MDX062 | 表格緊接段落前後 | 有空行、清單裡的表格 |
| MDX063 | code span 裡未跳脫的管線 | code span 裡跳脫的管線 |
| MDX070 | bare URL 與 `www.` | 角括號 autolink、連結文字、inline code |
| MDX071 | 空連結文字、空目的地 | 完整連結、inline code |
| MDX072 | `]` 與 `(` 之間有空白 | 正常連結、code 裡的 `a[0] (索引)` |
| MDX073 | 未定義的 reference label | 大小寫不同但已定義、collapsed reference、code 裡的 `arr[0][1]` |
| MDX074 | 大小寫不同的重複定義 | 不同 label、fence 裡的重複定義 |
| MDX075 | 空的與只有空白的 alt | 有 alt 的圖片、code 裡的空 alt |
| MDX080 | 段落後的 `---` | 有空行的 `---`、`***`、表格分隔列 |
| MDX081 | 未關閉的 frontmatter（拒絕修正） | 檔首 `---` 後不是 key: value |
| MDX082 | frontmatter 後緊接標題 | frontmatter 後有空行 |
| MDX083 | 未關閉的 HTML comment（拒絕修正） | 多行與單行 comment、inline code 裡的 `<!--` |
| MDX084 | 全形井號、減號、全形連結 | 散文裡的全形括號與方括號、行中的全形井號 |
| MDX085 | 未關閉的 `$$`（拒絕修正） | 行內 `$$x$$`、金額 `$5`、已關閉的區塊 |
| MDX090 | 超過 80 字元的英文與中文行（`line-length-80`） | 表格列、fence 裡的長行 |

## edge：邊界情況

| 檔案 | 內容 |
| --- | --- |
| `edge/blocking-frontmatter-unclosed.md` | 未關閉的 frontmatter（MDX081） |
| `edge/blocking-html-comment.md` | 未關閉的 HTML comment（MDX083） |
| `edge/blocking-fence-invalid.md` | info string 含反引號（MDX053） |
| `edge/blocking-quoted-fence-invalid.md` | 引言裡 info string 含反引號 |
| `edge/blocking-math-unclosed.md` | 未關閉的 `$$`（MDX085） |
| `edge/blocking-math-quote-ended.md` | 引言結束時 `$$` 還沒關閉 |
| `edge/frontmatter-trap-leading-blank.md` | 移除檔首空行會露出 `---` |
| `edge/frontmatter-trap-bom-line.md` | 空行後以 BOM 開頭的 `---` |
| `edge/frontmatter-trap-unclosed.md` | 空行後未關閉的 frontmatter |
| `edge/frontmatter-hr-no-trap.md` | 檔首 `---` 是分隔線、不是 frontmatter |
| `edge/cr-only.md`、`edge/mixed-eol.md` | 只用 CR、混合行尾 |
| `edge/double-bom.md`、`edge/bom-crlf-no-newline.md` | 兩個 BOM；BOM＋CRLF＋沒有檔尾換行 |
| `edge/empty.md`、`edge/only-newlines.md`、`edge/only-spaces.md`、`edge/single-hash.md` | 空檔與極短輸入 |
| `edge/trailing-blank-lines.md` | 檔尾多餘空行 |
| `edge/emoji-columns.md` | emoji 與 CJK 擴充字元的 UTF-16 欄號 |
| `edge/nested-containers.md`、`edge/quote-math-and-fence.md` | 巢狀引言、清單、fence、數學式 |
| `edge/html-blocks.md` | 各種 HTML block 類型（raw、comment、PI、declaration、CDATA、type 6／7） |
| `edge/indented-code-and-continuation.md` | 縮排程式碼與段落延續行、tab 展開 |
| `edge/thematic-breaks.md`、`edge/setext-and-tables.md` | 分隔線、Setext、表格邊界 |
| `edge/fence-variants.md` | fence 長度、字元、縮排的變化 |
| `edge/emphasis-flanking.md` | CommonMark flanking 與 CJK 強調配對 |
| `edge/reference-labels.md` | reference label 正規化（大小寫、Unicode、空白） |
| `edge/unicode-whitespace.md` | U+3000、NBSP、thin space |
| `edge/control-chars.md` | 控制字元、U+0085、U+2028、U+2029 |
| `edge/long-line.md`、`edge/deep-nesting.md` | 5,000 字元的行、30 層清單、12 層引言 |

## regressions：回歸案例

| 檔案 | 內容 |
| --- | --- |
| `regressions/js-mixed-idempotent.md` | oracle 測試「is idempotent on a mixed document」的輸入 |
| `regressions/mdx040-cjk-bold-already-paired.md` | oracle 測試：跨 CJK 標點已配對的粗體不可改寫 |
| `regressions/mdx042-money-amounts.md` | oracle 測試：金額 `$` 不可被當成未配對強調 |
| `regressions/oracle-mdx042-quoted-span.md` | v1.2.0：成對全形引號包住的 `**「X」**` 改為 `「**X**」`；單邊引號、同段多組與跨行只報告 |
| `regressions/oracle-mdx052-quoted-fence.md` | oracle 疑似問題 2：引言裡的 fence 被插入空行 |
| `regressions/oracle-mdx016-hashes-only.md` | oracle 疑似問題 3：`## ##` 不回報 MDX016 |
| `regressions/oracle-mdx084-inline-code.md` | oracle 疑似問題 4：inline code 裡的全形語法被回報 |
| `regressions/oracle-mdx018-setext-h1.md` | oracle 疑似問題 5：Setext H1 不算進標題結構 |
| `regressions/oracle-mdx030-section-number.md` | oracle 疑似問題 6：章節編號被改成清單 |
| `regressions/oracle-bom-exposed-by-leading-blanks.md` | oracle 疑似問題 7：露出的 BOM 讓修正不冪等 |
| `regressions/oracle-mdx042-multiple-transforms.md` | oracle 疑似問題 8：同一段落多個 MDX042 修正 |
| `regressions/definition-quote-prefixes.md` | 引言裡 reference definition 的前綴空白與 tab 變化（`>`、`>    `、`>\t`、四個空白開頭等）與近似失敗的寫法，每個定義後面都有引用 |
| `regressions/definition-deep-quotes.md` | 8～20 層引言裡的定義；14 層以上會讓 Rust 的 regex VM 改走記憶化回溯，確認結果仍與 oracle 相同 |

`scripts/fuzz-parity.mjs` 找到的差異會縮小後存成 `regressions/fuzz-s<種子>-d<編號>.md`。新增 regression 的流程見 [docs/testing.md](../docs/testing.md)。
