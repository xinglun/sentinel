---
author: Ray
title: Sentinel 状態機 V1.1 最適化プラン
description: Sentinel 状態機 V1.1 最適化プラン に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-v1-1-optimization
---

# Sentinel 状態機 V1.1 最適化プラン

## 1. 目的

V1.0 において、以下の機能が完了しました：

1. `reset gate` (リセット・ゲート)
2. `single-step downgrade` (単一ステップ降格)
3. `duration lock` (期間ロック)
4. `soft reset` (ソフトリセット)
5. `asset recovery ladder` (個別資産復帰ラダー)
6. `transition_audit` (遷移監査)

V1.1 の目標は、さらに指標を増やすことではなく、以下の抽象概念をより堅牢に実装することです：

1. `TrendDominant == false` を、単一の代理指標に依存させないようにする。
2. `CoreAssetsBreakdown` を、単純なヒューリスティックから「複合判定」にアップグレードする。
3. 状態機の「説明層」と「判定層」のデカップリングをさらに進める。
4. 監査ログとレポートにおいて、「なぜリセットされなかったのか / なぜ阻害されたのか」について、より安定した構造化出力を提供する。

---

## 2. 本フェーズの最適化範囲

### 2.1 核心的な方向性

本フェーズでは以下の3点のみを実施します：

1. `TrendDominant` 複合判定の実装。
2. `CoreAssetsBreakdown` 判定のアップグレード。
3. 状態遷移監査（Transition Audit）出力の拡張。

### 2.2 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. 新たなテクニカル指標の追加。
2. 状態機の問題を「隠蔽」するための戦略パラメータの調整。
3. Telegram のスタイルの変更。
4. 執行層のリスク管理ロジックの変更。

---

## 3. V1.1 コア設計

### 3.1 TrendDominant 複合判定

現状の実装：

1. `check_reset_gate()` において、`dominance_margin <= 0.0` を `TrendDominant == false` の代理指標として使用。

V1.1 の目標：

単一のしきい値による代理ではなく、明示的な `trend_dominant` 複合判定関数を導入します。

推奨定義：

```text
trend_dominant = (
    dominance_margin > 0
    AND up_weight >= down_weight
    AND system_confidence >= trend_dominant_min_confidence
)
```

オプションの強化案：

1. `up_count / down_count` によるブレ幅（Breadth）制約の導入。
2. `flow_acceleration` による方向制約の導入。

要件：

1. `TrendDominant` は、明示的なフィールドまたは明示的な関数の結果であること。
2. `reset gate` は、もはや単一の代理値を直接消費してはならない。

### 3.2 CoreAssetsBreakdown 複合判定

現状の実装：

1. `TrendStatus::Down` である。
2. または、`deviation < -5.0` である。
3. かつ、損壊しているコア資産の数が 50% を超えている。

V1.1 の目標：

`CoreAssetsBreakdown` を、設定可能な複合判定にアップグレードします。

推奨定義：

```text
core_assets_breakdown = (
    count_assets_below_threshold(core_assets) >= breakdown_k
    OR avg_core_deviation <= breakdown_avg_deviation
    OR core_breadth <= breakdown_breadth_floor
)
```

推奨される設定項目：

1. `breakdown_k` (損壊資産数しきい値)
2. `breakdown_avg_deviation` (コア平均乖離しきい値)
3. `breakdown_breadth_floor` (コア騰落幅フロア)

要件：

1. `core_assets` は引き続き設定（Config）から取得すること。
2. 崩壊（Breakdown）しきい値も設定から取得すること。
3. デフォルト値を内蔵することは可能だが、コード内にハードコードするだけにしてはならない。

### 3.3 Transition Audit の拡張

現在の `transition_audit` に含まれる項目：

1. `from`
2. `to`
3. `is_reset_blocked`
4. `is_downgrade_clamped`
5. `core_breakdown`
6. `duration_locked`

V1.1 における拡張推奨項目：

1. `trend_dominant`
2. `reset_gate_passed`
3. `indicator_cap`
4. `soft_reset_applied`
5. `defensive_override`

要件：

1. 監査フィールドは、構造化データを優先すること。
2. `reasons`（理由）は引き続き保持するが、唯一の「真実のソース」として扱わないこと。

---

## 4. 具体的な実装案

### 4.1 ファイル配置

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)
   - `trend_dominant` の追加。
   - `core_assets_breakdown` のアップグレード。

2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
   - `check_reset_gate()` を、明示的な `trend_dominant` を消費するように変更。
   - `transition_audit` の拡張。

3. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs)
   - `core_assets_breakdown` のしきい値設定の追加。
   - `trend_dominant` 判定のしきい値設定の追加。

4. [IMPLEMENTATION_WALKTHROUGH.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/architecture/IMPLEMENTATION_WALKTHROUGH.md)
   - V1.1 の監査および判定セマンティクスに合わせて同期更新。

### 4.2 推奨設定構造

`rules` の下に以下を追加することを推奨します：

```toml
[rules.inertia]
min_state_duration = 3
trend_dominant_min_confidence = 55.0
core_breakdown_k = 2
core_breakdown_avg_deviation = -5.0
core_breakdown_breadth_floor = 0.0
```

説明：

1. キーの名前が完全に一致している必要はありません。
2. ただし、設定によって将来の実験やバックテストが可能でなければなりません。

---

## 5. 承認基準

### 5.1 TrendDominant

以下を満たさなければなりません：

1. `TrendDominant` が、もはや単一の代理値によって直接表現されていないこと。
2. `reset gate` が、明示的な `trend_dominant` の結果を使用していること。

### 5.2 CoreAssetsBreakdown

以下を満たさなければなりません：

1. `CoreAssetsBreakdown` が、設定されたしきい値に依存していること。
2. もはや「半分以上の資産が Down である」ことだけに依存していないこと。

### 5.3 Transition Audit

以下を満たさなければなりません：

1. 以下の理由を構造化されたデータで説明できること：
   - なぜリセットされなかったのか。
   - なぜ duration lock によって阻害されたのか。
   - なぜ `DEFENSIVE` に突入したのか。

### 5.4 ベースライン

以下にパスしなければなりません：

1. `cargo test -q`
2. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 6. 一言要求

V1.1 は、ルールをさらに拡張することではありません。`TrendDominant` と `CoreAssetsBreakdown` を「代理ヒューリスティック」から「明示的で設定可能な複合判定」へとアップグレードすることです。
