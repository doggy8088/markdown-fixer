//! 命令列參數解析，逐行對應 oracle 的 `parseArgs`（手寫迴圈，不用 clap）。
//!
//! 錯誤在迴圈中依參數順序丟出，所以 `--help --bogus` 也是用法錯誤。
//! Rust 版專有的旗標（`--stdin`、`--exit-zero`、`--detect`）只在這裡記錄；
//! 它們與路徑的衝突檢查放在 `main`，排在 `--help` 等之後。

use std::ffi::OsString;

use crate::jsstr::{js_number, js_trim, positive_integer};

const RULE_LIST_REQUIRED: &str = "A rule list is required after --rule/--ignore-rule.";
const MAX_LINE_LENGTH_INVALID: &str = "--max-line-length requires a positive integer.";

/// 執行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Check,
    Write,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Check => "check",
            Mode::Write => "write",
        }
    }
}

/// 解析後的 CLI 選項（oracle `parseArgs` 的回傳值，加上 Rust 版專有旗標）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub mode: Mode,
    pub json: bool,
    pub quiet: bool,
    pub strict: bool,
    pub help: bool,
    pub version: bool,
    pub list_rules: bool,
    pub rules: Option<Vec<String>>,
    pub ignore_rules: Option<Vec<String>>,
    pub max_line_length: usize,
    pub paths: Vec<OsString>,
    /// Rust 版專有：從標準輸入讀取。
    pub stdin: bool,
    /// Rust 版專有：除了結束碼 2 以外一律以 0 結束。
    pub exit_zero: bool,
    /// Rust 版專有：只做 Markdown 偵測。
    pub detect: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        CliOptions {
            mode: Mode::Check,
            json: false,
            quiet: false,
            strict: false,
            help: false,
            version: false,
            list_rules: false,
            rules: None,
            ignore_rules: None,
            max_line_length: 0,
            paths: Vec::new(),
            stdin: false,
            exit_zero: false,
            detect: false,
        }
    }
}

impl CliOptions {
    /// 轉成函式庫的 `Options`。
    pub fn library_options(&self) -> markdown_fixer::Options {
        markdown_fixer::Options {
            rules: self.rules.clone(),
            ignore_rules: self.ignore_rules.clone(),
            max_line_length: self.max_line_length,
            ..markdown_fixer::Options::default()
        }
    }
}

fn add_rules(
    value: Option<&str>,
    target: &mut Option<Vec<String>>,
    is_known_rule: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let value = match value {
        Some(value) if !value.is_empty() => value,
        _ => return Err(RULE_LIST_REQUIRED.to_string()),
    };
    let ids: Vec<String> = value
        .split(',')
        .map(js_trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect();
    if ids.is_empty() {
        return Err(RULE_LIST_REQUIRED.to_string());
    }
    for id in &ids {
        if !is_known_rule(id) {
            return Err(format!("Unknown rule: {id}"));
        }
    }
    target.get_or_insert_with(Vec::new).extend(ids);
    Ok(())
}

fn parse_max_line_length(value: Option<&str>) -> Result<usize, String> {
    // `Number(undefined)` 是 NaN。
    let number = value.map_or(f64::NAN, js_number);
    positive_integer(number).ok_or_else(|| MAX_LINE_LENGTH_INVALID.to_string())
}

/// 解析參數（不含程式名稱）。`is_known_rule` 判斷規則 ID 是否存在。
///
/// 回傳的 `Err` 是不含 `Error: ` 前綴的訊息。
pub fn parse_args(
    argv: &[OsString],
    is_known_rule: &dyn Fn(&str) -> bool,
) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    // 旗標比對用 lossy 字串；路徑保留原本的 OsString。
    let text: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let mut i = 0;
    while i < argv.len() {
        let arg = text[i].as_str();
        match arg {
            "--check" => options.mode = Mode::Check,
            "--write" => options.mode = Mode::Write,
            "--json" => options.json = true,
            "--quiet" => options.quiet = true,
            "--strict" => options.strict = true,
            "--help" | "-h" => options.help = true,
            "--version" | "-v" => options.version = true,
            "--list-rules" => options.list_rules = true,
            "--stdin" => options.stdin = true,
            "--exit-zero" => options.exit_zero = true,
            "--detect" => options.detect = true,
            "--rule" => {
                i += 1;
                add_rules(
                    text.get(i).map(String::as_str),
                    &mut options.rules,
                    is_known_rule,
                )?;
            }
            "--ignore-rule" => {
                i += 1;
                add_rules(
                    text.get(i).map(String::as_str),
                    &mut options.ignore_rules,
                    is_known_rule,
                )?;
            }
            "--max-line-length" => {
                i += 1;
                options.max_line_length = parse_max_line_length(text.get(i).map(String::as_str))?;
            }
            "--" => {
                options.paths.extend(argv[i + 1..].iter().cloned());
                break;
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--rule=") {
                    add_rules(Some(value), &mut options.rules, is_known_rule)?;
                } else if let Some(value) = arg.strip_prefix("--ignore-rule=") {
                    add_rules(Some(value), &mut options.ignore_rules, is_known_rule)?;
                } else if let Some(value) = arg.strip_prefix("--max-line-length=") {
                    options.max_line_length = parse_max_line_length(Some(value))?;
                } else if arg.starts_with('-') {
                    return Err(format!("Unknown option: {arg}"));
                } else {
                    options.paths.push(argv[i].clone());
                }
            }
        }
        i += 1;
    }

    if options.max_line_length == 0
        && options
            .rules
            .as_ref()
            .is_some_and(|rules| rules.iter().any(|id| id == "MDX090"))
    {
        options.max_line_length = 120;
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: &str) -> bool {
        id.len() == 6 && id.starts_with("MDX") && id[3..].bytes().all(|b| b.is_ascii_digit())
    }

    fn parse(args: &[&str]) -> Result<CliOptions, String> {
        let argv: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse_args(&argv, &known)
    }

    // 移植自 markdown-fixer.test.mjs：「parseArgs handles modes, rule lists, limits, and errors」
    #[test]
    fn handles_modes_rule_lists_limits_and_errors() {
        let parsed = parse(&[
            "--write",
            "--json",
            "--strict",
            "--rule",
            "MDX010,MDX011",
            "--ignore-rule=MDX031",
            "--max-line-length",
            "80",
            "a.md",
        ])
        .unwrap();
        assert_eq!(parsed.mode, Mode::Write);
        assert!(parsed.json);
        assert!(parsed.strict);
        assert_eq!(parsed.rules, Some(vec!["MDX010".into(), "MDX011".into()]));
        assert_eq!(parsed.ignore_rules, Some(vec!["MDX031".into()]));
        assert_eq!(parsed.max_line_length, 80);
        assert_eq!(parsed.paths, vec![OsString::from("a.md")]);

        assert_eq!(parse(&[]).unwrap().mode, Mode::Check);
        assert_eq!(parse(&["--rule", "MDX090"]).unwrap().max_line_length, 120);
        assert_eq!(
            parse(&["--", "--not-an-option.md"]).unwrap().paths,
            vec![OsString::from("--not-an-option.md")]
        );
        assert!(
            parse(&["--rule", "NOPE"])
                .unwrap_err()
                .contains("Unknown rule")
        );
        assert!(
            parse(&["--max-line-length", "0"])
                .unwrap_err()
                .contains("positive integer")
        );
        assert!(parse(&["--bogus"]).unwrap_err().contains("Unknown option"));
    }

    #[test]
    fn rule_list_errors() {
        for args in [
            &["--rule"][..],
            &["--rule="],
            &["--rule", ""],
            &["--rule", " , "],
            &["--ignore-rule"],
            &["--ignore-rule=,"],
        ] {
            assert_eq!(parse(args).unwrap_err(), RULE_LIST_REQUIRED, "{args:?}");
        }
        // `--rule` 無條件吃掉下一個參數。
        assert_eq!(
            parse(&["--rule", "--json"]).unwrap_err(),
            "Unknown rule: --json"
        );
        // ID 以 JS trim() 修剪，包含 U+FEFF 與 NBSP。
        assert_eq!(
            parse(&["--rule=\u{FEFF}MDX010\u{00A0},,MDX011 "])
                .unwrap()
                .rules,
            Some(vec!["MDX010".into(), "MDX011".into()])
        );
        // 可重複，保留重複項目。
        assert_eq!(
            parse(&["--rule", "MDX010", "--rule=MDX010"]).unwrap().rules,
            Some(vec!["MDX010".into(), "MDX010".into()])
        );
    }

    #[test]
    fn max_line_length_uses_js_number() {
        let len = |v: &str| parse(&["--max-line-length", v]).map(|o| o.max_line_length);
        assert_eq!(len(" 80 "), Ok(80));
        assert_eq!(len("0x50"), Ok(80));
        assert_eq!(len("1e2"), Ok(100));
        assert_eq!(len("80.0"), Ok(80));
        for bad in ["", "8.5", "-1", "abc", "Infinity", "0"] {
            assert_eq!(
                len(bad),
                Err(MAX_LINE_LENGTH_INVALID.to_string()),
                "{bad:?}"
            );
        }
        assert_eq!(
            parse(&["--max-line-length"]).unwrap_err(),
            MAX_LINE_LENGTH_INVALID
        );
        assert_eq!(
            parse(&["--max-line-length="]).unwrap_err(),
            MAX_LINE_LENGTH_INVALID
        );
        assert_eq!(
            parse(&["--max-line-length=72"]).unwrap().max_line_length,
            72
        );
        // 明確給的行長優先於 MDX090 的預設 120。
        assert_eq!(
            parse(&["--max-line-length=72", "--rule", "MDX090"])
                .unwrap()
                .max_line_length,
            72
        );
        assert_eq!(
            parse(&["--ignore-rule", "MDX090"]).unwrap().max_line_length,
            0
        );
    }

    #[test]
    fn unknown_options_and_paths() {
        assert_eq!(parse(&["-"]).unwrap_err(), "Unknown option: -");
        assert_eq!(
            parse(&["--check=x"]).unwrap_err(),
            "Unknown option: --check=x"
        );
        assert_eq!(
            parse(&["--help", "--bogus"]).unwrap_err(),
            "Unknown option: --bogus"
        );
        let parsed = parse(&["a.md", "--json", "b.md", "-v", "-h"]).unwrap();
        assert!(parsed.json && parsed.version && parsed.help);
        assert_eq!(
            parsed.paths,
            vec![OsString::from("a.md"), OsString::from("b.md")]
        );
        let parsed = parse(&["--stdin", "--exit-zero", "--detect", "--list-rules"]).unwrap();
        assert!(parsed.stdin && parsed.exit_zero && parsed.detect && parsed.list_rules);
    }
}
