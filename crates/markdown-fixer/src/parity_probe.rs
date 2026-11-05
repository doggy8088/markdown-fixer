//! 與 oracle 比對用的探針（只在測試中編譯）。
//!
//! `MDFIX_PROBE_IN=<json 字串陣列> MDFIX_PROBE_OUT=<輸出檔> cargo test -p markdown-fixer --lib -- --ignored parity_probe`
//! 會把每份輸入的 scan／mask 結果寫成文字，與 scratch 裡的 node 腳本輸出逐字比較。
//! 平常不執行（需要外部輸入檔），也不依賴 Node。

use crate::jsstr::{to_string, u};
use crate::mask::{MaskOptions, build_mask};
use crate::scan::scan_document;

fn flag(b: bool, c: char) -> char {
    if b { c } else { '.' }
}

pub(crate) fn dump_scan(text: &str) -> String {
    let scan = scan_document(&u(text));
    let mut out = String::new();
    for (i, info) in scan.info.iter().enumerate() {
        let flags: String = [
            flag(info.blank, 'b'),
            flag(info.frontmatter, 'F'),
            flag(info.fence, 'f'),
            flag(info.fence_open, 'o'),
            flag(info.fence_close, 'c'),
            flag(info.fence_content, 'C'),
            flag(info.html_block, 'h'),
            flag(info.html_start, 's'),
            flag(info.indented_code, 'i'),
            flag(info.definition, 'd'),
            flag(info.math_block, 'm'),
            flag(info.math_open, 'O'),
            flag(info.math_close, 'X'),
        ]
        .iter()
        .collect();
        let fence_char = info.fence_char.map(|c| to_string(&[c])).unwrap_or_default();
        out.push_str(&format!(
            "{i}:{flags}|{}|{fence_char}{}|{}\n",
            to_string(&info.fence_info),
            info.fence_len,
            info.html_kind
        ));
    }
    for p in &scan.issues {
        out.push_str(&format!(
            "{}@{}:{} {}\n",
            p.rule, p.line, p.column, p.message
        ));
    }
    out.push_str(&format!(
        "fm={}\n",
        scan.frontmatter_end.map_or(-1, |e| e as i64)
    ));
    let sets = [
        MaskOptions::default(),
        MaskOptions::NO_BARE_URLS,
        MaskOptions {
            mask_bare_urls: false,
            mask_link_destinations: false,
            mask_reference_labels: false,
            mask_link_text: false,
        },
        MaskOptions {
            mask_bare_urls: false,
            mask_link_text: true,
            ..MaskOptions::default()
        },
        MaskOptions {
            mask_bare_urls: false,
            mask_link_destinations: false,
            ..MaskOptions::default()
        },
    ];
    for line in &scan.lines {
        for set in sets {
            let m: String = build_mask(line, set)
                .iter()
                .map(|&b| if b { '1' } else { '0' })
                .collect();
            out.push_str(&m);
            out.push(' ');
        }
        out.push('\n');
    }
    out
}

#[test]
#[ignore = "需要 MDFIX_PROBE_IN／MDFIX_PROBE_OUT，與 node 腳本搭配"]
fn parity_probe() {
    let (Ok(input), Ok(output)) = (
        std::env::var("MDFIX_PROBE_IN"),
        std::env::var("MDFIX_PROBE_OUT"),
    ) else {
        return;
    };
    let mode = std::env::var("MDFIX_PROBE_MODE").unwrap_or_else(|_| "scan".into());
    if std::env::var("MDFIX_FORCE_MEMO").is_ok() {
        crate::jsregex::FORCE_MEMO.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let data = std::fs::read_to_string(input).expect("read input");
    let docs: Vec<String> = serde_json::from_str(&data).expect("json");
    let mut out = String::new();
    for doc in &docs {
        out.push_str("=====\n");
        out.push_str(&match mode.as_str() {
            "scan" => dump_scan(doc),
            other => probe_mode(other, doc),
        });
    }
    std::fs::write(output, out).expect("write output");
}

fn probe_options() -> crate::Options {
    let Ok(raw) = std::env::var("MDFIX_PROBE_OPTS") else {
        return crate::Options::default();
    };
    let v: serde_json::Value = serde_json::from_str(&raw).expect("options json");
    let list = |key: &str| {
        v.get(key).and_then(|x| x.as_array()).map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
    };
    crate::Options {
        rules: list("rules"),
        ignore_rules: list("ignoreRules"),
        max_line_length: v.get("maxLineLength").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
        ..Default::default()
    }
}

fn probe_mode(mode: &str, doc: &str) -> String {
    match mode {
        "fix" => {
            let r = crate::fix_document(doc, &probe_options());
            format!(
                "{{\"text\":{},\"changed\":{},\"refused\":{},\"reason\":{}}}\n",
                serde_json::to_string(&r.text).expect("json"),
                r.changed,
                r.refused,
                serde_json::to_string(&r.reason).expect("json"),
            )
        }
        "diagnose" => crate::diagnose(doc, &probe_options())
            .iter()
            .map(|p| {
                format!(
                    "{}|{}|{}|{}|{}|{}|{}\n",
                    p.rule,
                    p.name,
                    p.severity.as_str(),
                    p.fixable,
                    p.line,
                    p.column,
                    p.message
                )
            })
            .collect(),
        _ => format!("unknown mode {mode}\n"),
    }
}

/// 由 oracle（`scanDocument` + `buildMask` 五組選項）產生的預期輸出。
const ORACLE_SCAN_CASES: &[(&str, &str)] = &[
    (
        "😀**foo**x\n\\\\😀** a**",
        "0:.............||0|\n1:.............||0|\nfm=-1\n0000000000 0000000000 0000000000 0000000000 0000000000 \n1100000000 1100000000 1100000000 1100000000 1100000000 \n",
    ),
    (
        "---\ntitle: x\n\nbody",
        "0:.F...........||0|\n1:.F...........||0|\n2:b............||0|\n3:.............||0|\nMDX081@1:1 Unclosed YAML frontmatter: no closing --- or ... line was found.\nfm=-1\n000 000 000 000 000 \n00000000 00000000 00000000 00000000 00000000 \n     \n0000 0000 0000 0000 0000 \n",
    ),
    (
        "> $$\n> x\nno",
        "0:..........mO.||0|\n1:..........m..||0|\n2:.............||0|\nMDX085@1:1 Unclosed $$ math block: the blockquote ended before a closing $$ was found.\nfm=-1\n0000 0000 0000 0000 0000 \n000 000 000 000 000 \n00 00 00 00 00 \n",
    ),
    (
        "<!-- a\nb -->\n```a`b\n~~~\nx",
        "0:......hs.....||0|comment\n1:......h......||0|\n2:.............||0|\n3:..fo.........||~3|\n4:..f..C.......||0|\nMDX053@3:1 Invalid fenced-code opener: a backtick fence info string cannot contain backticks.\nMDX051@4:1 Unclosed tilde code fence.\nfm=-1\n000000 000000 000000 000000 000000 \n00000 00000 00000 00000 00000 \n000000 000000 000000 000000 000000 \n000 000 000 000 000 \n0 0 0 0 0 \n",
    ),
    (
        "    code\n\n    more\npara\n[ref]: http://x",
        "0:........i....||0|\n1:b.......i....||0|\n2:........i....||0|\n3:.............||0|\n4:.........d...||0|\nfm=-1\n00000000 00000000 00000000 00000000 00000000 \n     \n00000000 00000000 00000000 00000000 00000000 \n0000 0000 0000 0000 0000 \n000000011111111 000000000000000 000000000000000 000000000000000 000000000000000 \n",
    ),
    (
        "url https://example.com/a\u{a0}b x mail a.b@c.de [l](<a)b> \"t)\") `c`",
        "0:.............||0|\nfm=-1\n000011111111111111111111111000000001111111100001111111111110111 000000000000000000000000000000000000000000000001111111111110111 000000000000000000000000000000000000000000000000111110000000111 000000000000000000000000000000000000000000001111111111111110111 000000000000000000000000000000000000000000000000111110000000111 \n",
    ),
    (
        "<😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀>",
        "0:.............||0|\nfm=-1\n000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 \n",
    ),
];

#[test]
fn scan_and_mask_match_oracle() {
    for (input, expected) in ORACLE_SCAN_CASES {
        assert_eq!(dump_scan(input), *expected, "input: {input:?}");
    }
}
