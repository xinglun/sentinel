---
author: Ray
title: Sentinel 意思決定エンジン重構築ロードマップ
description: Sentinel 意思決定エンジン重構築ロードマップ に関する Sentinel の設計・運用情報。
key: docs-archive-decision-engine-roadmap
---

# Sentinel 意思決定エンジン重構築ロードマップ

## 1. ドキュメントの目的

本ドキュメントでは、Sentinel が「話すダッシュボード」から「意思決定エンジン」へと進化するための最終的なアーキテクチャ、モジュール境界、フェーズごとのタスク、依存関係、および検収基準を定義します。

本ドキュメントは戦略カーネル（コアロジック）に焦点を当てており、Figma や Dashboard などの表示レイヤーの目標は含まれません。最終的な外部出力は引き続き Telegram をメインとしつつ、再生（リプレイ）、アラート、および将来の自動化のために、マシンリーダーブルな JSON 成果物を保持します。

## 2. 北極星指標 (North Star Goals)

Sentinel は最終的に、毎日以下の問いに対して安定して回答できなければなりません：

1. 市場は現在どのような状態にあるか。
2. 現在の状態において、何が許可され、何が禁止されているか。
3. ポートフォリオレベルの目標ポジション、ペース、およびリスク制約は何か。
4. 各資産の現在の執行状態は何か。
5. 市場状態と個別資産状態を組み合わせた後の最終的なアクションは何か。
6. 状態遷移が発生した場合、そのトリガー条件と原因は何か。

最終的なシステムのコア出力は、解釈文案ではなく、確定的な `decision_packet.json` です。Telegram のテキストは、この意思決定パケットのコンシューマー（消費レイヤー）の一つに過ぎません。

## 3. 設計原則

1. 市場状態を先に判定し、その後に個別資産のアクションを決定する。
2. 昇格は遅く、降格は速く。
3. アクションは状態によって決定し、感情によって決定しない。
4. 個別資産の強弱は市場状態に従わなければならない。
5. 予測せず、構造の変化にのみ対応する。
6. Telegram は出力チャネルであり、戦略ロジックの担い手ではない。
7. バックテストは、実盤と全く同じ意思決定パイプラインを再利用しなければならない。

## 4. 最終態アーキテクチャ

```text
Data Providers
  -> Feature Layer
  -> Market Regime State Machine
  -> Portfolio Policy Engine
  -> Asset State Engine
  -> Action Matrix
  -> Decision Packet
  -> Telegram Renderer / JSON Persistence / Backtest Replay / Trading Hooks
```

### 4.1 目標レイヤー

1. `Data Layer`
   市場価格、センチメント、持株、過去の状態履歴を入力。
2. `Feature Layer`
   市場特徴と個別資産特徴を統一的に算出。ここでは意思決定は行わない。
3. `Decision Layer`
   市場状態機、ポートフォリオポリシー、個別資産執行状態、アクションマトリックスを含む。
4. `Delivery Layer`
   Telegram、JSON、遷移ログ（transition log）、バックテスト出力を担当。

### 4.2 市場状態モデル

対外的な最初のバージョンでは、以下の 6 つのメイン状態を採用します：

1. `IGNITION`
2. `NEWBORN`
3. `EARLY_CONFIRMATION`
4. `ESTABLISHED`
5. `CONFIRMED`
6. `DEFENSIVE`

対内（内部実装）では、将来の拡張時にすべてのセマンティクスが一つの列挙型に詰め込まれるのを避けるため、2層表現を推奨します：

1. `lifecycle_state`
   `IGNITION / NEWBORN / EARLY_CONFIRMATION / ESTABLISHED / CONFIRMED`
2. `risk_overlay`
   `NORMAL / DECELERATING / DEFENSIVE / BROKEN`

対外表示時には、内部の2層状態をメイン状態と付加タグにマッピングします。例：

1. `ESTABLISHED + NORMAL -> ESTABLISHED`
2. `ESTABLISHED + DECELERATING -> ESTABLISHED (Decelerating)`
3. `ANY + DEFENSIVE/BROKEN -> DEFENSIVE`

### 4.3 個別資産状態モデル

個別資産の執行状態は、独立したサブ状態機として維持されます：

1. `OPTIMAL`
2. `CRUISE`
3. `PULLBACK`
4. `CAUTION`
5. `OVERHEAT`
6. `DEFEND`
7. `FORMING`

個別資産の状態は最終的なアクションを直接決定しません。必ず市場状態とポートフォリオポリシーの制約を経た後、アクションマトリックスによって執行結果が算出されます。

## 5. コア出力契約

最終的な出力は `decision_packet.json` を正とし、以下の構造を推奨します：

```json
{
  "date": "2026-03-19",
  "market_regime": {
    "market_state": "ESTABLISHED",
    "lifecycle_state": "ESTABLISHED",
    "risk_overlay": "NORMAL",
    "scores": {
      "confidence": 80.58,
      "stability": 30.0,
      "structural": 33.6,
      "maturity": 45.0,
      "flow_acceleration": 0.0
    },
    "transition": {
      "from": "EARLY_CONFIRMATION",
      "to": "ESTABLISHED",
      "changed": true,
      "reasons": [
        "stability crossed 25",
        "maturity crossed 35",
        "confidence remained above 78"
      ]
    }
  },
  "portfolio_policy": {
    "target_exposure_min": 0.60,
    "target_exposure_max": 0.80,
    "allow_chase": false,
    "allow_pullback_buy": true,
    "allow_new_risk": true,
    "risk_assets_mode": "DEFEND"
  },
  "assets": [
    {
      "symbol": "NVDA",
      "asset_state": "OPTIMAL",
      "action": "HOLD",
      "reasons": ["trend intact", "market regime allows hold"]
    }
  ],
  "telegram": {
    "headline": "Market State: ESTABLISHED",
    "summary": "Hold core leaders, buy controlled pullbacks, no chasing."
  }
}
```

JSON がメインの成果物であり、Telegram テキストはこの構造からレンダリングされなければなりません。レンダリング段階で追加の戦略判断を行うことは禁止されます。

## 6. モジュール設計

| モジュール | 責任 | 主要入力 | 主要出力 | 上流依存 |
| --- | --- | --- | --- | --- |
| `src/core/features.rs` | 市場と個別資産の特徴を統一的に抽出 | 価格、センチメント、過去テレメトリ、持株 | `MarketFeatures`, `AssetFeatures` | `data/*`, `ledger` |
| `src/core/market_regime.rs` | 市場状態の識別と遷移 | `MarketFeatures`, 前回の状態履歴 | `MarketRegimeSnapshot` | `features` |
| `src/core/portfolio_policy.rs` | ポートフォリオレベルの戦略制約 | `MarketRegimeSnapshot` | `PortfolioPolicy` | `market_regime` |
| `src/core/asset_state.rs` | 個別資産状態の識別 | `AssetFeatures` | `AssetStateSnapshot` | `features` |
| `src/core/action_matrix.rs` | 市場状態 × 個別資産状態 -> アクション | `MarketRegimeSnapshot`, `PortfolioPolicy`, `AssetStateSnapshot` | `AssetActionDecision` | `market_regime`, `portfolio_policy`, `asset_state` |
| `src/core/decision.rs` | 最終意思決定パケットの集約 | すべての上流結果 | `DecisionPacket` | 全意思決定モジュール |
| `src/core/report.rs` | Telegram と Markdown のレンダリング | `DecisionPacket` | Telegram 文本, Markdown | `decision` |
| `src/core/transition_log.rs` | 遷移ログの永続化 | `DecisionPacket`, 状態履歴 | `transition_log.jsonl`, `state_transitions.csv` | `decision` |
| `src/backtest.rs` | 意思決定の再生と指標評価 | 履歴データ、同一意思決定パイプライン | 回測報告, 遷移行列, 戦略指標 | `decision` |
| `src/cli.rs` | パイプラインのアセンブリとエントリ | 設定, provider, 永続化パス | 執行結果 | 全モジュール |

## 7. モジュール依存関係と実装順序

### 7.1 強依存チェーン

1. `features.rs` はシステム全体のデータ基盤である。
2. `market_regime.rs` は必ず `MarketFeatures` の上に構築されなければならない。
3. `portfolio_policy.rs` は必ず `MarketRegimeSnapshot` の上に構築されなければならない。
4. `asset_state.rs` と `market_regime.rs` は並行開発可能だが、`action_matrix.rs` は両方の完了を待つ必要がある。
5. `decision.rs` はすべての意思決定モジュールが完了した後に組み込まなければならない。
6. `report.rs` と `backtest.rs` は `DecisionPacket` を消費するように変更しなければならず、それぞれが独自の戦略推論を維持し続けることはできない。

### 7.2 既存コードの重構築方向

1. `engine.rs`
   現在の個別資産状態ロジックを段階的に `asset_state.rs` に切り出し、指標計算部分は `features.rs` に保持する。
2. `report.rs`
   現在の `GravityHealth` と `CapitalPosture` のマクロ意思決定ロジックを遷出させ、報告モジュールは意思決定結果の消費のみを担当するようにする。
3. `cli.rs`
   明確な pipeline orchestrator へと変更する。
4. `backtest.rs`
   「状態統計器」から「状態機再生ラボ」へとアップグレードする。

## 8. フェーズ別タスク分解

### Phase 0: 規格凍結

**目標**

状態定義、遷移ルール、アクションマトリックス、および意思決定パケットの契約を凍結する。

**交付物**

1. `docs/specs/STATE_DEFINITIONS.md`
2. `docs/specs/TRANSITION_RULES.md`
3. `docs/specs/ACTION_MATRIX.md`
4. `docs/archive/decision_engine_roadmap.md`
5. `decision_packet` schema 草案

**依存**

なし。

**検収基準**

1. すべての市場状態に、明確な定義、許可アクション、禁止アクション、昇格条件、降格条件があること。
2. すべての個別資産状態が、すべての市場状態において一意のアクションマッピングを持っていること。
3. `decision_packet` のフィールド定義が完全であり、後回しにされた重要な欠落がないこと。

### Phase 1: 特徴層の再構築

**目標**

マクロおよび個別資産の特徴抽出を、報告や状態判断から分離する。

**交付物**

1. `src/core/features.rs`
2. `MarketFeatures`, `AssetFeatures` データ構造
3. 遷移判定および regime age 計算のための過去状態読み込みインターフェース

**依存**

Phase 0。

**検収基準**

1. 同一の入力に対して、特徴計算結果が安定し再現可能であること。
2. Radar と Backtest が全く同じ特徴抽出関数を再利用していること。
3. 主要な特徴（`stability`, `structural`, `maturity`, `flow_acceleration`, `dominance_margin`）に対してユニットテストがカバーされていること。

### Phase 2: 市場状態機

**目標**

確定的な市場状態の識別、昇格、および降格メカニズムを実装する。

**交付物**

1. `src/core/market_regime.rs`
2. `MarketRegimeSnapshot`
3. 状態遷移理由生成器
4. 連続日数確認および迅速な降格ルール

**依存**

Phase 1。

**検収基準**

1. `IGNITION -> NEWBORN -> EARLY_CONFIRMATION -> ESTABLISHED -> CONFIRMED` の昇格パスをサポートしていること。
2. `ANY -> DEFENSIVE` の迅速な降格をサポートしていること。
3. 「閾値を割り込んで2日後に降格」などの連続日数ルールをサポートしていること。
4. 各状態変化に対して構造化された理由リストを出力できること。
5. 昇格、降格、境界値、チャタリング抑制に対してテストがカバーされていること。

### Phase 3: ポートフォリオポリシーエンジン

**目標**

「市場状態」を「ポートフォリオレベルの制約」に変換する。

**交付物**

1. `src/core/portfolio_policy.rs`
2. `PortfolioPolicy`
3. 各状態における exposure（露出）、許可アクション、禁止アクションルール

**依存**

Phase 2。

**検収基準**

1. 各市場状態に対して、目標ポジション区間を出力できること。
2. 各市場状態に対して、`allow_chase`, `allow_pullback_buy`, `allow_new_risk` を明確に定義できること。
3. `DEFENSIVE` 状態において、ポートフォリオ制約がリスク資産のアクションを直接凍結できること。

### Phase 4: 個別資産執行状態とアクションマトリックス

**目標**

個別資産の識別を最終アクションからデカップリングし、標準的なアクションマトリックスを形成する。

**交付物**

1. `src/core/asset_state.rs`
2. `src/core/action_matrix.rs`
3. `AssetStateSnapshot`
4. `AssetActionDecision`

**依存**

Phase 1, Phase 2, Phase 3。

**検収基準**

1. 個別資産状態機が市場状態機からデカップリングされていること。
2. アクションマトリックスがすべての `market_state × asset_state` の組み合わせに対して完備していること。
3. 各資産の最終アクションがアクションマトリックスからのみ取得され、報告レイヤーで追加の書き換えが行われないこと。
4. アクション結果に少なくとも `ACCUMULATE / HOLD / REDUCE / FREEZE / AVOID / OBSERVE` が含まれていること。

### Phase 5: 意思決定パケットと Telegram 出力の重構築

**目標**

`DecisionPacket` を唯一の事実源（SSOT）として、出力パイプラインを書き換える。

**交付物**

1. `src/core/decision.rs`
2. `DecisionPacket`
3. 純粋なレンダリングモジュールとしての `report.rs` の重構築
4. 毎日の `decision_packet.json`
5. 意思決定パケットから直接生成される Telegram テキスト

**依存**

Phase 2, Phase 3, Phase 4。

**検収基準**

1. Telegram 出力が独自の戦略推論を行わず、`DecisionPacket` をレンダリングするだけであること。
2. JSON と Telegram の核となる結論が一致していること。
3. 毎日のメイン出力ファイルに、市場状態、ポートフォリオポリシー、資産アクション、遷移理由が含まれていること。

### Phase 6: 状態永続化と遷移ログ

**目標**

システムに追跡、再生（リプレイ）、および監査能力を持たせる。

**交付物**

1. `src/core/transition_log.rs`
2. `transition_log.jsonl`
3. `state_transitions.csv`
4. 拡張された `telemetry.csv`

**依存**

Phase 5。

**検収基準**

1. どの日付においても、前の状態、現在の状態、遷移理由を遡れること。
2. `telemetry.csv` が将来の遷移リプレイや regime age 計算をサポートしていること。
3. ログ構造が backtest/replay から直接読み込み可能であること。

### Phase 7: バックテストと再生フレームワークの重構築

**目標**

バックテストで「単一ポイントの状態勝率」だけでなく、「状態機の品質」と「アクション制約の効果」を検証できるようにする。

**交付物**

1. `DecisionPacket` パイプラインを再利用するように改造された `backtest.rs`
2. 市場状態遷移行列
3. 状態持続時間の統計
4. 降格レスポンス速度の統計
5. ポートフォリオエクスポージャーとドローダウンの比較指標

**依存**

Phase 5, Phase 6。

**検収基準**

1. Backtest と Radar が同一の意思決定ロジックを使用していること。
2. 状態遷移頻度、平均持続時間、昇格/降格のラグを出力できること。
3. `DEFENSIVE` 発動後のドローダウン抑制効果を測定できること。

### Phase 8: 取引接続と執行ゲート

**目標**

意思決定エンジンの結果を自動取引エージェントに安全に接続する。ただし、Telegram が引き続き主要な対外出力であることを維持する。

**交付物**

1. `DecisionPacket` に接続された `trader_agent`
2. アクションから取引指示へのゲートルール
3. リスク予算、単日予算、状態レベルのサーキットブレーカー

**依存**

Phase 5。

**検収基準**

1. 取引エージェントがポートフォリオポリシーとアクションマトリックスをバイパスできないこと。
2. `DEFENSIVE` 状態において新規のリスクエクスポージャーの追加が禁止されていること。
3. 自動取引をオフにしても Telegram 出力に影響を与えないこと。

## 9. 段階的マイルストーン

| マイルストーン | 意義 | 完了基準 |
| --- | --- | --- |
| M1 | Sentinel が市場状態を定義できる | Phase 0-2 完了 |
| M2 | Sentinel がポートフォリオ制約を定義できる | Phase 3 完了 |
| M3 | Sentinel が統一されたアクションを提示できる | Phase 4 完了 |
| M4 | Sentinel が真の意思決定エンジンになる | Phase 5 完了 |
| M5 | Sentinel が追跡能力を持つ | Phase 6 完了 |
| M6 | Sentinel が検証能力を持つ | Phase 7 完了 |
| M7 | Sentinel が安全に取引連携できる | Phase 8 完了 |

## 10. 全体検収基準

以下の条件がすべて満たされたとき、意思決定エンジンの重構築が完了したとみなされます：

1. 日報のメイン成果物が `decision_packet.json` であり、Telegram はそれによってレンダリングされている。
2. 報告モジュールが戦略判断を一切担っていない。
3. すべての資産の最終アクションがアクションマトリックスによって決定されている。
4. すべての市場遷移に構造化された理由がある。
5. Radar, Backtest, Telegram, Trading が同一の意思決定パイプラインを再利用している。
6. `DEFENSIVE` の降格トリガー速度が昇格速度よりも明らかに速い。
7. 歴史的リプレイによって、任意の取引日の状態、戦略、およびアクションを復元できる。

## 11. 非目標

以下の内容は現在のロードマップの主要な目標ではありません：

1. Figma デザイン案や Dashboard フロントエンド。
2. 複雑な機械学習分類器によるルールベース状態機の代替。
3. 戦略ロジックを検討する前にビジュアルを美化すること。
4. 状態機が未完成の状態で自動取引の複雑さを直接拡張すること。

## 12. 推奨される執行順序

結果を優先して進める場合、実際の開発順序は以下を推奨します：

1. まず Phase 0 を完了し、状態定義、遷移ルール、アクションマトリックス、および意思決定パケットのフィールドを凍結する。
2. 次に Phase 1 と Phase 2 を完了し、システムが市場状態を安定して判断できるようにする。
3. その後、Phase 3 と Phase 4 を完了し、「状態」を真の「アクション制約」に変える。
4. 次に Phase 5 と Phase 6 を完了し、Telegram と永続化を統一された意思決定パケットに接続する。
5. 最後に Phase 7 と Phase 8 を完了し、同一のカーネルを用いてバックテストと取引ゲートを行う。

この順序であれば、Telegram の出力が弱まることなく、「結果の解釈」から「意思決定の表明」へとアップグレードされます。
