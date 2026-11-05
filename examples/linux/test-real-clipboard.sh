#!/bin/bash
# test-real-clipboard.sh（Linux）— 在真正的 X11 或 Wayland 剪貼簿上測試 markdown-fix-paste.sh。
#
# 會覆寫目前的剪貼簿。以 MARKDOWN_FIX_PASTE_NO_PASTE=1 執行腳本，不送出 Ctrl+V。
# 需要已啟動的顯示伺服器，CI 的做法見 .github/workflows/clipboard.yml：
#
#   MARKDOWN_FIXER_BIN=$PWD/target/debug/markdown-fixer \
#       xvfb-run -a examples/linux/test-real-clipboard.sh x11
#   MARKDOWN_FIXER_BIN=$PWD/target/debug/markdown-fixer \
#       examples/linux/test-real-clipboard.sh wayland   # 需要 WAYLAND_DISPLAY
#
# X11 需要 xclip；Wayland 需要 wl-clipboard 與支援 wlr-data-control 的合成器（例如 sway）。

set -u

backend=${1:-}
case $backend in
    x11 | wayland) ;;
    *)
        echo "usage: $0 x11|wayland" >&2
        exit 2
        ;;
esac

here=$(cd "$(dirname "$0")" && pwd)
script="$here/markdown-fix-paste.sh"
fixer=${MARKDOWN_FIXER_BIN:?set MARKDOWN_FIXER_BIN to the markdown-fixer binary}

work=$(mktemp -d "${TMPDIR:-/tmp}/test-real-clipboard.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT

export LANG=C.UTF-8 LC_ALL=C.UTF-8
export MARKDOWN_FIX_PASTE_BACKEND=$backend
export MARKDOWN_FIX_PASTE_NO_PASTE=1

# 1x1 的 PNG。
printf '%s' 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==' |
    base64 --decode >"$work/image.png"

# 剪貼簿工具會留背景行程持有內容，一律把輸出導到 /dev/null。
clip_set() { # <MIME 或 X11 target> <檔案>
    case $backend in
        x11) xclip -selection clipboard -t "$1" -i <"$2" >/dev/null 2>&1 ;;
        wayland) wl-copy -t "$1" <"$2" >/dev/null 2>&1 ;;
    esac
}

clip_set_text() { # <檔案>
    case $backend in
        x11) clip_set UTF8_STRING "$1" ;;
        wayland) clip_set 'text/plain;charset=utf-8' "$1" ;;
    esac
}

clip_get() { # <MIME 或 X11 target> <輸出檔>
    case $backend in
        x11) xclip -selection clipboard -o -t "$1" >"$2" 2>/dev/null ;;
        wayland) wl-paste -n -t "$1" >"$2" 2>/dev/null ;;
    esac
}

clip_get_text() { # <輸出檔>
    case $backend in
        x11) clip_get UTF8_STRING "$1" ;;
        wayland) clip_get text "$1" ;;
    esac
}

clip_types() {
    case $backend in
        x11) xclip -selection clipboard -o -t TARGETS 2>/dev/null ;;
        wayland) wl-paste -l 2>/dev/null ;;
    esac
}

clip_clear() {
    case $backend in
        # 結束持有剪貼簿的 xclip，剪貼簿就沒有擁有者。
        x11) pkill -x xclip; sleep 0.2 ;;
        wayland) wl-copy --clear >/dev/null 2>&1 ;;
    esac
}

failures=0
current=""

fail() {
    printf '  FAIL [%s] %s\n' "$current" "$*"
    failures=$((failures + 1))
}

# 執行腳本，stderr 存檔（不經過 $(...)，以免等背景的剪貼簿行程）。
run_script() { # [markdown-fixer 路徑]
    status=0
    MARKDOWN_FIXER_BIN=${1:-$fixer} "$script" >"$work/stdout" 2>"$work/stderr" || status=$?
    sed 's/^/    stderr: /' "$work/stderr"
}

expect_status() {
    [ "$status" -eq "$1" ] || fail "exit status $status, expected $1"
}

# 剪貼簿文字必須與檔案逐位元組相同。xclip／wl-copy 在背景接手剪貼簿可能晚一點，
# 最多等 2 秒。
expect_clipboard_text() { # <預期檔>
    local _
    for _ in $(seq 20); do
        clip_get_text "$work/actual" && cmp -s "$1" "$work/actual" && return 0
        sleep 0.1
    done
    fail "clipboard differs"
    echo "    expected:" && od -An -tx1 "$1" | sed 's/^/     /'
    echo "    actual:" && od -An -tx1 "$work/actual" | sed 's/^/     /'
}

printf '#標題\n**重點：**內容 😀\n' >"$work/messy"
printf '# 標題\n\n**重點**：內容 😀\n' >"$work/fixed"
printf '#!/bin/sh\ncat >/dev/null\nexit 0\n' >"$work/empty-fixer"
chmod +x "$work/empty-fixer"

echo "backend: $backend"

current=cjk-fixed
echo "case: $current"
clip_set_text "$work/messy"
run_script
expect_status 0
expect_clipboard_text "$work/fixed"

current=unchanged
echo "case: $current"
clip_set_text "$work/fixed"
run_script
expect_status 0
expect_clipboard_text "$work/fixed"

current=failing-fixer-keeps-original
echo "case: $current"
clip_set_text "$work/messy"
run_script /bin/false
expect_status 1
expect_clipboard_text "$work/messy"

current=missing-fixer-keeps-original
echo "case: $current"
clip_set_text "$work/messy"
run_script "$work/no-such-fixer"
expect_status 1
expect_clipboard_text "$work/messy"

current=empty-output-keeps-original
echo "case: $current"
clip_set_text "$work/messy"
run_script "$work/empty-fixer"
expect_status 1
expect_clipboard_text "$work/messy"

current=image-untouched
echo "case: $current"
clip_set image/png "$work/image.png"
run_script
expect_status 0
clip_types >"$work/types"
grep -qx image/png "$work/types" || fail "image/png no longer offered: $(tr '\n' ' ' <"$work/types")"
if grep -qiE '^(UTF8_STRING|STRING|TEXT|text/plain.*)$' "$work/types"; then
    fail "a text type appeared: $(tr '\n' ' ' <"$work/types")"
fi
if ! { clip_get image/png "$work/image-after" && cmp -s "$work/image.png" "$work/image-after"; }; then
    fail "image bytes changed"
fi

current=empty-clipboard
echo "case: $current"
clip_clear
run_script
expect_status 0
clip_get_text "$work/actual" && [ -s "$work/actual" ] && fail "text appeared on an empty clipboard"

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all real clipboard cases passed ($backend)"
