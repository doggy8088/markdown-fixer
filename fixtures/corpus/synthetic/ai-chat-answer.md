### 簡短回答

可以，用 `git rebase -i` 就能把多個 commit 合併成一個。**但要注意：**已經 push 到共用分支的 commit 不建議 rebase。

### 詳細步驟

1. **找出要合併的範圍**
   ```bash
   git log --oneline -5
   ```
2. **開始互動式 rebase**
   ```bash
   git rebase -i HEAD~3
   ```
3. **編輯指令清單：**把第二行之後的 `pick` 改成 `squash`（或 `s`）
   ```
   pick a1b2c3d 新增登入頁
   squash d4e5f6a 修正錯字
   squash 0718293 調整樣式
   ```
4. **儲存並編輯 commit 訊息**
5. **強制推送（如果已經 push 過）：**
   ```bash
   git push --force-with-lease
   ```

### 常見問題

**Q：rebase 到一半想放棄怎麼辦？**
A：執行 `git rebase --abort`，會回到 rebase 前的狀態。

**Q：`--force` 和 `--force-with-lease` 差在哪？**
A：`--force-with-lease` 會先確認遠端沒有別人推的新 commit，比較安全。

### 小結
| 情境 | 建議做法 |
|------|----------|
| 本地還沒 push | 放心 rebase |
| 已 push 到個人分支 | rebase 後 `--force-with-lease` |
| 已 push 到 main | **不要** rebase，改用 revert |

> 💡 **提示：**如果你只是想讓 PR 看起來乾淨，GitHub 的「Squash and merge」按鈕可以達到一樣的效果 。

希望這有幫助！如果還有問題，歡迎繼續問 😊
