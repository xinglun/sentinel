---
author: Ray
title: Gemini Agent Rules
description: Gemini が Sentinel で作業する時の AI Cockpit、言語、品質ゲート規則。
key: gemini-agent-rules
---

# Gemini Agent Rules

このリポジトリで Gemini が code、test、docs、CI、`.ai`、`skills`、`Makefile` を変更する場合は、`AGENTS.md` の規則と同じく `.ai/cockpit/` を作業入口として扱う。

## 必須手順

1. `.ai/cockpit/README.md` を確認する。
2. 対象 task の `.ai/work-items/active/<task>.contract.json` を確認する。
3. Contract がない場合は編集せず、先に Contract を作成する。
4. `notCodable: true` または `unknowns` が残る場合、production code を変更しない。
5. 変更対象が Contract の `scope` に含まれ、`outOfScope` に含まれないことを確認する。
6. 完了時は `.ai/work-items/active/<task>.summary.json` を更新し、Cockpit status を生成する。

## 標準コマンド

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-backtrack
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make quality
```

## 優先順位

- Work Item Contract の `scope`、`outOfScope`、`unknowns`、`notCodable`、`verification` を最優先する。
- Skill と Contract が衝突する場合は Contract を優先し、必要なら blocker として報告する。
- 実行コマンドは Makefile target を標準入口とし、直接 `cargo` や script を呼び出さない。
