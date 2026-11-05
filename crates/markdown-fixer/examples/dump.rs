//! parity 工具用的傾印程式：以 JSON 輸出 `diagnose` 與 `fix_document` 的結果。
//!
//! 需要 `serde` feature：
//!
//! ```sh
//! # 單一文件：從 stdin 讀 Markdown，第一個參數（可省略）是選項 JSON
//! cargo run -q -p markdown-fixer --example dump --features serde -- '{"maxLineLength":80}' < doc.md
//!
//! # 批次（NDJSON）：stdin 每行是 {"text": "...", "options": {...}}，stdout 每行輸出一個結果
//! cargo run -q -p markdown-fixer --example dump --features serde -- --batch < cases.ndjson
//! ```
//!
//! 輸出格式為 `{"diagnose":[...],"fix":{...}}`，鍵名與 JS oracle 相同；函式庫 panic 時
//! 輸出 `{"panic":"<訊息>"}`，讓批次比對不會因為單一文件中斷。選項 JSON 的鍵名與 JS
//! options 相同：`rules`、`ignoreRules`、`maxLineLength`。

#[cfg(not(feature = "serde"))]
fn main() {
    eprintln!(
        "dump 需要 serde feature：cargo run -p markdown-fixer --example dump --features serde"
    );
    std::process::exit(2);
}

#[cfg(feature = "serde")]
fn main() {
    std::process::exit(real::main());
}

#[cfg(feature = "serde")]
mod real {
    use std::io::{self, BufRead, Read, Write};
    use std::panic::{self, AssertUnwindSafe};

    use markdown_fixer::{DEFAULT_MAX_QUOTE_DEPTH, LimitPolicy, Options, diagnose, fix_document};
    use serde_json::{Value, json};

    fn string_list(value: Option<&Value>, key: &str) -> Result<Option<Vec<String>>, String> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{key} must be an array of strings"))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            Some(_) => Err(format!("{key} must be an array of strings")),
        }
    }

    /// 把 JS 風格的選項 JSON 轉成 `Options`。`maxLineLength` 不是正數時視為 0（停用 MDX090），
    /// 與 JS `options.maxLineLength > 0` 的判斷相同。
    pub fn parse_options(value: &Value) -> Result<Options, String> {
        if value.is_null() {
            return Ok(Options::default());
        }
        let object = value.as_object().ok_or("options must be a JSON object")?;
        let max_line_length = match object.get("maxLineLength") {
            None | Some(Value::Null) => 0,
            Some(number) => {
                let n = number.as_f64().ok_or("maxLineLength must be a number")?;
                if n > 0.0 { n as usize } else { 0 }
            }
        };
        // JS `createLimits`：非負數取整數部分，其他值用預設 32；`onLimit` 只認 "ignore"。
        let max_quote_depth = match object.get("maxQuoteDepth") {
            None | Some(Value::Null) => DEFAULT_MAX_QUOTE_DEPTH,
            Some(number) => match number.as_f64() {
                Some(n) if n >= 0.0 => n as usize,
                _ => DEFAULT_MAX_QUOTE_DEPTH,
            },
        };
        let on_limit = match object.get("onLimit").and_then(Value::as_str) {
            Some("ignore") => LimitPolicy::Ignore,
            _ => LimitPolicy::Error,
        };
        Ok(Options {
            rules: string_list(object.get("rules"), "rules")?,
            ignore_rules: string_list(object.get("ignoreRules"), "ignoreRules")?,
            max_line_length,
            max_quote_depth,
            on_limit,
            // parity 工具：關閉 Rust 專有的渲染保護，行為與 JS 參考實作相同。
            render_guard: false,
        })
    }

    fn run(text: &str, options: &Options) -> Value {
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            (diagnose(text, options), fix_document(text, options))
        }));
        match outcome {
            Ok((problems, fix)) => json!({ "diagnose": problems, "fix": fix }),
            Err(payload) => {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "non-string panic payload".to_owned());
                json!({ "panic": message })
            }
        }
    }

    fn batch() -> i32 {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut out = io::BufWriter::new(stdout.lock());
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(line) => line,
                Err(error) => {
                    eprintln!("Error: cannot read stdin: {error}");
                    return 2;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let result = match serde_json::from_str::<Value>(&line) {
                Ok(case) => match (
                    case.get("text").and_then(Value::as_str),
                    parse_options(case.get("options").unwrap_or(&Value::Null)),
                ) {
                    (Some(text), Ok(options)) => run(text, &options),
                    (None, _) => json!({ "error": "case needs a string \"text\"" }),
                    (_, Err(error)) => json!({ "error": error }),
                },
                Err(error) => json!({ "error": format!("invalid JSON: {error}") }),
            };
            if writeln!(out, "{result}")
                .and_then(|()| out.flush())
                .is_err()
            {
                return 2;
            }
        }
        0
    }

    pub fn main() -> i32 {
        // panic 訊息改由 JSON 回傳，不要洗版 stderr。
        panic::set_hook(Box::new(|_| {}));
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.first().map(String::as_str) == Some("--batch") {
            return batch();
        }
        let options = match args.first() {
            None => Options::default(),
            Some(raw) => match serde_json::from_str::<Value>(raw)
                .map_err(|e| e.to_string())
                .and_then(|v| parse_options(&v))
            {
                Ok(options) => options,
                Err(error) => {
                    eprintln!("Error: invalid options JSON: {error}");
                    return 2;
                }
            },
        };
        let mut bytes = Vec::new();
        if let Err(error) = io::stdin().read_to_end(&mut bytes) {
            eprintln!("Error: cannot read stdin: {error}");
            return 2;
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                eprintln!("Error: stdin is not valid UTF-8");
                return 2;
            }
        };
        println!("{}", run(&text, &options));
        0
    }
}
