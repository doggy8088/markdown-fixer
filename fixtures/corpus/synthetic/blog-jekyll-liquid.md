---
layout: post
title:  "Jekyll 的 Liquid 模板入門"
date:   2026-04-03 10:00:00 +0800
categories: jekyll liquid
permalink: /:categories/:year/:month/:title/
---

Jekyll 用 [Liquid](https://shopify.github.io/liquid/) 當模板語言。這篇整理我最常用的語法。

## 變數輸出

用兩個大括號輸出變數：{{ page.title }}，網站網址是 {{ site.url }}{{ site.baseurl }}。

## 迴圈

{% for post in site.posts limit:5 %}
- [{{ post.title }}]({{ post.url | relative_url }})
{% endfor %}

## 條件判斷

{% if page.comments %}
<div id="disqus_thread"></div>
{% endif %}

## 程式碼高亮

舊的寫法：

{% highlight ruby linenos %}
def greet(name)
  puts "Hello, #{name}"
end
{% endhighlight %}

新的寫法直接用 fenced code：

```ruby
def greet(name)
  puts "Hello, #{name}"
end
```

## Front matter 預設值

在 `_config.yml` 設定：

```yaml
defaults:
  - scope:
      path: ""
      type: "posts"
    values:
      layout: "post"
      comments: true
```

## 小技巧
- `{{ content | number_of_words }}` 算字數
-`{{ page.date | date: "%Y-%m-%d" }}` 格式化日期
- `{% raw %}` 與 `{% endraw %}` 可以讓 Liquid 不處理某段文字

---
本文最後更新：2026-04-05
