# 疑難排解 FAQ

## 安裝問題

### 執行時出現「找不到指令」？

請確認安裝目錄有加進 `PATH`：

```sh
echo $PATH | tr ':' '\n' | grep -n local
```

### macOS 說「無法打開，因為無法驗證開發者」。

到「系統設定 → 隱私權與安全性」，按下「強制打開」。或在終端機執行：

```
xattr -d com.apple.quarantine /usr/local/bin/mytool
```

## 執行問題

### 為什麼輸出是亂碼?
>多半是語系沒設好。在 `~/.zshrc` 加上：
>
>     export LANG=en_US.UTF-8
>
>然後重開終端機。

### 剪貼簿沒有變化！

| 原因 | 檢查方式 | 解法 |
| --- | --- | --- |
| 沒有輔助使用權限 | 系統設定 → 輔助使用 | 把終端機加進清單 |
| 剪貼簿是圖片 | `pbpaste` 沒有輸出 | 先複製文字 |
| 執行檔路徑錯誤 | `which mytool` | 用絕對路徑 |
| Karabiner 沒啟用 | Karabiner-Elements → Complex Modifications | 重新啟用規則 | 重開 |

### 結束碼代表什麼

- `0`：沒有問題
- `1`：還有 warning 或 error
- `2`：用法錯誤或 I/O 錯誤
-

## 回報問題

請附上：

1. 作業系統與版本
2. `mytool --version` 的輸出
3. 可以重現問題的最小輸入
回報網址：https://github.com/example/mytool/issues/new

---

還是解決不了？寄信到 support@example.com
