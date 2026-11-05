# CLI 使用說明

`markdown-fixer` 是 Rust 版的命令列工具，執行檔名稱 `markdown-fixer`，原始碼在 `crates/markdown-fixer-cli/`。它的旗標、輸出文字、`--json` 結構與結束碼都和 JS oracle（`reference/js/markdown-fixer.mjs`）逐位元組相同；另外加上三個 Rust 版專有的旗標：`--stdin`、`--exit-zero`、`--detect`。規格見 PRD〈6.4〉。

## 用法

```text
markdown-fixer [options] <file-or-directory>...
```

- 不給路徑時檢查目前目錄（`.`）。
- `--write` 一定要明確給路徑，不會隱式改寫整個工作目錄。
- 參數順序不重要：旗標可以放在路徑後面。`--` 之後的參數一律當成路徑，例如 `markdown-fixer -- --weird-name.md`。

常見的三種用法：

```sh
markdown-fixer --check README.md docs/      # 只報告
markdown-fixer --write README.md AGENTS.md  # 原地修正
markdown-fixer --check --json .             # 給程式解析的 JSON
```

## 旗標參考

### 與 oracle 相同的旗標

| 旗標 | 說明 |
| --- | --- |
| `--check` | 只報告，不寫檔。這是預設模式 |
| `--write` | 原地套用安全修正。沒給路徑時以結束碼 2 拒絕 |
| `--json` | 輸出 JSON，格式見〈JSON 結構〉 |
| `--quiet` | 不列出每個檔案的問題，只印摘要行 |
| `--strict` | info 等級的問題也算失敗 |
| `--rule <ids>`、`--rule=<ids>` | 只啟用這些規則。以逗號分隔，可重複 |
| `--ignore-rule <ids>`、`--ignore-rule=<ids>` | 停用這些規則。以逗號分隔，可重複 |
| `--max-line-length N`、`--max-line-length=N` | 啟用 MDX090，行長上限為 N |
| `--max-quote-depth N`、`--max-quote-depth=N` | 引言巢狀深度上限，預設 32，0 表示不限制 |
| `--on-limit <mode>`、`--on-limit=<mode>` | 超過上限時的處理方式：`error`（預設）或 `ignore`，見〈資源上限〉 |
| `--list-rules` | 列出規則目錄後結束 |
| `--version`、`-v` | 印出 `markdown-fixer <版本>` 後結束 |
| `--help`、`-h` | 印出說明後結束 |
| `--` | 之後的參數都是路徑 |

`--rule` 指定了 MDX090 但沒有給 `--max-line-length` 時，行長上限預設為 120。MDX090 平常是停用的，只有給了行長上限才會檢查。

### Rust 版專有的旗標

| 旗標 | 說明 |
| --- | --- |
| `--stdin` | 從標準輸入讀取 Markdown，顯示名稱為 `<stdin>`。不能和路徑一起使用 |
| `--exit-zero` | 除了結束碼 2（用法或 I/O 錯誤），一律以 0 結束 |
| `--detect` | 只判斷標準輸入是不是 Markdown，印出 `markdown` 或 `not-markdown` |
| `--no-render-guard` | 關閉渲染保護，套用所有安全修正，即使會改變渲染結果。輸出與 oracle 逐位元組相同 |

`--help` 把這四個旗標列在獨立的 `Rust-only options` 區塊，oracle 原有的說明行維持不變。

### 參數解析的細節

解析器是照 oracle `parseArgs` 手寫的迴圈，沒有用 clap。幾個容易意外的行為：

- 參數依序處理，第一個錯誤就停止。所以 `--help --bogus` 是用法錯誤（結束碼 2），不會印出說明。
- 錯誤處理完之後，優先順序是 `--help`、`--version`、`--list-rules`。
- `--rule` 與 `--ignore-rule` 一定會吃掉下一個參數，即使它看起來像旗標：`--rule --json` 的錯誤是 `Unknown rule: --json`。
- 規則 ID 以 JS `trim()` 去掉前後空白（包含 NBSP 與 U+FEFF），空項目略過。全部是空項目時回報 `A rule list is required after --rule/--ignore-rule.`。
- `--max-line-length` 的值以 JS `Number()` 解析：` 80 `、`0x50`、`1e2`、`80.0` 都合法；`8.5`、`0`、空字串與缺少的值都會回報 `--max-line-length requires a positive integer.`。
- `--max-quote-depth` 的值必須是 1 到 9 個 ASCII 數字（`0` 可以，`007` 是 7）。`-1`、`1e2`、`0x10`、` 5`、空字串與缺少的值都會回報 `--max-quote-depth requires a non-negative integer.`。
- `--on-limit` 只接受 `error` 與 `ignore`，大小寫有別；其他值或缺少的值回報 `--on-limit requires error or ignore.`。
- 重複的 `--max-quote-depth`、`--on-limit` 以最後一個為準。
- 單獨的 `-` 與 `--check=yes` 這類寫法是未知旗標。

用法錯誤的輸出固定是兩行，寫到 stderr，結束碼 2：

```text
Error: Unknown option: --bogus
Run with --help for usage.
```

## 路徑與目錄掃描

- **直接給的路徑**：跟隨 symlink，不看副檔名。`markdown-fixer notes.txt` 會檢查這個 `.txt` 檔。直接給排除清單裡的目錄（例如 `node_modules`）也會掃描。
- **目錄裡的項目**：只收副檔名為 `.md` 或 `.markdown` 的檔案（ASCII 不分大小寫，`README.MD` 也算）。`.mdx` 不算。目錄裡的 symlink 一律略過，不論指向檔案或目錄。
- **排除的目錄名稱**：`.git`、`.hg`、`.svn`、`node_modules`、`vendor`、`coverage`、`test-results`、`playwright-report`、`blob-report`、`.e2e-tmp`，在任何深度都略過。
- **去重與排序**：所有檔案先轉成絕對路徑（字面運算，不解析 symlink），去掉重複，再依 UTF-16 code unit 排序，與 JS `Array.prototype.sort()` 相同。例如 `😀.md` 會排在 `～.md`（U+FF5E）前面，與 byte 排序的結果相反。
- **顯示路徑**：相對於目前工作目錄，等同 Node `path.relative(process.cwd(), file)`。工作目錄以外的檔案會顯示成 `../README.md`。
- **找不到 Markdown 檔**：印出 `No Markdown files found.`（`--json` 時印出空的 JSON 結構），結束碼 0。

任何一個路徑無法存取時，CLI 先印出全部錯誤再以結束碼 2 結束，不檢查任何檔案。錯誤訊息照 Node 的格式：

```text
Error: Cannot access missing.md: ENOENT: no such file or directory, stat 'missing.md'
Error: Cannot read directory private: EACCES: permission denied, scandir 'private'
```

## 讀檔與寫檔

- 檔案含有 NUL 位元組時拒絕處理：`file contains NUL bytes; refusing to treat it as text`。
- 檔案不是合法 UTF-8 時拒絕處理：`The encoded data was not valid for encoding utf-8`。
- 這兩種錯誤只影響該檔案：其他檔案照常檢查與修正，但最後多印一行 `Some files could not be read or written; see the errors above.`，結束碼 2。
- 檔首的 BOM 保留在文字裡交給函式庫處理（由 MDX001 決定是否移除）。
- `--write` 採原子寫入：先解析 symlink 找到真正的檔案，在同一個目錄寫入暫存檔 `<檔名>.<pid>.<毫秒>.tmp`，套用原檔權限後 rename 蓋過原檔。透過 symlink 寫入時，symlink 本身維持不變。
- 只有內容真的改變、且沒有被拒絕修正的檔案才會寫入。

## 文字輸出

每個檔案一個區塊，最後是摘要行。`--quiet` 只省略檔案區塊，摘要行照印。

```text
README.md
  1:2        warning MDX010  Add a space after the heading marker: "# " rather than "#". (fixable)
  3:7        info    MDX070  Bare URL. Wrap it in <...> or use an explicit link only when that improves readability; otherwise leave it as text.

2 problem(s) in 1 file(s): 1 fixable, 0 error(s), 1 warning(s), 1 info.
```

- 問題行的格式是兩個空格、`行:欄` 補到 10 字元、嚴重度補到 7 字元、規則 ID、兩個空格、訊息，可修正的問題結尾加上 ` (fixable)`。
- 欄號以 UTF-16 code unit 計算，與 LSP 相同。
- 檔案沒有問題時印出 `  No problems.`。
- 被拒絕修正的檔案多印一行 `  ERROR  Structural error: <原因>`（`--write` 時是 `Not modified`）。
- `--write` 時，有修改的檔案多印一行 `  Fixed N issue(s).`。
- 渲染保護擋下的修正，在問題清單之後每個一行（Rust 版專有，見〈渲染保護〉）。

摘要行：

| 情況 | 摘要行 |
| --- | --- |
| `--check` 有問題 | `N problem(s) in M file(s): …` |
| `--check` 沒有問題 | `No Markdown issues found in M file(s).` |
| `--write` 有修改 | `Fixed N issue(s) in M file(s).` |
| `--write` 修正後仍有問題 | `N problem(s) remain: …` |
| `--write` 修正後沒有問題 | `No remaining Markdown issues.` |
| 有檔案讀寫失敗 | `Some files could not be read or written; see the errors above.` |
| `--check` 有被拒絕的檔案 | `N file(s) need manual review before safe fixes.` |
| `--write` 有被拒絕的檔案 | `N file(s) were not modified because of structural or frontmatter ambiguity.` |
| 渲染保護擋下了修正 | `Render guard skipped N fix(es) in M file(s) that would change how the document renders; fix them manually or rerun with --no-render-guard.` |

`--check` 顯示的是修正前的問題；`--write` 顯示的是修正後仍然存在的問題。

## 結束碼

| 結束碼 | 意思 |
| --- | --- |
| `0` | 沒有失敗的問題 |
| `1` | 仍有 warning 或 error（`--strict` 時含 info），或有檔案被拒絕修正 |
| `2` | 用法錯誤、路徑無法存取、檔案讀寫失敗、標準輸入不是合法 UTF-8 |

`--exit-zero` 把 1 改成 0，2 維持不變。`--detect` 的結束碼另見〈Markdown 偵測〉。

## JSON 結構

`--json` 輸出一個 JSON 物件，以 2 個空格縮排，非 ASCII 字元不跳脫，欄位順序固定：

```json
{
  "tool": "markdown-fixer",
  "version": "0.1.0",
  "mode": "check",
  "files": [
    {
      "path": "fm.md",
      "changed": false,
      "wouldChange": false,
      "refused": true,
      "reason": "Unclosed YAML frontmatter: no closing --- or ... line was found.",
      "fixed": 0,
      "wouldFix": 0,
      "problems": [
        {
          "rule": "MDX081",
          "name": "frontmatter-unclosed",
          "severity": "error",
          "fixable": false,
          "line": 1,
          "column": 1,
          "message": "Unclosed YAML frontmatter: no closing --- or ... line was found."
        }
      ]
    }
  ],
  "summary": {
    "files": 1,
    "changed": 0,
    "wouldChange": 0,
    "fixed": 0,
    "refused": 1,
    "problems": {
      "total": 1,
      "errors": 1,
      "warnings": 0,
      "info": 0,
      "fixable": 0
    }
  }
}
```

| 欄位 | 說明 |
| --- | --- |
| `version` | Rust 版輸出自己的 crate 版本，oracle 輸出自己的版本（目前 `1.1.0`）。比對時忽略 |
| `mode` | `check` 或 `write` |
| `files[].path` | 顯示路徑，與文字輸出相同 |
| `files[].changed` | 這次執行實際改寫了檔案（只有 `--write` 會是 `true`） |
| `files[].wouldChange` | 套用修正會改變檔案內容 |
| `files[].refused` | 因結構錯誤或 frontmatter 陷阱拒絕修正 |
| `files[].reason` | 拒絕原因；沒有拒絕時省略這個欄位 |
| `files[].fixed` | `--write` 修掉的可修正問題數；`--check` 一律是 0 |
| `files[].wouldFix` | 修正前的可修正問題數；被拒絕時是 0。渲染保護擋下修正時，只算真的會修好的問題 |
| `files[].problems` | `--check` 是修正前的問題，`--write` 是修正後剩下的問題 |
| `files[].skippedFixes` | Rust 版專有：渲染保護擋下的修正，每個元素有 `rule`、`line`、`reason`。陣列是空的時省略這個欄位，JSON 形狀與 oracle 相同 |
| `summary.problems` | 所有 `files[].problems` 的統計 |

讀寫失敗的檔案不會出現在 `files` 裡。即使所有檔案都讀取失敗，`--json` 仍然輸出完整結構（`files` 為空陣列），結束碼 2。

## 標準輸入模式

### `--stdin --check`

從標準輸入讀取，輸出到 stdout，格式與檔案模式相同，只是顯示名稱是 `<stdin>`。`--json`、`--quiet`、`--strict`、`--rule` 等旗標都照常作用。

```sh
pbpaste | markdown-fixer --stdin
git show HEAD:README.md | markdown-fixer --stdin --json
```

### `--stdin --write`

- stdout 一定是一份完整文件：修正後的文字；沒有變更或被拒絕修正時，就是原本的輸入，逐位元組相同。
- 報告與摘要寫到 stderr，格式與檔案模式相同；加上 `--json` 時 JSON 也寫到 stderr。
- `--quiet` 時 stderr 不輸出報告；只有被拒絕修正時印一行 `<stdin>: Not modified: <原因>`，渲染保護擋下修正時每個印一行 `<stdin>:<行>: Fix not applied (<規則>): <原因>`。
- 結束碼與檔案模式相同，可以搭配 `--exit-zero`。
- 剪貼簿裡的片段通常沒有檔尾換行，建議搭配 `--ignore-rule MDX003`，讓片段維持原樣。

```sh
pbpaste | markdown-fixer --stdin --write --quiet --exit-zero --ignore-rule MDX003 | pbcopy
```

上面的寫法在失敗時會把空字串寫進剪貼簿。剪貼簿腳本請先存到變數，確認成功再寫回，完整做法見 [clipboard.md](clipboard.md)。

### 輸入錯誤

標準輸入不是合法 UTF-8 或含有 NUL 位元組時，結束碼 2、stdout 不輸出任何東西，stderr 印出原因：

```text
Error: <stdin>: The encoded data was not valid for encoding utf-8
```

`--stdin` 與檔案或目錄參數一起使用時是用法錯誤，結束碼 2。

### Markdown 偵測（`--detect`）

`--detect` 讀取標準輸入（`--stdin` 可加可不加），以保守的規則判斷內容像不像 Markdown，判斷規則見 [detect.md](detect.md)。

- 是 Markdown：印出 `markdown`，結束碼 0。
- 不是 Markdown：印出 `not-markdown`，結束碼 1（加上 `--exit-zero` 時是 0）。
- 加上 `--json` 時改印函式庫 `Detection` 的 JSON，欄位為 `is_markdown`、`score`、`signals`。
- 不能和檔案或目錄參數一起使用；`--write`、`--rule` 等旗標在這個模式下沒有作用。

```sh
if pbpaste | markdown-fixer --detect >/dev/null; then
  echo "剪貼簿是 Markdown"
fi
```

## 渲染保護

Rust 版預設開啟渲染保護（PRD〈6.9〉）：修正後以 CommonMark／GFM 渲染比對，會意外改變渲染結果的修正不套用。運作方式見 [architecture.md](architecture.md#渲染保護)。

被擋下的修正在文字輸出裡列在該檔案的問題清單之後，格式對齊問題行：行號補到 10 字元、嚴重度欄固定是 `skipped`、規則 ID、兩個空格、`Fix not applied: ` 加原因。

以 `a`、空格、tab、空格、換行、`b` 這兩行為例（pulldown-cmark 把行尾的「空格、tab、空格」當成硬換行）：

```text
notes.md
  1:2        warning MDX004  Trailing whitespace. Exactly two trailing spaces create a hard line break; other trailing whitespace is noise. (fixable)
  1:3        warning MDX007  Tab character found. Use spaces for indentation and alignment; tabs render at different widths.
  1          skipped MDX004  Fix not applied: fixes for this layout rule would change how the document renders, so none were applied in this file

2 problem(s) in 1 file(s): 1 fixable, 0 error(s), 2 warning(s), 0 info.
Render guard skipped 1 fix(es) in 1 file(s) that would change how the document renders; fix them manually or rerun with --no-render-guard.
```

- 被擋下的修正對應的問題仍然列在問題清單裡（`--write` 時也是，因為它沒有被修掉），結束碼照一般規則計算，意義不變。
- `skipped` 行不計入摘要行的問題數。
- `--json` 時每個檔案多一個 `skippedFixes` 陣列，只在非空時出現。
- `--quiet` 時不列出 `skipped` 行，但摘要行照印。
- `--no-render-guard` 關閉渲染保護，輸出與 oracle 逐位元組相同。parity 測試用這個模式；CLI 的 golden 測試每個案例跑兩次，見 [parity.md](parity.md)。

## 資源上限

引言（`>`）巢狀超過 `--max-quote-depth`（預設 32）時：

- `--on-limit error`（預設）：在該行回報 MDX086（error、不可修正），並拒絕修正整份文件。`--check` 印出 `  ERROR  Structural error: <原因>`，`--write` 印出 `  ERROR  Not modified: <原因>`，結束碼 1。
- `--on-limit ignore`：超過上限的行不修改、不診斷，也不當成參考定義。

`--max-quote-depth 0` 表示不限制。

## 與 oracle 的差異

除了下列項目，輸出與 oracle 逐位元組相同。完整清單記錄在 [parity.md](parity.md)。

1. `--version` 印出 Rust crate 的版本；JSON 的 `version` 欄位亦同。
2. `--help` 的 `Usage:` 與 `Examples:` 行使用執行檔名稱 `markdown-fixer`，不是 `node markdown-fixer.mjs`。
3. `--help` 多一個 `Rust-only options` 區塊，`--stdin`、`--exit-zero`、`--detect`、`--no-render-guard` 四個旗標只有 Rust 版有。
4. 渲染保護預設開啟。會改變渲染的修正不套用，並在輸出中列出（見〈渲染保護〉）；加上 `--no-render-guard` 時沒有這項差異。

## 範例

### Git pre-commit hook

只檢查這次要 commit 的 Markdown 檔，有 warning 或 error 就擋下來：

```sh
#!/bin/sh
# .git/hooks/pre-commit
files=$(git diff --cached --name-only --diff-filter=ACM -- '*.md' '*.markdown')
[ -z "$files" ] && exit 0
# shellcheck disable=SC2086
markdown-fixer --check $files || {
  echo "Markdown 有問題。執行 markdown-fixer --write <檔案> 修正後重新 git add。" >&2
  exit 1
}
```

路徑含空白時，改用 `git diff --cached -z` 搭配 `xargs -0`。

### CI

```yaml
# .github/workflows/markdown.yml
name: markdown
on: [push, pull_request]
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install markdown-fixer-cli --locked
      - run: markdown-fixer --check --json . > markdown-report.json
```

結束碼 1 讓 job 失敗；要先收集報告再決定時，加上 `--exit-zero`，再用 `jq` 讀 `summary.problems`：

```sh
markdown-fixer --check --json --exit-zero docs/ | jq '.summary.problems'
```

### 修正後檢查 `git diff`

```sh
markdown-fixer --write README.md docs/
git diff --stat
git diff -- docs/
```

`--write` 只做保證不改變意思的修正，但仍建議看過 diff 再 commit。報告裡還沒修掉的問題（例如 MDX015 標題層級跳號）需要人工處理。

### 比較修正前後但不寫檔

```sh
markdown-fixer --stdin --write --quiet < README.md | diff -u README.md -
```

## 測試

CLI 的測試在 `crates/markdown-fixer-cli/tests/`：

- `cli_golden.rs`：重播 `tests/cli-golden/cases/*.json`。這些 golden 檔由 `node scripts/gen-cli-golden.mjs` 以 oracle CLI 產生，每個案例自帶輸入檔、參數與預期的 stdout、stderr、結束碼與執行後的檔案內容。案例清單在 `tests/cli-golden/manifest.json`。
- `cli_ported.rs`：移植自 `reference/js/markdown-fixer.test.mjs` 的 CLI 案例。
- `cli_rust_only.rs`：`--stdin`、`--exit-zero`、`--detect` 的測試。

```sh
node scripts/gen-cli-golden.mjs            # 重新產生全部 golden
node scripts/gen-cli-golden.mjs --check    # 只比對，不寫檔
MDFIX_CLI_GOLDEN=refused cargo test -p markdown-fixer-cli --test cli_golden
```

不要手改 golden 檔。新增案例時改 `manifest.json`，輸入檔放在 `tests/fixtures/`，再重新產生。
