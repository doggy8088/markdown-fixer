# Code Review 報告：PR #482「重構付款模組」

**審查執行：**2026-11-04
**審查範圍：**`src/payment/**`（共 14 個檔案，+812/−640）
**整體評估：**⚠️ 需要修改後再合併

---

## 摘要

| 嚴重度 | 數量 |
|---|---|
| 🔴 嚴重 | 2 |
| 🟠 中等 | 4 |
| 🟢 建議 | 6 |

## 🔴 嚴重問題

### 1. **重複扣款風險：**`charge()` 沒有冪等保護

**位置：**`src/payment/charge.rs:88`
**問題：**網路逾時後重試會再呼叫一次金流 API，可能重複扣款。
**建議：**帶上 `Idempotency-Key`，並在本地記錄請求狀態。

```rust
// 修改前
let resp = gateway.charge(&req).await?;

// 修改後
let key = IdempotencyKey::for_order(order.id);
let resp = gateway.charge_with_key(&req, &key).await?;
```

### 2. **金額精度：**用 `f64` 存金額

**位置：**`src/payment/model.rs:12`
**問題：**`0.1 + 0.2 != 0.3`，累積誤差會造成對帳不平。
**建議：**改用整數（以「分」為單位）或 `rust_decimal`。

## 🟠 中等問題

1.**錯誤訊息洩漏內部資訊：**`format!("{:?}", err)` 直接回給前端
2. **缺少逾時設定：**HTTP client 沒有設定 timeout，預設會等到天荒地老
3. **日誌含個資：**`info!("card={}", card_number)` 會把卡號寫進 log
4. **測試覆蓋不足：**退款流程沒有任何測試

## 🟢 建議

- 函式 `process_refund_and_notify_user_and_update_ledger` 太長，建議拆分
- `utils.rs` 這個檔名太籠統
* 可以用 `thiserror` 簡化錯誤型別
- **命名：** `amt` 改成 `amount` 比較好讀
- **註解：**有幾處註解還是舊邏輯，請一併更新
- 考慮加上 `#[must_use]`

## 結論：

修正兩個🔴嚴重問題後即可合併，其他項目可以開 follow-up issue。

**審查者：**AI Reviewer（由 @tech-lead 覆核）
