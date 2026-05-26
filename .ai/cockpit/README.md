---
author: Ray
title: AI Cockpit
description: Work Item Contract と Summary から現在の作業状態を確認するための軽量 Cockpit。
key: ai-cockpit
---

# AI Cockpit

AI Cockpit は、AI 作業の状態を一画面で確認するための軽量な計器盤です。

Cockpit は判断を代行しません。Contract、Summary、検証結果、Backtrack report を整理し、人間が review / merge / follow-up を判断しやすくします。

## 状態

| 状態 | 意味 |
|---|---|
| `blocked` | Contract が invalid、`unknowns` が残っている、または `notCodable: true`。 |
| `ready_for_review` | Contract と Summary があり、required verification が `passed`。 |
| `blocked_by_ai_loop` | AI loop guard が後退や同一失敗の反復を検出した。 |
| `no_active_work_item` | active Work Item が存在せず、archive 後の同期も完了している。 |

## 入口

| ファイル | 用途 |
|---|---|
| `checks.yaml` | Sentinel 向けの共通検証 command catalog。 |
| `current_status.md` | `make generate-cockpit-status` が生成する現在の状態。実装詳細は `scripts/ai_generate_status.py`。 |
| `status_policy.yaml` | active / no-active status、archive 後の同期、参照整合性の方針。 |

`status_policy.yaml` は Cockpit の machine-readable SSOT である。状態名、archive 後の `no_active_work_item` 表示、参照整合性 check はこの file と `make` target の契約に従う。script 実装と衝突する場合は `status_policy.yaml` と Makefile target を正とする。

## 推奨コマンド

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make fmt-check
make check-ai-guards CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-backtrack CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-coverage-guard
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make ai-preflight
```

管理対象 diff は Contract の `scope` に含まれない限り失敗し、`.ai/guards/file_ownership.yaml` で `aiWrite: restricted` とされた file は同じ仕組みで明示承認を要求する。test、snapshot、i18n、Work Item evidence の削除は、Contract の `destructiveChangePolicy` と Summary の `destructiveChanges` に明示されない限り失敗する。production Rust code の変更は test 変更証跡を必須とする。

Work Item を完了する時は次を使う。

```bash
make ai-finish TASK=<task>
```

`make ai-finish` は required checks を再実行し、成功時だけ Contract と Summary を `.ai/work-items/archive/<year>/` へ移動する。archive 後は `current_status.md` を `no_active_work_item` として再生成し、active Work Item JSON を残さない。

archive 後の整合性は `make ai-finish` の成功条件に含まれる。失敗時の調査や手動復旧では次を個別に実行する。

```bash
make check-work-items-lifecycle
make check-ai-status-consistency
```
