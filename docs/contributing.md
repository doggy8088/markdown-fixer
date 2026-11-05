# 貢獻指南

謝謝你願意改進 `markdown-fixer`。這個專案的核心要求只有一個：**Rust 版的行為必須和 JS oracle 逐位元組一致**。下面的流程與關卡都是為了守住這件事。

AI coding agent 請同時閱讀 repo 根目錄的 [`AGENTS.md`](../AGENTS.md)，裡面有給 agent 的完整規範。

## 開發環境

- Rust 1.85 以上（edition 2024），含 `rustfmt` 與 `clippy`。
- Node.js 20 以上：只有 parity 工具與重新產生 golden 檔需要，`cargo test` 不需要。
- `mhtml`：檢查文件有沒有未配對的強調符號。

```sh
cargo build --workspace
cargo test --workspace --all-features
make help        # 列出所有 Makefile 目標
```

多人或多個 agent 同時編譯時，各自指定 target 目錄，避免互相等鎖：

```sh
CARGO_TARGET_DIR=target/<你的名字> cargo test --workspace --all-features
```

## 工作流程

### 修改或移植規則

1. 在 [porting-map.md](porting-map.md) 找到對應的 JS 函式與行號，讀 `reference/js/markdown-fixer.mjs` 的原始碼。不要憑規則名稱猜行為。
2. 先寫會失敗的測試：移植 JS 測試、新增 golden 語料，或最小重現檔。
3. 實作，跑 `cargo test`。
4. 跑 `scripts/parity.sh fixtures/corpus`，確認沒有新的差異。
5. 更新 [porting-map.md](porting-map.md) 與相關文件。

### 修 parity 差異

照 [debugging.md](debugging.md) 的流程：重現、縮小、存成 `fixtures/corpus/regressions/<短名稱>.md`、執行 `node scripts/gen-golden.mjs`、修 Rust 到 golden 測試通過。

### 發現 oracle 疑似有 bug

Rust 照 oracle 做，不要順手修正。把重現案例與分析寫進 [parity.md](parity.md) 的「oracle 疑似問題」清單，由擁有者決定要不要修改原 skill。

## 品質關卡

送出變更前，下列檢查都要通過。`make check` 會跑完前五項（也就是 CI 跑的項目，不需要 Node）：

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --no-deps --workspace
make md-check                 # 用自己的執行檔檢查 repo 的 Markdown 文件
```

需要 Node 的 parity 檢查另外跑：

```sh
make parity                   # scripts/parity.sh fixtures/corpus
make golden-check             # golden 檔與 oracle 是否一致
make fuzz COUNT=2000          # 隨機文件差異測試
```

測試的規定：

- 改到行為就要有測試；改到規則就要有 golden 語料。
- 每修一個 bug，就留一個 regression 語料。
- `fixtures/golden/` 只能由 `scripts/gen-golden.mjs` 產生，不要手改，也不要為了讓測試通過去改。
- 測試不能依賴網路、剪貼簿、GUI 或 Node。

測試架構的細節見 [testing.md](testing.md)。

## 程式碼規範

- 函式庫不得 panic。來自輸入的值不用 `unwrap()`、`expect()`；切字串前先確認在字元邊界上。
- 函式庫不做 I/O、不讀環境變數、不印東西。
- `unsafe` 只能出現在 `crates/markdown-fixer-ffi`，每一處都要有 `// SAFETY:` 註解。
- 新增相依前先問能不能不加。函式庫的相依要在 [architecture.md](architecture.md#相依政策) 說明理由。
- 公開 API 的簽章是 crate 之間的契約。改動時同步更新 `PRD.md`〈6.3〉與 [library.md](library.md)。
- 格式以 `cargo fmt` 為準。

## 語言

- 文件、rustdoc、程式碼註解：臺灣正體中文。
- 規則訊息、CLI 輸出（`--help`、摘要行、錯誤訊息）：英文，和 oracle 逐字相同。改了就破壞 parity。
- 用語以臺灣慣用的技術詞彙為準，例如「程式碼」「檔案」「預設」「執行檔」「支援」。

## 文件寫作規則

這個 repo 的 Markdown 文件交付前：

- 粗體標籤寫成 `**標籤**：內容`，冒號放在粗體外面。
- 中繼資料行用清單或空行分隔，不要只靠單一換行。
- 示範「壞掉的 Markdown」時，放進 fenced code block，文件本身才能通過檢查。範例本身含 ```` ``` ```` 時，外層改用 `~~~` 或四個 backtick。
- 不要在文件裡直接貼 BOM、NBSP、零寬字元或 tab，改用 `U+00A0` 這類寫法描述。
- 用本工具自己檢查，目標 0 error、0 warning：

```sh
cargo run -q -p markdown-fixer-cli -- --check PRD.md AGENTS.md README.md docs/
```

- 用 `mhtml` 確認沒有未配對強調的警告：

```sh
mhtml docs/rules.md /tmp/x.html --no-config --check
```

有已知誤判時，在變更說明裡註明理由。

## Commit 規範

使用 [Conventional Commits 1.0.0](https://www.conventionalcommits.org/zh-hant/v1.0.0/)：`<type>[scope]: <description>`，描述用臺灣正體中文。

```text
feat(cli): 新增 --stdin 模式
fix(core): MDX042 欄號改用 UTF-16 計算
docs: 補充 detect 的調整流程
test(corpus): 新增 CRLF 與 BOM 混合的 regression 語料
```

常用的 type：`feat`、`fix`、`docs`、`test`、`refactor`、`perf`、`chore`、`ci`。

其他規定：

- 不要 commit `target/`、暫存檔，或含有本機絕對路徑的設定。
- 不要修改 `reference/`。oracle 升版的流程見 [parity.md](parity.md)。
- 不要對整個 repo 執行 `markdown-fixer --write .`，會改到 golden 檔與 oracle。
- 發現 `AGENTS.md` 或 `docs/` 的說明錯誤、過時，在同一個變更裡修正。
