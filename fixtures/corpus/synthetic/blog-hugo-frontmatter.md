---
title: "我如何把部落格從 WordPress 搬到 Hugo"
date: 2026-09-12T21:30:00+08:00
draft: false
tags: ["hugo", "部落格", "靜態網站"]
categories: ["技術筆記"]
summary: "#搬家 心得與踩過的坑"
---
搬家這件事拖了三年，這次終於下定決心。這篇文章記錄整個過程，以及幾個 Hugo 新手很容易踩到的坑。

<!--more-->

## 為什麼離開 WordPress？

說穿了只有三個原因：
- 主機費用每年漲
- 外掛更新常常把網站弄壞
- 我想用 Markdown 寫文章，不想再用區塊編輯器

## 匯出舊文章

我用 [wordpress-export-to-markdown](https://github.com/lonekorean/wordpress-export-to-markdown) 這個工具，一次把 300 多篇文章轉成 Markdown。轉完之後，問題才開始：

1. 圖片路徑全部是絕對網址
2. 程式碼區塊變成 `<pre>` 標籤
3. 很多文章開頭多了空行

### 圖片路徑

用一行 `sed` 解決：

```bash
find content/posts -name '*.md' -exec sed -i '' 's#https://old.example.com/wp-content/uploads#/images#g' {} +
```

### shortcode

Hugo 的 shortcode 長這樣，記得放在 code block 裡才不會被渲染：

```go-html-template
{{</* figure src="/images/cover.png" title="封面" */>}}
```

文章內直接使用：

{{< figure src="/images/hugo-logo.png" title="Hugo" >}}

## 設定 `config.toml`

```toml
baseURL = "https://blog.example.com/"
languageCode = "zh-tw"
title = "保哥的技術筆記"
theme = "PaperMod"

[params]
  ShowReadingTime = true
```

## 結論

整體來說搬家很值得，網站速度從 2.8 秒降到 0.4 秒 ！如果你也在考慮，可以先從**匯出一小部分文章**開始試。

有問題歡迎在下方留言，或寄信到 blog@example.com。
