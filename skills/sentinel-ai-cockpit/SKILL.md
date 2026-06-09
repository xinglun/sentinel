---
name: sentinel-ai-cockpit
description: Enforce Sentinel repository AI Cockpit workflow for Codex or Gemini when changing code, tests, docs, CI, .ai, skills, Makefile, AGENTS.md, or GEMINI.md. Use this before editing repository files, during Work Item execution, before review, and before commit/push to ensure Contract scope, Summary, make checks, and automatic archive are followed.
---

# Sentinel AI Cockpit

Sentinel repository で変更を行う時は、この Skill を実行纪律として使う。投資判断、戦略判断、売買判断は扱わない。

## Core Rule

`src/**`、`tests/**`、`docs/**`、`.github/workflows/**`、`.ai/**`、`skills/**`、`Makefile`、`AGENTS.md`、`GEMINI.md` を変更する場合、必ず Work Item Contract を入口にする。

## Workflow

1. `AGENTS.md` と `.ai/cockpit/README.md` を確認する。
2. active Work Item を探す。
   - 存在する場合: `.ai/work-items/active/<task>.contract.json` と summary を読む。
   - 存在しない場合: `make ai-start TASK=<task> TITLE="..." MODE=code` で作成する。
3. coding 前に次を確認する。
   - `mode: code`
   - `notCodable: false`
   - `unknowns: []`
   - `executionDecision: continue`
   - `checkpointPolicy` が `contract_start`、`before_edit`、`before_ready`、`after_verification` を含む。
   - `agentCapability.canImplement: true`
   - `agentCapability.canVerify: true`
   - `agentCapability.needsHumanDecision: false`
   - 変更対象が `scope` に含まれる。
   - 変更対象が `outOfScope` に含まれない。
4. `notCodable: true` または `unknowns` が残る場合、production code を変更しない。調査、TODO、blocker 記録に限定する。
5. `executionDecision` が `contract_update_required`、`blocked`、`downgraded_to_investigation` の場合は、実装を止めて Contract 更新、調査、TODO、blocker 記録に切り替える。
6. 実装前に、Gate / execution、report output、data branch、weekly calibration、i18n、evidence source、Make command、risk / review readiness の境界を Contract に反映する。
7. 作業後、summary の `changedFiles`、`sourcesUsed`、`verification`、`observedIssues` に加え、未解決の user correction / known gap / 未確認項目、residual risk、review readiness、expected review focus を更新する。
8. 必ず `make` 経由で check を実行する。


## Boundary Checklist

Work Item が機能追加、report 変更、データ永続化、AI governance 変更のいずれかを含む場合、次を明示する。

- Gate / execution / trader / action matrix へ影響するか。影響しない場合は表示・監査専用と書く。
- Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。
- data branch に保存するか、週次成果物だけに集約するか、保存しないか。
- zh / en / ja の i18n と snapshot / contract test が必要か。
- fact、manual observation、hypothesis、fixture、local cache を分離しているか。
- 新しい command は `make` target として提供されるか。
- Agent が実装できるか、検証できるか、人間判断が必要かを `agentCapability` に明記しているか。
- review で追加論点が出る可能性を `preReviewWarnings` と `reviewReadiness.expectedReviewFocus` に記録しているか。
- prompt だけに依存せず、Contract / scope / guard / backtrack / summary / status の `make` gate を required verification にしているか。
- 実行途中の重要判断を `checkpointPolicy` と Summary に戻しているか。

User correction が発生した場合は、単に修正せず、次回の backtrack 防止として Contract、Summary、doc、template、guard、skill のどれへ固化するかを `userCorrectionSolidification` に記録する。

## Risk / Readiness Channel

Agent が「できない」「今は危険」「review で追加論点が出る」と判断した場合は、通常の完了報告に混ぜず machine-readable field に記録する。

- Contract: `riskAssessment`、`agentCapability`、`executionDecision`、`preReviewWarnings`
- Summary: `residualRisks`、`reviewReadiness`、`expectedReviewFocus`、`userCorrectionSolidification`

`ready_with_risks` は required checks が通過したうえで、review focus が残る状態を表す。`not_ready` や `blocked` の task を ready と報告しない。

三大 risk の扱い:

- Prompt は助言であり命令ではない。重要制約は `make` gate と scope guard で確認する。
- Agent は途中で文脈を失う。checkpoint ごとに Contract / Summary を更新し、重要判断を曖昧な会話文脈へ押し込まない。
- Agent が分からない、実装できない、検証できない時は `executionDecision` を `blocked`、`contract_update_required`、`downgraded_to_investigation` にする。

## Required Commands

通常は次を実行する。

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-guards
make check-ai-backtrack
make check-ai-coverage-guard
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make quality
```

Work Item を完了する時は次を使う。

```bash
make ai-finish TASK=<task>
```

`make ai-finish` は required checks を再実行し、成功時だけ Work Item を archive する。失敗した場合は active のまま残す。

## Commit / Push Boundary

Commit 前に確認する。

- `make quality` または Work Item required checks が通っている。
- active Work Item が不要に残っていない。
- `.ai/cockpit/current_status.md` が現在状態と一致している。
- code / test / docs / i18n / report output / data or weekly record / Make or CI guard の確認結果が Summary に残っている。
- `ready_with_risks` の場合は residual risk と expected review focus が Summary と Cockpit status に残っている。
- commit message は日本語。

## Prohibited

- Contract なしで repository file を変更しない。
- scope 外の file をついでに変更しない。
- test green を Clippy green の代替にしない。
- `make quality` 失敗状態で ready / done と報告しない。
- Skill を投資判断や売買判断の手順として使わない。
