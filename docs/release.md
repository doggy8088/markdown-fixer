# 發佈流程

規格見 PRD〈9. 發佈與散佈〉。發佈由擁有者執行；agent 不 commit、不 push、不建立 tag。

## 版本號

- 採 SemVer。三個 crate 共用 workspace 版本：根目錄 `Cargo.toml` 的 `[workspace.package] version`。
- `crates/*/Cargo.toml` 之間的 path 相依也標了 `version = "..."`（發佈到 crates.io 時需要），升版時要一起改。
- `REFERENCE_VERSION`（`crates/markdown-fixer/src/lib.rs`）是對齊的 oracle 版本，與本專案版本無關。oracle 升版時才改，並重新產生 golden 檔，流程見 [parity.md](parity.md)。

## 發佈前檢查

```sh
make check            # fmt、clippy、測試、rustdoc、本 repo 的 Markdown 檢查
make golden-check     # 函式庫與 CLI 的 golden 檔與 oracle 一致（需要 Node）
make parity           # 語料即時比對（需要 Node）
make fuzz COUNT=10000 # 隨機文件差異測試（需要 Node）
make ffi-example      # C 範例
```

CI（`.github/workflows/ci.yml`）在 push 到 `main` 或 `ci/**` 分支，以及每個 PR 時跑同樣的檢查：

| Job | 內容 |
| --- | --- |
| `lint` | `cargo fmt --check`、`clippy -D warnings`、`cargo doc`（`-D warnings`） |
| `test` | ubuntu、macOS、Windows 上的 `cargo test --workspace --all-features`；非 Windows 另外跑剪貼簿腳本測試（含端對端）；Linux 上用自己的執行檔檢查本 repo 的 Markdown |
| `msrv` | 以 Rust 1.85 建置 |
| `shellcheck` | 範例腳本的 shellcheck 與 Karabiner JSON 驗證 |
| `parity` | 有 Node 的環境：oracle 自己的測試、`gen-golden.mjs --check`、`gen-cli-golden.mjs --check`、`parity.sh fixtures/corpus`、`fuzz-parity.mjs --count 2000 --seed 1 --no-save` |

`.github/workflows/clipboard.yml` 在 `examples/**`、`crates/markdown-fixer-cli/**` 或 workflow 本身變動時，於 Linux（X11 與 Wayland）、Windows、macOS runner 上以真正的剪貼簿測試剪貼簿腳本，也可以手動觸發。驗證範圍見 [`clipboard.md`](clipboard.md)〈CI 驗證〉。

## 發佈步驟

1. 更新版本號（workspace 版本與 crate 之間的相依版本），執行 `cargo check` 更新 `Cargo.lock`。
2. 跑完上面的發佈前檢查。
3. commit 後建立 tag 並推送：

   ```sh
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```

4. `.github/workflows/release.yml` 會：
   1. 確認 tag 與 `Cargo.toml` 的版本一致，不一致就失敗。
   2. 在每個目標平台建置 CLI 與 FFI 函式庫，執行一次冒煙測試（`--version` 與 `--stdin --write`）。
   3. 打包並為每個檔案產生 `.sha256`，最後彙整成 `SHA256SUMS`。
   4. 建立 GitHub Release，附上所有檔案與自動產生的 release notes。

5. `.github/workflows/publish.yml` 會以 crates.io Trusted Publishing（GitHub OIDC）依序發佈 `markdown-fixer` 與 `markdown-fixer-cli`，不需要長期 API token；已經在 crates.io 上的版本會跳過。`markdown-fixer-ffi` 設了 `publish = false`，只透過 GitHub Release 散佈。

## crates.io Trusted Publishing

crates.io 只能替已經存在的 crate 設定 trusted publisher，而且每個 crate 要各設一筆。0.1.0 已照下面的流程完成：2026-11-06 以 API token 發佈兩個 crate，再替兩者設定 trusted publisher；`v0.1.0` tag 觸發的 `publish.yml` 已成功以 OIDC 取得 token，並跳過已發佈的版本。第一版的流程是：

1. **第一次發佈用 API token**：在 crates.io 的 Account Settings → API Tokens 建立 token（scope 選 `publish-new` 與 `publish-update`，crate 名稱限制 `markdown-fixer*`），`cargo login` 後在 repo 根目錄執行：

   ```sh
   cargo publish -p markdown-fixer
   cargo publish -p markdown-fixer-cli
   ```

   發佈完就可以撤銷這個 token。

2. **設定 trusted publisher**：在 crates.io 分別打開兩個 crate 的 Settings → Trusted Publishing，新增 GitHub publisher：

   | 欄位 | 值 |
   | --- | --- |
   | Repository owner | `doggy8088` |
   | Repository name | `markdown-fixer` |
   | Workflow filename | `publish.yml` |
   | Environment | `release` |

3. **GitHub environment**：repo 已建立 `release` environment，部署規則只允許 `v*` tag。`publish.yml` 的 job 使用這個 environment，並以 `permissions: id-token: write` 向 crates.io 換取短期 token（`rust-lang/crates-io-auth-action@v1`，job 結束時自動撤銷）。

4. **之後的版本**：bump 版本、推送 tag，`release.yml` 建置 GitHub Release，`publish.yml` 發佈到 crates.io。

5. **選用**：確認 OIDC 發佈正常後，可以在兩個 crate 的 Settings → Trusted Publishing 勾選「Require trusted publishing for all new versions」，之後就不能再用 API token 發佈新版本。

## 後續

shell／PowerShell installer、Homebrew tap 與 npm 包裝列為後續工作（PRD〈9〉），做法參考 `mhtml` 與 `dedent-paste`。目前的 workflow 沿用 `mhtml` 的手寫 matrix，沒有使用 cargo-dist；之後若改用 cargo-dist，要確認它能同時打包 FFI 函式庫與標頭檔。
