---
author: Ray
title: AI ガバナンス入口
description: Sentinel における AI 作業契約、検証、回帰防止記録の最小運用入口。
key: ai-governance-entry
---

# AI ガバナンス入口

`.ai/` は、AI Agent による複雑な変更を「作業契約」「検証」「完了記録」「回帰防止」の単位で追跡するための機械可読な入口です。

目的は新しい分析機能を追加することではなく、Sentinel のエンジニアリング進化における以下のリスクを抑えることです。

1. scope 外の変更。
2. テストや snapshot の無宣言な削除。
3. Telegram / audit / i18n の契約回帰。
4. 検証未実行のまま完了扱いにすること。
5. Agent が実装不能、検証不能、または review risk を認識しているのに、それを表明する合法 channel がないこと。

## 構成

| パス | 用途 |
|---|---|
| `.ai/cockpit/` | 現在の Work Item 状態、共通 check catalog、Cockpit 説明。 |
| `.ai/guards/` | Backtrack Guard などの回帰防止 policy。 |
| `.ai/work-items/` | Contract、Summary、Review checklist の template と active task。 |

`current_status.md` の active / no-active 表示、archive 後の同期、参照整合性は `.ai/cockpit/status_policy.yaml` を正とします。

## 運用原則

AI 作業は通常の開発品質を免除しません。複雑な diff を伴う変更では、作業前に Contract を作成し、完了時に Summary と検証結果を残します。

軽微な質問、調査のみ、diff を伴わない説明は Work Item 化しません。対象は reviewer が scope、根拠、検証、リスクを追跡する価値がある変更です。


## Work Item 境界 checklist

新しい Work Item を code mode で開始する時は、実装前に次の境界を Contract の acceptance または sources に反映します。

1. **Decision boundary**: Gate、execution、trader、action matrix、新規建玉上限へ接続するか。接続しない場合は「表示 / 監査専用」と明記する。
2. **Report boundary**: Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。表示しない場合も明記する。
3. **Persistence boundary**: data branch、reports、JSONL、snapshot、weekly metrics のどこへ残すか。全文保存と構造化 record を混同しない。
4. **Language boundary**: user-facing 出力の対象言語、i18n 追加、snapshot / contract test の必要性を明記する。
5. **Evidence boundary**: fact、manual observation、hypothesis、fixture、local cache を区別し、Reality Layer と Hypothesis Layer を混ぜない。
6. **Command boundary**: 検証、補助 script、CI entrypoint は原則 `make` target 経由にする。裸の `python3 scripts/...` を新しい運用手順として追加しない。
7. **Data branch boundary**: 長期記録は data-only branch に置く。code tree や AI governance file を data branch へ持ち込まない。認知校正の長期比較は週次粒度を標準とする。
8. **Risk / readiness boundary**: `riskAssessment`、`agentCapability`、`executionDecision` で、実装可能性、検証可能性、人間判断の要否、review 前の警告を明記する。

この checklist は作業を遅くするためではなく、実装後に「どこへ表示するか」「何を保存するか」「Gate に影響したか」を人間が繰り返し指摘しなくて済むようにするための最小契約である。

## Risk と review readiness

Work Item は、Agent が問題を認識した時に「できる」と過大主張しないための停止・降格 channel を持ちます。

AI Agent の運用 risk は次の三つを前提に扱います。

1. Prompt は助言であり、命令ではない。重要な制約は `make` target、hook、gate、scope guard で検証する。
2. Agent は実行途中で文脈を失うことがある。Contract と Summary は中間判断を曖昧な会話文脈に残さず、checkpoint ごとに更新し、必要なら `make ai-checkpoint` で evidence snapshot を出力する。
3. Agent には合法に「分からない」「実装できない」「検証できない」と言える channel が必要である。証拠不足や能力不足を実装で埋めない。

Contract では次を使います。

- `riskAssessment`: risk level と risk type を記録する。`blocked` は実装前に人間判断が必要な状態を表す。
- `agentCapability`: Agent が実装できるか、検証できるか、人間判断が必要かを記録する。
- `executionDecision`: `continue`、`contract_update_required`、`blocked`、`downgraded_to_investigation` のいずれかを記録する。
- `preReviewWarnings`: review で問題化しそうな観点を事前に記録する。
- `checkpointPolicy`: `contract_start`、`before_edit`、`before_ready`、`after_verification` の確認点と reminder を記録する。
- `checkpointEvidence`: Summary に checkpoint ごとの evidence snapshot を残す。`contractHash`、`acceptanceCount`、`unknownCount`、`requiredChecks`、`requiredChecksPassed` を更新する。

`mode: code` かつ `executionDecision: continue` の Contract では、AI governance の hard gate を required verification として持つ必要があります。少なくとも Contract、scope、guard、backtrack、summary、status の `make` gate が必要です。

`agentCapability.canImplement: false`、`agentCapability.canVerify: false`、または `agentCapability.needsHumanDecision: true` の場合、`executionDecision: continue` にはできません。この場合は `blocked`、`contract_update_required`、または `downgraded_to_investigation` に切り替えます。

Summary では次を使います。

- `residualRisks`: required checks 通過後も残る risk を記録する。
- `reviewReadiness`: `ready`、`ready_with_risks`、`not_ready` のいずれかを記録する。
- `expectedReviewFocus`: reviewer が重点確認すべき観点を記録する。
- `userCorrectionSolidification`: user correction を contract、summary、doc、template、guard、skill のどこへ固化したかを記録する。

`ready_with_risks` は失敗状態ではありません。Contract の acceptance と required checks は満たしたが、review で確認すべき残余 risk が明確に残る状態です。`not_ready` は required checks 未通過、Contract 未確定、または人間判断待ちの状態です。

## 最小フロー

1. `make ai-start TASK=<task> TITLE="..." MODE=code` で Contract / Summary を作成する。
2. `unknowns` と `notCodable` を確認する。
3. `riskAssessment`、`agentCapability`、`executionDecision` を確認する。`contract_update_required`、`blocked`、`downgraded_to_investigation` の場合は実装を進めず、Contract 更新、調査、または blocker 記録に切り替える。
4. `checkpointPolicy` を確認し、少なくとも `contract_start`、`before_edit`、`before_ready`、`after_verification` の判断点を保持する。
5. `scope` と `outOfScope` の範囲内で実装する。
6. `verification` に記載した command を `make` 経由で実行する。
7. `.ai/work-items/active/<task>.summary.json` に結果、residual risk、review readiness、expected review focus、checkpoint evidence を記録する。
8. `make check-ai-guards CONTRACT=.ai/work-items/active/<task>.contract.json`、`make check-ai-backtrack CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json`、`make check-ai-coverage-guard` を実行し、scope 外変更、無宣言な回帰、および test 証跡不足がないことを確認する。PR に archive 配下の変更が含まれる場合は `make check-ai-pr AI_BASE_COMMIT=<merge-base-sha>` を CI で併用する。
9. `make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json` で `.ai/cockpit/current_status.md` を更新する。
10. `make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json` と `make check-ai-status-consistency` で参照整合性を確認する。
11. 完了時は `make ai-finish TASK=<task>` で required checks を再実行し、成功時だけ archive する。

`make test-ai-guards` は `.ai/**/*.yaml` を parse し、YAML として読めない governance 設定を失敗させる。`make check-ai` と `make quality` はこの parse guard を含む。

`scripts/ai_*.py` は Makefile target から呼び出される実装詳細であり、通常の運用入口は `make` target とします。

## 境界

この仕組みは開発工程の hard gate です。`src/**`、`tests/**`、`docs/**`、`scripts/**`、CI、AI governance などの管理対象 diff は Contract scope に明示された変更だけを許可します。加えて、`.ai/guards/file_ownership.yaml` の `aiWrite: restricted` file の無承認変更、test / snapshot / i18n / Work Item evidence の無宣言削除、および production Rust code の test 証跡不足を阻止します。

CI は `AI_DIFF_BASE` で push / pull request の committed diff を検証し、clean checkout で guard が空振りする状態を許容しません。

この hard gate は取引判断、Gate、Telegram 文案ロジック、証拠スコアには影響しません。
