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

5. 發佈到 crates.io（手動，名稱確認見 PRD〈11〉Q2）：

   ```sh
   cargo publish -p markdown-fixer
   cargo publish -p markdown-fixer-cli
   ```

   `markdown-fixer-ffi` 設了 `publish = false`，只透過 GitHub Release 散佈。

## 目標平台與產物

| Target | Runner | CLI | FFI |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | `macos-latest` | `.tar.gz` | `.a`、`.dylib` |
| `x86_64-apple-darwin` | `macos-latest`（交叉編譯） | `.tar.gz` | `.a`、`.dylib` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `.tar.gz` | `.a`、`.so` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` | `.a`、`.so` |
| `x86_64-unknown-linux-musl` | `ubuntu-latest` | `.tar.gz` | `.a` |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` | `.tar.gz` | `.a` |
| `x86_64-pc-windows-msvc` | `windows-latest` | `.zip` | `.lib`、`.dll`、`.dll.lib` |
| `aarch64-pc-windows-msvc` | `windows-11-arm` | `.zip` | `.lib`、`.dll`、`.dll.lib` |

每個目標產生兩個壓縮檔：

- `markdown-fixer-<版本>-<target>`：`markdown-fixer` 執行檔，以及 `README.md`、`LICENSE`（存在時）。
- `markdown-fixer-ffi-<版本>-<target>`：`include/markdown_fixer.h` 與 `lib/` 下的函式庫。

說明：

- **musl 沒有共用函式庫**：musl 目標預設 `crt-static`，rustc 會略過 `cdylib`，只產生 staticlib。需要 musl 的 `.so` 時，以 `RUSTFLAGS="-C target-feature=-crt-static"` 自行建置。
- **macOS dylib 的 install name**：打包時以 `install_name_tool -id @rpath/libmarkdown_fixer_ffi.dylib` 改寫，使用端連結時設定 rpath 即可。
- **Windows**：`markdown_fixer_ffi.dll.lib` 是 DLL 的匯入程式庫，`markdown_fixer_ffi.lib` 是靜態函式庫，兩者不要混用。
- release profile 已開啟 `lto`、`strip`、`codegen-units = 1`（根目錄 `Cargo.toml`）。PRD〈7〉要求 CLI 小於 3 MB。

## 驗證下載

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
```

## 後續

shell／PowerShell installer、Homebrew tap 與 npm 包裝列為後續工作（PRD〈9〉），做法參考 `mhtml` 與 `dedent-paste`。目前的 workflow 沿用 `mhtml` 的手寫 matrix，沒有使用 cargo-dist；之後若改用 cargo-dist，要確認它能同時打包 FFI 函式庫與標頭檔。
