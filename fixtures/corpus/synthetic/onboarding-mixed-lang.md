# 新人 Onboarding Guide

Welcome aboard! 這份文件幫你在第一週把開發環境建好。If anything is unclear, ping #dev-help on Slack.

## Day 1：帳號與權限

- [ ] 申請 GitHub org 權限（找你的 buddy）
- [ ] 加入 Slack channels：`#dev`、`#dev-help`、`#deploy`
- [ ] 設定 2FA
- [ ] 領取筆電，開機後跑 `./scripts/bootstrap.sh`

## Day 2：本機環境

### Prerequisites
* Docker Desktop >= 4.30
* Node.js 22 LTS（建議用 `fnm` 管理）
* Rust stable（`rustup default stable`）

### Clone & run
```bash
git clone git@github.com:example/monorepo.git
cd monorepo
make dev
```
Open http://localhost:8080 — 看到登入頁就成功了。

> **Tip:** `make dev` 第一次會下載約 3 GB 的 Docker image，建議接有線網路。

## Day 3–5：第一個 PR

挑一張標了 `good first issue` 的票。Workflow：

1. 開 branch：`feat/<ticket-id>-short-desc`
2. Commit message 遵守 [Conventional Commits](https://www.conventionalcommits.org/)
3. 開 PR，填寫 template 裡的 **What / Why / How to test**
4. 至少一位 reviewer approve 後才能 merge

### Code style
| Language | Formatter | Linter |
| --- | --- | --- |
| TypeScript | prettier | eslint |
| Rust | rustfmt | clippy |
| Markdown | markdown-fixer | markdown-fixer --check |

## 常見問題 FAQ

**Q: VPN 連不上？**
A: 先確認 Okta 有登入。還是不行就找 IT（分機 1234）。

**Q:  Docker 很慢怎麼辦？**
A: Settings → Resources，把 CPU 調到 6、記憶體調到 8 GB。

## Useful links
- 架構文件：[docs/architecture.md](docs/architecture.md)
- On-call 手冊：[runbook][]
- Grafana：https://grafana.example.com/d/overview

[runbook]: https://wiki.example.com/oncall
