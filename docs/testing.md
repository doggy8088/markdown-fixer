# 測試

這個專案的測試分成好幾層，目的都一樣：證明 Rust 版與 JS oracle 行為一致，而且在任何輸入下都安全。一致性的定義與 golden 格式見 [parity.md](parity.md)。

## 測試層級一覽

| 層級 | 位置 | 需要 Node | 驗證什麼 |
| --- | --- | --- | --- |
| 移植的 JS 測試 | `crates/markdown-fixer/tests/js_ported.rs` | 否 | oracle 自己的 42 個測試案例 |
| Golden 語料 | `crates/markdown-fixer/tests/golden.rs` | 否 | 語料每份文件、五組選項的 `diagnose` 與 `fix_document` |
| Property test | `crates/markdown-fixer/tests/properties.rs` | 否 | 不 panic、冪等、拒絕時不改、編碼保留、病態輸入的時間上限 |
| 內部單元測試 | `crates/markdown-fixer/src/**` | 否 | `scan`、`mask`、`table` 等內部函式（core 負責） |
| CLI 測試 | `crates/markdown-fixer-cli/tests/` | 否 | 旗標、結束碼、輸出、與 oracle CLI 的 golden 比對 |
| FFI 測試 | `crates/markdown-fixer-ffi/` | 否 | C ABI 與 C 範例 |
| 即時差異比對 | `scripts/parity.sh` | 是 | 任意檔案或目錄，oracle 與 Rust 逐一比對 |
| 差異模糊測試 | `scripts/fuzz-parity.mjs` | 是 | 隨機文件批次比對，縮小差異並存成 regression |

`cargo test` 不依賴網路、剪貼簿、GUI 或 Node；需要 Node 的工具都放在 `scripts/`。

## 常用指令

```sh
cargo test --workspace --all-features                  # 全部 Rust 測試
cargo test -p markdown-fixer --test golden             # 只跑 golden 比對
cargo test -p markdown-fixer --test js_ported          # 只跑移植的 JS 測試
PROPTEST_CASES=5000 cargo test -p markdown-fixer --test properties

node --test reference/js/markdown-fixer.test.mjs       # oracle 自己的測試
node scripts/gen-golden.mjs                            # 重新產生 golden（含 CLI golden）
node scripts/gen-golden.mjs --check                    # 檢查 golden 是否過期
scripts/parity.sh fixtures/corpus                      # 即時比對整個語料
node scripts/fuzz-parity.mjs --count 2000 --seed 7     # 隨機文件差異測試
```

多個 agent 同時編譯時，用自己的 target 目錄：`CARGO_TARGET_DIR=target/<名字> cargo test`。`parity.sh` 與 `fuzz-parity.mjs` 也讀 `CARGO_TARGET_DIR`（預設 `target/parity`）。

## 移植的 JS 測試

`js_ported.rs` 依原檔順序移植 `reference/js/markdown-fixer.test.mjs` 的 42 個案例，每個測試上方的註解寫著原始案例名稱與編號。

- 第 25、26、31 個案例測的是內部函式（`buildMask`、`splitTableCells`、`scanDocument`），沒有公開 API。這裡改用 `fix_document` 或 `diagnose` 觀察同樣的效果，每一條代替用的斷言都先用 oracle 驗證過；內部函式本身由 core 的單元測試負責。
- 第 28、29、30 個案例裡讀取 `scanDocument(...).info` 的斷言同樣屬於內部狀態，其他斷言全部移植。
- 第 32～38 個案例是 `parseArgs` 與實際執行 CLI 的測試，放在 `crates/markdown-fixer-cli/tests`。

## Golden 語料

語料放在 `fixtures/corpus/`，每份文件涵蓋什麼列在 [fixtures/INDEX.md](../fixtures/INDEX.md)。golden 由 `scripts/gen-golden.mjs` 以 oracle 產生，**不可以手改**。

`golden.rs` 讀取每個 golden 檔，以 Rust API 跑同樣的輸入與選項，全部跑完才判定失敗。失敗訊息包含：

- 通過與失敗的案例數。
- 每個失敗案例的來源檔、選項名稱與選項 JSON。
- 第一個不同的問題（索引、oracle 與 Rust 各自的內容），或修正後文字第一個不同的行。
- 重現指令：`scripts/parity.sh` 與 `cargo run --example dump`。

只想看某些檔案時，設定 `MDFIX_GOLDEN_FILTER=<路徑子字串>`。另外兩個測試檢查規則目錄與 `fixtures/golden/rules.json` 相同，以及每份語料都有對應的 golden。

`fixtures/.gitattributes` 設定 `* -text`，讓 git 不要改寫語料的 CRLF 與 BOM。

## Property test

`properties.rs` 用 proptest 產生兩種輸入：任意 Unicode 字串，以及由 Markdown 片段、CJK、astral 字元、不可見字元與混合行尾組成的「像 Markdown」文件。驗證的性質：

- `diagnose` 與 `fix_document` 不 panic；`diagnose` 結果具決定性，行號與欄號至少為 1。
- 拒絕修正時輸出等於輸入、`changed` 為 false、`reason` 不是空字串；沒拒絕時 `changed` 等於「輸出不等於輸入」。
- `fix_document` 冪等。oracle 的唯一已知例外記錄在 [parity.md](parity.md) 的〈oracle 疑似問題〉第 7 項，測試排除該情況。
- 停用 MDX001／MDX002 時保留原本整串 BOM，行尾還原成原文的行尾（CRLF 優先，其次 CR）；啟用時輸出沒有檔首 BOM 與 CR。
- 病態輸入在時間上限內完成：200 層引言、200 層清單、每種分隔符號 1 萬字元的長行、2 萬行空行等。上限預設 debug 10 秒、release 2 秒，可用 `MDFIX_PATHOLOGICAL_BUDGET_MS` 調整。

proptest 找到的失敗種子存在 `crates/markdown-fixer/tests/properties.proptest-regressions`，請一併 commit。

如果冪等 property 失敗，先用 oracle 跑同一份輸入。oracle 也不冪等時，把它記到 parity.md 並在測試中排除；只有 Rust 不冪等時才是 Rust 的 bug。

## 即時差異比對：parity.sh

```sh
scripts/parity.sh README.md docs/
PARITY_OPTIONS=default scripts/parity.sh ~/projects/*/README.md
```

目錄的展開方式與 oracle CLI 相同。每個檔案跑五組選項，列出第一個差異，最後印出摘要；有差異時結束碼 1。可以用它檢查 repo 外的真實文件，找出語料沒涵蓋的情況。

## 差異模糊測試：fuzz-parity.mjs

```sh
node scripts/fuzz-parity.mjs --count 10000 --seed 20261105
node scripts/fuzz-parity.mjs --idempotence --count 5000      # 只跑 oracle，統計不冪等的情況
```

文法涵蓋標題、段落、`**標籤：**內容`、清單與任務清單、巢狀引言、fence（含未關閉與非法 info string）、表格、`$` 與 `$$` 數學式、raw HTML 與註解、frontmatter、MDX／JSX 行、全形語法、CJK、emoji 與其他 astral 字元、NBSP、零寬字元、混合行尾、BOM、缺少檔尾換行。同一個 `--seed` 永遠產生同一批文件。

找到差異時會自動縮小（先刪行、再刪字元），印出縮小後的輸入與第一個差異，並存成 `fixtures/corpus/regressions/fuzz-s<種子>-d<編號>.md`。加 `--no-save` 不存檔。

## 新增 regression

每修一個 bug，就留下一份 regression 語料：

1. 用 `scripts/parity.sh <檔案>` 或 `fuzz-parity.mjs` 找到差異。
2. 縮小輸入到只剩幾行還能重現，存成 `fixtures/corpus/regressions/<短名稱>.md`。檔名用小寫與連字號，例如 `mdx042-closing-colon.md`；記錄 oracle 疑似問題的檔案以 `oracle-` 開頭。
3. 需要精確位元組（BOM、CRLF、NBSP、沒有檔尾換行）時，用 Node 寫檔，例如 `node -e "require('fs').writeFileSync(path, '\uFEFF#x\r\n')"`，寫完用 `od -c` 確認。
4. 執行 `node scripts/gen-golden.mjs` 產生 golden，在 [fixtures/INDEX.md](../fixtures/INDEX.md) 補一行說明。
5. 修 Rust，直到 `cargo test -p markdown-fixer --test golden` 通過。

新增規則層級的案例時，放在 `fixtures/corpus/rules/<規則 ID>/trigger*.md` 或 `negative*.md`，`gen-golden.mjs` 會自動檢查前者會回報、後者不會。
