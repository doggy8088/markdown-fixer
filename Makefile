# markdown-fixer 的常用工作。執行 `make`（或 `make help`）列出所有目標。

.DEFAULT_GOAL := help

.PHONY: help build release test test-rust test-scripts parity golden golden-check fuzz \
        fmt fmt-check lint doc check install uninstall ffi ffi-example md-check clean

CARGO ?= cargo
NODE  ?= node
CC    ?= cc

# fuzz 產生的隨機文件數：make fuzz COUNT=10000
COUNT ?= 2000
# fuzz 的亂數種子；同一個種子永遠產生同一批文件
SEED  ?= 1
# md-check 檢查的 Markdown（不存在的檔案自動略過）
MD_FILES := $(wildcard PRD.md AGENTS.md README.md docs)

FFI_INCLUDE := crates/markdown-fixer-ffi/include
FFI_LIB     := target/release/libmarkdown_fixer_ffi.a
ifeq ($(shell uname -s),Darwin)
FFI_NATIVE_LIBS :=
else
FFI_NATIVE_LIBS := -lpthread -ldl -lm
endif

help: ## 列出所有目標
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Debug 建置整個 workspace
	$(CARGO) build --workspace

release: ## Release 建置（target/release/markdown-fixer 與 FFI 函式庫）
	$(CARGO) build --workspace --release --locked

test: test-rust test-scripts ## 跑全部測試（不需要 Node）

test-rust: ## Rust 單元、整合與 golden 測試
	$(CARGO) test --workspace --all-features

test-scripts: ## 剪貼簿腳本測試（假的 pbpaste／pbcopy／osascript）
	examples/macos/test-markdown-fix-paste.sh

parity: ## 即時比對 oracle 與 Rust（需要 Node）
	scripts/parity.sh fixtures/corpus

golden: ## 由 oracle 重新產生函式庫與 CLI 的 golden 檔（需要 Node）
	$(NODE) scripts/gen-golden.mjs
	$(NODE) scripts/gen-cli-golden.mjs

golden-check: ## 確認函式庫與 CLI 的 golden 檔與 oracle 一致（需要 Node）
	$(NODE) scripts/gen-golden.mjs --check
	$(NODE) scripts/gen-cli-golden.mjs --check

fuzz: ## 隨機文件差異測試：make fuzz COUNT=10000 SEED=7（需要 Node）
	$(NODE) scripts/fuzz-parity.mjs --count $(COUNT) --seed $(SEED)

fmt: ## 格式化程式碼
	$(CARGO) fmt --all

fmt-check: ## 檢查格式（同 CI）
	$(CARGO) fmt --all --check

lint: ## Clippy，警告視為錯誤（同 CI）
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

doc: ## 產生 rustdoc，警告視為錯誤
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps --workspace

check: fmt-check lint test doc md-check ## CI 會跑的全部檢查（不含需要 Node 的項目）

install: ## 從這份 checkout 安裝 markdown-fixer 到 ~/.cargo/bin
	$(CARGO) install --path crates/markdown-fixer-cli --locked

uninstall: ## 移除 cargo 安裝的 markdown-fixer
	$(CARGO) uninstall markdown-fixer-cli

ffi: ## 建置 C ABI 函式庫（cdylib／staticlib）到 target/release
	$(CARGO) build -p markdown-fixer-ffi --release --locked
	@ls -1 target/release/ | grep -E '^(lib)?markdown_fixer_ffi\.(a|so|dylib|dll|lib|dll\.lib)$$' | sed 's|^|  target/release/|'
	@echo "  header: $(FFI_INCLUDE)/markdown_fixer.h"

ffi-example: ffi ## 編譯並執行 examples/c/main.c（靜態連結）
	$(CC) -std=c99 -Wall -Wextra -I $(FFI_INCLUDE) examples/c/main.c $(FFI_LIB) $(FFI_NATIVE_LIBS) -o target/mdfix-example
	./target/mdfix-example

md-check: ## 用自己的執行檔檢查本 repo 的 Markdown 文件
	$(CARGO) run -q -p markdown-fixer-cli -- --check $(MD_FILES)

clean: ## 移除建置產物
	$(CARGO) clean
