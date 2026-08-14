---
author: Ray
title: Validation Correction V1 業務 Utility と Cohort 統計修正
description: Validation Epoch V1 の業務上の母集団、censoring、cohort、utility、confirmation cost の欠落を修正する設計。
key: validation-correction-v1-design
---

# Validation Correction V1

## 目的

Validation Epoch V1 の実装は Decision Classification、基本 horizon、MFE / MAE、Raw Top-3 baseline を保持している。一方で、Protection Value の母集団、censored sample、decision cohort、Confirmation Cost、双方向の utility 集計に業務上の欠落が残っている。本 Work Item は既存の decision semantics を変更せず、Validation の集計境界と report fact を修正する。

## 固定する業務境界

- `NO_TRADE` の Protection 母集団は `raw_candidate && gate_blocked && decision_class == NO_TRADE` とする。`gate_blocked` は ACL で reason code を再解釈せず、既存 Radar fact の `trend_cohesion.gate_passed == false` を射影する。
- T+5 / T+10 / T+20 が未完了の record は、該当 horizon の平均、downside count、upside count に含めない。
- `decision_snapshot_version` と `universe_id` の組を cohort key とし、異なる cohort の utility を合算しない。
- transition context が欠けた record は `classification_available = false` として保存し、decision outcome の母集団から除外する。
- `CONFIDENCE_INSUFFICIENT` は既存 production rule の `system_confidence < 50` を理由 code に射影するだけで、threshold や class semantics は変更しない。

`classification_available` は現行 legacy packet の互換性を保つ provenance flag であり、transition context の存在だけを示す。将来の Decision Contract では production が明示する availability fact に置き換える候補として Summary に残す。

## Record と cohort

Validation record に `gate_blocked`、`classification_available`、`return_breakout_to_ready` を保持する。既存の `decision_close`、snapshot version、universe、lifecycle date、forward outcome は維持する。

各 `(decision_snapshot_version, universe_id)` について独立した `ValidationCohortReport` を生成する。cohort の内部では class outcome と Raw Top-3 / READY baseline を計算する。全 cohort を一つの平均へ混ぜる aggregate は出力しない。

## Protection と Opportunity Cost

NO_TRADE の同一 blocked-candidate cohort から、完成した T+20 outcome だけを使う。

- Protection: sample count、平均 / 中央値 / P90 / P95 MAE、下落サンプル数。
- Opportunity Cost: 平均 MFE、positive forward-return sample 数、positive return 平均、positive MFE の上位 10% 平均。
- pending / partial record は未来が未確定であるため、downside や missed upside とみなさない。

P90 / P95 は昇順の empirical nearest-rank とし、空の母集団は `N/A` とする。top-decile missed upside も完成した positive MFE のみを対象とし、サンプルが少ない場合は存在する上位 1 件以上から計算する。

## Confirmation Cost

各 symbol の lifecycle index から次を計算する。

- Strength → Breakout latency
- Breakout → Ready latency
- Strength → Ready latency
- Strength → Ready return
- Breakout → Ready return
- Strength → Ready maximum move

Strength は現行 Validation の Raw Top-3 初回観測を指す。意味を変更して新しい strength detector を作らない。

## Counterfactual baseline

同一 cohort、同一完成 horizon、同一 Raw Top-3 母集団で、Raw Top-3 と READY subset を比較する。両方について return、MAE、MFE を保存し、次の差分を report する。

- READY return - Raw return
- READY MAE - Raw MAE
- READY MFE - Raw MFE

差分は utility の構成事実であり、Net Decision Value はこれらを隠さない摘要として表示する。

## Sample maturity

Maturity の分母は classification context が利用可能な cohort decision records とする。無効 context や未来 horizon の pending record だけで `USABLE` に到達させない。閾値は既存の `<30 / 30–99 / >=100` を維持する。

## 出力境界と非目標

出力先は既存の `backtest/<run>/summary.md` と `validation.json` に限定する。Telegram、audit daily、weekly review、i18n、data branch、portfolio simulation、Gate / Execution / Trader semantics は変更しない。

## 検証

先に失敗する unit / integration test で、母集団限定、censoring、cohort 分離、tail metrics、双方向 utility、confirmation cost、baseline 差分、欠損 context を固定する。その後に最小実装を追加し、全 Rust quality gate と Cockpit required checks を通過させる。
