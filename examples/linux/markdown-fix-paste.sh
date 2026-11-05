#!/bin/bash
# markdown-fix-paste.sh（Linux）— 把剪貼簿裡的 Markdown 用 markdown-fixer 修正後貼上。
#
# macOS 版（examples/macos/markdown-fix-paste.sh）的 Linux 對應寫法，說明見
# docs/clipboard.md。自動判斷工作階段：
#
#   Wayland：wl-paste／wl-copy（wl-clipboard），貼上用 wtype（或 ydotool）
#   X11    ：xclip，貼上用 xdotool key --clearmodifiers ctrl+v
#
# 綁定熱鍵的方式依桌面環境而定（GNOME「自訂快捷鍵」、KDE「自訂快捷鍵」、
# sway 的 bindsym 等），指令填這支腳本的絕對路徑即可。
#
# 環境變數：
#   MARKDOWN_FIXER_BIN              markdown-fixer 執行檔路徑
#   MARKDOWN_FIX_PASTE_NO_PASTE=1   只修正剪貼簿，不送出 Ctrl+V
#   MARKDOWN_FIX_PASTE_DELAY        送出 Ctrl+V 前等待的秒數（Wayland），預設 0.3
#   MARKDOWN_FIX_PASTE_BACKEND      強制指定 wayland 或 x11
#
# 只把事件寫到 stderr，絕不記錄剪貼簿內容。

set -u

export LANG=${LANG:-C.UTF-8}
case ${LC_ALL:-$LANG} in
    *UTF-8* | *utf8* | *UTF8* | *utf-8*) ;;
    *) export LC_ALL=C.UTF-8 ;;
esac

log() {
    printf 'markdown-fix-paste: %s\n' "$*" >&2
}

fixer=${MARKDOWN_FIXER_BIN:-}
if [ -z "$fixer" ]; then
    for candidate in "$HOME/.local/bin/markdown-fixer" "$HOME/.cargo/bin/markdown-fixer"; do
        if [ -x "$candidate" ]; then
            fixer=$candidate
            break
        fi
    done
    [ -n "$fixer" ] || fixer=$(command -v markdown-fixer || echo markdown-fixer)
fi

backend=${MARKDOWN_FIX_PASTE_BACKEND:-}
if [ -z "$backend" ]; then
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then backend=wayland; else backend=x11; fi
fi

# 讀剪貼簿的文字到 $1。沒有文字時回傳非 0。
read_clipboard() {
    case $backend in
        # -n：不要自動補換行；-t text：剪貼簿沒有文字型別時失敗，不會吐出圖片位元組。
        wayland) wl-paste -n -t text >"$1" 2>/dev/null ;;
        x11)
            xclip -selection clipboard -o -t TARGETS 2>/dev/null | grep -qx UTF8_STRING || return 1
            xclip -selection clipboard -o -t UTF8_STRING >"$1" 2>/dev/null
            ;;
        *) log "unknown backend: $backend"; return 1 ;;
    esac
}

# wl-copy 與 xclip 都會留一個背景行程持有剪貼簿，直到別的程式取得剪貼簿；
# 把它們的 stdout／stderr 導到 /dev/null，否則呼叫端讀這支腳本的輸出時會一直等到
# 那個背景行程結束。
write_clipboard() {
    case $backend in
        wayland) wl-copy -t 'text/plain;charset=utf-8' <"$1" >/dev/null 2>&1 ;;
        x11) xclip -selection clipboard -t UTF8_STRING -i <"$1" >/dev/null 2>&1 ;;
    esac
}

paste_clipboard() {
    [ "${MARKDOWN_FIX_PASTE_NO_PASTE:-0}" = 1 ] && return 0
    case $backend in
        wayland)
            # Wayland 沒有通用的方法查詢實體修飾鍵狀態，wtype 也不會替你放開；
            # 先等一下，讓使用者放開熱鍵的修飾鍵。
            sleep "${MARKDOWN_FIX_PASTE_DELAY:-0.3}"
            if command -v wtype >/dev/null 2>&1; then
                wtype -M ctrl v -m ctrl
            elif command -v ydotool >/dev/null 2>&1; then
                # 29 = KEY_LEFTCTRL，47 = KEY_V（Linux input event codes）。
                ydotool key 29:1 47:1 47:0 29:0
            else
                log "neither wtype nor ydotool is installed; paste manually"
                return 1
            fi
            ;;
        x11)
            # --clearmodifiers 會暫時放開仍按著的 Shift／Alt，送完再還原。
            xdotool key --clearmodifiers ctrl+v
            ;;
    esac
}

umask 077
tmp=$(mktemp -d "${TMPDIR:-/tmp}/markdown-fix-paste.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT

if ! read_clipboard "$tmp/in" || [ ! -s "$tmp/in" ]; then
    log "clipboard has no text; nothing to do"
    exit 0
fi

status=0
"$fixer" --stdin --write --quiet --exit-zero --ignore-rule MDX003 <"$tmp/in" >"$tmp/out" 2>/dev/null || status=$?
if [ "$status" -ne 0 ] || [ ! -s "$tmp/out" ]; then
    log "markdown-fixer failed (status $status, fixer: $fixer); pasting the original clipboard"
    paste_clipboard
    exit 1
fi

if ! cmp -s "$tmp/in" "$tmp/out"; then
    if ! write_clipboard "$tmp/out"; then
        log "cannot write the clipboard; pasting the original clipboard"
        paste_clipboard
        exit 1
    fi
fi

paste_clipboard
