/*
 * markdown_fixer.h — markdown-fixer 的 C ABI（手寫標頭檔）。
 *
 * 對應 crates/markdown-fixer-ffi/src/lib.rs，規格見 PRD.md〈6.5 C ABI 共用函式庫〉，
 * 使用說明見 docs/ffi.md。
 *
 * 資料交換約定：
 *
 *   - 輸入文字以「指標＋長度」傳入，必須是 UTF-8，不需要 NUL 結尾，可以含 NUL 位元組。
 *     text 為 NULL 時 len 必須是 0（代表空字串）。
 *   - options_json 是 NUL 結尾的 UTF-8 JSON，NULL 表示全部預設。格式：
 *       {"rules":["MDX001",...],"ignoreRules":["MDX003",...],"maxLineLength":120}
 *     三個鍵都可以省略或設成 null；鍵名與 JS 的 options 相同。
 *       - rules：只啟用這些規則。空陣列表示全部停用（與 JS 相同）。
 *       - ignoreRules：停用這些規則。
 *       - maxLineLength：MDX090 行長上限，非負整數，0 表示停用。與 CLI 不同，
 *         rules 指定 MDX090 時不會自動套用 120。
 *     未知的鍵、型別錯誤與未知的規則 ID 都是錯誤。
 *   - 回傳的 char * 一律是 NUL 結尾的 UTF-8 JSON 物件，永遠不是 NULL。
 *     失敗時是 {"error":"訊息"}；請先檢查有沒有 "error" 鍵。
 *     回傳值必須以 mdfix_string_free 釋放，且只能釋放一次。
 *   - 函式庫內部的 panic 會被攔下並轉成 {"error":"internal error: ..."}，
 *     不會跨過 FFI 邊界。
 *   - 所有函式都沒有全域狀態，可以在多個執行緒同時呼叫。
 */

#ifndef MARKDOWN_FIXER_H
#define MARKDOWN_FIXER_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * 檢查 Markdown。
 *
 * 成功時回傳 {"problems":[...]}，每個問題依序有 rule、name、severity、fixable、
 * line、column、message 欄位，順序與內容和 JS diagnose(text, options) 相同。
 * line 與 column 從 1 起算；column 的單位是 UTF-16 code unit。
 */
char *mdfix_diagnose(const char *text, size_t len, const char *options_json);

/*
 * 套用安全修正。
 *
 * 成功時回傳 {"text":"...","changed":true,"refused":false,"reason":""}，
 * 與 JS fixDocument(text, options) 相同。refused 為 true 時 text 等於原文，
 * reason 說明拒絕原因（結構錯誤或 frontmatter 陷阱）；未拒絕時 reason 是空字串。
 */
char *mdfix_fix(const char *text, size_t len, const char *options_json);

/*
 * 保守地判斷文字是否像 Markdown。
 *
 * 成功時回傳 {"is_markdown":true,"score":5,"signals":["atx-heading",...]}。
 * signals 是穩定的訊號名稱，不含輸入內容。
 */
char *mdfix_detect(const char *text, size_t len);

/*
 * 回傳規則目錄 {"rules":[...]}，順序與 JS RULES 相同。每條規則依序有
 * id、name、category、severity、fixable、summary 欄位。
 */
char *mdfix_rules_json(void);

/*
 * 回傳函式庫版本，例如 "0.1.0"。
 *
 * 這是靜態字串，在整個行程期間有效，不可傳給 mdfix_string_free。
 */
const char *mdfix_version(void);

/*
 * 釋放 mdfix_diagnose、mdfix_fix、mdfix_detect、mdfix_rules_json 回傳的字串。
 * 傳入 NULL 時什麼都不做。不可用 free() 釋放這些字串。
 */
void mdfix_string_free(char *s);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* MARKDOWN_FIXER_H */
