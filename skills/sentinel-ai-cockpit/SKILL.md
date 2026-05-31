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
   - 変更対象が `scope` に含まれる。
   - 変更対象が `outOfScope` に含まれない。
4. `notCodable: true` または `unknowns` が残る場合、production code を変更しない。調査、TODO、blocker 記録に限定する。
5. 実装前に、Gate / execution、report output、data branch、weekly calibration、i18n、evidence source、Make command の境界を Contract に反映する。
6. 作業後、summary の `changedFiles`、`sourcesUsed`、`verification`、`observedIssues` に加え、未解決の user correction / known gap / 未確認項目を更新する。
7. 必ず `make` 経由で check を実行する。


## Boundary Checklist

Work Item が機能追加、report 変更、データ永続化、AI governance 変更のいずれかを含む場合、次を明示する。

- Gate / execution / trader / action matrix へ影響するか。影響しない場合は表示・監査専用と書く。
- Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。
- data branch に保存するか、週次成果物だけに集約するか、保存しないか。
- zh / en / ja の i18n と snapshot / contract test が必要か。
- fact、manual observation、hypothesis、fixture、local cache を分離しているか。
- 新しい command は `make` target として提供されるか。

User correction が発生した場合は、単に修正せず、次回の backtrack 防止として Contract、Summary、doc、template、guard のどれへ固化するかを判断する。

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
- commit message は日本語。

## Prohibited

- Contract なしで repository file を変更しない。
- scope 外の file をついでに変更しない。
- test green を Clippy green の代替にしない。
- `make quality` 失敗状態で ready / done と報告しない。
- Skill を投資判断や売買判断の手順として使わない。
