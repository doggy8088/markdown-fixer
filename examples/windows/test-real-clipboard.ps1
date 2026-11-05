# test-real-clipboard.ps1 — 在真正的 Windows 剪貼簿上測試 markdown-fix-paste.ahk。
#
# 以腳本的非互動測試模式（--fix-clipboard：只修正剪貼簿一次、不送出 Ctrl+V）執行，
# 再逐位元組檢查剪貼簿。會覆寫你的剪貼簿。需要 Windows PowerShell 5.1（STA）與
# AutoHotkey v2：
#
#   powershell -NoProfile -File examples\windows\test-real-clipboard.ps1 `
#       -AutoHotkey C:\path\AutoHotkey64.exe -Fixer target\debug\markdown-fixer.exe
#
# 測試字串以 UTF-8 十六進位寫成，比對的就是確切的位元組。這個檔案存成 UTF-8 with BOM，
# 否則 Windows PowerShell 5.1 會用 ANSI 字碼頁讀取。說明見 docs/clipboard.md。

param(
    [Parameter(Mandatory = $true)][string]$AutoHotkey,
    [Parameter(Mandatory = $true)][string]$Fixer
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$ahkScript = Join-Path $PSScriptRoot 'markdown-fix-paste.ahk'
$Fixer = (Resolve-Path $Fixer).Path
$work = Join-Path ([IO.Path]::GetTempPath()) ("test-real-clipboard-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
$failures = 0

function FromHex([string]$hex) {
    $bytes = [byte[]]::new($hex.Length / 2)
    for ($i = 0; $i -lt $bytes.Length; $i++) {
        $bytes[$i] = [Convert]::ToByte($hex.Substring($i * 2, 2), 16)
    }
    return [Text.Encoding]::UTF8.GetString($bytes)
}

function ToHex([string]$text) {
    if ($null -eq $text) { return '<null>' }
    return ([BitConverter]::ToString([Text.Encoding]::UTF8.GetBytes($text)) -replace '-', '').ToLowerInvariant()
}

function Set-ClipboardObject($data) {
    # copy = true：這個行程結束後資料仍留在剪貼簿。
    [System.Windows.Forms.Clipboard]::SetDataObject($data, $true, 20, 100)
}

function Set-ClipboardText([string]$text, [string]$html = $null) {
    $data = New-Object System.Windows.Forms.DataObject
    $data.SetText($text, [System.Windows.Forms.TextDataFormat]::UnicodeText)
    if ($html) { $data.SetText($html, [System.Windows.Forms.TextDataFormat]::Html) }
    Set-ClipboardObject $data
}

function Get-ClipboardText {
    if (-not [System.Windows.Forms.Clipboard]::ContainsText([System.Windows.Forms.TextDataFormat]::UnicodeText)) {
        return $null
    }
    return [System.Windows.Forms.Clipboard]::GetText([System.Windows.Forms.TextDataFormat]::UnicodeText)
}

function Invoke-FixClipboard([string]$fixerPath) {
    $env:MARKDOWN_FIXER_BIN = $fixerPath
    $stderr = Join-Path $work 'ahk.err'
    $process = Start-Process -FilePath $AutoHotkey `
        -ArgumentList @('/ErrorStdOut', "`"$ahkScript`"", '--fix-clipboard') `
        -Wait -PassThru -NoNewWindow -RedirectStandardError $stderr
    $message = Get-Content -Raw -LiteralPath $stderr -ErrorAction SilentlyContinue
    if ($message) { Write-Host "  ahk stderr: $message" }
    return $process.ExitCode
}

function Fail([string]$name, [string]$message) {
    Write-Host "  FAIL [$name] $message"
    $script:failures++
}

function Assert-Equal([string]$name, [string]$what, $expected, $actual) {
    if ($expected -ne $actual) { Fail $name "$what`n    expected: $expected`n    actual:   $actual" }
}

# 執行後剪貼簿的文字必須正好是 $expectedHex（UTF-8）。
function Test-Text([string]$name, [string]$inputHex, [string]$expectedHex, [int]$expectedStatus, [string]$fixerPath = $Fixer) {
    Write-Host "case: $name"
    Set-ClipboardText (FromHex $inputHex)
    $status = Invoke-FixClipboard $fixerPath
    Assert-Equal $name 'exit code' $expectedStatus $status
    Assert-Equal $name 'clipboard (UTF-8 hex)' $expectedHex (ToHex (Get-ClipboardText))
}

$crlfIn = '23e6a899e9a18c0d0a2a2ae9878de9bb9eefbc9a2a2ae585a7e5aeb920f09f98800d0a'
$crlfOut = '2320e6a899e9a18c0d0a0d0a2a2ae9878de9bb9e2a2aefbc9ae585a7e5aeb920f09f98800d0a'
$lfIn = '23e6a899e9a18c0a2a2ae9878de9bb9eefbc9a2a2ae585a7e5aeb920f09f98800a'
$lfOut = '2320e6a899e9a18c0a0a2a2ae9878de9bb9e2a2aefbc9ae585a7e5aeb920f09f98800a'

try {
    # 先確認這個工作階段有可用的剪貼簿。
    Set-ClipboardText 'probe'
    if ((Get-Clipboard -Raw) -ne 'probe') { throw 'clipboard probe failed: this session has no usable clipboard' }

    # 1. CJK 與 emoji、CRLF 行尾：修正後仍是 CRLF（--ignore-rule MDX002）。
    Test-Text 'crlf-cjk-fixed' $crlfIn $crlfOut 0
    # 2. LF 行尾維持 LF。
    Test-Text 'lf-cjk-fixed' $lfIn $lfOut 0

    # 3. 內容不需修正：不重寫剪貼簿，HTML 格式保留。
    $name = 'unchanged-keeps-html'
    Write-Host "case: $name"
    $html = "Version:0.9`r`nStartHTML:-1`r`nEndHTML:-1`r`nStartFragment:-1`r`nEndFragment:-1`r`n<b>x</b>"
    Set-ClipboardText (FromHex $crlfOut) $html
    $status = Invoke-FixClipboard $Fixer
    Assert-Equal $name 'exit code' 0 $status
    Assert-Equal $name 'clipboard (UTF-8 hex)' $crlfOut (ToHex (Get-ClipboardText))
    Assert-Equal $name 'HTML format still present' $true ([System.Windows.Forms.Clipboard]::ContainsText([System.Windows.Forms.TextDataFormat]::Html))

    # 4. 找不到 markdown-fixer：結束碼 1，原剪貼簿保留。
    Test-Text 'missing-fixer-keeps-original' $crlfIn $crlfIn 1 (Join-Path $work 'no-such-fixer.exe')

    # 5. markdown-fixer 沒有輸出：結束碼 1，原剪貼簿保留。
    $emptyFixer = Join-Path $work 'empty-fixer.cmd'
    Set-Content -LiteralPath $emptyFixer -Value "@echo off`r`nexit /b 0" -Encoding ASCII
    Test-Text 'empty-output-keeps-original' $crlfIn $crlfIn 1 $emptyFixer

    # 6. markdown-fixer 失敗：結束碼 1，原剪貼簿保留。
    $failingFixer = Join-Path $work 'failing-fixer.cmd'
    Set-Content -LiteralPath $failingFixer -Value "@echo off`r`nexit /b 2" -Encoding ASCII
    Test-Text 'failing-fixer-keeps-original' $crlfIn $crlfIn 1 $failingFixer

    # 7. 只有圖片：不動作，圖片保留，也不會多出文字。
    $name = 'image-untouched'
    Write-Host "case: $name"
    $bitmap = New-Object System.Drawing.Bitmap 2, 2
    $bitmap.SetPixel(0, 0, [System.Drawing.Color]::Red)
    $data = New-Object System.Windows.Forms.DataObject
    $data.SetImage($bitmap)
    Set-ClipboardObject $data
    $status = Invoke-FixClipboard $Fixer
    Assert-Equal $name 'exit code' 0 $status
    Assert-Equal $name 'image still present' $true ([System.Windows.Forms.Clipboard]::ContainsImage())
    Assert-Equal $name 'no text' $false ([System.Windows.Forms.Clipboard]::ContainsText())
    $image = [System.Windows.Forms.Clipboard]::GetImage()
    if ($null -eq $image) { Fail $name 'cannot read the image back' }
    else { Assert-Equal $name 'pixel (0,0)' ([System.Drawing.Color]::Red.ToArgb()) ($image.GetPixel(0, 0).ToArgb()) }

    # 8. 空的剪貼簿：不動作，維持空的。
    $name = 'empty-clipboard'
    Write-Host "case: $name"
    [System.Windows.Forms.Clipboard]::Clear()
    $status = Invoke-FixClipboard $Fixer
    Assert-Equal $name 'exit code' 0 $status
    Assert-Equal $name 'no text' $false ([System.Windows.Forms.Clipboard]::ContainsText())
}
finally {
    Remove-Item -Recurse -Force -LiteralPath $work -ErrorAction SilentlyContinue
}

if ($failures -gt 0) {
    Write-Host "$failures failure(s)"
    exit 1
}
Write-Host 'all real clipboard cases passed'
