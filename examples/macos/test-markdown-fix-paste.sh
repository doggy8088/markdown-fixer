#!/bin/bash
# test-markdown-fix-paste.sh — 用假的 pbpaste／pbcopy／osascript 測試 markdown-fix-paste.sh。
#
# 不需要 Karabiner，也不會碰到真正的剪貼簿，可以在 macOS 與 Linux 執行：
#
#   examples/macos/test-markdown-fix-paste.sh
#
# 預設用一個 stub 取代 markdown-fixer（只把行首的 "#標題" 改成 "# 標題"）。
# 設定 MARKDOWN_FIXER_TEST_BIN 指向真正的執行檔時，另外跑一組端對端案例。

set -u

here=$(cd "$(dirname "$0")" && pwd)
script="$here/markdown-fix-paste.sh"

work=$(mktemp -d "${TMPDIR:-/tmp}/test-markdown-fix-paste.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT

bin="$work/bin"
mkdir -p "$bin"

# ---- 假的外部指令 ----

# pbpaste：輸出 $FAKE_CLIP 的內容；FAKE_PBPASTE_FAIL=1 時失敗。
cat >"$bin/pbpaste" <<'EOF'
#!/bin/sh
[ "${FAKE_PBPASTE_FAIL:-0}" = 1 ] && exit 1
cat "$FAKE_CLIP"
EOF

# pbcopy：把標準輸入寫進 $FAKE_CLIP，記錄呼叫與當時的語系。
cat >"$bin/pbcopy" <<'EOF'
#!/bin/sh
printf 'pbcopy LANG=%s LC_ALL=%s\n' "${LANG:-}" "${LC_ALL:-}" >>"$FAKE_CALLS"
cat >"$FAKE_CLIP"
EOF

# osascript：只記錄被呼叫、語言與輪詢逾時參數。
cat >"$bin/osascript" <<'EOF'
#!/bin/sh
printf 'osascript %s %s timeout=%s\n' "$1" "$2" "$5" >>"$FAKE_CALLS"
exit "${FAKE_OSASCRIPT_STATUS:-0}"
EOF

# markdown-fixer stub：記錄參數；行為由 FAKE_FIXER_MODE 決定。
cat >"$bin/fake-fixer" <<'EOF'
#!/bin/sh
printf 'fixer %s\n' "$*" >>"$FAKE_CALLS"
case "${FAKE_FIXER_MODE:-fix}" in
    fix) exec perl -0777 -pe 's/^#(?=[^#\s])/# /mg' ;;
    identity) exec cat ;;
    empty) cat >/dev/null; exit 0 ;;
    fail) cat >/dev/null; echo 'Error: stdin is not valid UTF-8' >&2; exit 2 ;;
esac
EOF

chmod +x "$bin"/*

export PATH="$bin:$PATH"
export FAKE_CLIP="$work/clipboard"
export FAKE_CALLS="$work/calls"
export MARKDOWN_FIX_PASTE_LOG="$work/log"
export MARKDOWN_FIXER_BIN="$bin/fake-fixer"
unset MARKDOWN_FIX_PASTE_NO_PASTE MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS

failures=0
current=""

fail() {
    printf '  FAIL [%s] %s\n' "$current" "$*"
    failures=$((failures + 1))
}

# 設定剪貼簿內容（printf 格式字串，讓測試可以精確控制結尾換行）並清除紀錄。
reset_case() {
    current=$1
    shift
    # shellcheck disable=SC2059 # 刻意把參數當格式字串，方便寫 \n 與 \xc2\xa0。
    printf "$@" >"$FAKE_CLIP"
    cp "$FAKE_CLIP" "$work/original"
    : >"$FAKE_CALLS"
    : >"$MARKDOWN_FIX_PASTE_LOG"
    unset FAKE_FIXER_MODE FAKE_PBPASTE_FAIL FAKE_OSASCRIPT_STATUS
}

run_script() {
    status=0
    "$script" 2>"$work/stderr" || status=$?
}

expect_status() {
    [ "$status" -eq "$1" ] || fail "exit status $status, expected $1"
}

expect_calls() {
    # $1 = 指令名稱，$2 = 預期次數
    count=$(grep -c "^$1 " "$FAKE_CALLS")
    [ "$count" -eq "$2" ] || fail "$1 called $count time(s), expected $2"
}

# 剪貼簿內容必須與 printf 格式產生的位元組完全相同。
expect_clipboard() {
    # shellcheck disable=SC2059
    printf "$@" >"$work/expected"
    cmp -s "$work/expected" "$FAKE_CLIP" || fail "clipboard is $(od -c "$FAKE_CLIP" | head -3)"
}

expect_clipboard_unchanged() {
    cmp -s "$work/original" "$FAKE_CLIP" || fail "clipboard was modified"
}

expect_log_contains() {
    grep -q "$1" "$MARKDOWN_FIX_PASTE_LOG" || fail "log does not mention: $1"
}

expect_log_excludes() {
    if grep -q "$1" "$MARKDOWN_FIX_PASTE_LOG" "$work/stderr"; then
        fail "log or stderr leaked clipboard content: $1"
    fi
}

# ---- 案例 ----

reset_case "empty clipboard" ''
run_script
expect_status 0
expect_calls fixer 0
expect_calls pbcopy 0
expect_calls osascript 0
expect_clipboard_unchanged
expect_log_contains "no text"

reset_case "pbpaste fails" 'keep me'
export FAKE_PBPASTE_FAIL=1
run_script
expect_status 1
expect_calls fixer 0
expect_calls pbcopy 0
expect_calls osascript 0
expect_clipboard_unchanged

reset_case "fix CJK without trailing newline" '#標題\n\n中文段落，沒有檔尾換行'
run_script
expect_status 0
expect_clipboard '# 標題\n\n中文段落，沒有檔尾換行'
expect_calls fixer 1
grep -qx 'fixer --stdin --write --quiet --exit-zero --ignore-rule MDX003' "$FAKE_CALLS" ||
    fail "unexpected fixer arguments: $(grep '^fixer' "$FAKE_CALLS")"
expect_calls pbcopy 1
grep -q 'pbcopy LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8' "$FAKE_CALLS" || fail "pbcopy ran without a UTF-8 locale"
expect_calls osascript 1
grep -q '^osascript -l JavaScript timeout=1000$' "$FAKE_CALLS" || fail "paste was not sent through JXA with the default timeout"
expect_log_contains "fixed: "
expect_log_excludes "標題"
expect_log_excludes "中文段落"

reset_case "trailing blank lines are preserved byte for byte" '#a\n\n\n'
run_script
expect_status 0
expect_clipboard '# a\n\n\n'

reset_case "unchanged text does not rewrite the clipboard" '# Title\n\nBody\n'
export FAKE_FIXER_MODE=identity
run_script
expect_status 0
expect_calls pbcopy 0
expect_calls osascript 1
expect_clipboard_unchanged
expect_log_contains "no changes"

reset_case "fixer failure keeps the clipboard and pastes the original" '#secret text'
export FAKE_FIXER_MODE=fail
run_script
expect_status 1
expect_calls pbcopy 0
expect_calls osascript 1
expect_clipboard_unchanged
expect_log_contains "exited with status 2"
expect_log_excludes "secret"

reset_case "empty fixer output keeps the clipboard" '#secret text'
export FAKE_FIXER_MODE=empty
run_script
expect_status 1
expect_calls pbcopy 0
expect_calls osascript 1
expect_clipboard_unchanged
expect_log_contains "no output"

reset_case "missing binary keeps the clipboard" '#secret text'
MARKDOWN_FIXER_BIN="$work/does-not-exist" run_script
expect_status 1
expect_calls pbcopy 0
expect_calls osascript 1
expect_clipboard_unchanged
expect_log_contains "not found or not executable"

reset_case "MARKDOWN_FIX_PASTE_NO_PASTE skips Cmd+V" '#a'
MARKDOWN_FIX_PASTE_NO_PASTE=1 run_script
expect_status 0
expect_clipboard '# a'
expect_calls osascript 0

reset_case "custom modifier timeout is passed to JXA" '#a'
MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS=250 run_script
grep -q 'timeout=250$' "$FAKE_CALLS" || fail "timeout was not passed through"

reset_case "invalid modifier timeout falls back to the default" '#a'
MARKDOWN_FIX_PASTE_MODIFIER_TIMEOUT_MS='1; rm -rf /' run_script
grep -q 'timeout=1000$' "$FAKE_CALLS" || fail "invalid timeout was not rejected"

reset_case "paste failure is reported" '#a'
export FAKE_OSASCRIPT_STATUS=1
run_script
expect_status 1
expect_clipboard '# a'
expect_log_contains "Accessibility"

reset_case "binary found under HOME/.local/bin without MARKDOWN_FIXER_BIN" '#a'
mkdir -p "$work/home/.local/bin" "$work/home/Library/Logs"
cp "$bin/fake-fixer" "$work/home/.local/bin/markdown-fixer"
HOME="$work/home" MARKDOWN_FIXER_BIN='' run_script
expect_status 0
expect_clipboard '# a'

# ---- 端對端：真正的 markdown-fixer ----

if [ -n "${MARKDOWN_FIXER_TEST_BIN:-}" ]; then
    reset_case "real binary: CJK, NBSP, no trailing newline" '#標題\n\n中文\xc2\xa0段落'
    MARKDOWN_FIXER_BIN="$MARKDOWN_FIXER_TEST_BIN" run_script
    expect_status 0
    expect_clipboard '# 標題\n\n中文 段落'

    reset_case "real binary: unclosed frontmatter is refused, clipboard kept" '---\ntitle: x\n#標題\n'
    MARKDOWN_FIXER_BIN="$MARKDOWN_FIXER_TEST_BIN" run_script
    expect_status 0
    expect_calls pbcopy 0
    expect_clipboard_unchanged

    reset_case "real binary: invalid UTF-8 keeps the clipboard" '#a \xff\n'
    MARKDOWN_FIXER_BIN="$MARKDOWN_FIXER_TEST_BIN" run_script
    expect_status 1
    expect_calls pbcopy 0
    expect_clipboard_unchanged
else
    echo "skip: set MARKDOWN_FIXER_TEST_BIN=/path/to/markdown-fixer to run end-to-end cases"
fi

if [ "$failures" -gt 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "ok: markdown-fix-paste.sh"
