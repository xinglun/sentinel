# 🐕 Stock Sentinel (Capital Physics Engine)

## 目的
Stock Sentinelは、市場の変動を「物理的な観測」として捉え、感情に左右されない資本配分判断（DCA、防御、買い増し）を支援するための意思決定支援レーダーです。

## 読むタイミング
- システムの核心概念（飼い主-リード-犬モデル）と物理学的アプローチを理解したい時
- セットアップ方法や日常の運用・検証コマンドを確認したい時
- 資本状態（CAPITAL STATE）の読み方を知りたい時

---

## 🛰️ V1.2.1：Capital Dynamics Observatory (观测纪元)
本システムは単なるテクニカル指標の集合体ではなく、市場のエネルギー状態を測定し、長期的な量化研究を可能にする「资本动力学观测站」へと進化しました。現在は**観測紀元（Observation Epoch）**に入っており、データの整合性和蓄積を最優先しています。

- **CAPITAL STATE（资本姿态）:** ポートフォリオ全体の「趨勢主導」か「回帰主導」かを自動判定し、最適な配分戦略を提示。
- **5つの物理および序参量指標:**
    - **重力強度 (Strength):** 資本の推進力（移動平均の傾き）の測定。
    - **势能 Z-Score:** 統計的な歪みの正規化。
    - **曲率 (Curvature):** 加速・減速によるトレンド反転の早期検知。
    - **信心度 (Confidence):** 物理指標のベクトル一致度。
    - **统治优势差 (Dominance Margin):** 序参量（Order Parameter）。体制の安定度を記述。
- **三位一体（Ternary）重力モデル:** UP/FLAT/DOWN を明確に分離し、市場幅（Breadth）の真実を記録。
- **Telemetry V3 (19-Column Schema):** 毎日の読数を 19 列の完全な状態ベクトルとして `telemetry_v3.csv` に自動記録。
- **Parameter Universe Isolation:** `config.toml` のハッシュ値を記録することで、パラメータ変更履歴とデータを完全に整合。

## 🚀 使用方法 (Usage)

### 1. 準備
- Rust & Cargo (Edition 2021) がインストールされていること。
- `config.toml` を開き、自身のウォッチリストと Telegram 通知（`bot_token`, `chat_id`）を設定します。

### 2. 日常の観測 (Daily Radar)
毎日の終値確定後、以下のコマンドで現在の「資本の天気」を確認します。
```bash
cargo run --release
```
- ターミナルにカラーテーブルが出力されます。
- `./reports` に JSON, Markdown, および `telemetry.csv` が生成されます。
- Telegram にデイリーレポートがプッシュ通知されます。

### 3. 歴史的検証 (Backtest Mode)
過去のデータを用いて、システムの「目盛り（Calibration）」と「アルファ分離」を検証します。
```bash
cargo run --release -- backtest
```
- `./backtest/summary.md` に以下の詳細レポートが出力されます。
    - **Calibration Error:** 確率予測の正確性。
    - **Alpha Separation:** トレンド対回帰の期待値の差。
    - **Transition Matrix:** 状態遷移の確率統計。

## 📁 ドキュメント (Documentation)
- [要件と機能の定義 (PRD)](./docs/PRD.md) - システムの鉄則と核心要件
- [システムアーキテクチャ設計](./docs/architecture_design.md) - 内部構造とデータフロー
- [戦略設計哲学と評価](./docs/strategy_philosophy.md) - 「飼い主-犬」モデルと掃参（Sweep）プロトコル

---

## Author

Ray
