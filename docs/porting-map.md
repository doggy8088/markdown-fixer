# JS 與 Rust 對照表（porting map）

這份文件列出 oracle（`reference/js/markdown-fixer.mjs` v1.0.0）裡每個函式庫函式對應的 Rust 函式、JS 行號，以及移植時處理過的語意陷阱。要改某個行為前，先在這裡找到 JS 函式與行號，讀 JS 原始碼，再改 Rust。CLI 相關函式（`expandPaths`、`parseArgs`、`main` 等）見 [cli.md](cli.md)。

模組路徑都相對於 `crates/markdown-fixer/src/`。

## 共同基礎：UTF-16 與 JS 語意

oracle 的所有行內運算都以 JS 字串索引進行，也就是 UTF-16 code unit：

- `buildMask` 的遮罩陣列長度是 `line.length`，每個 code unit 一格。
- `text[i - 1]`、`content[0]` 這類取字元的寫法拿到的是單一 code unit；emoji 會被拆成兩個 lone surrogate。lone surrogate 不是空白、不是 `\p{P}`／`\p{S}`、也不是 CJK，在 flanking 判斷裡等同一般字母。
- `<[^>\n]{1,500}>` 的 500 是 code unit 數。
- 回報的欄號是 index + 1。

所以函式庫的行內處理一律以 `jsstr::JsString`（`Vec<u16>`）表示一行，只在 `fix_document`／`diagnose` 的入口與出口和 `String` 互轉。欄號因此天然與 JS 相同，不需要另外的換算層。

| 項目 | Rust | 說明 |
| --- | --- | --- |
| JS 字串 | `jsstr::JsString`、`jsstr::u`、`jsstr::to_string` | `to_string` 遇到 lone surrogate 以 U+FFFD 取代，不 panic |
| `\s`、`trim()` | `jsstr::is_js_space`、`jsstr::trim`、`jsstr::is_trim_empty` | 含 U+0009–U+000D、空格、U+00A0、U+1680、U+2000–U+200A、U+2028、U+2029、U+202F、U+205F、U+3000、U+FEFF；不含 U+0085 |
| `isBlank` | `scan::is_blank` | 只看空格與 tab，與 `trim() === ''` 不同，兩者不要混用 |
| `\w`、`\d`、`\b` | `jsstr::is_word`、`jsstr::is_digit`、`jsregex` 的 `\b` | 只有 ASCII |
| `/[\p{P}\p{S}]/u` | `jsstr::is_punct_or_symbol` | 表格 `unicode_tables.rs` 由 Node 以同一個 regex 對 BMP 逐一測試產生，只判斷單一 code unit |
| `split('\n')`、`join('\n')` | `jsstr::split_lines`、`jsstr::join_lines` | 保留最後一個空字串，不要用 `str::lines()` |
| `toLowerCase()` | `jsstr::to_lower_case` | 轉成 `String` 後用 `str::to_lowercase`，兩者都是 Unicode 預設對應（含 final sigma） |
| `Array.from(s).length` | `jsstr::code_point_len` | MDX090 的行長 |
| JS 正規表示式 | `jsregex::Regex`、`re!` 巨集 | 見下一節 |

### jsregex：JS 語意的正規表示式

oracle 用了大量 lookbehind、lookahead、惰性量詞與回溯語意，而且比對單位是 code unit。`regex` crate 不支援 lookaround、以 UTF-8 運作、`\s`／`\w`／`\b` 預設是 Unicode 語意，所以函式庫不用它，改用 `jsregex`：一個只涵蓋 oracle 語法子集的回溯式 VM，以 `&[u16]` 為輸入、非 `u` 模式的 JS 語意。

- **用法**：從 JS 移植 regex 時用 `re!(r"...")` 或 `re!(r"...", "i")` 原樣搬過來（延遲編譯、全程共用）。`/g` 的行為用 `find_iter`（空比對時 `lastIndex++`），`replace` 用 `replace_with`／`replace`。
- **支援**：字元類別、`.`（不含 U+000A、U+000D、U+2028、U+2029）、`^`、`$`、`\b`、`\B`、捕獲與非捕獲群組、`|`、貪婪與惰性量詞、lookahead、固定寬度 lookbehind、`i` 旗標（只做 ASCII 折疊；oracle 帶 `i` 的樣式都是 ASCII，這與 JS 非 `u` 模式的結果相同）。
- **不支援**：backreference、named group、可匹配空字串的無上限迴圈。這些會在編譯時回傳錯誤，`Regex::constant` 在 debug build 斷言失敗。
- **不會 stack overflow**：VM 用明確的回溯堆疊，不遞迴。
- **時間上限**：先以步數預算（`4096 + 64 × (長度 + 1)`）跑一般回溯，超過預算就改用記憶化回溯，以 `(pc, pos)` 已走過的位元集合跨起點共用，從頭重跑。沒有 backreference 時，一個狀態能不能走到 Match 與怎麼走到它無關，所以結果與 JS 完全相同，但最壞是 `O(指令數 × 長度)`。oracle 本身會在 `(?: {0,3}>[ \t]?)*` 這種重疊量詞上指數級回溯（40 層 `> ` 就卡住），Rust 不會。
- **起點預先過濾**：從程式開頭沿零寬度指令展開，算出第一個被消耗的 code unit 必須屬於的集合，不可能的起點直接略過。這只是必要條件，不影響結果。

## 規則目錄與選項

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `RULES` | 10–76 | `rules::RULES`、`lib::rules` | 由 oracle 產生，48 條、20 條可修正；`rules::tests::catalog_shape` 檢查 |
| `RULE_BY_ID` | 78 | `rules::find`、`lib::rule` | |
| `makeProblem` | 104–118 | `rules::make_problem`、`rules::Overrides` | `line`、`column` 小於 1 時夾成 1；`Overrides::NONE`、`NOT_FIXABLE`、`severity(..)` 對應 JS 的 `{}`、`{ fixable: false }`、`{ severity }` |
| `createEnabled` | 120–129 | `rules::Enabled::new`、`Enabled::is` | `rules: Some(vec![])` 停用全部規則（JS 空陣列是 truthy）；MDX090 需要 `max_line_length > 0` |
| `parseArgs` 的規則檢查 | 2306–2314 | `rules::validate`、`Options::validate` | 先檢查 `rules` 再檢查 `ignore_rules`，回報第一個未知 ID |

## 掃描（scan.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `RAW_HTML_TAGS`、`HTML_BLOCK_TAGS` | 93–102 | `scan::RAW_HTML_TAGS`、`scan::HTML_BLOCK_TAGS` | |
| `normalizeForAnalysis` | 131–133 | `scan::normalize_for_analysis` | 去掉開頭所有 BOM，CRLF／CR 換成 LF |
| `countIndent` | 135–141 | `scan::count_indent` | tab 對齊到 4 的倍數 |
| `isIndentedCodeLine` | 143–145 | `scan::is_indented_code_line` | |
| `isBlank` | 147–149 | `scan::is_blank` | 只看空格與 tab |
| `startsFrontmatterAfterLeadingBlanks` | 153–159 | `scan::starts_frontmatter_after_leading_blanks` | 這裡去掉開頭所有 U+FEFF（`/^\uFEFF+/`），`scanDocument` 的第一行只去掉一個 |
| `isThematicBreak` | 161–163 | `scan::is_thematic_break` | |
| `isAtxHeading` | 165–170 | `scan::is_atx_heading` | |
| `isListLine` | 172–174 | `scan::is_list_line` | |
| `isTopLevelLine` | 176–178 | `scan::is_top_level_line` | |
| `isParagraphLike` | 180–189 | `scan::is_paragraph_like` | |
| `detectHtmlBlockStart` | 191–226 | `scan::detect_html_block_start` | `closeRe` 改成 `HtmlClose` 列舉；raw 標籤的 `</tag\s*>` 以標籤名稱動態編譯 |
| `parseQuotePrefix` | 228–240 | `scan::parse_quote_prefix` | 回傳借用原行的 `QuotePrefix` |
| `parseQuoteFence` | 242–254 | `scan::parse_quote_fence` | |
| `parseMathOpen` | 258–287 | `scan::parse_math_open` | 最多剝 16 層容器前綴 |
| `isClosingFence` | 289–298 | `scan::is_closing_fence` | |
| `scanDocument` | 300–531 | `scan::scan_document`、`scan::Scan`、`scan::LineInfo` | `frontmatterEnd` 的 `-1` 改成 `None`；`fenceChar` 的 `''` 改成 `None` |
| 參考定義行 regex | 515、1331 | `scan::match_definition`、`scan::definition_re` | 回傳 label（第 1 組）的範圍；靠 jsregex 的記憶化避開指數回溯 |
| `isProtectedLine` | 533–542 | `scan::is_protected_line` | |
| 不含 `definition` 的保護判斷 | 1766、1780、1815、1836 等 | `scan::is_protected_block` | JS 在多處手寫 `frontmatter \|\| fenceContent \|\| htmlBlock \|\| indentedCode \|\| mathBlock` |

## 遮罩（mask.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `buildMask` | 544–656 | `mask::build_mask`、`mask::MaskOptions` | `MaskOptions::default()` 等於 JS 不帶選項；`MaskOptions::NO_BARE_URLS` 等於 `{ maskBareUrls: false }` |
| `markRegex` | 595–604 | `build_mask` 內的 `mark_regex` | 比對起點已被遮罩時整個比對略過，但仍從比對結尾繼續找下一個 |
| `/<[^>\n]{1,500}>/g` | 648 | `mask::angle_spans` | 線性實作：比對成立若且唯若 `<` 之後第一個 `>` 或 `\n` 是 `>` 且距離 1–500；`angle_spans_match_regex` 與原 regex 比對 |
| 裸 URL regex | 651、1796、2057 | `mask::url_re` | |
| `splitMasked` | 658–669 | `mask::split_masked`、`mask::Segment` | |
| `replaceWithOffsets` | 671–685 | `mask::replace_with_offsets` | |

`build_mask` 會先確認行內含有各樣式必需的字元（`$`、`]`、`[`、`<`、`@`、`:` 或 `.`）才跑對應的 regex，結果不變。

## 單行修正（line_fix.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `fixHeading` | 687–740 | `line_fix::fix_heading` | MDX010、MDX011、MDX012 |
| `fixBlockquote` | 742–773 | `line_fix::fix_blockquote` | MDX020 |
| `fixListMarker` | 775–822 | `line_fix::fix_list_marker` | MDX030；第一個樣式比對成功但被排除時直接回傳 `None`，不會再試後面的樣式 |
| `isBoundaryOrWhitespace` | 834–836 | `line_fix::is_boundary_or_whitespace` | `None` 代表行內內容的開頭或結尾 |
| `isUnicodePunct` | 838–840 | `line_fix::is_unicode_punct` | |
| `delimiterFlanking` | 845–854 | `line_fix::delimiter_flanking`、`line_fix::Flanking` | |
| `fixEmphasis` | 856–971 | `line_fix::fix_emphasis`、`line_fix::trim_pair` | MDX040；八個 `trimPair` 樣式原樣移植。`charBefore`／`charAfter` 讀的是該輪取代前的區段文字，超出區段時退回原始整行 |
| `fixNbsp` | 973–995 | `line_fix::fix_nbsp` | MDX008 |
| `transformLine` | 997–1026 | `line_fix::transform_line`、`line_fix::LineResult` | 回傳修正後的行與問題；`diagnose` 用 `.line` 組出 normalized lines |

`fix_emphasis` 在行內沒有 `*` 與 `_` 時、`fix_nbsp` 在沒有 U+00A0 時直接回傳原行，省下建立遮罩的成本，結果不變。

## 表格（table.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `splitTableCells` | 1028–1061 | `table::split_table_cells` | 每個 cell 都已 `trim()` |
| `isTableDelimiterRow` | 1063–1067 | `table::is_table_delimiter_row` | |
| `isTableHeaderCandidate` | 1069–1076 | `table::is_table_header_candidate` | |
| `isTableRow` | 1078–1086 | `table::is_table_row` | |
| `findTables` | 1088–1103 | `table::find_tables`、`table::Table` | |
| `fixTableDelimiter` | 1105–1133 | `table::fix_table_delimiter` | 欄號只在第一次修改前計算，等於原始各段長度的累加 |
| `normalizeLabel` | 1135–1137 | `table::normalize_label` | |
| `applyTableDelimiterFixes` | 1944–1954 | `table::apply_table_delimiter_fixes` | |

## CJK 強調（cjk_emphasis.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `CJK_ADJACENT_RE` | 829 | `cjk_emphasis::is_cjk_adjacent` | |
| `MOVABLE_PUNCT_RE` | 832 | `cjk_emphasis::is_movable_punct` | |
| `isEmphasisProseLine` | 1440–1442 | `cjk_emphasis::is_emphasis_prose_line` | |
| `startsInlineBlockBoundary` | 1444–1446 | `cjk_emphasis::starts_inline_block_boundary` | |
| `forEachEmphasisParagraph` | 1448–1466 | `cjk_emphasis::emphasis_paragraphs` | 回傳 `(from, to)` 清單，取代 callback |
| `findStarDelimiterRuns` | 1468–1493 | `cjk_emphasis::find_star_delimiter_runs` | |
| `matchEmphasisRuns` | 1499–1532 | `cjk_emphasis::leftover_runs` | 以索引取代物件身分。演算法改寫：JS 每個 closer 往回逐一掃描，是 O(n²)；Rust 把候選 opener 依 `(len % 3, can_close)` 分成 6 個 stack，dropped 區間用差分陣列最後一次標記，攤銷 O(n)。測試專用的 `leftover_runs_naive` 是直接移植，`leftover_equivalence_tests` 以 20,000 組隨機 run 序列比對兩者結果相同 |
| `analyzeCjkEmphasis` | 1534–1648 | `cjk_emphasis::analyze` | 沒有 `*` 的段落直接略過 |
| `computeCjkEmphasis` | 1650–1658 | `cjk_emphasis::compute_cjk_emphasis` | MDX042 |
| `applyCjkEmphasisFixes` | 1660–1677 | `cjk_emphasis::apply_cjk_emphasis_fixes` | |

`analyzeCjkEmphasis` 第 1621 行的 `run !== opener && run !== closer` 比較的是原始 run 與複本 item，永遠為真；但 opener、closer 本身不會落在 `[opener.end, closer.start]` 裡，所以 Rust 直接省略這兩個條件，結果相同。同一段落有多個 transform 時，JS 會把以原始段落計算的偏移量套用在已被前一個 transform 改過的行上，Rust 照做。

## 空行與行尾空白（blank_lines.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `computeBlankLineFixes` | 1679–1741 | `blank_lines::compute_blank_line_fixes`、`BlankLineFixes` | MDX013、MDX052、MDX062、MDX082；`issues` 給 `diagnose` 用 |
| `applyBlankLineFixes` | 1743–1759 | `blank_lines::apply_blank_line_fixes` | |
| `applyTrailingWhitespace` | 1775–1791 | `blank_lines::apply_trailing_whitespace` | 剛好兩個空格的 hard break 保留 |
| `applyBlankLineNormalization` | 1870–1911 | `blank_lines::apply_blank_line_normalization` | `replace(/\n*$/, '\n')` 手寫為「去掉結尾連續 `\n` 再補一個」 |
| `fixListStyle` | 1956–1990 | `blank_lines::fix_list_style` | MDX031 |

## 修正主流程（fix.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `FRONTMATTER_TRAP` | 2099 | `fix::FRONTMATTER_TRAP` | |
| `fixDocument` | 2101–2160 | `fix::fix_document`、`lib::fix_document` | 最多 6 輪；blocking 規則（MDX081、MDX083、MDX053、MDX085）不受 `ignore_rules` 影響 |
| blocking 判斷 | 2111、2122 | `fix::blocking_reason` | |

## 結構診斷與 diagnose（diag 負責）

| JS 函式 | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `computeHeadingStructure` | 1139–1195 | `structure::compute_heading_structure` | MDX014–MDX019。標題比對改用線性的 `match_atx_heading` 與 `strip_closing_hashes`；`/[.:;!?。：；！？]$/` 改成檢查最後一個 code unit |
| `computeListStructure` | 1197–1273 | `structure::compute_list_structure` | MDX031、MDX035、MDX032。bullet 比對改用線性的 `match_bullet`；MDX032 的項目判斷用 `scan::is_list_line`（同一個 regex） |
| `computeTableStructure` | 1275–1323 | `structure::compute_table_structure` | MDX061、MDX063、MDX060。MDX060 欄號的 `parts.slice(0, k).join('\|').length + (k > 0 ? 1 : 0)` 等於第 k 段的起點，以累加長度計算 |
| `computeReferenceStructure` | 1325–1389 | `structure::compute_reference_structure` | MDX074、MDX071、MDX075、MDX072、MDX073。定義行用 `scan::match_definition`；MDX071、MDX072、MDX073 改用線性比對器；label 正規化用 `table::normalize_label`，以 `JsString` 為 `HashSet` 鍵 |
| `computeSetextAmbiguity` | 1391–1414 | `structure::compute_setext_ambiguity` | MDX080。regex 原樣移植（都是錨定的線性樣式） |
| `computeFullwidthSyntax` | 1416–1431 | `structure::compute_fullwidth_syntax` | MDX084。前兩個錨定樣式原樣移植；連結樣式改用線性的 `has_fullwidth_link` |
| `computeTrailingWhitespace` | 1761–1773 | `structure::compute_trailing_whitespace` | MDX004。`/[ \t]+$/` 等於行尾連續空格與 tab，手寫計數 |
| `computeNbspInUrls` | 1793–1808 | `structure::compute_nbsp_in_urls` | MDX008。遮罩選項 `{ maskBareUrls: false, maskLinkDestinations: false }`，URL 樣式用 `mask::url_re` |
| `computeZeroWidth` | 1810–1828 | `structure::compute_zero_width` | MDX009。每行只回報第一個未保護區段中的 U+200B、U+2060、U+FEFF |
| `computeBlankLineRuns` | 1830–1868 | `structure::compute_blank_line_runs` | MDX006、MDX005、MDX003。在第一次 scan（行內修正前）上執行 |
| `computeLineLength` | 1913–1929 | `structure::compute_line_length` | MDX090。長度用 `jsstr::code_point_len`；表格行略過 |
| `dedupeProblems` | 1931–1942 | `diagnose::dedupe_problems` | 以 `rule:line:column:message` 去重；`sort_by` 是穩定排序，規則 ID 為 ASCII，`localeCompare` 等同位元組比較 |
| `diagnose` | 1992–2097 | `diagnose::diagnose` | 問題串接順序與 JS 相同。MDX007、MDX070、MDX050、MDX041 迴圈寫在函式內；MDX041 的 lookbehind 樣式原樣交給 `jsregex`，只在單一區段內比對，與 JS 相同 |

### diag 的線性比對器

oracle 的這幾個 regex 在回溯式引擎上是平方時間，下列函式都在 `structure.rs`，以線性掃描產生與原 regex 逐位相同的結果。

| 原 regex（JS 行號） | Rust | 做法 |
| --- | --- | --- |
| `/^( {0,3})(#{1,6})[ \t]+(.*?)[ \t]*$/`（1150） | `match_atx_heading` | 空白段貪婪吃完；剩餘部分含行終止字元（U+000A、U+000D、U+2028、U+2029）就失敗；第 3 組是去掉行尾空白後的剩餘部分 |
| `/[ \t]+#+[ \t]*$/` 取代（1154） | `strip_closing_hashes` | 去掉行尾空白後，取結尾的 `#` 段與它前面的空白段；兩段都非空才移除 |
| `/^( {0,3})([-*+])([ \t]+)(.*)$/`（1212） | `match_bullet` | 縮排至多 3、marker、至少一個空白，其後不得有行終止字元 |
| `/\[[ \t]*\]\([^)]*\)\|\][ \t]*\([ \t]*\)\|!\[[ \t]*\]\([^)]*\)/`（1357） | `find_empty_link` | `[^)]*\)` 等於「位置 p 之後還有 `)`」，用最後一個 `)` 的位置判斷；只需要起點 |
| `/\]\s+\([^)]*\)/`（1369） | `find_spaced_link` | `\s` 用 `jsstr::is_js_space` |
| `/!?\[([^\]\n]*)\]\[([^\]\n]*)\]/g`（1375） | `find_full_references` | 預先算出每個位置之後第一個 `]` 或 `\n`，每個起點 O(1)；與 `exec` 迴圈相同的不重疊全域比對 |
| `/！?［[^］\n]+］（[^）\n]+）/`（1425） | `has_fullwidth_link` | 兩個 next-stop 陣列；`！?` 不影響是否比對成功 |

**等價性測試**：`structure::matchers` 模組在小字母表上窮舉所有短字串（長度 6–8，字母表含 U+2028、NBSP、U+3000、`\n` 等邊界字元），與 `jsregex` 執行原樣式的結果比對起點與 capture 範圍；MDX073 比對整個 `find_iter` 序列。

## 已知的效能特性

- **連結目的地掃描**：`buildMask` 的 `](` 掃描（JS 609–639 行）在 `[](` 重複數千次、括號永遠不閉合的行上是平方時間，與 oracle 相同；10k code unit 約 30 ms（release）。正常文件不受影響。
- **1 MB 文件**：`fix_document` 約 145 ms、`diagnose` 約 60 ms（Apple M 系列、release）。量測方式見 `regex_hazards::one_megabyte_fix_document`（ignored）。

## 測試與探針

- **單元測試**：各模組的 `tests`。`parity_probe::scan_and_mask_match_oracle` 以 oracle 產生的預期輸出檢查 `scan_document` 與五組選項的 `build_mask`，包含 emoji、lone surrogate 與 `<` 加 260 個 emoji 的情況。
- **效能防護**：`regex_hazards` 對 core 模組用到的每一個 regex 與 `build_mask` 餵入 10k code unit 的病態輸入，並對整份病態文件跑 `fix_document` 與 `diagnose`。時間上限 50 ms 只在 release build 斷言；debug build 只印出超時的項目，避免慢速 CI 誤報。
- **探針（ignored）**：`parity_probe::parity_probe` 讀 `MDFIX_PROBE_IN`（JSON 字串陣列），依 `MDFIX_PROBE_MODE`（`scan`、`fix` 或 `diagnose`，選項放在 `MDFIX_PROBE_OPTS`）把結果寫到 `MDFIX_PROBE_OUT`，用來與 Node 端呼叫 oracle 的輸出逐字比較。設定 `MDFIX_FORCE_MEMO=1` 會讓 jsregex 從第一步就使用記憶化回溯（`jsregex::FORCE_MEMO`，只在測試中存在），用來驗證記憶模式的結果與 oracle 相同。
- **diag 的測試**：`structure::tests`、`diagnose::tests`、`diagnose::tests::diagnose_probe`（ignored）、`structure::timing::structure_timing`（ignored）。
