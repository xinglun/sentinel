---
name: sentinel-ai-cockpit
description: Sentinel repository changes must pass the .ai Work Item Contract, Summary, Cockpit, Backtrack, and Rust quality gate before review or commit.
author: Ray
---

# Sentinel AI Cockpit Skill

Sentinel で code、test、docs、CI、`.ai`、`skills`、`Makefile` を変更する時に使う。

## 手順

1. `.ai/cockpit/README.md` を確認する。
2. 対象 task の `.ai/work-items/active/<task>.contract.json` を確認する。
3. `mode: code` の場合、`notCodable: false` と `unknowns: []` を確認する。
4. 変更対象が `scope` に含まれ、`outOfScope` に含まれないことを確認する。
5. 作業後、`.ai/work-items/active/<task>.summary.json` を更新する。
6. 次の check を実行する。

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-backtrack
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-rust
```

## Required Checks

- `make check-ai-contract CONTRACT=<contract>`
- `make check-ai-scope CONTRACT=<contract>`
- `make check-ai-backtrack`
- `make check-ai-change-summary SUMMARY=<summary> CONTRACT=<contract>`
- `make generate-cockpit-status CONTRACT=<contract> SUMMARY=<summary>`
- `make check-ai-status CONTRACT=<contract> SUMMARY=<summary>`
- `make check-rust`

## 禁止事項

- Contract なしで `src/**`、`tests/**`、`.ai/**`、`skills/**`、`Makefile`、`AGENTS.md`、`GEMINI.md` を変更しない。
- `unknowns` が残る状態で production code を変更しない。
- 必須 check が失敗した状態で `ready_for_review` と報告しない。
- 取引判断、Gate、証拠層の意味を Work Item scope 外で変更しない。
