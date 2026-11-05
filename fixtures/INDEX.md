# 測試語料索引

`fixtures/corpus/` 是 golden 差異測試的語料，`fixtures/golden/` 是由 `scripts/gen-golden.mjs` 以 oracle 產生的預期輸出（不可手改）。這份索引本身不在語料目錄裡，不參與比對。測試怎麼跑見 [docs/testing.md](../docs/testing.md)，golden 格式見 [docs/parity.md](../docs/parity.md)。

- **規模**：192 份文件、4,893 行（2026-11-05）。
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
| `real/skills/*-SKILL.md` | `~/.agents/skills/<名稱>/SKILL.md` | 9 份 skill 說明（markdown-autocorrect、humanizer、writing-for-agents、teach、query-taiwan-holiday、tdd、writing-beats、eli5、marpit-markdown-slide-deck-framework），含 frontmatter 與英文長文 |

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

48 條規則各有一個目錄 `rules/<ID>/`：

- `trigger.md`：會回報該規則的最小但接近真實的案例。
- `negative.md`：看起來像、但不該回報該規則的案例，例如 `C#`、`--help`、`-1`、`1.5`、code 裡的語法、跳脫字元、ZWJ 表情符號、ZWNJ、全形標點、`$5` 金額、表格裡的長行。

blocking 規則（MDX081、MDX083、MDX053、MDX085）的 trigger 會被拒絕修正。MDX090 以 `line-length-80` 選項檢查，其餘以預設選項檢查。

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
| `regressions/oracle-mdx052-quoted-fence.md` | oracle 疑似問題 2：引言裡的 fence 被插入空行 |
| `regressions/oracle-mdx016-hashes-only.md` | oracle 疑似問題 3：`## ##` 不回報 MDX016 |
| `regressions/oracle-mdx084-inline-code.md` | oracle 疑似問題 4：inline code 裡的全形語法被回報 |
| `regressions/oracle-mdx018-setext-h1.md` | oracle 疑似問題 5：Setext H1 不算進標題結構 |
| `regressions/oracle-mdx030-section-number.md` | oracle 疑似問題 6：章節編號被改成清單 |
| `regressions/oracle-bom-exposed-by-leading-blanks.md` | oracle 疑似問題 7：露出的 BOM 讓修正不冪等 |

`scripts/fuzz-parity.mjs` 找到的差異會縮小後存成 `regressions/fuzz-s<種子>-d<編號>.md`。新增 regression 的流程見 [docs/testing.md](../docs/testing.md)。
