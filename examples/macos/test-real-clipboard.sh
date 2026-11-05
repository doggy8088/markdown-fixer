#!/bin/bash
# test-real-clipboard.sh（macOS）— 在真正的剪貼簿（pbcopy／pbpaste）上測試 markdown-fix-paste.sh。
#
# 會覆寫目前的剪貼簿。以 MARKDOWN_FIX_PASTE_NO_PASTE=1 執行腳本，不送出 Cmd+V，
# 也就不需要輔助使用權限：
#
#   MARKDOWN_FIXER_BIN=$PWD/target/debug/markdown-fixer examples/macos/test-real-clipboard.sh
#
# 用假剪貼簿測試腳本邏輯的是 test-markdown-fix-paste.sh；這支只驗證真正的剪貼簿。

set -u

here=$(cd "$(dirname "$0")" && pwd)
script="$here/markdown-fix-paste.sh"
fixer=${MARKDOWN_FIXER_BIN:?set MARKDOWN_FIXER_BIN to the markdown-fixer binary}

work=$(mktemp -d "${TMPDIR:-/tmp}/test-real-clipboard.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT

# 測試本身呼叫 pbcopy／pbpaste 時也要 UTF-8，否則 CJK 在進腳本前就變成 MacRoman。
export LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8
export MARKDOWN_FIX_PASTE_NO_PASTE=1
export MARKDOWN_FIX_PASTE_LOG=/dev/null

# 1x1 的 PNG。
printf '%s' 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==' |
    base64 --decode >"$work/image.png"

# 以 NSPasteboard 設定剪貼簿：argv = [純文字檔, HTML 字串]（HTML 可省略）。
clip_set_text_html() {
    osascript -l JavaScript - "$@" >/dev/null <<'EOF'
ObjC.import("AppKit");
function run(argv) {
    var pb = $.NSPasteboard.generalPasteboard;
    pb.clearContents;
    var text = $.NSString.stringWithContentsOfFileEncodingError(argv[0], $.NSUTF8StringEncoding, null);
    pb.setStringForType(text, $.NSPasteboardTypeString);
    if (argv.length > 1) pb.setStringForType($(argv[1]), $.NSPasteboardTypeHTML);
}
EOF
}

clip_set_image() {
    osascript -l JavaScript - "$1" >/dev/null <<'EOF'
ObjC.import("AppKit");
function run(argv) {
    var pb = $.NSPasteboard.generalPasteboard;
    pb.clearContents;
    pb.setDataForType($.NSData.dataWithContentsOfFile(argv[0]), $.NSPasteboardTypePNG);
}
EOF
}

clip_clear() {
    osascript -l JavaScript -e 'ObjC.import("AppKit"); $.NSPasteboard.generalPasteboard.clearContents; ""' >/dev/null
}

# 剪貼簿上的型別，一行一個。
clip_types() {
    osascript -l JavaScript -e '
ObjC.import("AppKit");
var types = $.NSPasteboard.generalPasteboard.types;
var out = [];
for (var i = 0; i < types.count; i++) out.push(ObjC.unwrap(types.objectAtIndex(i)));
out.join("\n");'
}

clip_get_image() {
    osascript -l JavaScript - "$1" >/dev/null <<'EOF'
ObjC.import("AppKit");
function run(argv) {
    var data = $.NSPasteboard.generalPasteboard.dataForType($.NSPasteboardTypePNG);
    if (data.isNil()) throw new Error("no PNG on the clipboard");
    data.writeToFileAtomically(argv[0], true);
}
EOF
}

failures=0
current=""

fail() {
    printf '  FAIL [%s] %s\n' "$current" "$*"
    failures=$((failures + 1))
}

run_script() { # [markdown-fixer 路徑]
    status=0
    MARKDOWN_FIXER_BIN=${1:-$fixer} "$script" >"$work/stdout" 2>"$work/stderr" || status=$?
    sed 's/^/    stderr: /' "$work/stderr"
}

expect_status() {
    [ "$status" -eq "$1" ] || fail "exit status $status, expected $1"
}

expect_clipboard_text() { # <預期檔>
    pbpaste >"$work/actual"
    cmp -s "$1" "$work/actual" && return 0
    fail "clipboard differs"
    echo "    expected:" && od -An -tx1 "$1" | sed 's/^/     /'
    echo "    actual:" && od -An -tx1 "$work/actual" | sed 's/^/     /'
}

printf '#標題\n**重點：**內容 😀\n' >"$work/messy"
printf '# 標題\n\n**重點**：內容 😀\n' >"$work/fixed"
printf '#!/bin/sh\ncat >/dev/null\nexit 0\n' >"$work/empty-fixer"
chmod +x "$work/empty-fixer"

current=probe
pbcopy <"$work/fixed"
expect_clipboard_text "$work/fixed"
if [ "$failures" -ne 0 ]; then
    echo "pbcopy/pbpaste round trip failed: this session has no usable clipboard"
    exit 1
fi

current=cjk-fixed
echo "case: $current"
pbcopy <"$work/messy"
run_script
expect_status 0
expect_clipboard_text "$work/fixed"

current=unchanged-keeps-html
echo "case: $current"
clip_set_text_html "$work/fixed" '<b>x</b>'
run_script
expect_status 0
expect_clipboard_text "$work/fixed"
clip_types | grep -qx public.html || fail "HTML flavor was dropped"

current=failing-fixer-keeps-original
echo "case: $current"
pbcopy <"$work/messy"
run_script /usr/bin/false
expect_status 1
expect_clipboard_text "$work/messy"

current=missing-fixer-keeps-original
echo "case: $current"
pbcopy <"$work/messy"
run_script "$work/no-such-fixer"
expect_status 1
expect_clipboard_text "$work/messy"

current=empty-output-keeps-original
echo "case: $current"
pbcopy <"$work/messy"
run_script "$work/empty-fixer"
expect_status 1
expect_clipboard_text "$work/messy"

current=image-untouched
echo "case: $current"
clip_set_image "$work/image.png"
run_script
expect_status 0
clip_types >"$work/types"
grep -qx public.png "$work/types" || fail "PNG no longer on the clipboard: $(tr '\n' ' ' <"$work/types")"
grep -qx public.utf8-plain-text "$work/types" && fail "text appeared: $(tr '\n' ' ' <"$work/types")"
if ! { clip_get_image "$work/image-after" && cmp -s "$work/image.png" "$work/image-after"; }; then
    fail "image bytes changed"
fi

current=empty-clipboard
echo "case: $current"
clip_clear
run_script
expect_status 0
[ -z "$(clip_types)" ] || fail "clipboard no longer empty: $(clip_types | tr '\n' ' ')"

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all real clipboard cases passed (macOS)"
