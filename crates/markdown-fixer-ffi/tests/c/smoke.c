/*
 * FFI 煙霧測試：只用不需要核心規則實作的路徑（版本、錯誤處理、釋放），
 * 確認標頭檔能以嚴格旗標編譯、函式庫能連結並執行。由 tests/c_example.rs 編譯執行。
 */

#include <stdio.h>
#include <string.h>

#include "markdown_fixer.h"

#define CHECK(cond)                                                     \
    do {                                                                \
        if (!(cond)) {                                                  \
            fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__,      \
                    __LINE__, #cond);                                   \
            return 1;                                                   \
        }                                                               \
    } while (0)

static int starts_with(const char *s, const char *prefix) {
    return strncmp(s, prefix, strlen(prefix)) == 0;
}

int main(int argc, char **argv) {
    const char invalid_utf8[] = {'a', (char)0xc3};
    const char *doc = "# Title\n";
    const char *version = mdfix_version();
    char *json;

    CHECK(version != NULL);
    CHECK(version == mdfix_version());
    if (argc > 1) {
        CHECK(strcmp(version, argv[1]) == 0);
    }

    json = mdfix_diagnose(invalid_utf8, sizeof invalid_utf8, NULL);
    CHECK(json != NULL);
    CHECK(starts_with(json, "{\"error\":\"text is not valid UTF-8"));
    mdfix_string_free(json);

    json = mdfix_fix(NULL, 5, NULL);
    CHECK(json != NULL);
    CHECK(strcmp(json, "{\"error\":\"text is NULL but len is 5\"}") == 0);
    mdfix_string_free(json);

    json = mdfix_detect(invalid_utf8, sizeof invalid_utf8);
    CHECK(json != NULL);
    CHECK(starts_with(json, "{\"error\":"));
    mdfix_string_free(json);

    json = mdfix_diagnose(doc, strlen(doc), "{\"maxLineLength\":\"80\"}");
    CHECK(json != NULL);
    CHECK(starts_with(json, "{\"error\":\"invalid options JSON"));
    mdfix_string_free(json);

    mdfix_string_free(NULL);

    printf("ok %s\n", version);
    return 0;
}
