---
marp: true
theme: default
paginate: true
---

<!-- _class: lead -->

# Markdown 自動修正
## 從 Node 到 Rust

保哥｜2026 技術分享
---

## 為什麼要移植？

- Node 啟動要數十毫秒
- 剪貼簿熱鍵需要即時回應
- Rust 應用程式想直接呼叫

---

## 架構

```text
┌──────────┐   ┌──────────┐
│  CLI     │──▶│ library  │
└──────────┘   └──────────┘
                    ▲
┌──────────┐        │
│  C ABI   │────────┘
└──────────┘
```

---

## 一致性怎麼驗證？

1. Golden 語料
2. 即時差異比對
3. 隨機文件 fuzz
---

<!-- _backgroundColor: #123 -->

## 成果

| 指標 | Node | Rust |
| --- | --- | --- |
| 啟動時間 | 45 ms | 2 ms |
| 1 MB 文件 | 310 ms | 38 ms |

---

# 謝謝！
Q & A
