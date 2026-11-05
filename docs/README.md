# 文件索引

`markdown-fixer` 的使用與開發文件。專案簡介、安裝與快速上手在 repo 根目錄的 [README.md](../README.md)，需求規格在 [PRD.md](../PRD.md)。

## 使用者

| 文件 | 內容 |
| --- | --- |
| [rules.md](rules.md) | 48 條規則：偵測什麼、能不能自動修、修正前後範例、什麼時候可以忽略、CJK 注意事項 |
| [cli.md](cli.md) | CLI 旗標、輸出格式、JSON 結構、結束碼、`--stdin` 與 `--detect` |
| [clipboard.md](clipboard.md) | Karabiner-Elements 剪貼簿熱鍵，以及 Linux、Windows 的對應做法 |
| [dedent-paste.md](dedent-paste.md) | dedent-paste 怎麼在貼上前正規化 Markdown |

## 整合開發者

| 文件 | 內容 |
| --- | --- |
| [library.md](library.md) | Rust 函式庫 API、選項、serde、片段處理、執行緒安全 |
| [ffi.md](ffi.md) | C ABI、標頭檔、記憶體管理、其他語言的呼叫範例 |
| [detect.md](detect.md) | Markdown 偵測的訊號、門檻與調整方式 |

## 貢獻者

| 文件 | 內容 |
| --- | --- |
| [contributing.md](contributing.md) | 開發流程、品質關卡、commit 與文件寫作規範 |
| [architecture.md](architecture.md) | crate 結構、處理流程、保護區域、欄號模型、相依政策 |
| [porting-map.md](porting-map.md) | JS 函式與 Rust 函式的對照表 |
| [testing.md](testing.md) | 測試架構：移植測試、golden 語料、差異測試、property test |
| [parity.md](parity.md) | 與 oracle 的一致性：刻意差異、oracle 疑似問題、升版流程 |
| [debugging.md](debugging.md) | 重現、縮小、修正 parity 差異的步驟與常見原因 |
| [release.md](release.md) | 版本號、建置與發佈流程 |

## 從哪裡開始

- **想檢查或修正文件**：先看根目錄的 [README.md](../README.md)，再查 [rules.md](rules.md)。
- **想在程式裡呼叫**：Rust 看 [library.md](library.md)，其他語言看 [ffi.md](ffi.md)。
- **想修 bug 或移植規則**：依序看 [contributing.md](contributing.md)、[architecture.md](architecture.md)、[porting-map.md](porting-map.md)、[debugging.md](debugging.md)。
- **AI coding agent**：先讀根目錄的 [AGENTS.md](../AGENTS.md)。
