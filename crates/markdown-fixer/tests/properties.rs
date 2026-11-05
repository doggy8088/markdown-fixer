//! Property test（proptest）：函式庫的安全契約（PRD 6.2）在任意輸入下都成立。
//!
//! - 任意字串與「像 Markdown」的隨機文件都不會 panic。
//! - `fix_document` 冪等。oracle v1.1.0 在 `scripts/fuzz-parity.mjs --idempotence` 的約 7 萬組
//!   （文件, 選項）與整個語料上都冪等（v1.0.0 的「移除檔首空行後露出行首 BOM」與
//!   「`>  >` 每輪只合併一層」已在 v1.1.0 修正）。若這個 property 失敗，先用 oracle 跑同一份
//!   輸入，oracle 也不冪等時記到 docs/parity.md。
//! - 拒絕修正時輸出等於輸入。
//! - 停用 MDX001／MDX002 時保留 BOM 與行尾；啟用時輸出沒有 BOM 與 CR。
//! - 病態輸入（深層巢狀、超長的分隔符號行）在時間上限內完成，不得有指數級回溯
//!   （PRD 7）。上限可用 `MDFIX_PATHOLOGICAL_BUDGET_MS` 調整。
//!
//! 案例數可用 `PROPTEST_CASES` 調整（預設 256）。

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use markdown_fixer::{FixResult, LimitPolicy, Options, diagnose, fix_document};
use proptest::prelude::*;

/// 與 oracle 比較的選項基底：關閉 Rust 專有的渲染保護（PRD 6.9）。
fn parity() -> Options {
    Options {
        render_guard: false,
        ..Options::default()
    }
}

fn ignore(ids: &[&str]) -> Options {
    Options {
        ignore_rules: Some(ids.iter().map(|s| (*s).to_owned()).collect()),
        ..parity()
    }
}

/// 與 scripts/gen-golden.mjs 的 OPTION_SETS 相同的六組選項。
fn option_sets() -> Vec<Options> {
    vec![
        parity(),
        ignore(&["MDX003"]),
        Options {
            max_line_length: 80,
            ..parity()
        },
        ignore(&["MDX001", "MDX002"]),
        Options {
            rules: Some(
                ["MDX010", "MDX011", "MDX012", "MDX013"]
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect(),
            ),
            ..parity()
        },
        Options {
            max_quote_depth: 3,
            on_limit: LimitPolicy::Ignore,
            ..parity()
        },
    ]
}

/// 組成隨機文件的片段：Markdown 語法、保護區域、CJK、astral 字元與不可見字元。
const ATOMS: &[&str] = &[
    "#",
    "## ",
    "###",
    "#\u{3000}",
    " #",
    "> ",
    ">",
    ">>",
    "- ",
    "-",
    "* ",
    "*",
    "+",
    "1.",
    "2) ",
    "- [ ] ",
    "```",
    "````",
    "~~~",
    "```js",
    "$$",
    "$",
    "**",
    "__",
    "_",
    "~~",
    "`",
    "``",
    "|",
    "| --- |",
    "|-|",
    " | ",
    ":--",
    "[",
    "]",
    "(",
    ")",
    "![",
    "]: ",
    "][",
    "<!--",
    "-->",
    "<div>",
    "</div>",
    "<script>",
    "<br>",
    "---",
    "===",
    "***",
    "...",
    "title: x",
    "    ",
    "   ",
    "  ",
    " ",
    "\t",
    "\u{00A0}",
    "\u{200B}",
    "\u{FEFF}",
    "\u{3000}",
    "\u{2028}",
    "：",
    "。",
    "，",
    "「",
    "」",
    "（",
    "）",
    "＃",
    "－",
    "［",
    "］",
    "中文",
    "標籤",
    "word",
    "Note:",
    "😀",
    "𠮷",
    "👨\u{200D}👩",
    "https://example.com/a",
    "www.example.org",
    "me@example.com",
    "\\",
    "\\*",
    "&",
    "<",
    "x",
    "1",
];

fn line_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(ATOMS), 0..8).prop_map(|parts| parts.concat())
}

/// 「像 Markdown」的文件：隨機行、隨機行尾（含混合）、可能有 BOM、可能沒有檔尾換行。
fn markdownish() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(line_strategy(), 0..24),
        prop::collection::vec(
            prop::sample::select(&["\n", "\n", "\n", "\r\n", "\r"][..]),
            1..4,
        ),
        prop::sample::select(&["", "", "", "\u{FEFF}", "\u{FEFF}\u{FEFF}"][..]),
        any::<bool>(),
    )
        .prop_map(|(lines, eols, bom, final_newline)| {
            let mut text = String::from(bom);
            for (i, line) in lines.iter().enumerate() {
                text.push_str(line);
                if i + 1 < lines.len() || final_newline {
                    text.push_str(eols[i % eols.len()]);
                }
            }
            text
        })
}

fn assert_refusal_contract(input: &str, result: &FixResult) {
    if result.refused {
        assert_eq!(result.text, input, "refused fix must return the input");
        assert!(!result.changed, "refused fix must not report a change");
        assert!(!result.reason.is_empty(), "refused fix needs a reason");
    } else {
        assert!(result.reason.is_empty(), "reason is only set when refused");
        assert_eq!(result.changed, result.text != input);
    }
}

fn leading_boms(text: &str) -> &str {
    let rest = text.trim_start_matches('\u{FEFF}');
    &text[..text.len() - rest.len()]
}

proptest! {
    #[test]
    fn never_panics_on_arbitrary_strings(text in any::<String>()) {
        for options in option_sets() {
            let problems = diagnose(&text, &options);
            for p in &problems {
                prop_assert!(p.line >= 1 && p.column >= 1);
            }
            let result = fix_document(&text, &options);
            assert_refusal_contract(&text, &result);
        }
    }

    #[test]
    fn never_panics_on_markdownish_documents(text in markdownish()) {
        for options in option_sets() {
            let problems = diagnose(&text, &options);
            prop_assert_eq!(&problems, &diagnose(&text, &options), "diagnose must be deterministic");
            for p in &problems {
                prop_assert!(p.line >= 1 && p.column >= 1);
            }
            let result = fix_document(&text, &options);
            assert_refusal_contract(&text, &result);
        }
    }

    #[test]
    fn fix_is_idempotent(text in markdownish()) {
        for options in option_sets() {
            let once = fix_document(&text, &options);
            if once.refused {
                continue;
            }
            let twice = fix_document(&once.text, &options);
            prop_assert!(!twice.refused, "fixed text must not be refused: {:?}", once.text);
            prop_assert_eq!(&twice.text, &once.text, "options {:?}, input {:?}", options, text);
            prop_assert!(!twice.changed);
        }
    }

    // fixDocument：停用 MDX001 時把原本的整串 BOM 加回去；停用 MDX002 時以原文的
    // 行尾（CRLF 優先，其次 CR）還原，所以混合行尾會變成一致的 originalEol。
    #[test]
    fn bom_and_line_endings_are_preserved_when_their_rules_are_ignored(text in markdownish()) {
        let result = fix_document(&text, &ignore(&["MDX001", "MDX002"]));
        if result.refused || !result.changed {
            prop_assert_eq!(&result.text, &text);
            return Ok(());
        }
        prop_assert_eq!(leading_boms(&result.text), leading_boms(&text));
        let body = result.text.trim_start_matches('\u{FEFF}');
        if text.contains("\r\n") {
            prop_assert_eq!(body.replace("\r\n", "").matches(['\r', '\n']).count(), 0,
                "CRLF input must come out with CRLF only: {:?}", result.text);
        } else if text.contains('\r') {
            prop_assert!(!body.contains('\n'), "CR input must come out with CR only: {:?}", result.text);
        } else {
            prop_assert!(!body.contains('\r'));
        }
    }

    #[test]
    fn encoding_is_normalized_when_rules_are_enabled(text in markdownish()) {
        let result = fix_document(&text, &Options::default());
        if !result.refused {
            prop_assert!(!result.text.starts_with('\u{FEFF}'), "{:?}", result.text);
            prop_assert!(!result.text.contains('\r'), "{:?}", result.text);
        }
    }
}

// ---------------------------------------------------------------- 病態輸入

fn budget() -> Duration {
    let ms = std::env::var("MDFIX_PATHOLOGICAL_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) {
            10_000
        } else {
            2_000
        });
    Duration::from_millis(ms)
}

/// 在另一條執行緒跑 diagnose 與預設選項的 fix_document，超過時間上限就判定失敗
/// （不會讓測試卡住）。
fn assert_finishes(name: &str, text: String) {
    let limit = budget();
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    thread::spawn(move || {
        // 預設的引言深度上限會讓深層引言直接被 MDX086 拒絕；`max_quote_depth: 0` 才會真的
        // 走完掃描與修正，檢查它們是線性時間。
        let unlimited = Options {
            max_quote_depth: 0,
            ..parity()
        };
        let mut problems = 0;
        let mut refused = false;
        for options in [Options::default(), unlimited] {
            problems += diagnose(&text, &options).len();
            refused |= fix_document(&text, &options).refused;
        }
        let _ = tx.send((problems, refused));
    });
    match rx.recv_timeout(limit) {
        Ok(_) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{name}: did not finish within {limit:?} (possible exponential backtracking)")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{name}: library panicked after {:?}", started.elapsed())
        }
    }
}

#[test]
fn deep_blockquotes_finish_in_time() {
    // oracle v1.0.0 的 reference definition regex 在這類輸入上會指數級回溯（docs/parity.md）。
    assert_finishes("200-deep '> ' quote", format!("{}q\n", "> ".repeat(200)));
    assert_finishes("200-deep '>' quote", format!("{} q\n", ">".repeat(200)));
    assert_finishes(
        "200-deep '> ' quote with a definition",
        format!("{}[a]: https://example.com\n", "> ".repeat(200)),
    );
    assert_finishes(
        "200 nested quoted lines",
        (1..=200)
            .map(|d| format!("{}text\n", "> ".repeat(d)))
            .collect(),
    );
}

#[test]
fn deep_lists_finish_in_time() {
    let text: String = (0..200)
        .map(|i| format!("{}- item {i}\n", "  ".repeat(i)))
        .collect();
    assert_finishes("200-deep list", text);
    assert_finishes(
        "200-deep list in quotes",
        (0..200)
            .map(|i| format!("{}- > - x\n", "> ".repeat(i % 20)))
            .collect(),
    );
}

#[test]
fn long_delimiter_lines_finish_in_time() {
    const N: usize = 10_000;
    let singles = [
        "*", "_", "`", "~", "$", "#", "|", "[", "]", "(", ")", "<", ">", "!", "-", "=", "\\", " ",
        "\t", "\u{00A0}", "\u{3000}", "：",
    ];
    for delimiter in singles {
        assert_finishes(
            &format!("{N} x {delimiter:?}"),
            format!("{}\n", delimiter.repeat(N)),
        );
    }
    let patterns = [
        "**a",
        "** a",
        "a **",
        "__a",
        "_ a",
        "*a",
        "~~a",
        "`a",
        "$a",
        "[a](",
        "[a][",
        "![](",
        "](",
        "[",
        "<a",
        "<!--",
        "**：",
        "**「",
        "中**",
        "| a ",
        "\\*",
        "a ",
        "http://a.b/",
    ];
    for pattern in patterns {
        let repeated = pattern.repeat(N / pattern.chars().count().max(1));
        assert_finishes(&format!("{pattern:?} repeated"), format!("{repeated}\n"));
        assert_finishes(
            &format!("{pattern:?} repeated in a table row"),
            format!("| h | h |\n| --- | --- |\n| {repeated} | x |\n"),
        );
    }
}

#[test]
fn many_lines_finish_in_time() {
    assert_finishes("20k blank lines", "\n".repeat(20_000));
    assert_finishes("10k list items", "- x\n* y\n".repeat(5_000));
    assert_finishes(
        "5k paragraph lines with unpaired CJK emphasis",
        "**標籤：**內容\n".repeat(5_000),
    );
    assert_finishes("5k table rows", {
        let mut t = String::from("| a | b |\n| - | -- |\n");
        t.push_str(&"| 1 | 2 | 3 |\n".repeat(5_000));
        t
    });
}
