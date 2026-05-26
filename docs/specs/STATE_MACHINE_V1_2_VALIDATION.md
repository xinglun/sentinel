---
author: Ray
title: Sentinel 状態機 V1.2 検証および可観測性プラン
description: Sentinel 状態機 V1.2 検証および可観測性プラン に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-v1-2-validation
---

# Sentinel 状態機 V1.2 検証および可観測性プラン

## 1. 目的

V1.1 において、状態機判定プリミティブの構成（Config）化と構造化監査が完了しました。  
V1.2 では核心となる判定ロジックは修正せず、以下の2点に重点を置きます：

1. バックテストと過去のサンプルを用いて、V1.1 が実際に誤ったリセットや状態のチャタリング（抖動）を減少させているかを検証する。
2. `transition_audit`（遷移監査）を日次レポートやデバッグ出力に接続し、日常的な可観測性（Observability）を確立する。

本フェーズの目標は「状態機を修正すること」ではなく、「状態機を検証し、見える化すること」です。

---

## 2. 範囲

### 2.1 実施事項

1. 状態機のバックテスト検証指標の追加。
2. `transition_audit` のファイル保存とサマリー出力の追加。
3. 日次レポートおよびデバッグ出力への状態遷移サマリーの追加。

### 2.2 非目標 (Out of Scope)

1. Telegram の主要スタイルの変更。
2. 新しい市場状態（Market Regime）の導入。
3. 新しい取引アクション（Action）の追加。
4. 執行層のさらなる拡張。

---

## 3. ワークフロー

### 3.1 バックテスト検証 (Backtest Validation)

目標：

V1.1 が旧状態機と比較して、以下の問題を実際に改善したかを評価します：

1. 誤ったリセット回数の減少。
2. 多段階の急激な変化（マルチステップ・ジャンプ）の減少。
3. 状態のチャタリング（抖動）の減少。
4. 防御的トリガーの説明性の向上。

推奨される新規指標：

1. `reset_count` (リセット回数)
2. `blocked_reset_count` (ブロックされたリセット回数)
3. `multi_step_downgrade_attempt_count` (多段階降格試行回数)
4. `duration_lock_count` (期間ロック回数)
5. `soft_reset_count` (ソフトリセット回数)
6. `defensive_override_count` (防御的オーバーライド回数)
7. `state_flip_count_5d`
   - 5日間における状態の頻繁な切り替わり回数。

推奨される出力：

1. `backtest/state_machine_metrics.json`
2. `backtest/state_machine_metrics.md`

### 3.2 遷移監査の可視化 (Transition Audit Surfacing)

目標：

`transition_audit` を「パケット内にのみ存在するデバッグデータ」から、以下のレベルへアップグレードします：

1. 日次実行において可視化される。
2. デバッグ時に追跡可能である。
3. 再生（リプレイ）時に集計可能である。

推奨される出力レイヤー：

1. `run_status_[DATE].json`
   - 既存。引き続き完全な監査ログを保持。
2. `reports/[DATE].md`
   - 簡潔な状態遷移サマリーを追加。
3. `decision_packet_[DATE].json`
   - 引き続き構造化された監査ログを保持。

日次レポートに追加推奨されるサマリーフィールド：

1. `Transition`
   - `from -> to`
2. `Reset`
   - `Confirmed / Blocked / N/A`
3. `Duration Lock`
   - `Triggered / Not Triggered`
4. `Core Breakdown`
   - `Yes / No`
5. `Soft Reset`
   - `Applied / Not Applied`

---

## 4. 具体的なタスク

### P0-1 バックテスト指標の接続

修正範囲：

1. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)
2. `market_regime` 関連の統計集計パス。

要件：

1. 過去のウィンドウに基づいて状態機イベントの頻度を統計できること。
2. 旧バージョンの結果と横断的に比較できること。

承認基準：

1. 構造化された指標ファイルを出力すること。
2. 「V1.1 が誤ったリセットを減少させたか」に回答できること。

### P0-2 日次レポートへの Transition Summary の追加

修正範囲：

1. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/report.rs)

要件：

アーカイブ用の Markdown に、以下のような極めてシンプルなブロックを追加します：

```text
Transition
- Lifecycle: ESTABLISHED -> EARLY_CONFIRMATION
- Reset: Blocked
- Duration Lock: No
- Core Breakdown: No
- Soft Reset: Yes
```

承認基準：

1. Telegram の現在の製品化されたスタイルを損なわないこと。
2. アーカイブレポートにおいて状態の変化を迅速に説明できること。

### P1-1 デバッグ出力の接続

目標：

ローカルデバッグや開発モードにおいて、より明確な状態遷移ログを追加します。

要件：

1. CLI の debug/info 出力に遷移サマリーを表示する。
2. Telegram の文言を汚さないこと。

承認基準：

1. 以下を一目で確認できること：
   - なぜ降格したのか。
   - なぜリセットが阻害されたのか。
   - なぜ防御に入ったのか。

### P1-2 バックテスト比較レポート

目標：

V1.0 vs V1.1 の状態機の品質比較を生成します。

推奨される指標：

1. リセット回数
2. ブロックされたリセット回数
3. 状態反転（State Flip）回数
4. 防御的トリガー回数
5. 平均滞在時間

出力：

1. `backtest/state_machine_comparison.md`

---

## 5. 承認基準

### 5.1 機能承認

1. バックテストごとに状態機の品質指標が生成されていること。
2. 日次パイプラインごとにアーカイブレポートで遷移サマリーを確認できること。
3. `run_status`、`decision_packet`、Markdown の3者の遷移に関する記述内容が一致していること。

### 5.2 エンジニアリング承認

1. `cargo test -q`
2. `cargo clippy --all-targets --all-features -- -D warnings`

### 5.3 設計承認

1. レポートにおいて「なぜ今日はリセットされなかったのか」を説明できること。
2. レポートにおいて「なぜ今日降格したのか」を説明できること。
3. レポートにおいて「なぜ今日防御に入ったのか」を説明できること。

---

## 6. 一言要求

V1.2 は状態機をさらに修正することではありません。V1.1 の状態機を「検証可能」「観測可能」「再現可能」なシステムにすることです。
