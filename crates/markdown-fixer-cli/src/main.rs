//! markdown-fixer CLI。規格見 PRD.md〈6.4 CLI〉，使用說明見 `docs/cli.md`。
//!
//! 行為逐位元組對應 oracle `reference/js/markdown-fixer.mjs` 的 `main`；
//! `--stdin`、`--exit-zero`、`--detect` 是 Rust 版專有（PRD〈6.4.2〉）。

mod args;
mod fileio;
mod jsstr;
mod node_err;
mod paths;
mod report;

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use markdown_fixer::{FixResult, Problem};

use args::{CliOptions, Mode};
use report::FileResult;

/// `<stdin>` 的顯示名稱。
const STDIN_DISPLAY: &str = "<stdin>";

fn main() -> ExitCode {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let code = run(&argv, &mut out, &mut err);
    let _ = out.flush();
    let _ = err.flush();
    ExitCode::from(code)
}

/// 標準輸出／錯誤輸出的寫入失敗（例如 `| head` 造成的 EPIPE）一律忽略，
/// 不讓 CLI panic；結束碼仍依檢查結果決定。
macro_rules! say {
    ($dst:expr) => {{
        let _ = writeln!($dst);
    }};
    ($dst:expr, $($arg:tt)*) => {{
        let _ = writeln!($dst, $($arg)*);
    }};
}

fn usage_error(err: &mut dyn Write, message: &str) -> u8 {
    say!(err, "Error: {message}");
    say!(err, "Run with --help for usage.");
    2
}

/// 套用 `--exit-zero`：結束碼 1 改為 0，2 保留。
fn finish(code: u8, options: &CliOptions) -> u8 {
    if options.exit_zero && code == 1 {
        0
    } else {
        code
    }
}

fn run(argv: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let is_known_rule = |id: &str| markdown_fixer::rule(id).is_some();
    let mut options = match args::parse_args(argv, &is_known_rule) {
        Ok(options) => options,
        Err(message) => return usage_error(err, &message),
    };

    if options.help {
        let _ = out.write_all(report::help_text().as_bytes());
        return 0;
    }
    if options.version {
        say!(out, "markdown-fixer {}", report::VERSION);
        return 0;
    }
    if options.list_rules {
        let _ = out.write_all(report::rules_text(markdown_fixer::rules()).as_bytes());
        return 0;
    }

    if options.detect {
        if !options.paths.is_empty() {
            return usage_error(
                err,
                "--detect reads standard input and cannot be combined with file or directory arguments.",
            );
        }
        return run_detect(&options, out, err);
    }
    if options.stdin {
        if !options.paths.is_empty() {
            return usage_error(
                err,
                "--stdin cannot be combined with file or directory arguments.",
            );
        }
        return run_stdin(&options, out, err);
    }

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            say!(err, "Error: {error}");
            return 2;
        }
    };

    if options.paths.is_empty() {
        if options.mode == Mode::Write {
            say!(
                err,
                "Error: --write requires explicit files or directories. Refusing to rewrite the whole working tree implicitly."
            );
            return 2;
        }
        options.paths.push(OsString::from("."));
    }

    let expanded = paths::expand_paths(&options.paths, &cwd);
    if !expanded.errors.is_empty() {
        for error in &expanded.errors {
            say!(err, "Error: {error}");
        }
        return 2;
    }
    if expanded.files.is_empty() {
        if options.json {
            say!(out, "{}", report::json_report(&[], options.mode));
        } else {
            say!(out, "No Markdown files found.");
        }
        return 0;
    }

    let lib_options = options.library_options();
    let mut results = Vec::new();
    let mut io_error = false;
    for file in &expanded.files {
        let display = paths::display(&cwd, file);
        let original = match fileio::read_markdown_file(file) {
            Ok(text) => text,
            Err(message) => {
                say!(err, "Error: {display}: {message}");
                io_error = true;
                continue;
            }
        };
        let before = markdown_fixer::diagnose(&original, &lib_options);
        let fix = markdown_fixer::fix_document(&original, &lib_options);
        if options.mode == Mode::Write && fix.changed && !fix.refused {
            if let Err(message) = fileio::write_file_atomic(file, &fix.text) {
                say!(err, "Error: cannot write {display}: {message}");
                io_error = true;
                continue;
            }
        }
        results.push(evaluate(display, before, &fix, &options));
    }

    if !options.json && results.is_empty() {
        return if io_error { 2 } else { 0 };
    }
    let failing = report::is_failing(&results, options.strict);
    if options.json {
        say!(out, "{}", report::json_report(&results, options.mode));
    } else {
        let _ = report::write_text_report(out, &results, &options, io_error);
    }
    let code = if io_error {
        2
    } else if failing {
        1
    } else {
        0
    };
    finish(code, &options)
}

/// oracle `main` 迴圈中建立 `results` 元素的部分。
fn evaluate(
    display: String,
    before: Vec<Problem>,
    fix: &FixResult,
    options: &CliOptions,
) -> FileResult {
    let write_mode = options.mode == Mode::Write;
    let after = if fix.refused {
        before.clone()
    } else {
        markdown_fixer::diagnose(&fix.text, &options.library_options())
    };
    let original_fixable = before.iter().filter(|p| p.fixable).count();
    let remaining_fixable = after.iter().filter(|p| p.fixable).count();
    let fixed = if write_mode {
        original_fixable.saturating_sub(remaining_fixable)
    } else {
        0
    };
    let would_fix = if fix.refused { 0 } else { original_fixable };
    FileResult {
        display,
        changed: write_mode && fix.changed && !fix.refused,
        would_change: fix.changed && !fix.refused,
        refused: fix.refused,
        reason: fix.reason.clone(),
        shown: if write_mode { after } else { before },
        fixed,
        would_fix,
    }
}

/// 讀取整個標準輸入並解碼；失敗時回傳錯誤訊息。
fn read_stdin() -> Result<String, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    fileio::decode_markdown(bytes)
}

/// `--detect`：輸出 `markdown`／`not-markdown` 或 `Detection` JSON。
fn run_detect(options: &CliOptions, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let text = match read_stdin() {
        Ok(text) => text,
        Err(message) => {
            say!(err, "Error: {STDIN_DISPLAY}: {message}");
            return 2;
        }
    };
    let detection = markdown_fixer::detect::looks_like_markdown(&text);
    if options.json {
        match serde_json::to_string_pretty(&detection) {
            Ok(json) => say!(out, "{json}"),
            Err(error) => {
                say!(err, "Error: {error}");
                return 2;
            }
        }
    } else if detection.is_markdown {
        say!(out, "markdown");
    } else {
        say!(out, "not-markdown");
    }
    finish(if detection.is_markdown { 0 } else { 1 }, options)
}

/// `--stdin`：`--check` 時報告寫到 stdout；`--write` 時完整文字寫到 stdout、
/// 報告寫到 stderr（`--quiet` 時只在拒絕修正時印出原因）。
fn run_stdin(options: &CliOptions, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let original = match read_stdin() {
        Ok(text) => text,
        Err(message) => {
            say!(err, "Error: {STDIN_DISPLAY}: {message}");
            return 2;
        }
    };
    let lib_options = options.library_options();
    let before = markdown_fixer::diagnose(&original, &lib_options);
    let fix = markdown_fixer::fix_document(&original, &lib_options);
    let result = evaluate(STDIN_DISPLAY.to_string(), before, &fix, options);
    let results = [result];
    let failing = report::is_failing(&results, options.strict);

    if options.mode == Mode::Write {
        // stdout 一定是一份完整文件：修正後、未變更或拒絕修正時的原文。
        let text = if fix.changed && !fix.refused {
            &fix.text
        } else {
            &original
        };
        if out
            .write_all(text.as_bytes())
            .and_then(|()| out.flush())
            .is_err()
        {
            say!(
                err,
                "Error: cannot write {STDIN_DISPLAY} result to standard output."
            );
            return 2;
        }
        if options.quiet {
            if fix.refused {
                say!(err, "{STDIN_DISPLAY}: Not modified: {}", fix.reason);
            }
        } else if options.json {
            say!(err, "{}", report::json_report(&results, options.mode));
        } else {
            let _ = report::write_text_report(err, &results, options, false);
        }
    } else if options.json {
        say!(out, "{}", report::json_report(&results, options.mode));
    } else {
        let _ = report::write_text_report(out, &results, options, false);
    }
    finish(if failing { 1 } else { 0 }, options)
}
