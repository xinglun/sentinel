---
author: Ray
title: Gemini Agent Rules
description: Gemini が Sentinel で作業する時のインストール版 AI Cockpit、言語、品質ゲート規則。
key: gemini-agent-rules
---

# Gemini Agent Rules

このリポジトリで Gemini が code、test、docs、CI、`.ai`、`skills`、`Makefile`、`AGENTS.md`、`GEMINI.md` を変更する場合は、`AGENTS.md` の規則と同じくインストール済み `ai-cockpit` を作業入口として扱う。

## 必須手順

1. `.ai/cockpit/README.md` と `ai-cockpit inspect --repo .` を確認する。
2. 対象 task の `.ai/work-items/active/<task>.contract.json` を確認する。
3. Contract がない場合は編集せず、`ai-cockpit work-item new --repo . --id <task> --mode code` で作成する。
4. `ai-cockpit start` で intent、goal、scope、authority、acceptance を確定する。
5. `notCodable: true`、`unknowns`、停止値の `executionDecision` が残る場合、production code を変更しない。
6. `ai-cockpit preflight` と `ai-cockpit checkpoint` を実装前に実行する。
7. 完了時は `ai-cockpit verify`、`finish`、`archive` を順に実行し、Summary の receipt を保持する。

## 標準コマンド

```bash
ai-cockpit inspect --repo .
ai-cockpit status --repo .
ai-cockpit doctor --repo .
ai-cockpit compatibility --repo .
ai-cockpit preflight --repo . --contract .ai/work-items/active/<task>.contract.json
ai-cockpit checkpoint --repo . --id <task>
ai-cockpit verify --repo . --work-item <task> --command make --args quality
ai-cockpit finish --repo . --id <task>
ai-cockpit archive --repo . --id <task>
make fmt-check
make test
make clippy
make quality
```

## 優先順位

- Work Item Contract の `scope`、`outOfScope`、`unknowns`、`notCodable`、`verification` を最優先する。
- Skill と Contract が衝突する場合は Contract を優先し、必要なら blocker として報告する。
- AI Cockpit / Work Item の lifecycle はインストール版 CLI を直接標準入口とし、プロジェクト品質だけを根 `Makefile` の `make quality` で検証する。旧 Python Runtime や `scripts/ai_*.py` は呼び出さない。
- `make test` は Clippy を代替しないため、`make clippy` を別途実行する。
