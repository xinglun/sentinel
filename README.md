---
author: Ray
title: Stock Sentinel README
description: Stock Sentinel の概要、運用コマンド、監査サマリー運用を説明するトップガイド。
key: readme
---

# 🐕 Stock Sentinel (Capital Physics Engine)

## 目的
Stock Sentinelは、市場の変動を「物理的な観測」として捉え、感情に左右されない資本配分判断（DCA、防御、買い増し）を支援するための意思決定支援レーダーです。

## 読むタイミング
- システムの核心概念（飼い主-リード-犬モデル）と物理学的アプローチを理解したい時
- セットアップ方法や日常の運用・検証コマンドを確認したい時
- 資本状態（CAPITAL STATE）の読み方を知りたい時

---

## 🛰️ V1.3.0：Dual-Engine Architecture (双発エンジンアーキテクチャ)
本システムは、単一のYahoo Financeバッチスクリプトから、高頻度かつ持続的な接続をサポートする**双発エンジンアーキテクチャ**へと進化し、**Moomoo (Futu) OpenD** 取引ゲートウェイインターフェースを全面的に統合しました。

- **Dual-Engine Routing:**
    - **Yahoo Finance Engine**: HTTP RESTベースの軽量エンジン。GitHub ActionsなどのステートレスなCI環境での日次レーダー掃引（Daily Radar）に使用されます。
    - **Moomoo (Futu) Engine**: ローカルまたはプライベートサーバー向けに設計された重量級取引エンジン。TCP Protobufを介してOpenDゲートウェイと持続的に直結し、精度の高い修正履歴データの取得や、将来の自動実盤注文をサポートします。
- **CLI Commands (CLIコマンドの分離):** `radar`、`daemon`、`backtest` という新しい分離された実行エントリポイントを導入しました。

## 🚀 使用方法 (Usage)

### 1. 環境とパラメータの準備 (`config.toml` と環境変数)
- システムに Rust & Cargo (Edition 2021) がインストールされていることを確認してください。
- `config.toml` に Telegram ボット情報を設定します（環境変数 `TELEGRAM_BOT_TOKEN` も使用可能です）。
- **Moomoo 実盤設定**：`config.toml` の `[futu]` セクションで基本的なローカルネットワークパラメータを指定します。
    - `opend_ip` と `opend_port`: 通常は `127.0.0.1:11111` です。
- **プライベート取引権限**：セキュリティのため、以下の環境変数を設定してください（コード内に明文化しないでください）。
    - `FUTU_ACC_ID`: 牛牛/Moomoo クライアント内で取得したリアルまたはシミュレーション口座 ID。
    - `FUTU_UNLOCK_PASSWORD_MD5`: 取引のロック解除に必要なパスワード（MD5変換後の文字列）。相場購読のみの場合は不要です。

> **注意：** Moomoo OpenD ゲートウェイは、ローカルPCまたはサーバー上で実行し、安全スキャン認証を完了させておく必要があります。GitHub Actions などの CI 環境では、自動的に OpenD をスキップし、Yahoo データソースへとフォールバックします。

### 2. 日常の観測 (Daily Radar)
毎日の終値確定後、以下のコマンドで現在の「資本の天気」を確認します。通常、GitHub Actions によって夜間に実行されます。
```bash
make radar
```
ローカルで強制的に Moomoo から相場を取得する場合は、以下の引数を追加します。
```bash
make radar RADAR_ARGS="--provider futu --opend 127.0.0.1:11111"
```

### 3. 常駐取引デーモン (Daemon Mode) & 自動取引
全自動取引向けに設計されたモードです。起動後、TCPセッションを持続的に管理し、`KeepAlive` ハートビートと `[trading]` ロジックの評価を自動的に行います。
```bash
make daemon DAEMON_ARGS="--provider futu"
```

**実盤スイッチの説明 (Simulated vs Real Trading):**
Sentinel は、**安全な自動取引サンドボックス**を備えています。
1. `config.toml` のデフォルト設定 `trd_env = 1` は**シミュレーション環境 (Simulate)**です。シグナルが発生しても、Moomoo が提供する仮想資金のみが使用され、実際の金銭的損失はありません。
2. 実盤取引を開始する場合は、以下の**二重ロック**を同時に解除してください。
   - `[futu]` セクションの `trd_env = 1` を `trd_env = 0 (Real)` に変更します。
   - `[trading]` セクションの `enabled = false` を `enabled = true` に変更し、許容される `global_budget`（最高予算）を設定します。
   
ロック解除後、レーダーが `optimal` や `fear` シグナルを発すると、即座に Moomoo エンジンを介して現物の指値/成行注文が執行されます。

### 4. 歴史的検証 (Backtest Mode)
過去のデータを用いて、システムの「目盛り（Calibration）」と「アルファ分離」を検証します。
```bash
make backtest
```
- `./backtest/summary.md` に詳細なレポートが出力されます。

### 5. 週次レビュー (Metrics Review)
直近7日間の状態マシンのパフォーマンス指標を収集・集計し、人間による復習のための定量的根拠を提供します。週末や復習が必要な時に実行します。
```bash
make review
```
- `./reports/weekly_state_metrics.json` に集計指標が出力されます。

### 6. 日次監査サマリー (Audit Daily)
`state_transitions.jsonl` を監査用途で集約し、以下の5項目を固定フォーマットで出力します。

1. Gate サマリー（NO TRADE / READY、NO TRADE レイヤー、継続日数、主な阻害因子）
2. Transition サマリー（市場状態・リスクオーバーレイ・主線状態・NO TRADE レイヤーの変化有無）
3. Breakout サマリー（新規 / 継続 / 消失）
4. 連続セグメント統計（NO TRADE と主線欠如の連続長）
5. 監査ワンライン要約

補足:
- 連続セグメントは**ログ連続ベース**で計算し、週末は自動的に連結されます。
- `--date` / `--days` の不正指定はデフォルトにフォールバックせず、CLI はエラー終了します。
- `NO TRADE` は `初級（シグナルなし）` / `偵察（シグナル未検証）` の2層で監査されます。
- `rules.market_state_engine.scout_abort_days` 日の間に breakout が 2 銘柄以上へ拡散しない場合、偵察は自動 reset されます。

基本実行：
```bash
make audit-daily
```

期間を指定：
```bash
make audit-daily AUDIT_DAILY_ARGS="--days 30"
```

対象日を指定（YYYY-MM-DD）：
```bash
make audit-daily AUDIT_DAILY_ARGS="--date 2026-04-22 --days 30"
```

エイリアスコマンド：
```bash
make transition-audit-summary TRANSITION_AUDIT_ARGS="--days 30"
```

言語切り替え（`config.toml` の `output.language`）:
- `zh-cn`
- `en-us`
- `ja-jp`

```toml
[output]
language = "ja-jp"
```

出力例（先頭行のみ）:

```text
# zh-cn: # Audit Daily (2026-04-22)
1. Gate 摘要
```

```text
# en-us: # Audit Daily (2026-04-22)
1. Gate Summary
```

```text
# ja-jp: # Audit Daily (2026-04-22)
1. Gate サマリー
```

## 📁 ドキュメント (Documentation)

### SSOT / Current Specs
- [Documentation Guide](./docs/README.md) - ドキュメントの階層構造、閲覧順序、管理ルール
- [PRD](./docs/specs/PRD.md) - システムの鉄則、製品の境界、コア要件
- [Decision Packet Schema](./docs/specs/DECISION_PACKET_SCHEMA.md) - `DecisionPacket` のメインコントラクト
- [State Definitions](./docs/specs/STATE_DEFINITIONS.md) - 市場状態の定義
- [Transition Rules](./docs/specs/TRANSITION_RULES.md) - 状態遷移ルール
- [Action Matrix](./docs/specs/ACTION_MATRIX.md) - 市場状態 × 資産状態 → アクション
- [Data Branch Layout](./docs/specs/DATA_BRANCH_LAYOUT.md) - `data` ブランチのアーカイブ基準
- [Hosting Spec](./docs/specs/hosting_spec.md) - GitHub Actions のホスティングとアーカイブ要件

### Implementation
- [DDD Clean Architecture](./docs/architecture/DDD_CLEAN_ARCHITECTURE.md) - feature-first / Clean Architecture の現行境界
- [Implementation Walkthrough](./docs/architecture/IMPLEMENTATION_WALKTHROUGH.md) - 現行 feature-first 実装ガイド
- [Architecture Design](./docs/architecture/architecture_design.md) - 構造設計とデータフロー
- [Strategy Philosophy](./docs/architecture/strategy_philosophy.md) - 戦略設計哲学

### Historical Materials
- [Archive Roadmap](./docs/archive/decision_engine_roadmap.md) - 過去の再構築ロードマップ
