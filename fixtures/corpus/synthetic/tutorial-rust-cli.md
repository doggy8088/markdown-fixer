# 用 Rust 寫第一支命令列工具

這篇教學帶你從零開始，用 Rust 寫一支會統計檔案行數的命令列工具 `wc-lite`。讀完之後，你會知道怎麼建立專案、解析參數、讀檔，以及寫測試。

## 事前準備

你需要：
1. 安裝 Rust（建議用 rustup）
2. 一個文字編輯器，VS Code 或 RustRover 都可以
3. 大約 30 分鐘

安裝 Rust：
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
裝好之後確認版本：

```
$ rustc --version
rustc 1.85.0 (4d91de4e4 2025-02-17)
```

## 步驟一：建立專案

```sh
cargo new wc-lite
cd wc-lite
```

Cargo 會幫你產生以下結構：

    wc-lite/
    ├── Cargo.toml
    └── src/
        └── main.rs

##步驟二：讀取參數

打開 `src/main.rs`，把內容換成：

```rust
use std::env;
use std::fs;

fn main() {
    let path = env::args().nth(1).expect("usage: wc-lite <file>");
    let text = fs::read_to_string(&path).unwrap();
    println!("{} lines", text.lines().count());
}
```

**說明：**
- `env::args()` 回傳一個 iterator，第 0 個是程式本身。
- `nth(1)` 取第一個參數。
  - 如果使用者沒給參數，`expect` 會讓程式 panic。
  - 正式的工具應該改用 `clap` 或自己印出用法。
- `read_to_string` 會把整個檔案讀進記憶體。

## 步驟三：執行

```sh
cargo run -- Cargo.toml
```

你應該會看到類似 `9 lines` 的輸出 。

### 常見錯誤

> **error[E0425]: cannot find value `path` in this scope**
>
>這通常是變數名稱打錯了。

>**warning: unused variable**
> 加上底線前綴 `_path` 可以消除警告。

## 步驟四：寫測試

把計算邏輯抽出來，才好測試：

```rust
fn count_lines(text: &str) -> usize {
    text.lines().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_lines() {
        assert_eq!(count_lines("a\nb\n"), 2);
    }
}
```

執行 `cargo test`，看到 `test result: ok.` 就代表成功了 。

## 下一步

- 支援多個檔案
- 加上 `--words` 統計字數
- 用 `clap` 解析參數
- 參考 The Rust Book：https://doc.rust-lang.org/book/
