---
author: Ray
title: Probe 転換観測の実装計画
description: canonical permission と eligibility の独立監査投影。
key: probe-eligibility-conversion-observation-plan
---

# Probe 転換観測

目的: permission が eligibility に転換する頻度と待ち時間を測定する。

## 境界と解釈

連続 Probe run を一つの episode とし、Eligible 到達は milestone とする。到達後も Probe 日数を加算する。UNKNOWN、欠損 session、左端切断は censor とし、完全 episode の分母に含めない。既知 Probe かつ不明 eligibility は独立 exclusion とする。率の分母がゼロなら null、quality は PARTIAL。20/60/all の session は既存 NYSE calendar に従う。

Decision/Gate/Eligibility/Execution/Trader/Position Sizing と全 trading threshold は変更しない。観測は decision_weight=0、trade_signal=false。既存の表示文字列から事実を復元しない。

## 実装順序

- [x] `domain/probe_eligibility_observation.rs`: canonical observation、episode、window を定義。A/B/C、欠損 eligibility、UNKNOWN、左端・右端、重複 revision、ゼロ分母を先にテストする。
- [x] `infrastructure/persistence/probe_observation.rs`: run identity を持つ独立 sidecar を atomic 保存し、履歴を読み取る。古い snapshot の欠損をゼロにしない。
- [x] `interface/probe_eligibility_read_model.rs`: FinalExecutionDecision の enum と count を投影する。既存 calendar で session を作る。Decision の immutable borrow のみ。
- [x] `interface/presentation.rs` / `radar_pipeline_runner.rs`: 観測を report assembly 後に接続し、保存失敗は観測品質だけを降格させる。
- [x] `interface/report.rs`: 三言語の共通 field list から Markdown/Telegram/Archive を生成。通常20、archive全 window。
- [x] canonical decision の before/after serialization、renderer 共通値、履歴 unknown と persistence roundtrip を検証する。
- [ ] fmt-check/test/clippy/quality、Runtime verify/finish/archive、draft PR、hosted CI/review/merge、finalize/close を実行する。

## Review focus

- 左端 Probe が既に継続中なら completed episode と誤認しない。
- holiday と欠損 trading session を区別する。
- 同日同時刻の相反する revision を順序依存で選択しない。
- 観測保存失敗で execution を変更しない。
- historical serde default で eligible missing を zero と誤認しない。

第二 WI はこの WI 完了後に独立 branch/PR で着手する。

## 検証境界

初期履歴の先頭が Probe の場合、既知の直前 non-Probe fact がないため左端 censor とする。仕様の B/C regression は直前 non-Probe が確認できる完全 episode として検証し、先頭切断版は別の fail-closed test とする。coverage と統計 quality は別で、サンプルゼロは quality PARTIAL。Eligible milestone と実際の exit permission は独立に保持する。

保存先は `probe_observations/<market-date>-<run-id>.json` と `probe_observation_archives/<run-id>.json`。後者は20/60/all、episode、率の生値、quality を含む。旧 snapshot を変更せず欠損日は UNKNOWN とする。読み込み・保存失敗は観測のみ降格する。
