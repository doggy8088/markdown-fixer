# AGENTS.md

給在這個 repo 工作的 AI coding agent 的指引。需求規格在 [`PRD.md`](PRD.md)，詳細文件在 [`docs/`](docs/)。所有文件以臺灣正體中文撰寫。

## 這是什麼

`markdown-fixer` 是 `markdown-autocorrect` skill 裡 `markdown-fixer.mjs` 的 Rust 移植版：一個安全、行導向的 Markdown 語法檢查與自動修正工具，提供函式庫、CLI 與 C ABI 共用函式庫。

最重要的一件事：**JS 參考實作（oracle）是行為的唯一標準。** `reference/js/markdown-fixer.mjs` 怎麼做，Rust 就怎麼做，連問題的順序、訊息文字、欄號都一樣（以 `render_guard = false` 比對；渲染保護是 Rust 專有的，見 PRD〈6.9〉）。看到 oracle 的行為「好像不合理」時，不要在 Rust 裡「順手修正」；先照做，再把觀察記到 `docs/parity.md` 的「oracle 疑似問題」清單。

oracle 的原始碼在 GitHub `doggy8088/skills` repo 的 `skills/markdown-autocorrect/`，不在這個 repo。要改行為時，JS 與 Rust 一起改：先改 skills repo 的 `scripts/markdown-fixer.mjs`（升 `VERSION`）、測試、`SKILL.md` 與 `reference/rules.md`，再複製到這裡的 `reference/`，重新產生 golden。完整步驟見 `docs/parity.md`〈Oracle 升版流程〉。

## Repo 結構

| 路徑 | 內容 |
| --- | --- |
| `crates/markdown-fixer/` | 純函式庫，不做 I/O。`diagnose`、`fix_document`、`rules`、`detect` |
| `crates/markdown-fixer-cli/` | CLI，執行檔名稱 `markdown-fixer`。參數解析照 oracle 手寫，不用 clap |
| `crates/markdown-fixer-ffi/` | C ABI（`cdylib`／`staticlib`）與 `include/markdown_fixer.h` |
| `reference/js/` | Oracle：`markdown-fixer.mjs` v1.1.0 與它的測試，從 `doggy8088/skills` 複製。**不要直接改**，改 skills repo 再複製 |
| `reference/rules.md`、`reference/SKILL.md` | 原 skill 的規則說明與使用說明，同樣從 skills repo 複製 |
| `fixtures/corpus/` | 接近真實的 Markdown 語料 |
| `fixtures/golden/` | 由 oracle 產生的預期輸出。**不要手改**，用 `scripts/gen-golden.mjs` 重新產生 |
| `scripts/` | `gen-golden.mjs`、`parity.sh`、`fuzz-parity.mjs` 等工具 |
| `examples/` | Karabiner-Elements、C 呼叫範例 |
| `docs/` | 架構、規則、CLI、函式庫、FFI、剪貼簿、測試、parity、除錯、發佈 |

JS 函式與 Rust 函式的對照表在 [`docs/porting-map.md`](docs/porting-map.md)。要改某個行為前，先在對照表找到對應的 JS 函式與行號。

## 常用指令

```sh
cargo build --workspace
cargo test --workspace --all-features        # 全部測試，含 golden 比對
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --workspace

node --test reference/js/markdown-fixer.test.mjs   # oracle 自己的測試
node scripts/gen-golden.mjs                        # 重新產生 golden 檔（只在語料或 oracle 改變時）
scripts/parity.sh <file-or-dir>...                 # 即時比對 oracle 與 Rust
node scripts/fuzz-parity.mjs --count 2000          # 隨機文件差異測試
```

`make help` 列出所有 Makefile 目標；`make check` 跑 CI 會跑的全部檢查。

多個 agent 同時在這個 repo 編譯時，用自己的 target 目錄避免互相等鎖：`CARGO_TARGET_DIR=target/<你的名字> cargo test`。

## 工作流程

### 移植或修改規則

1. 在 `docs/porting-map.md` 找到 JS 函式與行號，讀 JS 原始碼，不要憑規則名稱猜行為。
2. 先寫會失敗的測試：可以是移植的 JS 測試、golden 語料，或新的最小重現檔。
3. 實作，跑 `cargo test`。
4. 跑 `scripts/parity.sh fixtures/corpus`，確認沒有新的差異。
5. 更新 `docs/porting-map.md` 與相關文件。

### 修 parity 差異

1. 用 `scripts/parity.sh <file>` 找出第一個不一致的問題或第一個不同的位元組。
2. 縮小輸入，直到只剩幾行還能重現。
3. 把縮小後的輸入存成 `fixtures/corpus/regressions/<短名稱>.md`，跑 `node scripts/gen-golden.mjs` 產生 golden。
4. 修 Rust，直到 golden 測試通過。

### 常見的差異來源

- **欄號**：JS 用 UTF-16 index。Rust 若直接用 byte offset 或 `char` 數，遇到 CJK 或 emoji 就會錯。函式庫的行內處理一律在 UTF-16 code unit 上進行（`jsstr::JsString` = `Vec<u16>`），欄號因此天然與 JS 相同；不要在中途改用 `&str` 的 byte offset。
- **正規表示式**：函式庫不用 `regex` crate，而是 `jsregex` 模組（以 `&[u16]` 為輸入、JS 語意的小型回溯 regex VM，支援 lookahead／lookbehind）。從 JS 移植 regex 時用 `re!(r"...")` 原樣搬過來，不要改寫成 Rust `regex` 語法。
- **`\s` 與 `\w`**：JS 的 `\s` 包含 NBSP（U+00A0）、U+FEFF、U+3000 等；Rust `char::is_whitespace` 不含 U+FEFF。`\w` 在 JS 只有 ASCII。`\b` 也是 ASCII 字元邊界。
- **`split('\n')` 與 `lines()`**：JS `split` 會保留最後一個空字串；Rust `str::lines()` 不會。一律用 `split('\n')`。
- **`String.prototype.trim()`**：JS 會去掉 U+FEFF 與 Unicode 空白；Rust `trim()` 不去 U+FEFF。
- **排序**：JS `Array.prototype.sort()` 以 UTF-16 code unit 比較字串。
- **`toLowerCase()`** 與 Unicode case folding 的差異（reference label 正規化會用到）。
- **regex 的 lookbehind**：Rust `regex` 不支援，改寫成手寫掃描時特別注意邊界（字串開頭、前一個字元是 delimiter 本身）。

## 自我改進迴圈

- 每修一個 bug，就留下一個 regression 語料，讓同樣的錯不會回來。
- 發現 `AGENTS.md` 或 `docs/` 的說明錯誤、過時或讓你走錯路，同一個變更裡就修正它。
- 發現 oracle 疑似有 bug：Rust 照 oracle，把重現案例與分析記到 `docs/parity.md`，讓擁有者決定要不要修正原 skill。
- 跑 `scripts/parity.sh` 檢查 repo 外的真實文件（例如 `~/projects/*/README.md`、`~/projects/*/docs/`）找出語料沒涵蓋的差異。

## 程式碼規範

- 函式庫不得 panic。不要用 `unwrap()`／`expect()` 處理來自輸入的值；索引運算先確認在字元邊界上。
- 函式庫不做 I/O、不讀環境變數、不印東西。
- `unsafe` 只能出現在 `crates/markdown-fixer-ffi`，每一處都要有 `// SAFETY:` 註解。
- 新增相依前先問自己能不能不加。函式庫的相依要在 `docs/architecture.md` 說明理由。
- 公開 API 的簽章是 crate 之間的契約，改動時同步更新 `PRD.md`〈6.3〉與 `docs/library.md`。
- rustdoc 與註解用臺灣正體中文；規則訊息與 CLI 輸出保持英文（parity）。
- 用 `cargo fmt` 的格式。

## 測試要求

- 改到行為就要有測試；改到規則就要有 golden 語料。
- 不要為了讓測試通過而改 `fixtures/golden/`。golden 只能由 `scripts/gen-golden.mjs` 產生。
- 測試不能依賴網路、剪貼簿、GUI 或 Node。需要 Node 的 parity 工具放在 `scripts/`，不在 `cargo test` 裡。

## Markdown 寫作規則

這個 repo 的 Markdown 文件交付前：

- 粗體標籤寫成 `**標籤**：內容`，冒號放在粗體外。
- 中繼資料行用清單或空行分隔，不要只靠單一換行。
- 用本工具自己檢查：`cargo run -q -p markdown-fixer-cli -- --check PRD.md AGENTS.md docs/`，目標 0 error／warning。
- 用 `mhtml <file> /tmp/x.html --no-config --check` 確認沒有未配對強調警告。
- 有已知誤判時，在變更說明裡註明理由。

## Commit 規範

- 使用 Conventional Commits 1.0.0：`<type>[scope]: <description>`，描述用臺灣正體中文。
- 只在使用者要求時 commit 或 push。
- 不要 commit `target/`、暫存檔或含有本機絕對路徑的設定。

## 安全注意事項

- 不要修改 `~/.agents/skills/markdown-autocorrect/`（安裝版由維護者在審查後更新）。`reference/` 只能從 skills repo 複製，不要直接改（升版流程見 `docs/parity.md`）。
- 不要對整個 repo 跑 `markdown-fixer --write .`，會改到 golden 檔與 oracle。
- Karabiner 範例不得覆蓋使用者既有的 Karabiner 設定。
