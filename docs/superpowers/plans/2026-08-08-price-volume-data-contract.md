---
author: Ray
title: Price-Volume Data Contract 実装計画
description: S-28-01 の OHLCV 市場データ契約をテスト先行で実装する計画。
key: price-volume-data-contract-plan
---

# Price-Volume Data Contract Implementation Plan

**Goal:** Price-Volume Structure が利用する欠損可能な OHLCV 観測値を提供する。

**Architecture:** `DailyBar` に open/high/low を追加し、Yahoo と Futu adapter が provider の日足値を投影する。既存の close-only consumer は変更しない。

**Tech Stack:** Rust、chrono、yahoo_finance_api、Futu protobuf、Cargo test。

## Global Constraints

- Observation only。取引 decision への接続は禁止する。
- 欠損値を補推しない。
- テストを先に追加して失敗を確認する。

### Task 1: DailyBar の OHLCV 契約

- [ ] DailyBar の OHLC field と、欠損を保持する unit test を追加する。
- [ ] `make test` を実行し、field 未定義による失敗を確認する。
- [ ] `open`、`high`、`low` を `Option<f64>` として追加し、全 fixture literal を更新する。
- [ ] `make test` を実行する。

### Task 2: Provider 投影

- [ ] Yahoo/Futu の OHLC mapping を検証する test を追加する。
- [ ] `make test` を実行し、期待する mapping failure を確認する。
- [ ] provider の取得可能な OHLC 値を DailyBar に投影する。
- [ ] `make test`、`make fmt-check`、`make clippy` を実行する。
