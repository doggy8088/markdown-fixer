# 除錯手冊

這份手冊給人與 AI agent 共用：Rust 版與 oracle 的輸出不一致、某條規則誤判或漏判、修正結果不如預期時，照這裡的步驟找出原因。

先記住一個原則：**oracle 怎麼做，Rust 就怎麼做。** 看到 oracle 的行為好像不合理時，不要在 Rust 裡順手修正。先讓 Rust 跟 oracle 一致，再把觀察記到 [parity.md](parity.md) 的「oracle 疑似問題」清單。

## 快速流程

1. 用 `--json` 重現問題，拿到精確的規則、行號、欄號。
2. 用 `scripts/parity.sh` 比對 oracle 與 Rust，找出第一個差異。
3. 縮小輸入，直到只剩幾行還能重現。
4. 存成 regression 語料，產生 golden 檔。
5. 修 Rust，直到 golden 測試通過。
6. 重跑整個語料的 parity 檢查，確認沒有新的差異。

## 1. 重現

同一份檔案分別交給 oracle 與 Rust，輸出 JSON：

```sh
node reference/js/markdown-fixer.mjs --check --json path/to/file.md > /tmp/oracle.json
cargo run -q -p markdown-fixer-cli -- --check --json path/to/file.md > /tmp/rust.json
diff /tmp/oracle.json /tmp/rust.json
```

兩邊的 `version` 欄位本來就不同（oracle 是 `1.0.0`，Rust 是 crate 版本），比較時忽略那一行。

修正結果的差異要比對檔案內容。不要對原檔用 `--write`，先複製一份：

```sh
cp path/to/file.md /tmp/a.md && cp path/to/file.md /tmp/b.md
node reference/js/markdown-fixer.mjs --write /tmp/a.md
cargo run -q -p markdown-fixer-cli -- --write /tmp/b.md
cmp /tmp/a.md /tmp/b.md
```

只想看函式庫層級的結果，不經過 CLI，可以直接呼叫 oracle 匯出的函式：

```sh
node --input-type=module -e "
import fs from 'node:fs';
import { diagnose, fixDocument } from './reference/js/markdown-fixer.mjs';
const text = fs.readFileSync(process.argv[1], 'utf8');
console.log(JSON.stringify(diagnose(text, {}), null, 2));
console.log(JSON.stringify(fixDocument(text, {}).text));
" path/to/file.md
```

## 2. 用 parity.sh 找第一個差異

`scripts/parity.sh` 比對的是**函式庫層級**的結果，不是 CLI 輸出。它對每個檔案、每組選項，分別在 Node 裡呼叫 oracle 的 `diagnose` 與 `fixDocument`，在 Rust 端透過 `crates/markdown-fixer/examples/dump.rs`（`cargo run -p markdown-fixer --example dump --features serde`）取得 `diagnose` 與 `fix_document` 的 JSON，逐一比較。每個檔案只列出第一個差異。

選項組合沿用 `scripts/gen-golden.mjs` 的 `OPTION_SETS`：`default`、`snippet`（忽略 MDX003）、`line-length-80`、`keep-encoding`（忽略 MDX001、MDX002）、`headings-only`。只想跑其中一組時設定 `PARITY_OPTIONS`，例如 `PARITY_OPTIONS=default`。

```sh
scripts/parity.sh fixtures/corpus
scripts/parity.sh path/to/file.md
```

也可以拿 repo 外的真實文件來找語料沒涵蓋到的差異：

```sh
scripts/parity.sh ~/projects/*/README.md ~/projects/*/docs/
```

輸出格式：

```text
DIFF docs/example.md [snippet] {"ignoreRules":["MDX003"]}
  <第一個差異的說明>
  repro: cargo run -q -p markdown-fixer --example dump --features serde -- '{"ignoreRules":["MDX003"]}' < docs/example.md
parity: 2 file(s), 10 case(s), 1 file(s) diverged
```

`repro` 那一行可以直接執行，看 Rust 端的完整結果；oracle 端用〈1. 重現〉裡的 node 指令，把同樣的選項傳給 `diagnose` 與 `fixDocument`。有差異時結束碼是 1，用法錯誤是 2。Rust 執行檔建在 `CARGO_TARGET_DIR`（預設 `target/parity`），也可以用 `MDFIX_DUMP` 指定已經建好的 dump 執行檔。

CLI 層級的差異（文字輸出格式、`--json` 結構、結束碼、目錄排序）不在 `parity.sh` 的範圍內，請用〈1. 重現〉的 `--check --json` 比對，CLI 的 golden 測試見 [testing.md](testing.md)。

隨機文件的差異測試用 `node scripts/fuzz-parity.mjs --count 2000`。

## 3. 縮小輸入

找到不一致的檔案後，一邊刪內容一邊重跑，直到只剩幾行：

1. 先刪掉與差異行號距離很遠的段落。多數差異只和前後幾行有關。
2. 保留會影響區塊判斷的東西：檔首的 `---`、還沒關閉的 fence、HTML 區塊、縮排。
3. 把 CJK、emoji、NBSP 等字元留到最後才刪，它們常常就是原因。
4. 檔尾換行、CRLF、BOM 也可能是原因，用 `xxd file.md | head` 確認。

縮小後的檔案越短越好，但要保留讓差異出現的最少內容。

## 4. 存成 regression 語料

```sh
cp /tmp/min.md fixtures/corpus/regressions/<短名稱>.md
node scripts/gen-golden.mjs
```

`fixtures/golden/` 只能由 `scripts/gen-golden.mjs` 產生，不要手改，也不要為了讓測試通過去改 golden 檔。

## 5. 修 Rust

```sh
CARGO_TARGET_DIR=target/<你的名字> cargo test --workspace --all-features
```

修 bug 前，先在 [porting-map.md](porting-map.md) 找到對應的 JS 函式與行號，讀 JS 原始碼，不要憑規則名稱猜行為。

## 6. 確認沒有退步

```sh
scripts/parity.sh fixtures/corpus
make check
```

## 檢查 scanDocument 的逐行標記

大部分差異的根源是「某一行被判斷成不同的區塊」：Rust 以為是段落，oracle 以為是縮排 code 或 HTML 區塊。這時直接印出 oracle 的 `scanDocument` 結果最快。

`scanDocument` 預期收到已經正規化的文字（沒有 BOM、只有 `\n`），所以要先做和 `normalizeForAnalysis` 一樣的處理：

```sh
node --input-type=module -e "
import fs from 'node:fs';
import { scanDocument } from './reference/js/markdown-fixer.mjs';
const text = fs.readFileSync(process.argv[1], 'utf8')
  .replace(/^﻿+/, '').replace(/\r\n?/g, '\n');
const { info, issues, frontmatterEnd } = scanDocument(text);
for (const i of info) {
  const flags = Object.entries(i)
    .filter(([k, v]) => v === true && k !== 'blank').map(([k]) => k);
  console.log(String(i.index + 1).padStart(4),
    (i.blank ? 'blank ' : '') + flags.join(','), JSON.stringify(i.line));
}
console.log({ frontmatterEnd, issues: issues.map((p) => p.rule + '@' + p.line) });
" path/to/file.md
```

輸出範例：

```text
   1  "# T"
   2 blank  ""
   3 fence,fenceOpen "```js"
   4 fence,fenceContent "x"
   5 fence,fenceClose "```"
   6  "> q"
   7 definition "[a]: http://x"
   8 blank  ""
{ frontmatterEnd: -1, issues: [] }
```

各個標記的意思見 [architecture.md](architecture.md#scandocument逐行的區塊標記)。注意 `diagnose` 的結構檢查用的是「單行修正之後」再掃一次的結果，例如 `#Title` 會先變成 `# Title` 再掃描。

### Rust 端的對應輸出

函式庫有一個只在測試中編譯的探針 `parity_probe`，可以把 Rust 的 scan 與 mask 結果寫成文字檔，和 oracle 的輸出逐行比較。輸入是 JSON 字串陣列（每個元素是一份已正規化的文件）：

```sh
printf '%s\n' '["# T\n\n```js\nx\n```\n"]' > /tmp/probe-in.json
MDFIX_PROBE_IN=/tmp/probe-in.json MDFIX_PROBE_OUT=/tmp/probe-out.txt \
  cargo test -p markdown-fixer --lib -- --ignored parity_probe
cat /tmp/probe-out.txt
```

每份文件以 `=====` 開頭，每一行輸出 `行索引:旗標|fenceInfo|fence 符號與長度|htmlKind`。旗標是固定順序的 13 個字元，沒有設定的位置是 `.`：

| 字元 | 欄位 | 字元 | 欄位 |
| --- | --- | --- | --- |
| `b` | `blank` | `s` | `htmlStart` |
| `F` | `frontmatter` | `i` | `indentedCode` |
| `f` | `fence` | `d` | `definition` |
| `o` | `fenceOpen` | `m` | `mathBlock` |
| `c` | `fenceClose` | `O` | `mathOpen` |
| `C` | `fenceContent` | `X` | `mathClose` |
| `h` | `htmlBlock` | | |

接著是掃描問題（`規則@行:欄 訊息`）、`fm=<frontmatterEnd>`，最後每一行輸出五組 0／1 遮罩，對應 oracle 程式碼裡用到的五種 `buildMask` 選項組合。其他環境變數：

| 變數 | 作用 |
| --- | --- |
| `MDFIX_PROBE_MODE` | `scan`（預設）、`fix` 或 `diagnose`。`fix` 每份文件輸出一行 `FixResult` JSON；`diagnose` 每個問題輸出一行 `rule\|name\|severity\|fixable\|line\|column\|message` |
| `MDFIX_PROBE_OPTS` | 選項 JSON，鍵名與 JS 相同，例如 `{"ignoreRules":["MDX003"],"maxLineLength":80}` |
| `MDFIX_FORCE_MEMO` | 設定後 `jsregex` 從第一步就用記憶化回溯，用來確認記憶化路徑與 oracle 一致 |

這是內部除錯工具，格式可能隨實作調整，以 `crates/markdown-fixer/src/parity_probe.rs` 為準。

## 檢查一行的遮罩

規則在某一行裡「該動沒動」或「不該動卻動了」，通常是遮罩範圍不同。`buildMask` 也有匯出：

```sh
node --input-type=module -e "
import { buildMask } from './reference/js/markdown-fixer.mjs';
const line = process.argv[1];
console.log(line);
console.log(buildMask(line).map((m) => (m ? '^' : ' ')).join(''));
" 'see `code` and https://x.y/z ok'
```

`^` 標示受保護的位置。遮罩以 UTF-16 code unit 為單位，行裡有 CJK 或 emoji 時，終端機上的對齊會偏掉，這時改印 `JSON.stringify` 的索引比較準。

不同規則呼叫 `buildMask` 時用的選項不同（例如 `maskBareUrls: false`），完整清單見 [architecture.md](architecture.md#遮罩行內的保護區段)。要重現某條規則看到的遮罩，請把同樣的選項當成第二個參數傳進去。

## 常見的差異來源

| 現象 | 常見原因 |
| --- | --- |
| 欄號差 1 或差很多 | 用了 byte offset 或 `char` 數，沒有換成 UTF-16。遇到 CJK 差很多，遇到 emoji 差 1 |
| 某行的空白判斷不同 | JS `\s` 包含 NBSP、U+FEFF、U+3000；Rust `char::is_whitespace` 不含 U+FEFF |
| 英數字邊界判斷不同 | JS `\w`、`\b` 只看 ASCII；Rust `regex` 的 `\w` 預設是 Unicode |
| 最後一行不見或多出來 | 用了 `str::lines()`；JS `split('\n')` 會保留最後的空字串 |
| `trim` 結果不同 | JS `trim()` 會去掉 U+FEFF 與所有 Unicode 空白 |
| 目錄掃描的檔案順序不同 | JS `sort()` 以 UTF-16 code unit 比較；Rust `str` 以 byte 比較 |
| 參考標籤配對不同 | JS `toLowerCase()` 與 Rust 的 Unicode 小寫轉換在少數字元不同 |
| 字串開頭或連續 delimiter 的判斷不同 | Rust `regex` 不支援 lookbehind，手寫掃描漏了邊界 |
| 修正結果差一輪 | 修正迴圈要剛好 6 輪上限，而且每一步都要重新掃描 |
| 問題順序不同 | 去重的 key 是 `規則:行:欄:訊息`，之後依行、欄、規則 ID 穩定排序；同位置同規則的問題要照產生順序 |

## 給 AI agent 的提示

- 先跑指令拿到實際輸出，再推論原因。不要只看規則名稱猜。
- 一次只修一個差異，修完重跑 parity，避免一個修正掩蓋另一個問題。
- 判斷是 oracle 疑似 bug 時，Rust 照 oracle，把重現案例與分析寫進 [parity.md](parity.md)，讓擁有者決定。
- 發現這份文件或 `AGENTS.md` 的說明有錯、讓你走錯路，在同一個變更裡修正它。
