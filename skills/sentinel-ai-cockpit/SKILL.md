---
name: sentinel-ai-cockpit
description: Enforce Sentinel repository AI Cockpit workflow for Codex or Gemini when changing code, tests, docs, CI, .ai, skills, Makefile, AGENTS.md, or GEMINI.md. Use this before editing repository files, during Work Item execution, before review, and before commit/push to ensure the installed ai-cockpit Runtime, Contract scope, Summary, make checks, and lifecycle are followed.
---

# Sentinel AI Cockpit

Sentinel repository で変更を行う時は、この Skill を実行規律として使う。投資判断、戦略判断、売買判断は扱わない。

## Core Rule

`src/**`、`tests/**`、`docs/**`、`.github/workflows/**`、`.ai/**`、`skills/**`、`Makefile`、`AGENTS.md`、`GEMINI.md` を変更する場合、必ずインストール済み `ai-cockpit` の Work Item Contract を入口にする。

## Workflow

1. `AGENTS.md`、`.ai/cockpit/README.md`、`ai-cockpit inspect --repo .` を確認する。
2. active Work Item を探す。
   - 存在する場合: `.ai/work-items/active/<task>.contract.json` と Summary を読む。
   - 存在しない場合: `ai-cockpit work-item new --repo . --id <task> --mode code` で scaffold を作成する。
3. `ai-cockpit start` で intent、goal、scope、outOfScope、risk、authority、acceptance、required evidence を確定する。
4. coding 前に次を確認する。
   - `mode: code`
   - `notCodable: false`
   - `unknowns: []`
   - `executionDecision: continue`
   - `agentCapability.canImplement: true`
   - `agentCapability.canVerify: true`
   - `agentCapability.needsHumanDecision: false`
   - 変更対象が `scope` に含まれる。
   - 変更対象が `outOfScope` に含まれない。
   - 高リスク作業では `scenarioCoverage`、破壊的変更では authority と approval evidence がある。
5. `notCodable: true`、`unknowns`、検証不能、または停止値の `executionDecision` が残る場合、production code を変更しない。
6. `ai-cockpit preflight` を実行し、結果を確認してから `ai-cockpit checkpoint` を編集前に実行する。
7. scope の範囲内だけを変更する。既存の project code、data、evidence、履歴 Work Item を無断で削除しない。
8. `ai-cockpit verify` で Contract の required `verification[].check` を `make` 経由で実行し、`finish`、`archive` を順に実行する。Summary は Runtime の lifecycle receipt を正とする。
9. required check が失敗した状態で `ready_for_review` と報告しない。残余 risk は `residualRisks`、review 注視点は `expectedReviewFocus` に分けて記録する。
10. `close` は人間の最終判断、authority source、evidence ref が必要な場合だけ実行する。

## Boundary Checklist

Work Item が機能追加、report 変更、データ永続化、AI governance 変更のいずれかを含む場合、次を明示する。

- Gate / execution / trader / action matrix へ影響するか。影響しない場合は表示・監査専用と書く。
- Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。
- data branch に保存するか、週次成果物だけに集約するか、保存しないか。
- zh / en / ja の i18n と snapshot / contract test が必要か。
- fact、manual observation、hypothesis、fixture、local cache を分離しているか。
- AI Cockpit lifecycle は独立 CLI の command として提供され、プロジェクト固有の検証 command だけが根 `Makefile` の `make` target になっているか。
- Agent が実装できるか、検証できるか、人間判断が必要かを `agentCapability` に明記しているか。
- review で追加論点が出る可能性を `preReviewWarnings` と Summary の `expectedReviewFocus` に記録しているか。
- 重要判断を会話文脈だけに残さず、Contract、Summary、checkpoint evidence に戻しているか。

User correction が発生した場合は、単に修正せず、次回の回帰防止として Contract、Summary、document、template、guard、skill のどこへ固化するかを `userCorrectionSolidification` に記録する。

## Risk / Readiness Channel

- Contract: `riskAssessment`、`agentCapability`、`executionDecision`、`preReviewWarnings`
- Summary: `residualRisks`、`reviewReadiness`、`expectedReviewFocus`、`userCorrectionSolidification`
- 高リスク作業: `scenarioCoverage` と Summary の実行 evidence

`ready_with_risks` は required checks 通過後に review focus が残る状態を表す。`not_ready` や `blocked` の task を ready / done と報告しない。

## Required Commands

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

## Commit / Push Boundary

Commit 前に確認する。

- `make quality` または Contract の required verification が通っている。
- active Work Item が不要に残っていない。
- `ai-cockpit status --repo .` と Summary の lifecycle state が一致している。
- code / test / docs / i18n / report output / data or weekly record / Make or CI guard の確認結果が Summary に残っている。
- `ready_with_risks` の場合は residual risk と expected review focus が Summary に残っている。
- commit message は日本語。

## Prohibited

- Contract なしで repository file を変更しない。
- scope 外の file をついでに変更しない。
- test green を Clippy green の代替にしない。
- `make quality` 失敗状態で ready / done と報告しない。
- 旧 `Makefile.ai`、旧 `make ai-*`、`scripts/ai_*.py` を新しい AI Cockpit 運用入口として呼び出さない。
- Skill を投資判断や売買判断の手順として使わない。
