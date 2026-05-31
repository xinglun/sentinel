---
author: Ray
title: Sentinel Data Branch
description: Sentinel の長期検証用データ branch に保存する永続 artifact の一覧。
key: sentinel-data-branch
---

# Sentinel Data Branch

この branch は、Sentinel の実行結果を長期検証するための data-only branch である。production code、AI Cockpit、設定 file は保持せず、日次・週次の検証に必要な生成 artifact だけを保存する。

## 保存対象

- `reports/YYYY-MM-DD.md`: 日次 Radar report。
- `reports/decision_packet_YYYY-MM-DD.json`: 日次 decision packet。
- `reports/run_status_YYYY-MM-DD.json`: 日次 pipeline 実行状態。
- `reports/portfolio_snapshot_YYYY-MM-DD.json`: portfolio snapshot。
- `reports/account_snapshot_YYYY-MM-DD.json`: account snapshot。
- `reports/state_transitions.jsonl`: 状態遷移監査 log。
- `reports/state_transitions.csv`: 状態遷移監査 CSV。
- `reports/evidence_records.jsonl`: 実質証拠 record。
- `reports/evidence_collection_status_latest.json`: 証拠収集状態の latest snapshot。
- `reports/weekly_state_metrics.json`: 週次状態 metric。
- `reports/weekly_state_review_auto.md`: 週次自動 review。
- `backtest/state_machine_metrics_latest.json`: 最新 backtest metric。
- `backtest/archive/state_machine_metrics_*.json`: backtest metric archive。

## Gray Rhino 保存対象

- `reports/gray_rhino_candidates.jsonl`: 自動発見 candidate。
- `reports/gray_rhino_discovery_runs.jsonl`: discovery run 監査 log。
- `reports/gray_rhino_snapshots.jsonl`: 日次 escalation snapshot。
- `reports/gray_rhino_governance_extraction_audit.jsonl`: governance extraction audit。
- `reports/gray_rhino_governance_source_manifest.jsonl`: governance source manifest。
- `reports/gray_rhino_refresh_status.jsonl`: refresh status log。
- `reports/gray_rhino_refresh_status_latest.json`: latest refresh status。
- `reports/gray_rhino_refresh_status_YYYY-MM-DD.json`: 日付別 refresh status。
- `reports/gray_rhino_sources/**`: SEC、Finnhub、FRED などの source cache。

## 運用原則

- code branch と full tree diff で比較しない。この branch は data-only branch として意図的に code tree を持たない。
- data branch へ同期する場合は、上記保存対象だけを追加・更新する。
- local cache を無差別に上書きしない。既存 data を SSOT とし、欠落している artifact だけを追記する。
- JSONL は追記型とし、同一行の重複を避ける。
- daily-calibration の表示結果そのものは標準保存対象ではない。長期検証には `gray_rhino_snapshots.jsonl`、`state_transitions.jsonl`、`weekly_state_metrics.json` を使う。
