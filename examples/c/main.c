/*
 * 從 C 呼叫 markdown-fixer 的範例。
 *
 * 建置與執行（macOS／Linux，在 repo 根目錄）：
 *
 *   cargo build --release -p markdown-fixer-ffi
 *   cc -std=c99 -Wall -Wextra -I crates/markdown-fixer-ffi/include \
 *      examples/c/main.c target/release/libmarkdown_fixer_ffi.a \
 *      -lm -lpthread -ldl -o target/mdfix-example      # Linux
 *   cc -std=c99 -Wall -Wextra -I crates/markdown-fixer-ffi/include \
 *      examples/c/main.c target/release/libmarkdown_fixer_ffi.a \
 *      -o target/mdfix-example                         # macOS
 *   ./target/mdfix-example
 *
 * 或直接執行 `make ffi-example`。詳細說明見 docs/ffi.md。
 *
 * 程式依序呼叫每個 API，印出回傳的 JSON。任何「應該成功」的呼叫回傳了
 * {"error":...}，或「應該失敗」的呼叫沒有回傳錯誤，就以結束碼 1 結束。
 */

#include <stdio.h>
#include <string.h>

#include "markdown_fixer.h"

static int failures = 0;

/* 粗略判斷回傳的 JSON 是不是錯誤物件。正式程式請用 JSON 函式庫解析。 */
static int is_error(const char *json) {
    return strncmp(json, "{\"error\":", 9) == 0;
}

/* 印出結果、檢查是否符合預期，然後釋放。 */
static void report(const char *label, char *json, int expect_error) {
    printf("%s: %s\n", label, json);
    if (is_error(json) != expect_error) {
        fprintf(stderr, "unexpected result for %s\n", label);
        failures++;
    }
    mdfix_string_free(json);
}

int main(void) {
    /* 刻意的筆誤：# 後少了空白、NBSP（U+00A0）、三個空行。 */
    const char *doc = "#Title\n\n\n\nSome text\xc2\xa0here.\n";
    const char *options = "{\"ignoreRules\":[\"MDX003\"],\"maxLineLength\":0}";
    const char invalid_utf8[] = {'#', ' ', (char)0xff, '\n'};

    printf("markdown-fixer %s\n", mdfix_version());

    report("diagnose", mdfix_diagnose(doc, strlen(doc), NULL), 0);
    report("diagnose with options", mdfix_diagnose(doc, strlen(doc), options), 0);
    report("fix", mdfix_fix(doc, strlen(doc), NULL), 0);
    report("detect", mdfix_detect(doc, strlen(doc)), 0);

    /* 規則目錄很長，只印開頭。 */
    {
        char *rules = mdfix_rules_json();
        printf("rules: %.60s...\n", rules);
        if (is_error(rules)) {
            fprintf(stderr, "unexpected result for rules\n");
            failures++;
        }
        mdfix_string_free(rules);
    }

    /* 錯誤處理：非法 UTF-8、非法 JSON、未知規則。 */
    report("invalid UTF-8", mdfix_diagnose(invalid_utf8, sizeof invalid_utf8, NULL), 1);
    report("invalid options", mdfix_fix(doc, strlen(doc), "{not json"), 1);
    report("unknown rule", mdfix_diagnose(doc, strlen(doc), "{\"rules\":[\"MDX999\"]}"), 1);

    mdfix_string_free(NULL); /* 允許，什麼都不做。 */

    if (failures > 0) {
        fprintf(stderr, "%d unexpected result(s)\n", failures);
        return 1;
    }
    return 0;
}
