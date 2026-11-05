# 剪貼簿熱鍵

按一個熱鍵，把剪貼簿裡的 Markdown 修正後直接貼上。規格見 PRD〈6.7〉。

| 平台 | 檔案 | 預設熱鍵 |
| --- | --- | --- |
| macOS | `examples/macos/markdown-fixer-paste.json`（Karabiner-Elements）＋`examples/macos/markdown-fix-paste.sh` | `左 Option+Shift+V` |
| Linux | `examples/linux/markdown-fix-paste.sh` | 由桌面環境綁定 |
| Windows | `examples/windows/markdown-fix-paste.ahk`（AutoHotkey v2） | `左 Alt+Shift+V` |

三個版本的核心流程相同：

1. 讀剪貼簿的文字。沒有文字（空的或只有圖片）就直接結束，不改剪貼簿、不貼上。
2. 執行 `markdown-fixer --stdin --write --quiet --exit-zero --ignore-rule MDX003`。`MDX003` 被停用，所以沒有檔尾換行的片段維持原樣。
3. 執行失敗或輸出為空時保留原剪貼簿，照樣貼上原文（你按的是貼上鍵）。
4. 內容有變才寫回剪貼簿。內容沒變就不動剪貼簿，保留原本附帶的 RTF／HTML 格式。
5. 等熱鍵的修飾鍵放開，再送出貼上鍵。

腳本只記錄事件與位元組數，絕不記錄剪貼簿內容。

## macOS（Karabiner-Elements）

### 安裝

1. 安裝 `markdown-fixer`，確認在 `~/.local/bin`、`~/.cargo/bin`、`/opt/homebrew/bin` 或 `/usr/local/bin` 其中一處：

   ```sh
   make install          # 安裝到 ~/.cargo/bin
   ```

2. 複製輔助腳本：

   ```sh
   mkdir -p ~/.local/bin
   install -m 755 examples/macos/markdown-fix-paste.sh ~/.local/bin/markdown-fix-paste.sh
   ```

3. 匯入 Karabiner 規則：

   ```sh
   cp examples/macos/markdown-fixer-paste.json ~/.config/karabiner/assets/complex_modifications/
   ```

   打開 Karabiner-Elements →「Complex Modifications」→「Add predefined rule」，啟用「左 Option+Shift+V：修正剪貼簿裡的 Markdown 後貼上（markdown-fixer）」。

   這一步只是把範例放進 Karabiner 的範例資料夾，不會改動你現有的 `karabiner.json`。

4. 第一次按熱鍵時，macOS 會要求允許 Karabiner-Elements（或 `osascript`）控制電腦。到「系統設定」→「隱私權與安全性」→「輔助使用」打開權限。

### 設定

規則會先載入 `~/.config/markdown-fixer/env`（如果存在）。這是 shell 檔，變數要用 `export` 才會傳給腳本：

```sh
# ~/.config/markdown-fixer/env
export MARKDOWN_FIXER_BIN="$HOME/.cargo/bin/markdown-fixer"
export MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS=1500
```

| 變數 | 預設 | 意思 |
| --- | --- | --- |
| `MARKDOWN_FIXER_BIN` | 依序找上面列的安裝位置，最後查 `PATH` | `markdown-fixer` 執行檔路徑 |
| `MARKDOWN_FIX_PASTE_NO_PASTE` | `0` | 設成 `1` 時只修正剪貼簿，不送出 `Cmd+V` |
| `MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS` | `1000` | 等待修飾鍵放開的上限（毫秒） |
| `MARKDOWN_FIX_PASTE_LOG` | `~/Library/Logs/markdown-fix-paste.log` | log 檔；設成 `/dev/null` 可關閉 |

### 改熱鍵

編輯 `~/.config/karabiner/assets/complex_modifications/markdown-fixer-paste.json` 的 `from`，再到 Karabiner 移除舊規則、重新加入。例如改成 `右 Command+Shift+V`：

```json
"from": {
    "key_code": "v",
    "modifiers": { "mandatory": ["right_command", "right_shift"] }
}
```

腳本會等「所有」修飾鍵放開，所以換成任何修飾鍵組合都不用改腳本。

### 為什麼要等修飾鍵放開

Karabiner 在按下 `V` 的瞬間就執行腳本，這時 `Option` 與 `Shift` 通常還按著。直接送出 `Cmd+V`，前景 App 收到的會是 `Cmd+Option+Shift+V`，在很多 App 裡是另一個指令（例如「貼上並符合樣式」），或是什麼都不做。

腳本用一次 `osascript -l JavaScript` 在行程內每 10 毫秒輪詢 `CGEventSourceFlagsState`（透過 JXA 的 ObjC bridge），直到 Shift、Control、Option、Command 都放開，或超過逾時（預設 1 秒），再用 System Events 送出 `Cmd+V`。API 與逾時都和 `dedent-paste` 相同。

考慮過的其他做法：

- **Karabiner 的 `to_after_key_up`**：它在 `from` 的按鍵（`V`）放開時觸發，不保證修飾鍵已經放開。使用者常常先放開 `V` 再放開 `Option`，問題依舊。
- **每次輪詢都啟動一個 `osascript`**：每次要 60～100 毫秒，太慢。
- **固定 `sleep`**：太短沒效果，太長讓每次貼上都變慢。

讀取修飾鍵狀態不需要輔助使用權限，送出按鍵需要。

### 與 dedent-paste 的關係

[dedent-paste](dedent-paste.md) 預設用 `左 Option+V`，本熱鍵用 `左 Option+Shift+V`，兩者不衝突，可以同時啟用。

| | dedent-paste（`左 Option+V`） | markdown-fixer 熱鍵（`左 Option+Shift+V`） |
| --- | --- | --- |
| 去縮排、提示符展開 | 有 | 沒有 |
| 先偵測是不是 Markdown | 是，不像 Markdown 就不修 | 否，一律當成 Markdown 修正 |
| 呼叫方式 | 函式庫 | CLI `--stdin --write` |

不確定剪貼簿是什麼時用 dedent-paste；確定是 Markdown、想強制修正時用本熱鍵。

### 疑難排解

先看 log：

```sh
tail -n 20 ~/Library/Logs/markdown-fix-paste.log
```

| 症狀 | 原因與處理 |
| --- | --- |
| 按了沒反應，log 寫 `paste failed (check Accessibility permission ...)` | 到「系統設定」→「隱私權與安全性」→「輔助使用」允許 Karabiner-Elements 與 `osascript`。權限變更後可能要重新啟動 Karabiner-Elements |
| log 寫 `markdown-fixer not found or not executable` | Karabiner 的 `PATH` 很短。把執行檔放到 `~/.local/bin`，或在 `~/.config/markdown-fixer/env` 設定 `export MARKDOWN_FIXER_BIN=...` |
| 貼上的中文變成亂碼 | Karabiner 啟動的 shell 沒有語系，`pbpaste`／`pbcopy` 會用 MacRoman。腳本已經匯出 `LANG`／`LC_ALL=en_US.UTF-8`；如果你改寫過腳本，確認這兩行還在 |
| 貼上的是原文，log 寫 `exited with status 2` | 剪貼簿不是合法 UTF-8，或 `markdown-fixer` 版本太舊、不支援 `--stdin`。手動執行 `pbpaste \| markdown-fixer --stdin --check` 看錯誤訊息 |
| 貼上的是原文，log 寫 `no changes` | 內容本來就沒有可自動修正的問題，或修正被拒絕（未閉合的 frontmatter、HTML comment、`$$` 或非法 fence）。用 `pbpaste \| markdown-fixer --stdin --check` 檢查 |
| 只有圖片時按熱鍵沒反應 | 刻意的設計：沒有文字就不動剪貼簿也不貼上 |
| 貼上時多了 `Option` 或 `Shift` 的效果 | 修飾鍵超過逾時還沒放開。調高 `MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS` |

### 測試

`examples/macos/test-markdown-fix-paste.sh` 用假的 `pbpaste`、`pbcopy`、`osascript` 與 `markdown-fixer` 測試腳本流程，不需要 Karabiner，也不碰真正的剪貼簿，macOS 與 Linux 都能跑：

```sh
examples/macos/test-markdown-fix-paste.sh
# 加上真正的執行檔，另外跑端對端案例
cargo build -p markdown-fixer-cli
MARKDOWN_FIXER_TEST_BIN="$PWD/target/debug/markdown-fixer" examples/macos/test-markdown-fix-paste.sh
```

## Linux

`examples/linux/markdown-fix-paste.sh` 依 `WAYLAND_DISPLAY` 自動選擇 Wayland 或 X11，也可以用 `MARKDOWN_FIX_PASTE_BACKEND=wayland|x11` 指定。

| | Wayland | X11 |
| --- | --- | --- |
| 讀剪貼簿 | `wl-paste -n -t text` | `xclip -selection clipboard -o -t UTF8_STRING` |
| 寫剪貼簿 | `wl-copy` | `xclip -selection clipboard -i` |
| 送出貼上 | `wtype`（或 `ydotool`） | `xdotool key --clearmodifiers ctrl+v` |
| 套件 | `wl-clipboard`、`wtype` | `xclip`、`xdotool` |

注意事項：

- `wl-paste` 預設會在結尾補一個換行，一定要加 `-n`；`-t text` 讓只有圖片時失敗，而不是吐出圖片位元組。
- `xdotool key --clearmodifiers` 會暫時放開仍按著的修飾鍵，送完再還原，所以 X11 不用自己等。
- `wtype` 不會處理實體修飾鍵，Wayland 也沒有通用的方法查詢修飾鍵狀態。腳本在送出前等 `MARKDOWN_FIX_PASTE_DELAY` 秒（預設 0.3）。如果桌面環境支援「放開按鍵時觸發」（例如 sway 的 `bindsym --release`），用它最可靠。
- GNOME 的 Wayland 工作階段不支援 `wtype` 使用的虛擬鍵盤協定，請改用 `ydotool`（需要 `ydotoold` 常駐與 `/dev/uinput` 權限），或設 `MARKDOWN_FIX_PASTE_NO_PASTE=1` 只修正剪貼簿、自己按 `Ctrl+V`。

綁定熱鍵，以 sway 為例：

```text
bindsym --release Mod1+Shift+v exec ~/.local/bin/markdown-fix-paste.sh
```

GNOME、KDE 在「鍵盤快捷鍵」→「自訂快捷鍵」新增一個，指令填腳本的絕對路徑。

## Windows（AutoHotkey v2）

1. 安裝 [AutoHotkey v2](https://www.autohotkey.com/)。
2. 把 `markdown-fixer.exe` 放到 `%USERPROFILE%\.local\bin\` 或 `%USERPROFILE%\.cargo\bin\`，或設定環境變數 `MARKDOWN_FIXER_BIN`。
3. 雙擊 `examples/windows/markdown-fix-paste.ahk` 執行。要開機自動執行，把它的捷徑放進 `shell:startup`。

預設熱鍵是 `左 Alt+Shift+V`（`<!+v`）。改熱鍵就改這一行，語法見 AutoHotkey 的〈Hotkeys〉文件。

Windows 版的差異：

- 以暫存檔傳遞內容：寫入用 `UTF-8-RAW`（不加 BOM，否則 MDX001 會把它當成要移除的 BOM），讀回用 `UTF-8`。
- Windows 剪貼簿的換行是 CRLF。腳本另外加上 `--ignore-rule MDX002`，`markdown-fixer` 會照原文的行尾還原，不會把 CRLF 改成 LF。
- 用 `KeyWait` 等 `LAlt` 與 `Shift` 放開（各最多 1 秒）再送出 `Ctrl+V`。
