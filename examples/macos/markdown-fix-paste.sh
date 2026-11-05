#!/bin/bash
# markdown-fix-paste.sh — 把剪貼簿裡的 Markdown 用 markdown-fixer 修正後貼上。
#
# 給 Karabiner-Elements 的 shell_command 呼叫（見 markdown-fixer-paste.json），
# 安裝與疑難排解見 docs/clipboard.md。
#
# 流程：
#   1. 讀剪貼簿到暫存檔（不用 $(...)，因為它會吃掉結尾換行）。
#   2. 剪貼簿沒有文字（例如只有圖片）就直接結束，不改剪貼簿、不貼上。
#   3. markdown-fixer --stdin --write --quiet --exit-zero --ignore-rule MDX003
#   4. 修正失敗或輸出為空時保留原剪貼簿；內容沒變時也不重寫剪貼簿，
#      保留原本的 RTF／HTML 格式。
#   5. 等使用者放開修飾鍵，再送出 Cmd+V；逾時仍按著就不送出，以結束碼 1 結束。
#
# 環境變數（都可以省略，也可以寫在 ~/.config/markdown-fixer/env）：
#   MARKDOWN_FIXER_BIN                  markdown-fixer 執行檔路徑
#   MARKDOWN_FIX_PASTE_NO_PASTE=1       只修正剪貼簿，不送出 Cmd+V
#   MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS
#                                       等待放開修飾鍵的上限，預設 1000 毫秒
#   MARKDOWN_FIX_PASTE_LOG              log 檔路徑，預設
#                                       ~/Library/Logs/markdown-fix-paste.log；
#                                       設成 /dev/null 可關閉
#
# Log 只記錄事件與位元組數，絕不記錄剪貼簿內容。

set -u

# Karabiner 啟動的 shell 沒有語系設定；少了這兩行，pbpaste／pbcopy 會把 CJK
# 當成 MacRoman 處理而變成亂碼。
export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8

log_file=${MARKDOWN_FIX_PASTE_LOG:-$HOME/Library/Logs/markdown-fix-paste.log}

log() {
    # 只寫事件，不寫內容。寫入失敗（例如目錄不存在）不影響主流程。
    printf '%s markdown-fix-paste: %s\n' "$(date '+%Y-%m-%dT%H:%M:%S')" "$*" >>"$log_file" 2>/dev/null || true
    printf 'markdown-fix-paste: %s\n' "$*" >&2
}

find_fixer() {
    if [ -n "${MARKDOWN_FIXER_BIN:-}" ]; then
        printf '%s\n' "$MARKDOWN_FIXER_BIN"
        return
    fi
    # Karabiner 的 PATH 很短，先找常見的安裝位置，最後才查 PATH。
    for candidate in \
        "$HOME/.local/bin/markdown-fixer" \
        "$HOME/.cargo/bin/markdown-fixer" \
        /opt/homebrew/bin/markdown-fixer \
        /usr/local/bin/markdown-fixer; do
        if [ -x "$candidate" ]; then
            printf '%s\n' "$candidate"
            return
        fi
    done
    command -v markdown-fixer || printf '%s\n' markdown-fixer
}

byte_count() {
    wc -c <"$1" | tr -d ' '
}

# 等所有修飾鍵（Shift、Control、Option、Command）放開，再送出 Cmd+V。
#
# 用一次 osascript（JXA）在行程內輪詢 CGEventSourceFlagsState，比每次輪詢都啟動
# 一個 osascript 快得多。讀取修飾鍵狀態不需要輔助使用權限；送出按鍵需要。
#
# 逾時後修飾鍵仍按著時「不」送出按鍵：否則前景 App 收到的是 Cmd+Option+Shift+V
# 之類的組合。修正後的文字留在剪貼簿，使用者自己按 Cmd+V 即可。
#
# <paste-jxa> 與 </paste-jxa> 之間的程式由 test-paste-jxa.mjs 抽出，以模擬的
# CoreGraphics 與時間實際執行；修改時保留這兩個標記。
paste_clipboard() {
    if [ "${MARKDOWN_FIX_PASTE_NO_PASTE:-0}" = 1 ]; then
        return 0
    fi
    timeout_ms=${MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS:-1000}
    case $timeout_ms in
        '' | *[!0-9]*) timeout_ms=1000 ;;
    esac
    # shellcheck disable=SC2016 # $ 是 JXA 的 ObjC bridge，不是 shell 變數。
    result=$(osascript -l JavaScript -e '
// <paste-jxa>
function run(argv) {
    ObjC.import("CoreGraphics");
    // Shift 0x20000 | Control 0x40000 | Option 0x80000 | Command 0x100000
    var mask = 0x1E0000;
    var deadline = Date.now() + Number(argv[0]);
    // 0 = kCGEventSourceStateCombinedSessionState，與 dedent-paste 相同。
    while ((Number($.CGEventSourceFlagsState(0)) & mask) !== 0) {
        if (Date.now() >= deadline) {
            return "modifiers-held";
        }
        delay(0.01);
    }
    // 給剪貼簿一點時間讓其他 App 看到新內容。
    delay(0.03);
    Application("System Events").keystroke("v", { using: "command down" });
    return "pasted";
}
// </paste-jxa>
' "$timeout_ms") || {
        log "paste failed (check Accessibility permission for Karabiner-Elements / osascript)"
        return 1
    }
    case $result in
        pasted) return 0 ;;
        modifiers-held)
            log "modifier keys still held after ${timeout_ms} ms; not sending Cmd+V (the clipboard is ready, paste manually)"
            return 1
            ;;
        *)
            log "unexpected osascript result; not sure whether Cmd+V was sent"
            return 1
            ;;
    esac
}

umask 077
tmp=$(mktemp -d "${TMPDIR:-/tmp}/markdown-fix-paste.XXXXXX") || {
    log "cannot create a temporary directory"
    exit 1
}
trap 'rm -rf "$tmp"' EXIT

if ! pbpaste >"$tmp/in" 2>/dev/null; then
    log "pbpaste failed; clipboard left unchanged"
    exit 1
fi
if [ ! -s "$tmp/in" ]; then
    # 沒有文字（空的或只有圖片）：什麼都不做，不清空剪貼簿。
    log "clipboard has no text; nothing to do"
    exit 0
fi

fixer=$(find_fixer)
status=0
"$fixer" --stdin --write --quiet --exit-zero --ignore-rule MDX003 <"$tmp/in" >"$tmp/out" 2>/dev/null || status=$?

if [ "$status" -ne 0 ]; then
    if [ "$status" -eq 126 ] || [ "$status" -eq 127 ]; then
        log "markdown-fixer not found or not executable: $fixer; pasting the original clipboard"
    else
        log "markdown-fixer exited with status $status; pasting the original clipboard"
    fi
    paste_clipboard
    exit 1
fi
if [ ! -s "$tmp/out" ]; then
    log "markdown-fixer produced no output; pasting the original clipboard"
    paste_clipboard
    exit 1
fi

if cmp -s "$tmp/in" "$tmp/out"; then
    log "no changes ($(byte_count "$tmp/in") bytes)"
else
    if ! pbcopy <"$tmp/out"; then
        log "pbcopy failed; pasting the original clipboard"
        paste_clipboard
        exit 1
    fi
    log "fixed: $(byte_count "$tmp/in") -> $(byte_count "$tmp/out") bytes"
fi

paste_clipboard
