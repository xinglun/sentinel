---
author: Ray
title: AI ガバナンス入口
description: Sentinel でインストール済み ai-cockpit を使って Work Item、検証、完了記録を管理する最小運用入口。
key: ai-governance-entry
---

# AI ガバナンス入口

`.ai/` は、AI Agent による変更を Work Item Contract、検証、完了記録、回帰防止の単位で追跡するための機械可読な入口です。Cockpit は判断を代行せず、scope、根拠、検証、リスクを人間が確認できる状態に整理します。

## インストール版 Runtime

このリポジトリは、リポジトリ内 Python Runtime を使用しません。共有インストール版 `ai-cockpit` を唯一の Runtime とし、すべての操作で `--repo` を明示します。

- 設定: `.ai/cockpit.toml`、`.ai/project.json`、`.ai/agent-interface.json`
- Work Item: `.ai/work-items/active/` と `.ai/work-items/archive/`
- adapter: `.ai/adapters/`
- Runtime: インストール済み `ai-cockpit 0.2.33`

状態や互換性は、次の読み取り専用コマンドで確認します。

```bash
ai-cockpit inspect --repo .
ai-cockpit status --repo .
ai-cockpit doctor --repo .
ai-cockpit compatibility --repo .
```

## Work Item 境界

新しい Work Item では、次の境界を Contract の `scope`、`outOfScope`、acceptance、sources、verification に明記します。

1. Gate、execution、trader、action matrix、position sizing に影響するか。
2. Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。
3. data branch、reports、JSONL、snapshot、weekly metrics のどこに保存するか。
4. zh / en / ja の i18n、snapshot、contract test が必要か。
5. fact、manual observation、hypothesis、fixture、local cache を区別できているか。
6. `make` target を検証・運用入口として提供できているか。
7. `risk`、`agentCapability`、`executionDecision`、`preReviewWarnings` を記録できているか。

本リポジトリでは、取引判断、Gate、Telegram 文案、証拠スコアのロジックを Cockpit 移行だけで変更しません。data branch、履歴 Work Item、evidence、project declaration も、明示的な scope がない限り変更対象外です。

## 最小 lifecycle

```text
work-item new
    ↓
start（Contract を確定）
    ↓
preflight（実装前の安全確認）
    ↓
checkpoint（編集開始点を記録）
    ↓
verify（宣言した make check を実行）
    ↓
finish（required check を再確認）
    ↓
archive（active から履歴へ移動）
    ↓
close（人間による最終判断が必要な場合のみ）
```

実行例:

```bash
ai-cockpit work-item new --repo . --id <task> --mode code
ai-cockpit start --repo . --id <task> --intent "..." --goal "..." --scope "..." --authority authorized
ai-cockpit preflight --repo . --contract .ai/work-items/active/<task>.contract.json
ai-cockpit checkpoint --repo . --id <task>
ai-cockpit verify --repo . --work-item <task> --command make --args quality
ai-cockpit finish --repo . --id <task>
ai-cockpit archive --repo . --id <task>
```

`preflight` が `yellow` の場合は不足 evidence を収集し、`red` または Contract の unknowns が残る場合は実装を進めません。生成された Summary は Runtime に管理させ、検証結果は `ai-cockpit verify` で記録します。

## 品質ゲート

プロジェクトの Rust gate は次の `make` target で実行します。

```bash
make fmt-check
make test
make clippy
make quality
```

新しい AI Cockpit lifecycle はインストール版 CLI を直接使用し、プロジェクト品質の検証だけを根 `Makefile` の `make quality` で実行します。旧 `Makefile.ai`、旧 `.ai/cockpit/` 生成 policy、旧 `scripts/ai_*.py` lifecycle は移行対象であり、運用入口として使用しません。
