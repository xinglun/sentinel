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
| `draft` | Contract はあるが、Summary または検証結果が不足している。 |
| `blocked` | Contract が invalid、`unknowns` が残っている、または `notCodable: true`。 |
| `ready_for_review` | Contract と Summary があり、required verification が `passed`。 |

## 入口

| ファイル | 用途 |
|---|---|
| `checks.yaml` | Sentinel 向けの共通検証 command catalog。 |
| `current_status.md` | `scripts/ai_generate_status.py` が生成する現在の状態。 |
| `status_policy.yaml` | active / no-active status、archive 後の同期、参照整合性の方針。 |

## 推奨コマンド

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make fmt-check
make check-ai-backtrack
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make ai-preflight
```
