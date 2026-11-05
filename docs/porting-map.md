# JS 與 Rust 對照表（porting map）

這份文件列出 oracle（`reference/js/markdown-fixer.mjs` v1.1.0）裡每個函式庫函式對應的 Rust 函式、JS 行號，以及移植時處理過的語意陷阱。要改某個行為前，先在這裡找到 JS 函式與行號，讀 JS 原始碼，再改 Rust。CLI 相關函式（`expandPaths`、`parseArgs`、`main` 等）見 [cli.md](cli.md)。

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
- **時間上限**：先以步數預算（`4096 + 64 × (長度 + 1)`）跑一般回溯，超過預算就改用記憶化回溯，以 `(pc, pos)` 已走過的位元集合跨起點共用，從頭重跑。沒有 backreference 時，一個狀態能不能走到 Match 與怎麼走到它無關，所以結果與 JS 完全相同，但最壞是 `O(指令數 × 長度)`。oracle v1.0.0 會在定義行 regex 的 `(?: {0,3}>[ \t]?)*` 這種重疊量詞上指數級回溯（40 層 `> ` 就卡住）；v1.1.0 兩邊都改成線性的 `matchDefinition`。
- **起點預先過濾**：從程式開頭沿零寬度指令展開，算出第一個被消耗的 code unit 必須屬於的集合，不可能的起點直接略過。這只是必要條件，不影響結果。

## 規則目錄與選項

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `RULES` | 10–77 | `rules::RULES`、`lib::rules` | 由 oracle 產生，49 條、19 條可修正；`rules::tests::catalog_shape` 檢查 |
| `RULE_BY_ID` | 79 | `rules::find`、`lib::rule` | |
| `makeProblem` | 105–119 | `rules::make_problem`、`rules::Overrides` | `line`、`column` 小於 1 時夾成 1；`Overrides::NONE`、`NOT_FIXABLE`、`severity(..)` 對應 JS 的 `{}`、`{ fixable: false }`、`{ severity }` |
| `createEnabled` | 133–144 | `rules::Enabled::new`、`Enabled::is` | `rules: Some(vec![])` 停用全部規則（JS 空陣列是 truthy）；MDX090 需要 `max_line_length > 0` |
| `parseArgs` 的規則檢查 | 2516–2525 | `rules::validate`、`Options::validate` | 先檢查 `rules` 再檢查 `ignore_rules`，回報第一個未知 ID |

## 掃描（scan.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `RAW_HTML_TAGS`、`HTML_BLOCK_TAGS` | 93–102 | `scan::RAW_HTML_TAGS`、`scan::HTML_BLOCK_TAGS` | |
| `normalizeForAnalysis` | 146–148 | `scan::normalize_for_analysis` | 去掉開頭所有 BOM，CRLF／CR 換成 LF |
| `countIndent` | 150–156 | `scan::count_indent` | tab 對齊到 4 的倍數 |
| `isIndentedCodeLine` | 158–160 | `scan::is_indented_code_line` | |
| `isBlank` | 162–164 | `scan::is_blank` | 只看空格與 tab |
| `startsFrontmatterAfterLeadingBlanks` | 168–174 | `scan::starts_frontmatter_after_leading_blanks` | 這裡去掉開頭所有 U+FEFF（`/^\uFEFF+/`），`scanDocument` 的第一行只去掉一個 |
| `isThematicBreak` | 176–178 | `scan::is_thematic_break` | |
| `isAtxHeading` | 180–185 | `scan::is_atx_heading` | |
| `isListLine` | 187–189 | `scan::is_list_line` | |
| `isTopLevelLine` | 191–193 | `scan::is_top_level_line` | |
| `isParagraphLike` | 195–204 | `scan::is_paragraph_like` | |
| `detectHtmlBlockStart` | 206–241 | `scan::detect_html_block_start` | `closeRe` 改成 `HtmlClose` 列舉；raw 標籤的 `</tag\s*>` 以標籤名稱動態編譯。v1.1.0：raw 標籤的 lookahead 加上行尾 `$`，標籤名稱在行尾（`<iframe`）也算 |
| `parseQuotePrefix` | 243–255 | `scan::parse_quote_prefix` | 回傳借用原行的 `QuotePrefix` |
| `parseQuoteFence` | 257–269 | `scan::parse_quote_fence` | |
| `parseMathOpen` | 273–302 | `scan::parse_math_open` | 最多剝 16 層容器前綴 |
| `isClosingFence` | 304–313 | `scan::is_closing_fence` | |
| `scanDocument` | 361–622 | `scan::scan_document_with`、`scan::Scan`、`scan::LineInfo` | `frontmatterEnd` 的 `-1` 改成 `None`；`fenceChar` 的 `''` 改成 `None`。v1.1.0 多一個 `limits` 參數與 `opaque` 欄位；引言或清單標記後開始、同一行沒有關閉、以結束標記收尾的 HTML 區塊（raw、comment、`<?`、`<!`、CDATA）也算 HTML 區塊（第 591–605 行，前綴由 `scan::container_prefix` 計算）；`scan::scan_document`（預設上限）只給測試用，函式庫內的呼叫一律傳入選項的上限 |
| `scanQuoteMarkers` | 319–332 | `scan::scan_quote_markers` | v1.1.0。以 `(?: {0,3}>[ \t]?)*` 貪婪的方式數引言標記，回傳（深度, 結束位置）；MDX086 的深度與定義行掃描共用 |
| `matchDefinition` | 342–355 | `scan::match_definition` | v1.1.0 取代 v1.0.0 第 515、1331 行的 regex。先吃完引言前綴，再比對至多 3 個空格、`[label]:`、`[ \t]*` 後的非空白字元；回傳 label（第 1 組）的範圍。舊 regex 保留為測試專用的 `scan::definition_re`，`definition_equivalence_tests` 窮舉長度 5 以內的字串並以 2 萬行結構化輸入（引言深度到 24）比對兩者 |
| `createLimits`、`DEFAULT_MAX_QUOTE_DEPTH` | 121–131 | `scan::Limits`、`Limits::new`、`lib::DEFAULT_MAX_QUOTE_DEPTH` | v1.1.0。JS 把它掛在 `isEnabled.limits`；Rust 放在 `rules::Enabled::limits`，每次 `scan_document_with` 都傳入 |
| `resourceLimitMessage` | 357–359 | `scan::resource_limit_message` | MDX086 的訊息 |
| 資源上限檢查 | 502–513 | `scan_document_with` 內 | v1.1.0。frontmatter、fence、HTML、數學區塊、空行與縮排 code 之後才檢查；超過上限的行標成 `opaque` 並 `continue`，`error` 模式另外回報 MDX086 |
| `isProtectedLine` | 624–634 | `scan::is_protected_line` | v1.1.0 含 `opaque` |
| 不含 `definition` 的保護判斷 | 1514、1978、1992、2027 等 | `scan::is_protected_block` | JS 在多處手寫 `frontmatter \|\| fenceContent \|\| htmlBlock \|\| indentedCode \|\| mathBlock \|\| opaque` |

## 遮罩（mask.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `buildMask` | 636–748 | `mask::build_mask`、`mask::MaskOptions` | `MaskOptions::default()` 等於 JS 不帶選項；`MaskOptions::NO_BARE_URLS` 等於 `{ maskBareUrls: false }` |
| `markRegex` | 687–696 | `build_mask` 內的 `mark_regex` | 比對起點已被遮罩時整個比對略過，但仍從比對結尾繼續找下一個 |
| `/<[^>\n]{1,500}>/g` | 740 | `mask::angle_spans` | 線性實作：比對成立若且唯若 `<` 之後第一個 `>` 或 `\n` 是 `>` 且距離 1–500；`angle_spans_match_regex` 與原 regex 比對 |
| 裸 URL regex | 743、2008、2247 | `mask::url_re` | |
| `splitMasked` | 750–761 | `mask::split_masked`、`mask::Segment` | |
| `replaceWithOffsets` | 763–777 | `mask::replace_with_offsets` | |

`build_mask` 會先確認行內含有各樣式必需的字元（`$`、`]`、`[`、`<`、`@`、`:` 或 `.`）才跑對應的 regex，結果不變。

## 單行修正（line_fix.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `fixHeading` | 779–832 | `line_fix::fix_heading` | MDX010、MDX011、MDX012 |
| `fixBlockquote` | 839–887 | `line_fix::fix_blockquote` | MDX020。v1.1.0：一次吃完 `>  >` 串；只移除一般內容前多的一個空格或空內容前至多三個空格 |
| `startsIndentSensitiveBlock` | 835–837 | `line_fix::starts_indent_sensitive_block` | v1.1.0。fence 與清單標記 |
| `containerPrefix` | 972–983 | `scan::container_prefix` | v1.1.0。引言與清單標記的結束位置與 `*` bullet 的索引；MDX040 跳過 bullet，MDX008 判斷內容是否只剩空白 |
| `fixListMarker` | 889–936 | `line_fix::fix_list_marker` | MDX030；第一個樣式比對成功但被排除時直接回傳 `None`，不會再試後面的樣式 |
| `isBoundaryOrWhitespace` | 948–950 | `line_fix::is_boundary_or_whitespace` | `None` 代表行內內容的開頭或結尾 |
| `isUnicodePunct` | 952–954 | `line_fix::is_unicode_punct` | |
| `delimiterFlanking` | 959–968 | `line_fix::delimiter_flanking`、`line_fix::Flanking` | |
| `fixEmphasis` | 985–1104 | `line_fix::fix_emphasis`、`line_fix::trim_pair` | MDX040；八個 `trimPair` 樣式原樣移植，起點是清單標記 `*` 的比對不套用（v1.1.0）。`charBefore`／`charAfter` 讀的是該輪取代前的區段文字，超出區段時退回原始整行 |
| `fixNbsp` | 1106–1160 | `line_fix::fix_nbsp` | MDX008。v1.1.0：換完後容器前綴之後只剩空白時不換；緊接在清單標記後的 NBSP 不換，改以 `fixable: false` 回報（`Change::overrides`） |
| `transformLine` | 1162–1191 | `line_fix::transform_line`、`line_fix::LineResult` | 回傳修正後的行與問題；`diagnose` 用 `.line` 組出 normalized lines |

`fix_emphasis` 在行內沒有 `*` 與 `_` 時、`fix_nbsp` 在沒有 U+00A0 時直接回傳原行，省下建立遮罩的成本，結果不變。

## 表格（table.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `splitTableCells` | 1193–1226 | `table::split_table_cells` | 每個 cell 都已 `trim()` |
| `gfmCellCount`、`isGfmTable` | 1231–1245 | `table::gfm_cell_count`、`table::is_gfm_table` | v1.1.0。只有 `\|` 不分割；MDX060 只處理表頭與分隔列儲存格數相同的表格 |
| `isTableDelimiterRow` | 1247–1251 | `table::is_table_delimiter_row` | |
| `isTableHeaderCandidate` | 1253–1260 | `table::is_table_header_candidate` | |
| `isTableRow` | 1262–1270 | `table::is_table_row` | |
| `findTables` | 1272–1287 | `table::find_tables`、`table::Table` | |
| `fixTableDelimiter` | 1289–1317 | `table::fix_table_delimiter` | 欄號只在第一次修改前計算，等於原始各段長度的累加 |
| `normalizeLabel` | 1319–1321 | `table::normalize_label` | |
| `applyTableDelimiterFixes` | 2163–2174 | `table::apply_table_delimiter_fixes` | |

## CJK 強調（cjk_emphasis.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `CJK_ADJACENT_RE` | 943 | `cjk_emphasis::is_cjk_adjacent` | |
| `MOVABLE_PUNCT_RE` | 946 | `cjk_emphasis::is_movable_punct` | |
| `isEmphasisProseLine` | 1625–1627 | `cjk_emphasis::is_emphasis_prose_line` | |
| `startsInlineBlockBoundary` | 1629–1631 | `cjk_emphasis::starts_inline_block_boundary` | |
| `forEachEmphasisParagraph` | 1633–1651 | `cjk_emphasis::emphasis_paragraphs` | 回傳 `(from, to)` 清單，取代 callback |
| `findStarDelimiterRuns` | 1653–1678 | `cjk_emphasis::find_star_delimiter_runs` | |
| `matchEmphasisRuns` | 1684–1717 | `cjk_emphasis::leftover_runs` | 以索引取代物件身分。演算法改寫：JS 每個 closer 往回逐一掃描，是 O(n²)；Rust 把候選 opener 依 `(len % 3, can_close)` 分成 6 個 stack，dropped 區間用差分陣列最後一次標記，攤銷 O(n)。測試專用的 `leftover_runs_naive` 是直接移植，`leftover_equivalence_tests` 以 20,000 組隨機 run 序列比對兩者結果相同 |
| `analyzeCjkEmphasis` | 1719–1835 | `cjk_emphasis::analyze` | 沒有 `*` 的段落直接略過。演算法改寫：JS 對每個候選 closer 重掃全部 leftovers 找 opener、用 `runs.some` 找中間的 run、反向線性找行號，大量 `**X：**Y` 時是 O(n²)；Rust 事先算好每個 leftover 之前最後一個合格 opener，中間有沒有 run 改成比較 run 索引，行號用二分搜尋，每段 O(n log n)。測試專用的 `naive::analyze_naive` 是直接移植 |
| `computeCjkEmphasis` | 1837–1845 | `cjk_emphasis::compute_cjk_emphasis` | MDX042 |
| `applyCjkEmphasisFixes` | 1847–1864 | `cjk_emphasis::apply_cjk_emphasis_fixes` | 演算法改寫：JS 每個 transform 都重新 join、改寫、split 整個段落，是 O(transform 數 × 段落長度)；Rust 在整份文件上原地改寫每個 transform 的範圍，前提不成立時退回逐一套用（`apply_transforms_sequentially`）。測試專用的 `naive::apply_naive` 是直接移植，`linear_equivalence_tests` 比對兩者 |

`analyzeCjkEmphasis` 第 1808 行的 `run !== opener && run !== closer` 比較的是原始 run 與複本 item，永遠為真；但 opener、closer 本身不會落在 `[opener.end, closer.start]` 裡，所以 Rust 直接省略這兩個條件，結果相同。同一段落有多個 transform 時，JS 會把以原始段落計算的偏移量套用在已被前一個 transform 改過的行上。Rust 的結果與此相同，但不逐一重組段落：

- 每個 transform 只有兩個 edit，刪除 `[p, c)`，再在 `e` 插入原本的標點 `T`（`c <= e`，`T` 長度為 `c - p`）。依 start 由大到小套用在任何目前內容 `buf` 上，結果都是 `buf[..p] + buf[c..e] + T + buf[e..]`：長度不變，只動到 `[p, e)`。
- `buf[p..e]` 與 `T` 都沒有 `\n` 時，段落的行數與每行長度也不變，所以 JS 的 join、改寫、split、寫回各行，等於直接改寫整份文件中「段落起點加偏移量」的那一段，後面的 transform 看到的內容也與 JS 相同。
- `rotate_in_place` 在每個 transform 套用前檢查上述前提（edit 形狀、長度、沒有換行、不超出段落）。不成立時把目前結果切回各行，剩下的 transform 改用與 JS 相同的 `apply_transforms_sequentially`。`analyze` 產生的 transform 一定符合前提；`fallback_matches_sequential_application` 以手造的不合格 transform 驗證退回路徑。

## 空行與行尾空白（blank_lines.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `computeBlankLineFixes` | 1872–1949 | `blank_lines::compute_blank_line_fixes`、`BlankLineFixes` | MDX013、MDX052、MDX062、MDX082；`issues` 給 `diagnose` 用。v1.1.0：引言裡的 fence 以 `fill` 記錄要插入的 `>` 行 |
| `isQuoteBlankLine` | 1867–1870 | `blank_lines::is_quote_blank_line` | v1.1.0 |
| `applyBlankLineFixes` | 1951–1971 | `blank_lines::apply_blank_line_fixes` | |
| `applyTrailingWhitespace` | 1987–2003 | `blank_lines::apply_trailing_whitespace` | 剛好兩個空格的 hard break 保留 |
| `applyBlankLineNormalization` | 2086–2130 | `blank_lines::apply_blank_line_normalization` | `replace(/\n*$/, '\n')` 手寫為「去掉結尾連續 `\n` 再補一個」 |

## 修正主流程（fix.rs）

| JS | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `BLOCKING_RULES` | 2291 | `fix::blocking_reason` | v1.1.0 加入 MDX086 |
| `FRONTMATTER_TRAP` | 2293 | `fix::FRONTMATTER_TRAP` | |
| `fixDocument` | 2295–2353 | `fix::fix_document`、`lib::fix_document` | 最多 6 輪；blocking 規則（MDX081、MDX083、MDX053、MDX085、MDX086）不受 `ignore_rules` 影響。v1.1.0 不再呼叫 `fixListStyle`（MDX031 只回報） |
| blocking 判斷 | 2305、2316 | `fix::blocking_reason` | |

## 結構診斷與 diagnose（diag 負責）

| JS 函式 | JS 行號 | Rust | 備註 |
| --- | --- | --- | --- |
| `computeHeadingStructure` | 1323–1379 | `structure::compute_heading_structure` | MDX014–MDX019。標題比對改用線性的 `match_atx_heading` 與 `strip_closing_hashes`；`/[.:;!?。：；！？]$/` 改成檢查最後一個 code unit |
| `computeListStructure` | 1381–1457 | `structure::compute_list_structure` | MDX031、MDX035、MDX032。bullet 比對改用線性的 `match_bullet`；MDX032 的項目判斷用 `scan::is_list_line`（同一個 regex） |
| `computeTableStructure` | 1459–1507 | `structure::compute_table_structure` | MDX061、MDX063、MDX060。MDX060 欄號的 `parts.slice(0, k).join('\|').length + (k > 0 ? 1 : 0)` 等於第 k 段的起點，以累加長度計算 |
| `computeReferenceStructure` | 1509–1574 | `structure::compute_reference_structure` | MDX074、MDX071、MDX075、MDX072、MDX073。定義行用 `scan::match_definition`；MDX071、MDX072、MDX073 改用線性比對器；label 正規化用 `table::normalize_label`，以 `JsString` 為 `HashSet` 鍵 |
| `computeSetextAmbiguity` | 1576–1599 | `structure::compute_setext_ambiguity` | MDX080。regex 原樣移植（都是錨定的線性樣式） |
| `computeFullwidthSyntax` | 1601–1616 | `structure::compute_fullwidth_syntax` | MDX084。前兩個錨定樣式原樣移植；連結樣式改用線性的 `has_fullwidth_link` |
| `computeTrailingWhitespace` | 1973–1985 | `structure::compute_trailing_whitespace` | MDX004。`/[ \t]+$/` 等於行尾連續空格與 tab，手寫計數 |
| `computeNbspInUrls` | 2005–2020 | `structure::compute_nbsp_in_urls` | MDX008。遮罩選項 `{ maskBareUrls: false, maskLinkDestinations: false }`，URL 樣式用 `mask::url_re` |
| `computeZeroWidth` | 2022–2040 | `structure::compute_zero_width` | MDX009。每行只回報第一個未保護區段中的 U+200B、U+2060、U+FEFF |
| `computeBlankLineRuns` | 2042–2084 | `structure::compute_blank_line_runs` | MDX006、MDX005、MDX003。在第一次 scan（行內修正前）上執行。v1.1.0：空行後那一行以 U+FEFF 開頭時 MDX006 以 `fixable: false` 回報 |
| `computeLineLength` | 2132–2148 | `structure::compute_line_length` | MDX090。長度用 `jsstr::code_point_len`；表格行略過 |
| `dedupeProblems` | 2150–2161 | `diagnose::dedupe_problems` | 以 `rule:line:column:message` 去重；`sort_by` 是穩定排序，規則 ID 為 ASCII，`localeCompare` 等同位元組比較 |
| `pushAll` | 2178–2180 | `Vec::extend` | v1.1.0 取代 `issues.push(...spread)`，避免問題很多時 `RangeError` |
| `diagnose` | 2182–2287 | `diagnose::diagnose` | 問題串接順序與 JS 相同。MDX007、MDX070、MDX050、MDX041 迴圈寫在函式內；MDX041 的 lookbehind 樣式原樣交給 `jsregex`，只在單一區段內比對，與 JS 相同 |

### diag 的線性比對器

oracle 的這幾個 regex 在回溯式引擎上是平方時間，下列函式都在 `structure.rs`，以線性掃描產生與原 regex 逐位相同的結果。

| 原 regex（JS 行號） | Rust | 做法 |
| --- | --- | --- |
| `/^( {0,3})(#{1,6})[ \t]+(.*?)[ \t]*$/`（1334） | `match_atx_heading` | 空白段貪婪吃完；剩餘部分含行終止字元（U+000A、U+000D、U+2028、U+2029）就失敗；第 3 組是去掉行尾空白後的剩餘部分 |
| `/[ \t]+#+[ \t]*$/` 取代（1338） | `strip_closing_hashes` | 去掉行尾空白後，取結尾的 `#` 段與它前面的空白段；兩段都非空才移除 |
| `/^( {0,3})([-*+])([ \t]+)(.*)$/`（1396） | `match_bullet` | 縮排至多 3、marker、至少一個空白，其後不得有行終止字元 |
| `/\[[ \t]*\]\([^)]*\)\|\][ \t]*\([ \t]*\)\|!\[[ \t]*\]\([^)]*\)/`（1542） | `find_empty_link` | `[^)]*\)` 等於「位置 p 之後還有 `)`」，用最後一個 `)` 的位置判斷；只需要起點 |
| `/\]\s+\([^)]*\)/`（1554） | `find_spaced_link` | `\s` 用 `jsstr::is_js_space` |
| `/!?\[([^\]\n]*)\]\[([^\]\n]*)\]/g`（1560） | `find_full_references` | 預先算出每個位置之後第一個 `]` 或 `\n`，每個起點 O(1)；與 `exec` 迴圈相同的不重疊全域比對 |
| `/！?［[^］\n]+］（[^）\n]+）/`（1610） | `has_fullwidth_link` | 兩個 next-stop 陣列；`！?` 不影響是否比對成功 |

**等價性測試**：`structure::matchers` 模組在小字母表上窮舉所有短字串（長度 6–8，字母表含 U+2028、NBSP、U+3000、`\n` 等邊界字元），與 `jsregex` 執行原樣式的結果比對起點與 capture 範圍；MDX073 比對整個 `find_iter` 序列。

## 已知的效能特性

- **連結目的地掃描**：`buildMask` 的 `](` 掃描（JS 701–731 行）在 `[](` 重複數千次、括號永遠不閉合的行上是平方時間，與 oracle 相同；10k code unit 約 30 ms（release）。正常文件不受影響。
- **1 MB 文件**：`fix_document` 約 145 ms、`diagnose` 約 60 ms（Apple M 系列、release）。量測方式見 `regex_hazards::one_megabyte_fix_document`（ignored）。

## 測試與探針

- **單元測試**：各模組的 `tests`。`parity_probe::scan_and_mask_match_oracle` 以 oracle 產生的預期輸出檢查 `scan_document` 與五組選項的 `build_mask`，包含 emoji、lone surrogate 與 `<` 加 260 個 emoji 的情況。
- **效能防護**：`regex_hazards` 對 core 模組用到的每一個 regex 與 `build_mask` 餵入 10k code unit 的病態輸入，並對整份病態文件跑 `fix_document` 與 `diagnose`。時間上限 50 ms 只在 release build 斷言；debug build 只印出超時的項目，避免慢速 CI 誤報。
- **探針（ignored）**：`parity_probe::parity_probe` 讀 `MDFIX_PROBE_IN`（JSON 字串陣列），依 `MDFIX_PROBE_MODE`（`scan`、`fix` 或 `diagnose`，選項放在 `MDFIX_PROBE_OPTS`）把結果寫到 `MDFIX_PROBE_OUT`，用來與 Node 端呼叫 oracle 的輸出逐字比較。設定 `MDFIX_FORCE_MEMO=1` 會讓 jsregex 從第一步就使用記憶化回溯（`jsregex::FORCE_MEMO`，只在測試中存在），用來驗證記憶模式的結果與 oracle 相同。
- **diag 的測試**：`structure::tests`、`diagnose::tests`、`diagnose::tests::diagnose_probe`（ignored）、`structure::timing::structure_timing`（ignored）。
