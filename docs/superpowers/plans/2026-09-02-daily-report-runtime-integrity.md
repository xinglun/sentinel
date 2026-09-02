---
author: Ray
title: 日次レポート Runtime Integrity 実装計画
description: Runtime identity、provenance、RS/Leadership integrity、snapshot conflict、report lifecycle を段階的に実装する計画。
key: daily-report-runtime-integrity-plan
---

# 日次レポート Runtime Integrity 実装計画

## 目的

既存の Decision semantics を変更せず、日次レポートの生成 run、code revision、data snapshot、
観測入力、Leadership facts、正式 snapshot、再送 lifecycle を同じ identity chain に結び付ける。
設計の正規文書は `docs/superpowers/specs/2026-09-02-daily-report-runtime-integrity-design.md` である。

## 実装前の確認

- 専用 branch/worktree と current origin/develop base を維持する。
- `ai-cockpit inspect/status/doctor/agent doctor --repo .` を再確認する。
- current Contract の preflight、human decision evidence、before-edit checkpoint が current snapshot に
  bind されていることを確認する。
- `NO_TRADE`、`PROBE`、`READY`、Gate、Action Matrix、Leadership/RS threshold、Breadth、Breakout、
  Supply、Position Sizing、Execution、Trader、Validation Engine を変更対象から除外する。

## 実装手順

### 1. Runtime Identity と共有 contract

対象: `build.rs`、`src/features/shared/application/run_status.rs`、
`src/features/radar/application/radar.rs`、`src/features/radar/interface/radar_pipeline_runner.rs`

- build script で build revision と branch の compile-time metadata を埋め込む。取得不能時は `UNKNOWN` とする。
- `RadarRunContext` の生成時に `report_run_id` と `report_run_at` を一度だけ作る。
- `ReportRuntimeIdentity`、`DataProvenance`、`DataProvenanceBundle`、`RuntimeIntegrity`、
  `ReportLifecycle` を serde 付きで追加する。legacy JSON の deserialize は維持する。
- execution SHA は workflow が渡す値を優先し、build SHA と比較する。branch 名を SHA の代用にしない。
- identity/provenance/integrity を `RunOutcome` と `PresentationPacket` へ渡す。

先に追加するテスト:

- identity の同一 run 内安定性。
- build revision と execution revision の一致、欠落、`RUNTIME_MISMATCH`。
- 旧 `RunOutcome` JSON の読み込み。

### 2. RS Observation Health

対象: `src/features/radar/domain/current_relative_strength.rs`、
`src/features/radar/application/engine.rs`、
`src/features/radar/interface/presentation.rs`、
`src/features/radar/interface/presentation_assembler.rs`

- `RelativeStrengthState::Unavailable` と observation health/diagnostic を追加する。
- 完全な 1d/5d input の既存計算は変更しない。値が実際に 0 の場合は `NEUTRAL` を保持する。
- benchmark、asset price、comparable session が欠落する場合は ticker observation を落とさず `UNAVAILABLE` とする。
- presentation と diffusion の分母から unavailable observation を除外し、全入力欠落時の `0/0` を
  `UNAVAILABLE` と明示する。

先に追加するテスト:

- 完全 RS input の Neutral/5/9 regression。
- benchmark history 欠落。
- 全 RS input 欠落と all-NEUTRAL 防止。
- legacy observation JSON の読み込み。

### 3. Leadership facts の共有

対象: `src/features/radar/domain/leader_persistence/mod.rs`、
`src/features/radar/domain/leader_persistence/persistence.rs`、
`src/features/radar/interface/presentation.rs`、
`src/features/radar/interface/presentation_assembler.rs`、
`src/features/radar/interface/market_interpretation_read_model.rs`

- Leadership Snapshot に snapshot identity、coverage、calculation mode を追加する。
- 既存 persistence algorithm の streak/since/duration/last confirmed leader の計算式は変更しない。
- persisted formal baseline が完全なら `PERSISTED_FACT`、部分履歴なら
  `RECOMPUTED_FROM_PARTIAL_HISTORY`、履歴なしなら `UNAVAILABLE` とする。
- Summary、Leader Persistence、Market Interpretation が同じ Leadership facts の since/duration/last
  confirmed leader を参照するようにする。

先に追加するテスト:

- persisted snapshot の三 consumer 間での since/duration/last confirmed leader 一致。
- partial history の mode と estimated/reconstructed notice。
- 既存の leader persistence tests の semantic-equivalent。

### 4. Formal snapshot digest と conflict

対象: `src/features/radar/infrastructure/persistence/model.rs`、
`src/features/radar/infrastructure/persistence.rs`、
`src/features/radar/interface/radar_pipeline_runner.rs`

- `TradingDaySnapshot` に optional な report run、revision、data/decision/observation digest、
  runtime integrity を追加する。
- canonical JSON から三つの digest を計算する。volatile identity は既存の semantic comparison から除外する。
- 同じ cycle/date で digest が異なる場合は `SNAPSHOT_CONFLICT` とし、既存 bytes を保持する。
- legacy snapshot の digest 不在は read-compatible として扱い、current run の provenance を partial にする。
- runner の snapshot conflict/error path が integrity と run status に診断を残す。

先に追加するテスト:

- 同一 snapshot の rerun が `SameDayRerun` になる。
- digest mismatch が conflict になり、既存 snapshot が変わらない。
- legacy snapshot の deserialize と baseline recovery。

### 5. Report metadata と fail-closed presentation

対象: `src/features/radar/interface/report.rs`、
`src/features/shared/interface/i18n.rs`、
`src/features/radar/interface/radar_pipeline_runner.rs`

- Markdown 先頭へ runtime identity metadata block を追加する。
- Archive Markdown に provenance、integrity、RS ticker diagnostics、Leadership provenance を追加する。
- Telegram は短縮 identity と integrity notice を表示し、詳細 diagnostics は省略する。
- `DEGRADED`/`UNAVAILABLE` の表示は read-only とし、decision packet と execution path へ渡さない。
- 最新生成 report の Risk Summary wording と metadata が同じ generation run に bind されることを確認する。

先に追加するテスト:

- Archive が全 RS diagnostic と provenance を含む。
- Telegram が integrity notice を含み、通常 report body の既存 wording を壊さない。
- integrity の `decision_weight` が常に 0。

### 6. Generated/Resent lifecycle と workflow

対象: `src/features/shared/application/run_status.rs`、
`src/features/radar/interface/radar_pipeline_runner.rs`、`.github/workflows/daily_radar.yml`

- generated run は `GENERATED`、resend は `RESENT` として元の generation run/revision と resend revision を記録する。
- resend step は data branch の正式 report/status を read-only で読み、radar 再計算、snapshot write、
  report re-render を行わない。
- current workflow の checkout SHA validation を保ち、通常生成時に execution SHA を binary へ渡す。
- resend は原文を維持し、現在の Risk Summary wording で旧 report を上書きしない。

先に追加するテスト:

- workflow static assertion で resend が generate path を呼ばないことを確認する。
- 旧 report の内容が保持され、generation と resend revision が別フィールドになることを確認する。

### 7. 不変性回帰と統合確認

対象: `tests/daily_report_runtime_integrity.rs` と既存 tests

- 8 シナリオを一つの focused integration test suite に整理する。
- Decision、Gate、Leader calculation、Action Matrix、Position Sizing、Execution の projection を
  before/after semantic-equivalent として比較する。
- 変更差分を Contract scope と照合し、forbidden decision surface に変更がないことを確認する。

## 検証手順

実装後、次を順に実行する。

```text
make fmt-check
make test
make clippy
make quality
```

次に Rust AI Cockpit の `ai-cockpit verify --repo . --work-item daily-report-runtime-integrity --command make --args quality`
を実行し、required verification evidence を記録する。required scenario の evidence を Contract/Summary に
戻した後、`ai-cockpit preflight`、`finish`、`archive` を再実行する。

## 失敗時の扱い

- production code の test が赤い間は次の機能へ進まず、直前の Work Item scope 内で修正する。
- digest mismatch、revision 不明、partial history、旧 artifact の metadata 不在は推測で補完せず、
  report integrity を `DEGRADED` または `UNAVAILABLE` として残す。
- required check が失敗した状態では `ready_for_review` や green Outcome を報告しない。
- 新 provider、threshold、Decision semantics、旧 report の migration が必要になった場合は実装を止め、
  Contract amendment または別の Work Item を要求する。

