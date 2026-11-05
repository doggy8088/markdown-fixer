//! 移植自 `reference/js/markdown-fixer.test.mjs`（v1.1.0）的測試。
//!
//! 每個測試上方的註解是原始 JS 案例名稱與編號（依檔案中的順序）。只能透過內部函式
//! （`buildMask`、`splitTableCells`、`scanDocument`）觀察的斷言，在這裡改以公開 API
//! 可觀察的行為驗證，或標註由 core 的單元測試負責；CLI 案例（`parseArgs` 與
//! `spawnSync` 執行 CLI 的測試）屬於 `crates/markdown-fixer-cli/tests`。

use markdown_fixer::{LimitPolicy, Options, Severity, diagnose, fix_document, rules};

/// parity 測試一律關閉 Rust 專有的渲染保護（PRD 6.9），行為才與 oracle 逐位元組相同。
fn opts() -> Options {
    Options {
        render_guard: false,
        ..Options::default()
    }
}

fn ignore(ids: &[&str]) -> Options {
    Options {
        ignore_rules: Some(ids.iter().map(|s| (*s).to_owned()).collect()),
        ..opts()
    }
}

fn only(ids: &[&str]) -> Options {
    Options {
        rules: Some(ids.iter().map(|s| (*s).to_owned()).collect()),
        ..opts()
    }
}

fn line_length(n: usize) -> Options {
    Options {
        max_line_length: n,
        ..opts()
    }
}

/// JS `rulesOf(text, options)`。
fn rules_of_with(text: &str, options: &Options) -> Vec<&'static str> {
    diagnose(text, options)
        .into_iter()
        .map(|p| p.rule)
        .collect()
}

fn rules_of(text: &str) -> Vec<&'static str> {
    rules_of_with(text, &opts())
}

/// JS `fixed(text, options)`。
fn fixed_with(text: &str, options: &Options) -> String {
    fix_document(text, options).text
}

fn fixed(text: &str) -> String {
    fixed_with(text, &opts())
}

fn has(text: &str, rule: &str) -> bool {
    rules_of(text).contains(&rule)
}

// 1. 'rule catalog is well formed and unique'
#[test]
fn rule_catalog_is_well_formed_and_unique() {
    assert_eq!(markdown_fixer::REFERENCE_VERSION, "1.1.0");
    let ids: Vec<&str> = rules().iter().map(|r| r.id).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    for rule in rules() {
        assert!(
            rule.id.len() == 6
                && rule.id.starts_with("MDX")
                && rule.id[3..].bytes().all(|b| b.is_ascii_digit()),
            "{}",
            rule.id
        );
        assert!(
            matches!(
                rule.severity,
                Severity::Error | Severity::Warning | Severity::Info
            ),
            "{}",
            rule.id
        );
        assert!(!rule.summary.is_empty(), "{}", rule.id);
    }
}

// 2. 'fixes heading spacing, gaps, and closing hashes'
#[test]
fn fixes_heading_spacing_gaps_and_closing_hashes() {
    assert_eq!(fixed("#Title\n"), "# Title\n");
    assert_eq!(fixed("##  Sub  ##\n"), "## Sub\n");
    assert_eq!(fixed("#  Title  ##\n"), "# Title\n");
    assert_eq!(fixed("#Title #\n"), "# Title\n");
}

// 3. 'fixes list marker spacing and reports bullet style without touching flags or versions'
#[test]
fn fixes_list_marker_spacing_and_reports_bullet_style_without_touching_flags_or_versions() {
    // CommonMark 裡換一種項目符號就是新的清單，所以 MDX031 只回報（v1.1.0）。
    assert_eq!(fixed("- a\n* b\n+ c\n"), "- a\n* b\n+ c\n");
    let mdx031: Vec<_> = diagnose("- a\n* b\n+ c\n", &opts())
        .into_iter()
        .map(|p| (p.rule, p.line, p.fixable))
        .collect();
    assert_eq!(mdx031, [("MDX031", 2, false), ("MDX031", 3, false)]);
    assert_eq!(fixed("-item text\n"), "- item text\n");
    assert_eq!(fixed("+other text\n"), "+ other text\n");
    assert_eq!(fixed("1.item text\n"), "1. item text\n");
    assert_eq!(
        fixed("--help\n-1\n+1\n1.a\n1.5\n"),
        "--help\n-1\n+1\n1.a\n1.5\n"
    );
}

// 4. 'does not turn a thematic break into a list item'
#[test]
fn does_not_turn_a_thematic_break_into_a_list_item() {
    let input = "* item\n\n- - -\n";
    assert_eq!(fixed(input), input);
    assert!(rules_of(input).is_empty());
}

// 5. 'fixes emphasis inner spacing but keeps symmetric single-delimiter spacing'
#[test]
fn fixes_emphasis_inner_spacing_but_keeps_symmetric_single_delimiter_spacing() {
    assert_eq!(
        fixed("**  bold  ** and **bold ** and __ x __\n"),
        "**bold** and **bold** and __x__\n"
    );
    assert_eq!(
        fixed("Text * spaced* and *spaced * and * spaced *\n"),
        "Text *spaced* and *spaced* and * spaced *\n"
    );
    // 行首的 `*` 是清單標記，不是強調符號。
    assert_eq!(
        fixed("* spaced* and *spaced *\n"),
        "* spaced* and *spaced*\n"
    );
    assert_eq!(fixed("> - * x*\n"), "> - * x*\n");
    let link = "* 說明**[](https://example.com)**\n";
    assert_eq!(fixed(link), link);
    assert_eq!(
        fixed("_ spaced_ and _spaced _ and _ spaced _\n"),
        "_spaced_ and _spaced_ and _ spaced _\n"
    );
}

// 6. 'fixes table delimiter width but leaves prose pipes alone'
#[test]
fn fixes_table_delimiter_width_but_leaves_prose_pipes_alone() {
    assert_eq!(
        fixed("| a | b |\n| - | -- |\n| 1 | 2 |\n"),
        "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
    );
    assert_eq!(
        fixed("| a | b |\n|:--|--:|\n| 1 | 2 |\n"),
        "| a | b |\n|:---|---:|\n| 1 | 2 |\n"
    );
    assert_eq!(fixed("foo | -- | bar\n"), "foo | -- | bar\n");
    assert_eq!(fixed("Text\n| - | - |\n"), "Text\n| - | - |\n");
    assert!(rules_of("Text\n| - | - |\n").is_empty());
    assert_eq!(
        fixed("  | a | b |\n  | - | -- |\n"),
        "  | a | b |\n  | --- | --- |\n"
    );
    assert!(has(
        "  | a | b |\n  | --- | --- |\n  | 1 | 2 | 3 |\n",
        "MDX061"
    ));
    assert!(!has("    | a | b |\n    | --- | --- |\n", "MDX061"));
    assert!(rules_of("- item\n  | a | b |\n  | --- | --- |\n").is_empty());
    assert_eq!(
        fixed("- item\n  | a | b |\n  | --- | --- |\n"),
        "- item\n  | a | b |\n  | --- | --- |\n"
    );
    assert!(has(
        "| a | b |\n| --- | --- |\n| https://example.com/a|b | c |\n",
        "MDX063"
    ));
}

// 7. 'normalizes blockquote markers and preserves indented code'
#[test]
fn normalizes_blockquote_markers_and_preserves_indented_code() {
    assert_eq!(
        fixed(">quote\n>>nested\n>  text\n>     code\n"),
        "> quote\n> > nested\n> text\n>     code\n"
    );
    assert_eq!(fixed(">\tcode\n"), ">\tcode\n");
}

// 8. 'protects indented code inside a blockquote'
#[test]
fn protects_indented_code_inside_a_blockquote() {
    let input = ">     https://example.com\n>     ** bold **\n> \tcode\n";
    assert_eq!(fixed(input), input);
    assert!(rules_of(input).is_empty());
    assert!(has(
        ">     [label]: https://example.com\n> [text][label]\n",
        "MDX073"
    ));
    let tab_code = ">  \tcode\n";
    assert!(rules_of(tab_code).is_empty());
    assert_eq!(fixed(tab_code), tab_code);
}

// 9. 'preserves a hard break when other trailing whitespace is removed'
#[test]
fn preserves_a_hard_break_when_other_trailing_whitespace_is_removed() {
    assert_eq!(fixed("text\t  \nnext\n"), "text  \nnext\n");
}

// 10. 'normalizes whitespace, BOM, CRLF, and final newline'
#[test]
fn normalizes_whitespace_bom_crlf_and_final_newline() {
    assert_eq!(
        fixed("\u{FEFF}# Title\r\n\r\n\r\ntext  \r\n"),
        "# Title\n\ntext  \n"
    );
    assert_eq!(fixed("text"), "text\n");
    assert_eq!(fixed("text\n\n"), "text\n");
    assert_eq!(fixed("\n\ntext\n"), "text\n");
    assert_eq!(fixed("line with 3   \nnext\n"), "line with 3  \nnext\n");
    assert_eq!(fixed("line with one \nnext\n"), "line with one\nnext\n");
    assert_eq!(
        fixed("line with break  \nnext\n"),
        "line with break  \nnext\n"
    );
    assert_eq!(fixed(""), "");
}

// 11. 'protects frontmatter, code, math, HTML, and link destinations'
#[test]
fn protects_frontmatter_code_math_html_and_link_destinations() {
    let input = [
        "---",
        "title: \"#Title\"",
        "---",
        "",
        "#Title",
        "",
        "`#NoSpace` and $x** y**$ and [link](https://example.com/a**b**).",
        "",
        "```js",
        "#NoSpace",
        "** bold **",
        "```",
        "",
        "<div>",
        "#NoSpace",
        "</div>",
        "",
        "    #NoSpace",
        "",
    ]
    .join("\n");

    let output = fixed(&input);
    assert!(output.contains("# Title"));
    assert!(!output.contains("# NoSpace"));
    assert!(output.contains("`#NoSpace`"));
    assert!(output.contains("$x** y**$"));
    assert!(output.contains("https://example.com/a**b**"));
    assert!(output.contains("    #NoSpace"));
}

// 12. 'refuses to write structurally ambiguous files'
#[test]
fn refuses_to_write_structurally_ambiguous_files() {
    let unclosed_frontmatter = "---\ntitle: x\n#Title\n";
    let result = fix_document(unclosed_frontmatter, &opts());
    assert!(result.refused);
    assert_eq!(result.text, unclosed_frontmatter);
    assert!(result.reason.to_lowercase().contains("frontmatter"));

    let leading_blanks = "\n\n---\ntitle: x\n#Title\n";
    let result = fix_document(leading_blanks, &opts());
    assert!(result.refused);
    assert_eq!(result.text, leading_blanks);

    let unclosed_comment = "# Title\n\n<!--\n#Title\n";
    let result = fix_document(unclosed_comment, &opts());
    assert!(result.refused);
    assert_eq!(result.text, unclosed_comment);
    assert!(result.reason.to_lowercase().contains("comment"));

    let invalid_fence = "# Title\n\n```js`bad\ncode\n```\n";
    let result = fix_document(invalid_fence, &opts());
    assert!(result.refused);
    assert_eq!(result.text, invalid_fence);
    assert!(result.reason.to_lowercase().contains("backtick"));

    let frontmatter_trap = "\n---\ntext\n---\n";
    let result = fix_document(frontmatter_trap, &opts());
    assert!(result.refused);
    assert_eq!(result.text, frontmatter_trap);
    assert!(result.reason.to_lowercase().contains("frontmatter"));
}

// 13. 'refuses to expose frontmatter hidden behind a leading BOM line'
#[test]
fn refuses_to_expose_frontmatter_hidden_behind_a_leading_bom_line() {
    let closed = "\n\u{FEFF}---\ntitle: x\n---\n";
    let result = fix_document(closed, &opts());
    assert!(result.refused);
    assert_eq!(result.text, closed);
    assert!(result.reason.to_lowercase().contains("frontmatter"));

    let unclosed = "\n\u{FEFF}---\ntitle: x\n";
    let result = fix_document(unclosed, &opts());
    assert!(result.refused);
    assert_eq!(result.text, unclosed);
}

// 14. 'is idempotent on a mixed document'
#[test]
fn is_idempotent_on_a_mixed_document() {
    let input = [
        "#Title",
        "",
        "Paragraph",
        "##  Sub  ##",
        "-item",
        "* other",
        "+ third",
        "",
        "**  bold  ** and * italic* and _ italic _",
        "",
        ">quote",
        ">     code",
        "",
        "| a | b |",
        "| - | -- |",
        "| 1 | 2 |",
        "",
        "```js",
        "code",
        "```",
        "",
    ]
    .join("\n");

    let once = fix_document(&input, &opts());
    assert!(!once.refused);
    let twice = fix_document(&once.text, &opts());
    assert_eq!(twice.text, once.text);
    assert!(!twice.changed);
    assert!(once.text.contains("# Title"));
    assert!(once.text.contains("## Sub"));
    assert!(once.text.contains("* other"));
    assert!(once.text.contains("**bold**"));
    assert!(once.text.contains("| --- | --- |"));
}

// 15. 'honors rule selection and ignore lists'
#[test]
fn honors_rule_selection_and_ignore_lists() {
    assert_eq!(fixed_with("- a\n* b\n", &ignore(&["MDX031"])), "- a\n* b\n");
    assert_eq!(fixed_with("- a\n* b\n", &only(&["MDX031"])), "- a\n* b\n");
    assert_eq!(
        fixed_with("| a | b |\n| - | -- |\n", &only(&["MDX031"])),
        "| a | b |\n| - | -- |\n"
    );
    assert_eq!(
        fixed_with("| a | b |\n| - | -- |\n", &only(&["MDX060"])),
        "| a | b |\n| --- | --- |\n"
    );
}

// 16. 'diagnoses structural, link, table, and CJK problems'
#[test]
fn diagnoses_structural_link_table_and_cjk_problems() {
    let input = [
        "Text before",
        "---",
        "",
        "## Section",
        "### Sub",
        "##### Deep",
        "",
        "[link][nope] and [dup][label].",
        "",
        "[label]: https://example.com",
        "[label]: https://example.com",
        "",
        "## Another",
        "",
        "![](img.png)",
        "",
        "| a | b |",
        "| --- | --- |",
        "| 1 | 2 | 3 |",
        "| `x|y` | z |",
        "",
        "＃ Fullwidth",
        "",
        "> quote",
        ">no space",
        "",
        "Bare https://example.com.",
        "",
        "#### Jump",
        "",
    ]
    .join("\n");

    let rules = rules_of(&input);
    for rule in [
        "MDX080", "MDX018", "MDX015", "MDX073", "MDX074", "MDX075", "MDX061", "MDX063", "MDX084",
        "MDX020", "MDX070",
    ] {
        assert!(rules.contains(&rule), "expected {rule} in {rules:?}");
    }
}

// 17. 'detects line length with CJK characters and skips tables'
#[test]
fn detects_line_length_with_cjk_characters_and_skips_tables() {
    let problems = diagnose(
        "你好世界你好\n\nshort\n\n| long | table |\n| --- | --- |\n| a | b |\n",
        &line_length(5),
    );
    let ids: Vec<&str> = problems.iter().map(|p| p.rule).collect();
    assert_eq!(ids, ["MDX090"]);
    assert_eq!(problems[0].line, 1);
    assert_eq!(problems[0].severity, Severity::Info);
}

// 18. 'does not report a thematic break after a code fence as Setext'
#[test]
fn does_not_report_a_thematic_break_after_a_code_fence_as_setext() {
    assert!(!has("```\ncode\n```\n---\n", "MDX080"));
}

// 19. 'handles Setext ambiguity and quoted reference definitions'
#[test]
fn handles_setext_ambiguity_and_quoted_reference_definitions() {
    assert!(has("Text\n---\n", "MDX080"));
    assert!(!has("  Text\n  ---\n", "MDX080"));
    assert!(!has(
        "> [label]: https://example.com\n\n> [text][label]\n",
        "MDX073"
    ));
}

// 20. 'removes repeated leading BOMs and still fixes the heading'
#[test]
fn removes_repeated_leading_boms_and_still_fixes_the_heading() {
    assert_eq!(fixed("\u{FEFF}\u{FEFF}#Title\n"), "# Title\n");
    assert_eq!(
        fixed_with("\u{FEFF}\u{FEFF}#Title\n", &ignore(&["MDX001"])),
        "\u{FEFF}\u{FEFF}# Title\n"
    );
}

// 21. 'detects a BOM, CRLF, and a missing heading space together'
#[test]
fn detects_a_bom_crlf_and_a_missing_heading_space_together() {
    let rules = rules_of("\u{FEFF}#Title\r\n");
    assert!(rules.contains(&"MDX001"));
    assert!(rules.contains(&"MDX002"));
    assert!(rules.contains(&"MDX010"));
}

// 22. 'preserves BOM, CRLF, and missing final newline when their rules are disabled'
#[test]
fn preserves_bom_crlf_and_missing_final_newline_when_their_rules_are_disabled() {
    let result = fix_document(
        "\u{FEFF}#Title\r\ntext",
        &ignore(&["MDX001", "MDX002", "MDX003", "MDX013"]),
    );
    assert_eq!(result.text, "\u{FEFF}# Title\r\ntext");
}

// 23. 'does not rewrite ignored BOM or mixed line endings when nothing else changes'
#[test]
fn does_not_rewrite_ignored_bom_or_mixed_line_endings_when_nothing_else_changes() {
    let input = "\u{FEFF}# Title\r\n\r\nText\n";
    let result = fix_document(input, &ignore(&["MDX001", "MDX002"]));
    assert!(!result.changed);
    assert_eq!(result.text, input);
}

// 24. 'reports NBSP inside a URL without breaking the URL'
#[test]
fn reports_nbsp_inside_a_url_without_breaking_the_url() {
    let text = "text\u{00A0}here\n\nhttps://example.com/a\u{00A0}b\n";
    let nbsp: Vec<_> = diagnose(text, &opts())
        .into_iter()
        .filter(|p| p.rule == "MDX008")
        .collect();
    assert_eq!(nbsp.len(), 2);
    assert_eq!(nbsp.iter().filter(|p| p.fixable).count(), 1);
    assert_eq!(fixed(text), "text here\n\nhttps://example.com/a\u{00A0}b\n");
}

// 25. 'buildMask covers code, math, links, and URLs selectively'
// buildMask 是內部函式，沒有公開 API；遮罩行為由 core 的單元測試直接驗證。
// 這裡改以 fix_document 觀察同一組遮罩效果：code、連結目的地、數學式內的
// 「** x **」不被修正，一般文字中的會被修正。
#[test]
fn build_mask_effects_are_observable_through_fix() {
    let line = "`** a **` [t](https://e.com/** b **) $** c **$ ** d **\n";
    assert_eq!(
        fixed(line),
        "`** a **` [t](https://e.com/** b **) $** c **$ **d**\n"
    );
}

// 26. 'splitTableCells handles escaped and code-span pipes'
// splitTableCells 是內部函式；以 MDX061 的欄數判斷觀察：跳脫的管線與 code span
// 內的管線都不分割儲存格。
#[test]
fn split_table_cells_effects_are_observable_through_mdx061() {
    assert!(!has(
        "| a | b |\n| --- | --- |\n| a \\| b | c |\n",
        "MDX061"
    ));
    assert!(!has("| a | b |\n| --- | --- |\n| `a|b` | c |\n", "MDX061"));
    assert!(has("| a | b |\n| --- | --- |\n| a | b | c |\n", "MDX061"));
}

// 27. 'treats tab-expanded four-column indentation as code'
#[test]
fn treats_tab_expanded_four_column_indentation_as_code() {
    let code = "   \t** bold **\n";
    assert_eq!(fixed(code), code);
    assert!(rules_of(code).is_empty());
    let table = "   \t| a | b |\n   \t| --- | --- |\n   \t| 1 | 2 | 3 |\n";
    assert!(!has(table, "MDX061"));
    assert_eq!(fixed(table), table);
}

// 28. 'keeps multi-line indented paragraph continuations out of code'
// 原測試另外斷言 scanDocument 的 info[1..2].indentedCode 為 false（內部狀態，
// 由 core 的單元測試負責）；修正結果本身已證明這兩行沒有被當成 code。
#[test]
fn keeps_multi_line_indented_paragraph_continuations_out_of_code() {
    let input = "paragraph\n    ** bold **\n    ** bold **\n";
    assert_eq!(fixed(input), "paragraph\n    **bold**\n    **bold**\n");
}

// 29. 'classifies indented code after a heading or fence close'
// 原測試的 scanDocument(...).info[3].indentedCode 斷言屬於內部狀態；這裡以
// 「code 內的 # 不被當成標題修正」觀察。
#[test]
fn classifies_indented_code_after_a_heading_or_fence_close() {
    assert_eq!(fixed("# Title\n    code  \n"), "# Title\n    code  \n");
    assert!(!has("```\ncode\n```\n    #code\n", "MDX010"));
    assert_eq!(
        fixed("```\ncode\n```\n    code  \n"),
        "```\ncode\n```\n\n    code  \n"
    );
}

// 30. 'protects multi-line $$ math blocks and reports unclosed ones'
// 最後三個 scanDocument 的 mathOpen／mathBlock／mathClose 斷言屬於內部狀態，由
// core 的單元測試負責；其餘斷言全部移植。
#[test]
fn protects_multi_line_math_blocks_and_reports_unclosed_ones() {
    let math = "$$\n#Title\n** bold **\n$$\n";
    assert!(rules_of(math).is_empty());
    let result = fix_document(math, &opts());
    assert!(!result.changed);
    assert_eq!(result.text, math);

    let quoted = "> $$\n> #Title\n> ** bold **\n> $$\n";
    assert!(rules_of(quoted).is_empty());
    assert_eq!(fixed(quoted), quoted);

    let math_content = "$$\nx = 1  \nx\u{00A0}y\n\n\n\u{200B}[label]: https://example.com\n$$\n";
    assert!(rules_of(math_content).is_empty());
    assert_eq!(fixed(math_content), math_content);

    for nested in [
        "- $$\n  x = 1\n  $$\n",
        "> - $$\n>   x = 1\n>   $$\n",
        "- [ ] $$\n  x = 1\n  $$\n",
        "- > $$\n  > x = 1\n  > $$\n",
    ] {
        assert!(rules_of(nested).is_empty(), "{nested:?}");
        assert_eq!(fixed(nested), nested);
    }

    assert_eq!(fixed("\n\n$$\n#Title\n$$\n"), "$$\n#Title\n$$\n");
    assert_eq!(fixed("$$\nx\n$$\n#Title\n"), "$$\nx\n$$\n# Title\n");

    assert!(diagnose("$$\na\tb\n$$\n", &opts()).is_empty());
    let long = format!("$$\n{}\n$$\n", "x".repeat(50));
    assert!(diagnose(&long, &line_length(10)).is_empty());

    let unclosed = "$$\n#Title\n";
    assert!(has(unclosed, "MDX085"));
    let result = fix_document(unclosed, &opts());
    assert!(result.refused);
    assert_eq!(result.text, unclosed);
    assert!(result.reason.to_lowercase().contains("math"));
    assert!(fix_document(unclosed, &ignore(&["MDX085"])).refused);

    let fenced = "```\n$$\n#Title\n```\n";
    assert!(!has(fenced, "MDX010"));
    assert!(!has(fenced, "MDX085"));

    let list_fence = "- example:\n  ```\n  $$\n  #Title\n  ```\n";
    assert!(!has(list_fence, "MDX010"));
    assert!(!has(list_fence, "MDX085"));

    assert_eq!(fixed("#Title $$x$$"), "# Title $$x$$\n");
}

// 31. 'scanDocument classifies frontmatter, fences, and indented code'
// scanDocument 是內部函式，整個案例由 core 的單元測試負責。這裡只驗證同一份輸入
// 沒有結構問題，且修正結果不動 frontmatter、fence 與縮排 code。
#[test]
fn scan_document_input_has_no_structural_issues() {
    let input = "---\ntitle: x\n---\n\n```js\ncode\n```\n\n    indented\n";
    assert!(rules_of(input).is_empty());
    let result = fix_document(input, &opts());
    assert!(!result.changed && !result.refused);
}

// 32. 'parseArgs handles modes, rule lists, limits, and errors'
// 33. 'CLI reports version, rules, and check results'
// 34. 'CLI writes fixes, supports JSON, and reports refusals'
// 35. 'CLI requires explicit paths for --write and honors --strict'
// 36. 'CLI reports invalid UTF-8 and NUL bytes as I/O errors'
// 37. 'does not claim success when some files cannot be read'
// 38. 'writes through a symlink without replacing it'
// 以上是 CLI 層級的測試，移植到 crates/markdown-fixer-cli/tests（agent「cli」負責）。
// 函式庫層級唯一相關的部分是「--rule MDX090 時預設行長 120」屬於 CLI 解析層，
// 函式庫的 Options 不做這件事（PRD 6.3），以下確認這個契約。
#[test]
fn cli_level_tests_live_in_the_cli_crate() {
    let text = format!("{}\n", "x".repeat(130));
    assert!(rules_of_with(&text, &only(&["MDX090"])).is_empty());
    let with_limit = Options {
        max_line_length: 120,
        ..only(&["MDX090"])
    };
    assert_eq!(rules_of_with(&text, &with_limit), ["MDX090"]);
}

// 39. 'MDX040 leaves bold that already pairs across CJK punctuation alone'
#[test]
fn mdx040_leaves_bold_that_already_pairs_across_cjk_punctuation_alone() {
    let line = "1. **缺漏｜流程動作不在 diff。** E＋F（試發→OK→部署）與 G（進版）屬流程，diff 無法驗證。**（本會話已實證完成：試發 Message ID `1567269943236362380`、部署 `52a62cc`＋容器 `/app` 雜湊回讀一致、進版 `8013246`。）**\n";
    assert!(rules_of(line).is_empty());
    assert_eq!(fixed(line), line);
}

// 40. 'MDX042 reports unpaired emphasis next to CJK punctuation and fixes the safe closing form'
#[test]
fn mdx042_reports_unpaired_emphasis_next_to_cjk_punctuation_and_fixes_the_safe_closing_form() {
    let one = "**審查執行：**2026-11-04\n";
    assert!(has(one, "MDX042"));
    assert_eq!(fixed(one), "**審查執行**：2026-11-04\n");
    assert_eq!(
        fixed("**Spec 來源：**本次工作\n"),
        "**Spec 來源**：本次工作\n"
    );
    let problems: Vec<_> = diagnose(one, &opts())
        .into_iter()
        .filter(|p| p.rule == "MDX042")
        .collect();
    assert!(!problems.is_empty());
    assert!(problems.iter().all(|p| p.fixable));

    let two = "**審查執行：**2026 與 **追蹤：**2027\n";
    assert!(!has(two, "MDX041"));
    assert!(has(two, "MDX042"));
    assert_eq!(fixed(two), "**審查執行**：2026 與 **追蹤**：2027\n");

    let once = fixed(one);
    assert_eq!(fixed(&once), once);
}

// 41. 'MDX042 leaves working emphasis, ASCII cases, code, and escapes alone'
#[test]
fn mdx042_leaves_working_emphasis_ascii_cases_code_and_escapes_alone() {
    for text in [
        "**狀態：待修正**（2026-…）\n",
        "標籤：**內容**\n",
        "**Note:**text\n",
        "`**審查執行：**2026`\n",
        "```text\n**審查執行：**2026\n```\n",
        "這是\\*「引號」\\*文字\n",
        "打錯字（gpt-5.x、claude-*、gemini-2.5-flash）。\n",
        "2 * 三 * 四\n",
        "| **$10.35** | – | $56.97 | **$490.20**（x） |\n",
        "- **重度使用者**：victor0891（0.419B，**100% glm-5.2，花掉 $593.06**，是最貴的）、wei（$183.93）。\n",
    ] {
        assert!(!has(text, "MDX042"), "{text:?}");
    }
    assert_eq!(
        fixed("**狀態：待修正**（2026-…）\n"),
        "**狀態：待修正**（2026-…）\n"
    );
    assert_eq!(fixed("標籤：**內容**\n"), "標籤：**內容**\n");
}

// 42. 'MDX042 reports opening-side problems without rewriting them'
#[test]
fn mdx042_reports_opening_side_problems_without_rewriting_them() {
    let line = "這是**「引號」**文字\n";
    let problems: Vec<_> = diagnose(line, &opts())
        .into_iter()
        .filter(|p| p.rule == "MDX042")
        .collect();
    assert!(!problems.is_empty());
    assert!(problems.iter().all(|p| !p.fixable));
    assert_eq!(fixed(line), line);

    let star = "*標籤：*內容\n";
    assert!(has(star, "MDX042"));
    assert_eq!(fixed(star), star);
}

// 43. 'matchDefinition matches the v1.0.0 reference-definition regex'
// 內部函式；由 `scan::definition_equivalence_tests` 負責（Rust 可以測到更深的引言）。

fn depth(max_quote_depth: usize) -> Options {
    Options {
        max_quote_depth,
        ..opts()
    }
}

// 44. 'MDX086 reports blockquotes nested deeper than maxQuoteDepth and blocks fixing'
#[test]
fn mdx086_reports_blockquotes_nested_deeper_than_max_quote_depth_and_blocks_fixing() {
    let deep = format!("{}x", "> ".repeat(33));
    let text = format!("# T\n\n{deep} \t\n");
    let problems = diagnose(&text, &opts());
    let mdx086: Vec<_> = problems
        .iter()
        .filter(|p| p.rule == "MDX086")
        .map(|p| (p.line, p.severity, p.fixable))
        .collect();
    assert_eq!(mdx086, [(3, Severity::Error, false)]);
    assert!(
        problems
            .iter()
            .find(|p| p.rule == "MDX086")
            .is_some_and(|p| p.message.contains("depth 33 exceeds the limit of 32"))
    );
    let on_line_3: Vec<_> = problems
        .iter()
        .filter(|p| p.line == 3)
        .map(|p| p.rule)
        .collect();
    assert_eq!(on_line_3, ["MDX086"]);

    let result = fix_document(&text, &opts());
    assert!(result.refused);
    assert!(!result.changed);
    assert!(result.reason.contains("Blockquote nesting depth 33"));
    assert!(fix_document(&text, &ignore(&["MDX086"])).refused);
    assert!(!rules_of_with(&text, &ignore(&["MDX086"])).contains(&"MDX086"));

    assert!(!has(&format!("{}x\n", "> ".repeat(32)), "MDX086"));
    assert!(!rules_of_with(&text, &depth(0)).contains(&"MDX086"));
    assert_eq!(fixed_with(&text, &depth(0)), format!("# T\n\n{deep}\n"));
    assert!(rules_of_with(">>>> x\n", &depth(3)).contains(&"MDX086"));
    assert!(!has(&format!("```\n{deep}\n```\n"), "MDX086"));
}

// 45. 'onLimit ignore leaves over-deep lines untouched and undiagnosed'
#[test]
fn on_limit_ignore_leaves_over_deep_lines_untouched_and_undiagnosed() {
    let deep = format!("{}#Head##  **  x**\u{a0}y\thttps://x  ", ">".repeat(5));
    let deep_definition = format!("{}[a]: https://x", ">".repeat(5));
    let text = format!("#Title\n\n{deep}\n{deep_definition}\n\n[t][a]\n");
    let options = Options {
        max_quote_depth: 4,
        on_limit: LimitPolicy::Ignore,
        ..opts()
    };
    let problems = diagnose(&text, &options);
    assert!(
        problems.iter().all(|p| p.line != 3 && p.line != 4),
        "{problems:?}"
    );
    let result = fix_document(&text, &options);
    assert!(!result.refused);
    assert_eq!(
        result.text,
        format!("# Title\n\n{deep}\n{deep_definition}\n\n[t][a]\n")
    );
    assert!(rules_of_with(&text, &options).contains(&"MDX073"));
    assert!(!rules_of_with(&text, &depth(0)).contains(&"MDX073"));
    assert_ne!(
        fixed_with(&text, &depth(0)).split('\n').nth(2),
        Some(deep.as_str())
    );
}

// 46. 'fixing blockquote spacing never changes the counted quote depth'
#[test]
fn fixing_blockquote_spacing_never_changes_the_counted_quote_depth() {
    let mut seed: u64 = 86;
    let mut random = |n: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) % n) as usize
    };
    for _ in 0..1000 {
        let mut line = " ".repeat(random(4));
        for _ in 0..1 + random(8) {
            line.push('>');
            line.push_str(&" ".repeat(random(6)));
        }
        if random(2) == 1 {
            line.push('x');
        }
        let text = format!("{line}\n");
        let once = fixed_with(&text, &depth(0));
        for limit in 1..=8 {
            let before = rules_of_with(&text, &depth(limit)).contains(&"MDX086");
            let after = rules_of_with(&once, &depth(limit)).contains(&"MDX086");
            assert!(!after || before, "{line:?} limit {limit}");
        }
    }
}

// 47. 'deep blockquotes and very long lines finish without exceptions'
#[test]
fn deep_blockquotes_and_very_long_lines_finish_without_exceptions() {
    let _ = fix_document(&format!("{} q\n", ">".repeat(200)), &depth(0));
    let _ = diagnose(&format!("{}q\n", "> ".repeat(200)), &depth(0));
    let long = format!("{}\n", "a".repeat(200_000));
    assert!(!fix_document(&long, &opts()).changed);
    assert!(diagnose(&long, &opts()).is_empty());
    assert_eq!(diagnose(&"a \n".repeat(150_000), &opts()).len(), 150_000);
}

// 48. 'MDX052 inside a blockquote inserts a quoted blank line'
#[test]
fn mdx052_inside_a_blockquote_inserts_a_quoted_blank_line() {
    assert_eq!(
        fixed("# T\n\n> text\n> ```js\n> x\n> ```\n> more\n"),
        "# T\n\n> text\n>\n> ```js\n> x\n> ```\n>\n> more\n"
    );
    assert_eq!(
        fixed("# T\n\n> para\nlazy\n> ```js\n> x\n> ```\n"),
        "# T\n\n> para\nlazy\n>\n> ```js\n> x\n> ```\n"
    );
    assert_eq!(
        fixed("# T\n\n> > a\n> > ```\n> > x\n> > ```\n> > b\n"),
        "# T\n\n> > a\n> >\n> > ```\n> > x\n> > ```\n> >\n> > b\n"
    );
    let separated = "# T\n\n> text\n>\n> ```js\n> x\n> ```\n>\n> more\n";
    assert!(!has(separated, "MDX052"));
    assert!(!fix_document(separated, &opts()).changed);
    assert_eq!(
        fixed("# T\n\ntext\n```js\nx\n```\nmore\n"),
        "# T\n\ntext\n\n```js\nx\n```\n\nmore\n"
    );
}

// 49. 'CLI accepts --max-quote-depth and --on-limit'
// CLI 層級，屬於 crates/markdown-fixer-cli/tests。

// 50. 'MDX020 keeps spaces after > that are part of the content'
#[test]
fn mdx020_keeps_spaces_after_gt_that_are_part_of_the_content() {
    assert_eq!(
        fixed(">quote\n>>nested\n>  > x\n>  text\n"),
        "> quote\n> > nested\n> > x\n> text\n"
    );
    assert_eq!(fixed("> a\n>   \n> b\n"), "> a\n>\n> b\n");
    let chain = format!("{}x\n", ">  ".repeat(12));
    assert_eq!(fixed(&chain), format!("{}x\n", "> ".repeat(12)));
    assert!(!fix_document(&fixed(&chain), &opts()).changed);
    let kept = "# T\n\n> - item\n>   > nested\n>\n>   continued\n\n>  ```\n>  code\n>  ```\n\n>  - one\n>    two\n";
    assert_eq!(fixed(kept), kept);
    assert!(!has(kept, "MDX020"));
    let html = "<iframe\n></iframe>\n";
    assert_eq!(fixed(html), html);
}

// 51. 'MDX060 skips header and delimiter rows that GFM does not treat as a table'
#[test]
fn mdx060_skips_header_and_delimiter_rows_that_gfm_does_not_treat_as_a_table() {
    let not_table = "| a | `x|y` |\n|:--|--:|\n";
    assert_eq!(fixed(not_table), not_table);
    assert!(!has(not_table, "MDX060"));
    assert_eq!(
        fixed("| a | `x\\|y` |\n|:--|--:|\n"),
        "| a | `x\\|y` |\n|:---|---:|\n"
    );
}

// 52. 'MDX006 does not turn a U+FEFF into a BOM by removing leading blank lines'
#[test]
fn mdx006_does_not_turn_a_feff_into_a_bom_by_removing_leading_blank_lines() {
    let text = "\n\u{feff}# T\n";
    assert!(!fix_document(text, &opts()).changed);
    let fixable: Vec<_> = diagnose(text, &opts())
        .into_iter()
        .filter(|p| p.rule == "MDX006")
        .map(|p| p.fixable)
        .collect();
    assert_eq!(fixable, [false]);
    assert_eq!(fixed("\n\n# T\n"), "# T\n");
}

// 53. 'MDX008 keeps an NBSP that is the only content of a line or list item'
#[test]
fn mdx008_keeps_an_nbsp_that_is_the_only_content_of_a_line_or_list_item() {
    for text in ["a\n\u{a0}\nb\n", "- \u{a0}\n", "> \u{a0}\u{a0}\n"] {
        assert_eq!(fixed(text), text);
        assert!(!has(text, "MDX008"));
    }
    assert_eq!(fixed("- a\u{a0}b\n"), "- a b\n");
}

// 54. 'MDX008 reports but keeps an NBSP right after a list marker'
#[test]
fn mdx008_reports_but_keeps_an_nbsp_right_after_a_list_marker() {
    for text in [
        "*\u{a0}x\n",
        "1.\u{a0}x\n",
        "- *\u{a0}x\n",
        "> +\u{a0}\u{a0}x\n",
    ] {
        assert_eq!(fixed(text), text);
        let fixable: Vec<_> = diagnose(text, &opts())
            .into_iter()
            .filter(|p| p.rule == "MDX008")
            .map(|p| p.fixable)
            .collect();
        assert_eq!(fixable, [false], "{text:?}");
    }
    assert_eq!(fixed("*\u{a0}x\u{a0}y\n"), "*\u{a0}x y\n");
    let problems: Vec<_> = diagnose("*\u{a0}x\u{a0}y\n", &opts())
        .into_iter()
        .map(|p| (p.rule, p.column, p.fixable))
        .collect();
    assert_eq!(problems, [("MDX008", 2, false), ("MDX008", 4, true)]);
}

// Rust 專有：渲染保護開啟時，資源上限的選項也要照樣生效（保護自己組出的選項不能遺漏它們）。
#[test]
fn resource_limits_apply_with_the_render_guard_on() {
    let deep = format!("# T\n\n{}x\n\n##標題\n", "> ".repeat(33));
    let guarded = fix_document(&deep, &Options::default());
    assert!(guarded.refused);
    assert!(guarded.reason.contains("Blockquote nesting depth 33"));

    let text = "#Title\n\n>>>>#Head##  **  x**\n>>>>[a]: https://x\n\n[t][a]\n";
    for on_limit in [LimitPolicy::Ignore, LimitPolicy::Error] {
        let with_guard = Options {
            max_quote_depth: 3,
            on_limit,
            ..Options::default()
        };
        let without_guard = Options {
            render_guard: false,
            ..with_guard.clone()
        };
        let a = fix_document(text, &with_guard);
        let b = fix_document(text, &without_guard);
        assert_eq!((a.text, a.refused, a.reason), (b.text, b.refused, b.reason));
        assert_eq!(diagnose(text, &with_guard), diagnose(text, &without_guard));
    }
}

// 55. 'protects an HTML comment or raw block opened after a list or blockquote marker'
#[test]
fn protects_an_html_comment_or_raw_block_opened_after_a_list_or_blockquote_marker() {
    let text = "# T\n\n- <!-- note\n\n\n  still  comment\n-->\n\n> <!-- quoted\n>\n>\n> -->\n";
    assert_eq!(fixed(text), text);
    assert!(!has(text, "MDX005"));
    let result = fix_document("# T\n\n- <!-- note\n\n\n\t#x\n", &opts());
    assert!(result.refused);
    assert!(result.reason.contains("Unclosed HTML comment"));
    assert_eq!(fixed("- <!-- x -->  **  y**\n"), "- <!-- x -->  **y**\n");
    for block in [
        "- <pre>\n    \n\n\tx  **  y**\n  </pre>\n",
        "> <script>\n>\n>\n> </script>\n",
        "- <?php\n\n\n?>\n",
        "- <![CDATA[\n\n\n]]>\n",
    ] {
        let text = format!("# T\n\n{block}");
        assert_eq!(fixed(&text), text, "{block:?}");
    }
    let unclosed = "# T\n\n- <pre>\n\n\nx\n";
    assert!(!fix_document(unclosed, &opts()).refused);
    assert_eq!(fixed(unclosed), unclosed);
}

// 56. 'keeps whitespace-only lines that GitHub treats as content after an empty list item or in a footnote'
#[test]
fn keeps_whitespace_only_lines_that_github_treats_as_content() {
    // (輸入, 修正結果, 保留的只含空白行的行號)
    let cases = [
        ("- \n  \n  x\n", "-\n  \n  x\n", 2),
        ("- a\n- \n  \n  b\n- c\n", "- a\n-\n  \n  b\n- c\n", 3),
        ("1.\n   \n   x\n", "1.\n   \n   x\n", 2),
        ("- \n\t\n  x\n", "-\n\t\n  x\n", 2),
        ("> - \n>   \n>   x\n", "> -\n>   \n>   x\n", 2),
        (
            "x[^1]\n\n[^1]: a\n  \n    b\n",
            "x[^1]\n\n[^1]: a\n  \n    b\n",
            4,
        ),
        ("# T\n\n- \n  \n\n\n  x\n", "# T\n\n-\n  \n\n  x\n", 4),
    ];
    for (input, output, kept) in cases {
        assert_eq!(fixed(input), output, "{input:?}");
        let on_kept: Vec<_> = diagnose(input, &opts())
            .into_iter()
            .filter(|p| p.line == kept && matches!(p.rule, "MDX004" | "MDX005" | "MDX020"))
            .collect();
        assert!(on_kept.is_empty(), "{input:?}: {on_kept:?}");
    }
    assert_eq!(fixed("text  \n\n  \n\nmore\n"), "text  \n\nmore\n");
    assert_eq!(fixed("- a\n  \n  b\n"), "- a\n\n  b\n");
}
