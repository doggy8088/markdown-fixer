; markdown-fix-paste.ahk — 把剪貼簿裡的 Markdown 用 markdown-fixer 修正後貼上（AutoHotkey v2）。
;
; macOS 版 examples/macos/markdown-fix-paste.sh 的 Windows 對應寫法，說明見 docs/clipboard.md。
; 預設熱鍵：左 Alt+Shift+V。想開機自動執行，把這個檔案的捷徑放進 shell:startup。
;
; 執行檔路徑依序找：環境變數 MARKDOWN_FIXER_BIN、%USERPROFILE%\.local\bin\markdown-fixer.exe、
; %USERPROFILE%\.cargo\bin\markdown-fixer.exe。
;
; 注意：
; - 以暫存檔傳遞內容，寫入用 UTF-8-RAW（不加 BOM），讀回用 UTF-8。
; - Windows 剪貼簿的換行是 CRLF。加上 --ignore-rule MDX002 時，markdown-fixer 會照原文
;   的行尾還原，不會把 CRLF 改成 LF。
; - 修正失敗或輸出為空時保留原剪貼簿，照樣貼上原文。

#Requires AutoHotkey v2.0
#SingleInstance Force

<!+v:: MarkdownFixPaste()

FindFixer() {
    path := EnvGet("MARKDOWN_FIXER_BIN")
    if (path != "")
        return path
    profile := EnvGet("USERPROFILE")
    for candidate in [profile "\.local\bin\markdown-fixer.exe", profile "\.cargo\bin\markdown-fixer.exe"] {
        if FileExist(candidate)
            return candidate
    }
    return "markdown-fixer.exe"
}

MarkdownFixPaste() {
    ; 13 = CF_UNICODETEXT。剪貼簿沒有文字（例如只有圖片）時什麼都不做。
    if !DllCall("IsClipboardFormatAvailable", "UInt", 13)
        return
    original := A_Clipboard
    if (original = "")
        return

    stamp := A_TickCount
    inFile := A_Temp "\markdown-fix-paste-" stamp ".in.md"
    outFile := A_Temp "\markdown-fix-paste-" stamp ".out.md"
    try {
        FileAppend(original, inFile, "UTF-8-RAW")
        fixer := FindFixer()
        ; cmd /c 的外層引號讓含空白的路徑正常運作。
        command := A_ComSpec ' /c ""' fixer '" --stdin --write --quiet --exit-zero'
            . ' --ignore-rule MDX003,MDX002 < "' inFile '" > "' outFile '""'
        exitCode := RunWait(command, , "Hide")
        if (exitCode = 0 && FileExist(outFile)) {
            fixed := FileRead(outFile, "UTF-8")
            ; !== 是區分大小寫的比較；內容沒變就不重寫剪貼簿，保留 RTF／HTML 格式。
            if (fixed != "" && fixed !== original)
                A_Clipboard := fixed
        }
    } catch {
        ; 任何錯誤都保留原剪貼簿，不顯示內容。
    } finally {
        try FileDelete(inFile)
        try FileDelete(outFile)
    }

    ; 等熱鍵的修飾鍵放開，避免送出 Alt+Shift+Ctrl+V。
    KeyWait("LAlt", "T1")
    KeyWait("Shift", "T1")
    Send("^v")
}
