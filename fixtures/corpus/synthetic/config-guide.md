# 設定檔指南

本工具會依序讀取以下設定檔，後面的覆寫前面的：

1. `/etc/mytool/config.toml`
2. `~/.config/mytool/config.toml`
3. 專案目錄下的 `.mytool.toml`

## TOML 格式

```toml
# 全域設定
[general]
log_level = "info"   # debug | info | warn | error
color = true

[paths]
cache = "~/.cache/mytool"
```

## YAML 格式（舊版）

舊版用 YAML，仍然支援但不建議：

```yaml
general:
  log_level: info
  color: true
# 註解
paths:
  cache: ~/.cache/mytool
```

## 環境變數

| 變數 | 對應設定 | 預設值 |
| --- | --- | --- |
| `MYTOOL_LOG` | `general.log_level` | `info` |
| `MYTOOL_COLOR` | `general.color` | `true` |
| `MYTOOL_CACHE` | `paths.cache` | `~/.cache/mytool` |
| `NO_COLOR` | 停用顏色 | 未設定 |

## Makefile 整合

```make
lint:
	mytool --check docs/

fix:
	mytool --write docs/
```

舊專案的 Makefile 也可以直接縮排四格貼上：

    lint:
    	mytool --check docs/

## INI 格式？

不支援。如果你看到 `config.ini`，那是 v0.x 的遺留檔案，可以刪除。

## 驗證設定

```sh
mytool config --validate
```

輸出	範例：

```
✔ /etc/mytool/config.toml
✔ ~/.config/mytool/config.toml
✘ .mytool.toml: unknown key "colour" (did you mean "color"?)
```
