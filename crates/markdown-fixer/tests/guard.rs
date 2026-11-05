//! 渲染保護（PRD〈6.9〉）的整合測試。
//!
//! 這裡用自己寫的渲染比對（pulldown-cmark ＋ 看不見差異的正規化 ＋ 簡單的 LCS 行 diff），
//! 不呼叫函式庫內部的 guard 模組，讓檢查與實作互相獨立。

#![cfg(feature = "render-guard")]

use markdown_fixer::{FixResult, Options, fix_document};
use proptest::prelude::*;
use pulldown_cmark::{Event, Options as CmarkOptions, Parser, Tag, TagEnd, html};

const LAYOUT_RULES: &[&str] = &[
    "MDX003", "MDX004", "MDX005", "MDX006", "MDX011", "MDX012", "MDX013", "MDX020", "MDX052",
    "MDX060", "MDX082",
];

fn guarded() -> Options {
    Options::default()
}

fn oracle() -> Options {
    Options {
        render_guard: false,
        ..Options::default()
    }
}

fn cmark_options() -> CmarkOptions {
    CmarkOptions::ENABLE_TABLES
        | CmarkOptions::ENABLE_STRIKETHROUGH
        | CmarkOptions::ENABLE_TASKLISTS
        | CmarkOptions::ENABLE_FOOTNOTES
        | CmarkOptions::ENABLE_GFM
        | CmarkOptions::ENABLE_MATH
}

/// 去掉 BOM、統一行尾，並把檔首 frontmatter 換成同樣行數的程式碼區塊（GitHub 只認檔首的
/// frontmatter；pulldown-cmark 的 metadata block 選項在文件中間也會觸發，所以不用它）。
fn normalize(text: &str) -> String {
    let text = text
        .trim_start_matches('\u{FEFF}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    // frontmatter 的分隔線後面只容許空格與 tab（U+3000 等其他空白不算）。
    let bare = |l: &str| l.trim_end_matches([' ', '\t']).to_string();
    if lines.first().map(|l| bare(l)).as_deref() == Some("---") {
        if let Some(close) =
            (1..lines.len()).find(|&i| matches!(bare(&lines[i]).as_str(), "---" | "..."))
        {
            let fence = "`".repeat(
                3 + lines[..close]
                    .iter()
                    .map(|l| l.matches('`').count())
                    .sum::<usize>(),
            );
            lines[0] = fence.clone();
            lines[close] = fence;
        }
    }
    lines.join("\n")
}

/// 一個區塊的「看得見的」內容：文字摺疊空白，程式碼與 raw HTML 去掉行尾空白與結尾空行。
fn visible(events: Vec<Event<'_>>) -> String {
    let mut out = String::new();
    let mut text = String::new();
    let mut in_code = false;
    let flush = |out: &mut String, text: &mut String, in_code: bool| {
        if text.is_empty() {
            return;
        }
        if in_code {
            let lines: Vec<&str> = text
                .split('\n')
                .map(|l| l.trim_end_matches([' ', '\t']))
                .collect();
            out.push_str(lines.join("\n").trim_end_matches('\n'));
        } else {
            let mut space = false;
            for c in text.chars() {
                if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000C}') {
                    if !space {
                        out.push(' ');
                    }
                    space = true;
                } else {
                    out.push(c);
                    space = false;
                }
            }
        }
        text.clear();
    };
    for event in events {
        match event {
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => text.push_str(&t),
            Event::SoftBreak if !in_code => text.push(' '),
            other => {
                flush(&mut out, &mut text, in_code);
                if matches!(other, Event::Start(Tag::CodeBlock(_))) {
                    in_code = true;
                }
                if matches!(
                    other,
                    Event::End(TagEnd::CodeBlock) | Event::End(TagEnd::HtmlBlock)
                ) {
                    in_code = false;
                }
                if matches!(other, Event::Start(Tag::HtmlBlock)) {
                    in_code = true;
                }
                html::push_html(&mut out, std::iter::once(other));
            }
        }
    }
    flush(&mut out, &mut text, in_code);
    out
}

/// 頂層區塊：(第一行, 最後一行, 看得見的內容)，行號從 0 起算。
fn blocks(text: &str) -> Vec<(usize, usize, String)> {
    let bytes = text.as_bytes();
    let line_of = |offset: usize| {
        bytes[..offset.min(bytes.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
    };
    let events: Vec<_> = Parser::new_ext(text, cmark_options())
        .into_offset_iter()
        .collect();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut begin = 0usize;
    for (i, (event, _)) in events.iter().enumerate() {
        let span = match event {
            Event::Start(_) => {
                if depth == 0 {
                    begin = i;
                }
                depth += 1;
                None
            }
            Event::End(_) => {
                depth -= 1;
                (depth == 0).then_some(begin..i + 1)
            }
            _ if depth == 0 => Some(i..i + 1),
            _ => None,
        };
        if let Some(span) = span {
            let range = events[span.start].1.clone();
            let first = line_of(range.start);
            let last = line_of(range.end.max(range.start + 1) - 1).max(first);
            let content = visible(events[span].iter().map(|(e, _)| e.clone()).collect());
            out.push((first, last, content));
        }
    }
    out
}

fn render(text: &str) -> String {
    blocks(&normalize(text))
        .into_iter()
        .map(|(_, _, html)| html)
        .collect()
}

/// LCS 行對應：回傳原文每一行在新文字中的行號（被改掉的行是 `None`）。
fn line_map(a: &[&str], b: &[&str]) -> Vec<Option<usize>> {
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut map = vec![None; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            map[i] = Some(j);
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    map
}

/// 區域性檢查：原文中離任何被改動的行至少隔一個區塊的頂層區塊，在結果中必須有相同行範圍
/// （經對應後）與相同內容的區塊。回傳違規區塊的第一行（從 1 起算）。
fn distant_changes(original: &str, result: &str) -> Vec<usize> {
    let (original, result) = (normalize(original), normalize(result));
    let a: Vec<&str> = original.split('\n').collect();
    let b: Vec<&str> = result.split('\n').collect();
    let map = line_map(&a, &b);
    let old_blocks = blocks(&original);
    let new_blocks = blocks(&result);
    // 第 l 行被改掉，或第 l 行之前插入了新行。
    let changed_line = |l: usize| map.get(l).is_none_or(|m| m.is_none());
    let inserted_before = |l: usize| -> bool {
        let Some(Some(now)) = map.get(l) else {
            return false;
        };
        match l.checked_sub(1) {
            None => *now > 0,
            Some(p) => map[p].is_some_and(|prev| *now > prev + 1),
        }
    };
    let n = old_blocks.len();
    let mut touched = vec![false; n];
    for (i, (s, e, _)) in old_blocks.iter().enumerate() {
        if (*s..=*e).any(changed_line) || (*s + 1..=*e).any(inserted_before) {
            touched[i] = true;
        }
    }
    // 區塊之間（含檔首、檔尾）的改動與插入，算前後兩個區塊都被改到。
    for gap in 0..=n {
        let lo = if gap == 0 {
            0
        } else {
            old_blocks[gap - 1].1 + 1
        };
        let hi = if gap == n { a.len() } else { old_blocks[gap].0 };
        let end_insert = gap == n
            && map
                .iter()
                .rev()
                .flatten()
                .next()
                .map_or(!b.is_empty(), |&last| last + 1 < b.len());
        if (lo..hi).any(changed_line)
            || (lo..=hi.min(a.len().saturating_sub(1))).any(inserted_before)
            || end_insert
        {
            if gap > 0 {
                touched[gap - 1] = true;
            }
            if gap < n {
                touched[gap] = true;
            }
        }
    }
    let allowed: Vec<bool> = (0..old_blocks.len())
        .map(|i| {
            touched[i] || (i > 0 && touched[i - 1]) || touched.get(i + 1).copied().unwrap_or(false)
        })
        .collect();
    let mut violations = Vec::new();
    for (i, (s, e, content)) in old_blocks.iter().enumerate() {
        if allowed[i] {
            continue;
        }
        let (Some(Some(ns)), Some(Some(ne))) = (map.get(*s), map.get(*e)) else {
            violations.push(s + 1);
            continue;
        };
        let same = new_blocks
            .iter()
            .any(|(bs, be, bc)| bs == ns && be == ne && bc == content);
        if !same {
            violations.push(s + 1);
        }
    }
    violations
}

fn skipped_rules(result: &FixResult) -> Vec<&'static str> {
    result.skipped.iter().map(|s| s.rule).collect()
}

// ---- 修復類：必須照常套用，輸出與 oracle 相同 ----

#[test]
fn repairs_still_apply() {
    for (input, expected) in [
        ("#Title\n", "# Title\n"),
        ("-item\n", "- item\n"),
        ("** x **\n", "**x**\n"),
        ("**標籤：**內容\n", "**標籤**：內容\n"),
        (
            "# A\n\n#B\n\n段落 **粗體 ** 結尾\n",
            "# A\n\n# B\n\n段落 **粗體** 結尾\n",
        ),
    ] {
        let result = fix_document(input, &guarded());
        assert_eq!(result.text, expected, "{input:?}");
        assert!(result.skipped.is_empty(), "{input:?}: {:?}", result.skipped);
        assert_eq!(result, fix_document(input, &oracle()), "{input:?}");
    }
}

#[test]
fn mdx062_lazy_table_row_repair_is_local() {
    // GFM：表格後緊接的文字行會被當成表格的一列。MDX062 補空行，讓它變回段落。
    let input = "# T\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\ntext\n\n段落\n";
    let plain = fix_document(input, &oracle());
    assert_ne!(
        render(input),
        render(&plain.text),
        "這個修復本來就會改變渲染"
    );
    let result = fix_document(input, &guarded());
    assert_eq!(result, plain);
    assert!(distant_changes(input, &result.text).is_empty());
}

// ---- 版面類：改變渲染時不套用 ----

#[test]
fn trailing_tab_hard_break_is_kept() {
    // pulldown-cmark 把行尾的「空格、tab、空格」當成 hard break；MDX004 移除它會改變渲染。
    let input = "a \t \nb\n";
    assert_ne!(render(input), render(&fix_document(input, &oracle()).text));
    let result = fix_document(input, &guarded());
    assert_eq!(result.text, input);
    assert!(!result.changed);
    assert_eq!(skipped_rules(&result), ["MDX004"]);
    assert_eq!(result.skipped[0].line, 1);
}

/// 確認案例是「活的」：關閉渲染保護時（oracle 行為），修正確實會改變渲染。
fn assert_live(input: &str) {
    let plain = fix_document(input, &oracle());
    assert_ne!(
        render(&plain.text),
        render(input),
        "{input:?}: oracle 的修正已經不會改變渲染，這個案例不再測到 guard"
    );
}

#[test]
fn layout_fixes_that_change_rendering_are_skipped() {
    // 這些都是 oracle 的 fence 模型與 CommonMark 容器規則不一致（docs/parity.md 的 U4），
    // 或 pulldown-cmark 的渲染差異（R2）。oracle v1.1.0 仍會改到渲染，guard 必須擋下。
    for (input, rules) in [
        // MDX052／MDX020：引言裡開的 fence 被 `>>>` 行「關閉」；CommonMark 裡那一行仍是程式碼。
        (
            "> ```js\n >>> ```\n> >> >  #Markdown\n",
            &["MDX020", "MDX052"][..],
        ),
        // MDX005：清單項目裡的 fence 隨清單結束，第 4 行開了新的 fence，後面的空行是程式碼內容。
        ("- a\n  ```\nx\n  ```\ncode\n\n\n\nmore\n", &["MDX005"]),
        // MDX012：ATX 結尾的 `#` 之後有 tab，pulldown-cmark 不把它當成結尾序列。
        ("## T # \t\n", &["MDX012"]),
    ] {
        assert_live(input);
        let result = fix_document(input, &guarded());
        assert_eq!(
            render(&result.text),
            render(input),
            "{input:?} -> {:?}",
            result.text
        );
        let skipped = skipped_rules(&result);
        for rule in rules {
            assert!(
                skipped.contains(rule),
                "{input:?}: {rule} not in {skipped:?}"
            );
        }
    }
}

#[test]
fn fixed_upstream_layout_cases_stay_render_neutral() {
    // 第 1 輪稽核找到、oracle v1.1.0 已修正根因的案例（U1、U5、U6、U7、R3）。
    // 不論 oracle 會不會改，開啟 guard 時渲染都必須不變。
    for input in [
        // U1 MDX052：引言裡的 fence（PRD 6.9 第 1 點的例子）。
        "> 說明：\n> ```sh\n> echo hi\n> ```\n> 結尾\n",
        "> x\n> ```\n",
        // MDX005：縮排程式碼區塊裡的多個空行是程式碼內容（oracle 本來就不改）。
        "段落\n\n    code\n\n\n    more\n",
        // U5 MDX020：清單項目裡的巢狀引言、引言裡縮排的 fence。
        "> - item\n>   >  quote\n",
        ">  ```\n>  code\n>  ```\n",
        // U7 MDX006：移除檔首空行後，文中的 U+FEFF 會被當成 BOM。
        "\n\u{FEFF}# T\n",
        // U6 MDX060：GFM 會用 inline code 裡的 `|` 分割儲存格，這不是表格。
        "| a | `x|y` |\n|:--|--:|\n",
        // R3 MDX082：空的 frontmatter。
        "---\n...\ntext\n",
    ] {
        let result = fix_document(input, &guarded());
        assert!(!result.refused, "{input:?}");
        assert_eq!(
            render(&result.text),
            render(input),
            "{input:?} -> {:?} (skipped {:?})",
            result.text,
            result.skipped
        );
    }
}

// ---- 修復類不得改到程式碼，也不得破壞區塊結構 ----

#[test]
fn repairs_inside_commonmark_code_are_skipped() {
    // CommonMark：`x` 讓清單與第一個 fence 結束，第 4 行開了新的 fence，最後一行是程式碼。
    for (input, rule) in [
        ("- a\n  ```\nx\n  ```\n** y **\n", "MDX040"),
        ("- a\n  ```\nx\n  ```\n#Title\n", "MDX010"),
    ] {
        assert_live(input);
        let result = fix_document(input, &guarded());
        assert_eq!(result.text, input);
        assert_eq!(skipped_rules(&result), [rule], "{input:?}");
        assert_eq!(result.skipped[0].line, 5);
    }
}

#[test]
fn inline_repairs_keep_list_items() {
    // U3（MDX040 吃掉 `* `）與 N1（MDX008 讓只有 NBSP 的清單項目變空）已在 oracle 修正；
    // 開啟 guard 時清單項目數必須不變。
    for input in [
        "* 說明**[](https://example.com)**\n",
        "*  **x**:  \n    - a\n",
        "- ***x：***  \n  +  \u{a0}\n",
    ] {
        let result = fix_document(input, &guarded());
        assert_eq!(
            render(input).matches("<li>").count(),
            render(&result.text).matches("<li>").count(),
            "{input:?} -> {:?}",
            result.text
        );
    }
}

// ---- 一般性質 ----

#[test]
fn refused_results_are_untouched() {
    for input in [
        "---\ntitle: x\n#Title\n",
        "<!-- open\n#Title\n",
        "> > > x\n",
    ] {
        let options = Options {
            max_quote_depth: 2,
            ..guarded()
        };
        let result = fix_document(input, &options);
        assert!(result.refused, "{input:?}");
        assert_eq!(result.text, input);
        assert!(result.skipped.is_empty());
        assert_eq!(
            result,
            fix_document(
                input,
                &Options {
                    render_guard: false,
                    ..options
                }
            )
        );
    }
}

#[test]
fn guard_off_matches_oracle_mode_and_has_no_skips() {
    let input = "a \t \nb\n";
    let result = fix_document(input, &oracle());
    assert_eq!(result.text, "a\nb\n");
    assert!(result.skipped.is_empty());
}

/// 組成隨機文件的整行片段。
const LINES: &[&str] = &[
    "",
    "",
    "",
    "# 標題",
    "#標題",
    "##  多空白 ##",
    "## T # \t",
    "段落文字 **粗體** 與 `code`。",
    "段落 ** x ** 與 _ y _",
    "**標籤：**內容",
    "a \t ",
    "結尾兩個空白  ",
    "行尾空白   ",
    "- item",
    "-item",
    "* star",
    "+ plus",
    "1. one",
    "  - nested",
    "    - deep",
    "> quote",
    ">quote",
    ">  > nested",
    "> - quoted item",
    ">   >  inner",
    "> ```",
    "> ```js",
    "```",
    "```js",
    "~~~",
    "    indented code",
    "| a | b |",
    "| --- | --- |",
    "|:--|--:|",
    "| 1 | 2 |",
    "| `x|y` | z |",
    "<div>",
    "</div>",
    "<!-- c -->",
    "$$",
    "x = 1",
    "---",
    "...",
    "\u{FEFF}text",
    "中文\u{a0}NBSP",
    "* 說明**粗體**",
    "[ref]: https://example.com",
    "[link][ref]",
];

fn document() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(prop::sample::select(LINES), 0..24),
        any::<bool>(),
    )
        .prop_map(|(lines, newline)| {
            let mut text = lines.join("\n");
            if newline {
                text.push('\n');
            }
            text
        })
}

/// 每個選項組合都要成立的性質。`base` 是使用者的選項（例如停用 MDX002、MDX003）。
fn check_guard(text: &str, base: &Options) -> Result<(), TestCaseError> {
    let with = |render_guard: bool| Options {
        render_guard,
        ..base.clone()
    };
    let plain = fix_document(text, &with(false));
    let result = fix_document(text, &with(true));
    // 拒絕修正時與 oracle 完全相同。
    if plain.refused {
        prop_assert_eq!(&result, &plain);
        return Ok(());
    }
    prop_assert!(!result.refused);
    // oracle 的結果已經不改變渲染時，guard 不能改動它。
    if render(&plain.text) == render(text) {
        prop_assert_eq!(&result, &plain);
    }
    // 沒有修復類修正可做時，結果的渲染必須與原文相同。
    let mut ignore = base.ignore_rules.clone().unwrap_or_default();
    ignore.extend(LAYOUT_RULES.iter().map(|s| s.to_string()));
    let repairs = fix_document(
        text,
        &Options {
            ignore_rules: Some(ignore),
            ..with(false)
        },
    );
    if normalize(&repairs.text) == normalize(text) {
        prop_assert_eq!(
            render(&result.text),
            render(text),
            "result {:?}",
            result.text
        );
    }
    // 修復只能改變被改到的區塊與相鄰區塊。
    prop_assert_eq!(
        distant_changes(text, &result.text),
        Vec::<usize>::new(),
        "result {:?}",
        result.text
    );
    // 沒被擋下任何修正時，輸出就是 oracle 的輸出。
    if result.skipped.is_empty() {
        prop_assert_eq!(&result.text, &plain.text);
    }
    // 編碼：停用 MDX001 時保留 BOM、停用 MDX002 時保留原本的行尾（PRD 6.2 第 6 點）；
    // guard 還原部分修正時也一樣。
    let ignored = |id: &str| base.ignore_rules.iter().flatten().any(|r| r == id);
    let bom = |t: &str| t.len() - t.trim_start_matches('\u{FEFF}').len();
    if ignored("MDX001") {
        prop_assert_eq!(bom(&result.text), bom(text));
    } else {
        prop_assert_eq!(bom(&result.text), 0);
    }
    if ignored("MDX002") && result.changed {
        if text.contains("\r\n") {
            prop_assert!(!result.text.replace("\r\n", "").contains(['\n', '\r']));
        } else if text.contains('\r') {
            prop_assert!(!result.text.contains('\n'));
        }
    } else if result.changed {
        prop_assert!(!result.text.contains('\r'));
    }
    Ok(())
}

/// 加上行尾（LF、CRLF、CR）與檔首 BOM 的變化。
fn encoded_document() -> impl Strategy<Value = String> {
    (document(), 0..3usize, any::<bool>()).prop_map(|(text, eol, bom)| {
        let eol = ["\n", "\r\n", "\r"][eol];
        let mut out = String::new();
        if bom {
            out.push('\u{FEFF}');
        }
        out.push_str(&text.replace('\n', eol));
        out
    })
}

fn ignoring(ids: &[&str]) -> Options {
    Options {
        ignore_rules: Some(ids.iter().map(|s| s.to_string()).collect()),
        ..Options::default()
    }
}

/// 字元層級的混亂片段：比整行片段更容易撞出邊界情況。
const ATOMS: &[&str] = &[
    "#", "## ", ">", "> ", ">>", " >", "-", "- ", "*", "* ", "+", "1.", "10. ", "- [ ] ", "```",
    "````", "~~~", "$$", "$", "**", "__", "_", "~~", "`", "|", "| --- |", ":--", "[", "]", "(",
    ")", "<div>", "</div>", "<!--", "-->", "---", "...", "===", "    ", "  ", "\t", "\u{a0}",
    "\u{FEFF}", "\u{3000}", "：", "中文", "a", "x y", "😀", "\\", "&amp;",
];

fn chaotic_document() -> impl Strategy<Value = String> {
    prop::collection::vec(
        (
            prop::sample::select(ATOMS),
            prop::sample::select(&["", "", " ", "\n", "\n", "\n\n"][..]),
        ),
        0..40,
    )
    .prop_map(|parts| parts.into_iter().flat_map(|(a, b)| [a, b]).collect())
}

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400)
}

/// property test 找到、已修正的案例，固定在這裡重跑（種子也記在 `guard.proptest-regressions`）。
#[test]
fn property_regressions() {
    for text in [
        // CI（macos-latest，run 42032175558）：清單項目裡未關閉的 HTML comment，tab 展開剩下的
        // 欄位讓 guard 把 HTML 區塊拆段，內部空行被當成看不見的差異，MDX005 的合併因此沒被擋下。
        "- <!-- >\n    \n\n\t#",
        // 同一類，其他跨空行的 HTML 區塊（U8b；oracle v1.1.0 已改為保護）。
        "- <pre>\n    \n\n\tx\n  </pre>\n",
        "- <script>\n    \n\n\tx\n</script>\n",
        "- <!-- >\n\t ",
        // 測試自己的比對器曾把 `---` 後的 U+3000 當成空白，誤判成 frontmatter。
        "---\u{3000}\n- #\n```\n---",
        "$$\n```$$\n    \n\n#",
        "* star\n> ```\n",
        "a \t \n\n---\n* star\n\n>quote\n---",
        "## T # \t\n* star",
        "> ```\n```\n> ```\n# 標題",
    ] {
        check_guard(text, &Options::default()).unwrap();
        check_guard(text, &ignoring(&["MDX002", "MDX003"])).unwrap();
    }
}

proptest! {
    // 預設 400 個案例；`PROPTEST_CASES=20000 cargo test --test guard` 可以加大。
    #![proptest_config(ProptestConfig::with_cases(cases()))]

    #[test]
    fn guard_properties_on_chaotic_input(text in chaotic_document()) {
        check_guard(&text, &Options::default())?;
        check_guard(&text, &ignoring(&["MDX002", "MDX003"]))?;
    }

    #[test]
    fn guard_properties(text in document()) {
        check_guard(&text, &Options::default())?;
    }

    #[test]
    fn guard_preserves_encoding(text in encoded_document()) {
        // dedent-paste 停用 MDX002、MDX003；剪貼簿腳本停用 MDX003。
        check_guard(&text, &ignoring(&["MDX002", "MDX003"]))?;
        check_guard(&text, &ignoring(&["MDX001"]))?;
        check_guard(&text, &ignoring(&["MDX003"]))?;
    }
}
