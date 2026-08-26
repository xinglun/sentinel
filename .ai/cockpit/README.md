---
author: Ray
title: AI Cockpit
description: インストール済み ai-cockpit Runtime を使った Sentinel の Work Item lifecycle と検証入口。
key: ai-cockpit
---

# AI Cockpit

Sentinel の Cockpit は、インストール済み `ai-cockpit` が管理する Contract、Summary、検証 receipt、archive を人間 review 用に整理します。Cockpit は取引判断や merge 判断を代行しません。

## Runtime と repository-local 設定

Runtime は共有インストール版 `ai-cockpit` です。リポジトリ内に Python Runtime や Runtime 用の実行スクリプトを置きません。

| パス | 用途 |
|---|---|
| `.ai/cockpit.toml` | repository-local Runtime 設定。 |
| `.ai/project.json` | attach 時に解決された repository identity と profile。 |
| `.ai/agent-interface.json` | Agent adapter が読む interface。 |
| `.ai/adapters/` | Codex などの provider adapter。 |
| `.ai/work-items/active/` | 作業中の Contract と Summary。 |
| `.ai/work-items/archive/` | 完了した Work Item の履歴。 |

互換性、Runtime digest、repository-local 状態は次で確認します。

```bash
ai-cockpit inspect --repo .
ai-cockpit status --repo .
ai-cockpit doctor --repo .
ai-cockpit compatibility --repo .
```

## 状態

| 状態 | 意味 |
|---|---|
| `not_ready` | Work Item の Contract に必要な人間入力が不足している。 |
| `implementation_active` | Contract が確定し、実装を進められる状態。 |
| `checkpointed` | 編集開始点が immutable checkpoint として記録されている。 |
| `verification_pending` | required verification が未実行または未完了。 |
| `ready_for_review` | required verification が通過し、review へ渡せる状態。 |
| `archived` | Work Item が active から archive へ移動している。 |

## Lifecycle

```text
work-item new → start → preflight → checkpoint → verify → finish → archive
```

`close` は人間の最終判断を記録する必要がある時だけ使用します。

```bash
ai-cockpit work-item new --repo . --id <task> --mode code
ai-cockpit start --repo . --id <task> --intent "..." --goal "..." --scope "..." --authority authorized
ai-cockpit preflight --repo . --contract .ai/work-items/active/<task>.contract.json
ai-cockpit checkpoint --repo . --id <task>
ai-cockpit verify --repo . --work-item <task> --command make --args quality
ai-cockpit finish --repo . --id <task>
ai-cockpit archive --repo . --id <task>
```

`preflight` が `yellow` なら required evidence を収集します。`red`、Contract の `unknowns`、`notCodable: true`、または `executionDecision` の停止値がある場合は、実装を進めず Contract、調査、または blocker を更新します。

## Work Item 契約

- `scope` と `outOfScope` を変更対象の境界として扱う。
- destructive change は `destructiveChangePolicy` と明示的な authority で扱う。
- 高リスク作業では `scenarioCoverage` を宣言し、Summary に実行 evidence を記録する。
- `verification` は `make` target などの再現可能な check として宣言する。
- `agentCapability`、`executionDecision`、`preReviewWarnings` は、実装可能性・検証可能性・人間判断の要否を表す。
- Summary の未確認事項は `residualRisks` と `expectedReviewFocus` に分けて記録する。

## プロジェクト品質

Cockpit lifecycle の required check として、本リポジトリでは次を使います。

```bash
make fmt-check
make test
make clippy
make quality
```

Rust の production code、project data、evidence、i18n、snapshot、既存 archive は Cockpit 移行の対象ではありません。変更する場合は別の Work Item として scope、acceptance、verification を宣言してください。
