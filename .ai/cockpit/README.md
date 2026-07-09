---
author: Ray
title: AI Cockpit
description: Work Item Contract と Summary から現在の作業状態を確認するための軽量 Cockpit。
key: ai-cockpit
---

# AI Cockpit

AI Cockpit は、AI 作業の状態を一画面で確認するための軽量な計器盤です。

Cockpit は判断を代行しません。Contract、Summary、検証結果、Backtrack report を整理し、人間が review / merge / follow-up を判断しやすくします。`make ai-checkpoint` は checkpoint ごとの contract hash と required check の進捗を確認する補助入口です。

Contract と Summary の field 順序は review の可読性を上げるための約束であり、意味を変えない。Sentinel では新規または大きく書き換える Work Item で、metadata、intent、boundary、codability、risk / execution、acceptance / verification、safety の順を保つ。

## 状態

| 状態 | 意味 |
|---|---|
| `blocked` | Contract が invalid、`unknowns` が残っている、または `notCodable: true`。 |
| `ready_for_review` | Contract と Summary があり、required verification が `passed`。 |
| `ready_with_risks` | required verification は `passed` だが、Summary の `reviewReadiness` が residual risk と review focus を明示している。 |
| `blocked_by_ai_loop` | AI loop guard が後退や同一失敗の反復を検出した。 |
| `no_active_work_item` | active Work Item が存在せず、archive 後の同期も完了している。 |

## 入口

| ファイル | 用途 |
|---|---|
| `checks.yaml` | Sentinel 向けの共通検証 command catalog。 |
| `preflight_review_policy.yaml` | Preflight Review の advisory / gate 方針。 |
| `current_status.md` | `make generate-cockpit-status` が生成する現在の状態。実装詳細は `scripts/ai_generate_status.py`。 |
| `status_policy.yaml` | active / no-active status、archive 後の同期、参照整合性の方針。 |
| `scenario_coverage_policy.yaml` | Scenario Coverage の hard risk 判定と guard 方針。 |

`status_policy.yaml` は Cockpit の machine-readable SSOT である。状態名、archive 後の `no_active_work_item` 表示、参照整合性 check はこの file と `make` target の契約に従う。script 実装と衝突する場合は `status_policy.yaml` と Makefile target を正とする。

`preflight_review_policy.yaml` は Preflight Review の machine-readable SSOT である。デフォルトは advisory で、`needs_human_confirmation` と `not_ready` を gate にするかどうかは policy で明示する。


## 作業前の境界定義

`mode: code` の Work Item では、実装前に次を Contract に落とし込む。

| 境界 | 確認内容 |
|---|---|
| Gate / execution | Gate、execution、action matrix、trader、position sizing に影響するか。 |
| 表示 | Telegram、Markdown、CLI、audit daily、weekly review のどこに出すか。 |
| 永続化 | data branch、reports、JSONL、snapshot、weekly metrics のどこへ残すか。 |
| 言語 | zh / en / ja の i18n、snapshot、contract test が必要か。 |
| 証拠 | fact、manual observation、hypothesis、fixture、local cache を混同していないか。 |
| command | 新しい検証・運用入口が `make` target に収まっているか。 |
| risk / readiness | 実装可能性、検証可能性、人間判断の要否、review focus、残余 risk を記録したか。 |
| scenario coverage | `scenarioCoverage`、`followUps`、`unverifiedScenarios` が必要な Work Item か。|

機能完了の報告では、少なくとも code / test / docs / i18n / report output / data or weekly record / CI or Make guard のどれを確認したかを Summary に残す。未確認項目がある場合は「未確認」と明記し、完了を過大主張しない。

## Risk と review 準備性

Contract の `riskAssessment`、`agentCapability`、`executionDecision` は、Agent が実装前に問題を表明するための machine-readable channel である。

Agent の三大 risk は、Cockpit では次の hardening target として扱う。

| risk | 対応 |
|---|---|
| prompt は助言であり命令ではない | code mode の required verification に Contract / scope / guard / backtrack / summary / status の `make` gate を要求する。 |
| 実行途中で文脈を失う | `checkpointPolicy` に `contract_start`、`before_edit`、`before_ready`、`after_verification` を要求し、判断点を Contract に残す。 |
| 分からない時に脳内補完する | `agentCapability` と `executionDecision` を照合し、実装不能・検証不能・人間判断待ちでは `continue` を拒否する。 |

- `executionDecision: continue`: Contract が確定し、scope 内で実装できる。
- `executionDecision: contract_update_required`: 実装前に scope、acceptance、verification、sources の更新が必要。
- `executionDecision: blocked`: 人間判断、外部状態、または不足証拠により実装を止める。
- `executionDecision: downgraded_to_investigation`: production code を変更せず、調査や TODO 整理に降格する。

`mode: code` で `executionDecision: continue` の Work Item は、Agent の自己申告だけで進めない。`make check-ai-contract`、`make check-ai-scope`、`make check-ai-guards`、`make check-ai-backtrack`、`make check-ai-change-summary`、`make generate-cockpit-status`、`make check-ai-status` を required verification として持つ。checkpoint の可視化が必要な長時間作業では `make ai-checkpoint` を併用する。PR で archive 配下を含む diff を扱う場合は、CI で `make check-ai-pr AI_BASE_COMMIT=<merge-base>` を併用する。

`make ai-preflight` は実装前の共通入口である。active Contract がある場合は Preflight Review を生成して表示し、`ready` 以外の review は agent workflow に pause を促す。Cockpit Status は reviewer visibility であり、pre-implementation pause の代替ではない。

Scenario Coverage は risk 域の検証場面を表す。test case の一覧でも residual risk の置き換えでもなく、verified / unverified / not_applicable の状態で Work Item の検証範囲を見える化する。低リスク Work Item では必須ではないが、中高リスクで未検証の場面があるなら Summary に残す。

Summary の `reviewReadiness` は、完了報告の強さを制御する。

- `ready`: required checks が通過し、known residual risk がない。
- `ready_with_risks`: required checks は通過したが、`residualRisks` と `expectedReviewFocus` が残る。
- `not_ready`: required checks、Contract、または人間判断待ちが残る。

Scenario Coverage がある場合は、`current_status.md` に `Scenario Coverage: complete / incomplete / not_required / unknown` を別表示する。これは Summary の `scenarioCoverage` から派生する補助信号で、reviewReadiness と独立して読む。

User correction が発生した場合は、`userCorrectionSolidification` に固化先を記録する。修正だけで終えず、同種の review finding を次回の Contract、template、guard、doc、skill のどこで防ぐかを明示する。

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
make check-ai-pr AI_BASE_COMMIT=<merge-base-sha>
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
