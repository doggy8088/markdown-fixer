# markdown-fixer-cli

[`markdown-fixer`](https://crates.io/crates/markdown-fixer) 的命令列工具，執行檔名稱是 `markdown-fixer`。不需要 Node.js，旗標、文字輸出、JSON 與結束碼和 `markdown-autocorrect` skill 的 JS 版相同。

```sh
cargo install markdown-fixer-cli

markdown-fixer --check README.md docs/       # 只檢查（預設）
markdown-fixer --write README.md             # 只寫入明確指定的檔案
markdown-fixer --check --json docs/          # 給 CI 用的 JSON
markdown-fixer --list-rules                  # 列出規則

pbpaste | markdown-fixer --stdin --write --quiet --exit-zero --ignore-rule MDX003 | pbcopy
```

結束碼：`0` 沒有需要處理的問題；`1` 仍有 warning／error；`2` 用法、I/O 或解析錯誤。

完整說明、剪貼簿熱鍵範例與規則參考見 [GitHub repo](https://github.com/doggy8088/markdown-fixer)。

授權：MIT。
