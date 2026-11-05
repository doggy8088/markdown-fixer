# 訂單服務 API 參考文件 #

本文件說明訂單服務（Order Service）v2 的 REST API。所有端點都以 `https://api.example.com/v2` 為前綴，回應格式一律是 JSON。
完整的 OpenAPI 規格請見 https://api.example.com/v2/openapi.json 。

## 認證

所有請求都必須帶上 Bearer token：

```http
GET /v2/orders HTTP/1.1
Host: api.example.com
Authorization: Bearer <token>
```

**注意：**token 有效期限為 1 小時，過期後請用 refresh token 換新。

##  端點一覽

| 方法 | 路徑 | 說明 |
| --- | --- | --- |
| GET | `/orders` | 列出訂單 |
| GET | `/orders/{id}` | 取得單筆訂單 |
| POST | `/orders` | 建立訂單 |
| PATCH | `/orders/{id}` | 更新訂單狀態 |
| DELETE | `/orders/{id}` | 取消訂單 | 僅限草稿 |

## 列出訂單
`GET /orders`

### 查詢參數

| 參數 | 型別 | 必填 | 說明 |
|---|:-:|--|---|
| `status` | string | 否 | `draft`、`paid` 或 `shipped` |
| `limit` | integer | 否 | 每頁筆數，預設 20，上限 100 |
| `cursor` | string | 否 | 分頁游標 |
| `q` | string | 否 | 支援 `name|email` 這種 OR 查詢 |

### 回應範例

```json
{
  "data": [
    { "id": "ord_123", "status": "paid", "total": 1280 }
  ],
  "next_cursor": "eyJpZCI6Im9yZF8xMjMifQ=="
}
```

#### 錯誤碼

- `400`：參數格式錯誤
- `401`：token 無效或過期
* `429`：超過速率限制，請參考 `Retry-After` 標頭

## 建立訂單

`POST /orders`

**請求主體：**

```
{
  "items": [{ "sku": "A-001", "qty": 2 }],
  "note": "請於週五前出貨"
}
```

**回應：**`201 Created`，主體為新建立的訂單。

> **提示：**建立訂單是冪等的。帶上 `Idempotency-Key` 標頭，重送同一個請求不會建立第二筆訂單。

## 速率限制

每個 API key 每分鐘最多 600 次請求。超過時回傳 `429`，並在 `Retry-After` 標頭告訴你要等幾秒。

## 變更紀錄

- 2026-10-01：新增 `cursor` 分頁，`page` 參數標記為 deprecated。
- 2026-08-15：`DELETE /orders/{id}` 只能取消草稿狀態的訂單。

相關文件：[錯誤處理指南][errors]、[Webhook 設定][webhooks]。

[errors]: https://docs.example.com/errors
[webhooks]: https://docs.example.com/webhooks
[errors]: https://docs.example.com/errors-v1
