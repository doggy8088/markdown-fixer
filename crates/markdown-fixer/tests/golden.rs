//! Golden 語料差異測試。
//!
//! `fixtures/golden/**/*.json` 由 `scripts/gen-golden.mjs` 以 JS oracle 產生，每個檔案對應
//! `fixtures/corpus/` 下一份文件，內含數組選項的 `diagnose` 與 `fixDocument` 結果。
//! 這裡用 Rust API 跑同樣的輸入與選項，逐欄比對。不需要 Node，也不需要 `serde` feature。
//!
//! 全部案例跑完才判定失敗，並列出每個失敗案例的第一個差異與重現指令，
//! 避免一個早期失敗遮住其他問題。設定環境變數 `MDFIX_GOLDEN_FILTER=<子字串>`
//! 可以只跑來源路徑包含該子字串的檔案。

use std::fmt::Write as _;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use markdown_fixer::{
    DEFAULT_MAX_QUOTE_DEPTH, FixResult, LimitPolicy, Options, Problem, diagnose, fix_document,
    rules,
};
use serde_json::Value;

/// 最多詳細列出幾個失敗案例；其餘只計數。
const MAX_REPORTED: usize = 40;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden_dir() -> PathBuf {
    repo_root().join("fixtures/golden")
}

fn corpus_dir() -> PathBuf {
    repo_root().join("fixtures/corpus")
}

fn walk(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, extensions, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
        {
            out.push(path);
        }
    }
}

fn golden_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk(&golden_dir(), &["json"], &mut files);
    files.retain(|p| p != &golden_dir().join("rules.json"));
    files.sort();
    files
}

fn load_json(path: &Path) -> Value {
    let raw =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", path.display()))
}

fn string_list(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|items| {
        items
            .iter()
            .map(|v| v.as_str().expect("option lists hold strings").to_owned())
            .collect()
    })
}

/// golden 的 `options`（JS 鍵名）轉成 `Options`。
fn options_from(value: &Value) -> Options {
    Options {
        rules: string_list(value.get("rules")),
        ignore_rules: string_list(value.get("ignoreRules")),
        max_line_length: value
            .get("maxLineLength")
            .and_then(Value::as_u64)
            .map_or(0, |n| n as usize),
        max_quote_depth: value
            .get("maxQuoteDepth")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_MAX_QUOTE_DEPTH, |n| n as usize),
        on_limit: match value.get("onLimit").and_then(Value::as_str) {
            Some("ignore") => LimitPolicy::Ignore,
            _ => LimitPolicy::Error,
        },
        // golden 由 JS 參考實作產生：關閉 Rust 專有的渲染保護。
        render_guard: false,
    }
}

fn describe_problem(p: Option<&Value>) -> String {
    match p {
        None => "(none)".to_owned(),
        Some(p) => format!(
            "{} {}:{} {}{} {}",
            p["rule"].as_str().unwrap_or("?"),
            p["line"],
            p["column"],
            p["severity"].as_str().unwrap_or("?"),
            if p["fixable"].as_bool() == Some(true) {
                " fixable"
            } else {
                ""
            },
            p["message"]
        ),
    }
}

fn problem_value(p: &Problem) -> Value {
    serde_json::json!({
        "rule": p.rule,
        "name": p.name,
        "severity": p.severity.as_str(),
        "fixable": p.fixable,
        "line": p.line,
        "column": p.column,
        "message": p.message,
    })
}

fn fix_value(f: &FixResult) -> Value {
    serde_json::json!({
        "text": f.text,
        "changed": f.changed,
        "refused": f.refused,
        "reason": f.reason,
    })
}

/// 比對一個案例，回傳第一個差異的說明；一致時回傳 `None`。
fn first_difference(expected: &Value, problems: &[Problem], fix: &FixResult) -> Option<String> {
    let expected_problems = expected["diagnose"].as_array().cloned().unwrap_or_default();
    let actual_problems: Vec<Value> = problems.iter().map(problem_value).collect();
    let longest = expected_problems.len().max(actual_problems.len());
    for i in 0..longest {
        let e = expected_problems.get(i);
        let a = actual_problems.get(i);
        if e != a {
            return Some(format!(
                "diagnose differs at problem #{i} (oracle {} problem(s), Rust {})\n    oracle: {}\n    rust:   {}",
                expected_problems.len(),
                actual_problems.len(),
                describe_problem(e),
                describe_problem(a)
            ));
        }
    }

    let expected_fix = &expected["fix"];
    let actual_fix = fix_value(fix);
    for key in ["text", "changed", "refused", "reason"] {
        if expected_fix[key] == actual_fix[key] {
            continue;
        }
        if key == "text" {
            let e = expected_fix["text"].as_str().unwrap_or_default();
            let a = fix.text.as_str();
            let el: Vec<&str> = e.split('\n').collect();
            let al: Vec<&str> = a.split('\n').collect();
            let line = (0..el.len().max(al.len()))
                .find(|&i| el.get(i) != al.get(i))
                .unwrap_or(0);
            return Some(format!(
                "fix.text differs at line {}\n    oracle: {:?}\n    rust:   {:?}",
                line + 1,
                el.get(line).copied().unwrap_or("(end of text)"),
                al.get(line).copied().unwrap_or("(end of text)")
            ));
        }
        return Some(format!(
            "fix.{key} differs: oracle {}, Rust {}",
            expected_fix[key], actual_fix[key]
        ));
    }
    None
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

#[test]
fn golden_corpus_matches_oracle() {
    let filter = std::env::var("MDFIX_GOLDEN_FILTER").ok();
    let files = golden_files();
    assert!(
        !files.is_empty(),
        "no golden files found; run: node scripts/gen-golden.mjs"
    );

    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let golden = load_json(path);
        let source = golden["source"].as_str().expect("golden has a source");
        if filter.as_deref().is_some_and(|f| !source.contains(f)) {
            continue;
        }
        assert_eq!(
            golden["referenceVersion"].as_str(),
            Some(markdown_fixer::REFERENCE_VERSION),
            "{} was generated by another oracle version",
            path.display()
        );
        let bytes = fs::read(corpus_dir().join(source))
            .unwrap_or_else(|e| panic!("cannot read fixtures/corpus/{source}: {e}"));
        let text = String::from_utf8(bytes)
            .unwrap_or_else(|_| panic!("fixtures/corpus/{source} is not valid UTF-8"));

        for case in golden["cases"].as_array().expect("golden has cases") {
            total += 1;
            let name = case["name"].as_str().unwrap_or("?");
            let options = options_from(&case["options"]);
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                (diagnose(&text, &options), fix_document(&text, &options))
            }));
            let difference = match outcome {
                Ok((problems, fix)) => first_difference(case, &problems, &fix),
                Err(payload) => Some(format!("Rust panicked: {}", panic_message(&*payload))),
            };
            if let Some(difference) = difference {
                let opts = case["options"].to_string();
                failures.push(format!(
                    "{source} [{name}] {opts}\n    {difference}\n    repro: scripts/parity.sh fixtures/corpus/{source}\n           cargo run -q -p markdown-fixer --example dump --features serde -- '{opts}' < fixtures/corpus/{source}"
                ));
            }
        }
    }

    if failures.is_empty() {
        eprintln!("golden: {total}/{total} case(s) passed");
        return;
    }
    let mut report = format!(
        "golden: {}/{total} case(s) passed, {} failed\n\n",
        total - failures.len(),
        failures.len()
    );
    for failure in failures.iter().take(MAX_REPORTED) {
        let _ = writeln!(report, "{failure}\n");
    }
    if failures.len() > MAX_REPORTED {
        let _ = writeln!(
            report,
            "... and {} more; narrow with MDFIX_GOLDEN_FILTER=<path substring>",
            failures.len() - MAX_REPORTED
        );
    }
    panic!("{report}");
}

#[test]
fn rule_catalog_matches_oracle() {
    let golden = load_json(&golden_dir().join("rules.json"));
    let expected = golden["rules"].as_array().expect("rules.json has rules");
    let actual = rules();
    assert_eq!(
        actual.len(),
        expected.len(),
        "rule count differs from the oracle catalog"
    );
    for (i, (e, a)) in expected.iter().zip(actual).enumerate() {
        let a = serde_json::json!({
            "id": a.id,
            "name": a.name,
            "category": a.category,
            "severity": a.severity.as_str(),
            "fixable": a.fixable,
            "summary": a.summary,
        });
        assert_eq!(&a, e, "rule #{i} differs from the oracle catalog");
    }
    for e in expected {
        let id = e["id"].as_str().expect("rule id");
        assert_eq!(
            markdown_fixer::rule(id).map(|r| r.id),
            Some(id),
            "rule({id}) lookup failed"
        );
    }
    assert!(markdown_fixer::rule("MDX999").is_none());
}

/// 每份語料都要有 golden，每個 golden 都要有語料；否則代表忘了跑 gen-golden。
#[test]
fn every_corpus_file_has_a_golden() {
    let mut corpus = Vec::new();
    walk(&corpus_dir(), &["md", "markdown"], &mut corpus);
    let root = corpus_dir();
    let mut missing = Vec::new();
    for file in &corpus {
        let rel = file.strip_prefix(&root).expect("under corpus");
        let golden = golden_dir().join(rel).with_extension("json");
        if !golden.exists() {
            missing.push(rel.display().to_string());
        }
    }
    let mut orphans = Vec::new();
    for golden in golden_files() {
        let source = load_json(&golden)["source"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if !root.join(&source).exists() {
            orphans.push(source);
        }
    }
    assert!(
        missing.is_empty() && orphans.is_empty(),
        "golden files out of sync (run: node scripts/gen-golden.mjs)\n  missing: {missing:?}\n  orphans: {orphans:?}"
    );
    assert!(
        corpus.len() >= 45,
        "corpus should hold at least 45 documents"
    );
}
