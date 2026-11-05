---
aliases: [Rust 所有權, ownership]
tags: [rust, 學習筆記]
created: 2026-08-20
---

# Rust 所有權筆記

#rust #學習筆記

相關筆記：[[Rust 借用規則]]、[[生命週期|Lifetime]]、[[Rust 智慧指標#Box]]

> [!note] 核心觀念
> 每個值都有一個擁有者（owner），擁有者離開作用域時，值就會被釋放。

> [!warning]
>移動（move）之後，原本的變數就不能再用了。

## 三條規則

1. 每個值都有一個擁有者
2. 同一時間只能有一個擁有者
3. 擁有者離開作用域，值就被 drop

## 範例

```rust
let s1 = String::from("hello");
let s2 = s1; // s1 被移動
// println!("{}", s1); // 編譯錯誤
```

![[ownership-diagram.png]]

## 疑問
- [ ] `Copy` 和 `Clone` 的差別？→ 見 [[Copy vs Clone]]
- [x] 為什麼 `&str` 不用 move？
- [ ] 

%%
這段是 Obsidian 的註解，不會顯示在預覽裡。
#TODO 補上 Rc 的例子
%%

## 參考
- The Book 第 4 章 https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
- ^block-ref-123
