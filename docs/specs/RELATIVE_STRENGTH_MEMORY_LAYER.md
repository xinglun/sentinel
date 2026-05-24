---
author: Ray
title: 相対強度記憶レイヤー (Relative Strength Memory Layer)
description: 相対強度記憶レイヤー (Relative Strength Memory Layer) に関する Sentinel の設計・運用情報。
key: docs-specs-relative-strength-memory-layer
---

# 相対強度記憶レイヤー (Relative Strength Memory Layer)

## 1. モジュール設計目標

本モジュールの目標は、市場状態機（Market State Machine）を再定義することではなく、資産層におけるランキングと状態昇格の「短視眼化（近視眼的な判断）」という問題を修正することにあります。

現在のシステムには以下の要素がすでに備わっています：

1. `Market InertiaLayer` (市場慣性レイヤー)
2. `Reset Gate` (リセットゲート)
3. `Downgrade Gate` (降格ゲート)
4. `Duration Lock` (継続期間ロック)

しかし、資産層には依然として明白な欠落（ギャップ）が存在します：

1. 横断面（クロスセクション）のスナップショットを過重視している。
2. 「短期的な見た目の綺麗さ」を「持続的な強さ」と誤認しやすい。
3. 「持続的な強者の短期的な変動」を状態の劣化と誤認しやすい。

本モジュールが解決すべき核心的な問題は以下の通りです：

1. 強者が短期的な変動によってコア領域から追い出されるのを防ぐ。
2. 弱者が短期的な見た目の改善によって急速に上位に食い込むのを防ぐ。
3. 資産状態のランキングを「単日の構造スコア」から「現在の構造 ＋ 時間的連続性」の複合判断へとアップグレードする。

一言で言えば：

**Relative Strength Memory Layer は、持続的な強者を保護し、短期的な偽の回復（Fake Recovery）を抑制するために使用されます。**

---

## 2. モジュール境界

本モジュールは資産層の時間的連続性のみを担当し、市場層のレジーム（Regime）判定は担当しません。

### 2.1 担当範囲

1. 資産の相対強度の短・中期的な記憶。
2. 資産 Top Tier（トップ層）の状態保護。
3. 弱い資産の昇格における時間コストと上限の制約。
4. 資産ランキング時の時間的連続性による補正。

### 2.2 担当外範囲

1. `MarketState` の判定。
2. `Reset Gate` の処理。
3. `DEFENSIVE` リスクのカバー。
4. Telegram の文言生成。
5. 取引アクションの直接的な決定。

### 2.3 階層位置

以下の配置を推奨します：

1. `RawSignalLayer` (生シグナルレイヤー)
2. `RelativeStrengthMemoryLayer` (本レイヤー)
3. `AssetState Decision` (資産状態決定)
4. `ActionMatrix` (アクションマトリックス)

つまり、これは資産状態機の「入力強化レイヤー」であり、アクションレイヤーではありません。

---

## 3. 最小ルールセット

V1 バージョンでは最小限のルールのみを実装し、複雑な因子は導入しません。

### 3.1 Top Tier Lock (トップ層ロック)

ある資産が過去 `10` 取引日において：

1. 少なくとも `6` 日間、強度ランキング上位 `3` 位以内に入っている場合

その場合：

1. `min_state = CRUISE`

意味：

1. 持続的な強者は、一度の短期的な押し目（プルバック）によって直接 `OBSERVE` まで落ちることはありません。
2. これは一種の「最低状態保護」です。

目標：

1. `NVDA / GOOG / SPY` のような持続的な強資産を保護すること。

---

### 3.2 Weak Asset Promotion Cap (弱資産昇格上限)

ある資産が過去 `20` 取引日内において、以下のいずれかの条件を満たす場合：

1. `DEFEND` に入ったことがある。
2. 大部分の日で `CRUISE` 未満であった。

その場合：

1. デフォルトで `max_state = CRUISE`

追加の解除条件を満たさない限り、`OPTIMAL` への昇格は許可されません。

目標：

1. 長期的な弱資産が、1〜2日の「横断面での見た目の良さ」だけで直接上位に食い込むのを防ぐこと。

---

### 3.3 Promotion Unlock Conditions (昇格解除条件)

`Promotion Cap` の制限を受けている資産について、制限を解除するには少なくとも以下の条件を満たす必要があります：

1. 長周期の構造の回復。
2. ボラティリティの収束。
3. 連続 `N` 日間の状態の安定。

V1 では、複雑な新規特徴量は導入せず、既存の資産回復条件を再利用します。

---

### 3.4 Rolling Strength Memory (ローリング強度記憶)

資産の最終ランキングは、もはや「現在の構造スコア」だけでは決まりません。

推奨される最小実装：

```text
memory_adjusted_score =
    current_structure_score * 0.7
  + rolling_strength_rank_score * 0.3
```

ここで：

1. `current_structure_score`
   - 既存の資産状態入力を引き続き使用します。
2. `rolling_strength_rank_score`
   - 過去 5〜10 日間の相対強度順位から変換された値です。

V1 の目標は完璧を追求することではなく、ランキングに時間的連続性を持たせることにあります。

---

### 3.5 No Instant Redemption (即時復権の禁止)

以下のパターンを禁止します：

1. `DEFEND -> OPTIMAL`
2. `OBSERVE -> OPTIMAL`
3. `FORMING -> OPTIMAL`

既存の資産回復ステップを通過し、かつ memory layer の解除条件を満たす必要があります。

目標：

1. 「実体のない（スカスカな）銘柄」や「回復したばかりの資産」が直接コア領域に入るのを防ぐこと。

---

## 4. データ構造案

### 4.1 AssetStrengthMemory

新規追加を推奨：

```rust
pub struct AssetStrengthMemory {
    pub top3_days_last_10: u8,
    pub top5_days_last_10: u8,
    pub defend_days_last_20: u8,
    pub below_cruise_days_last_20: u8,
    pub rolling_strength_rank: f64,
    pub top_tier_locked: bool,
    pub promotion_capped: bool,
}
```

説明：

1. `top3_days_last_10`
   - 過去10日間で Top 3 に入った回数。
2. `top5_days_last_10`
   - 補助フィールドとして将来の拡張に使用可能。
3. `defend_days_last_20`
   - 弱資産へのペナルティ判定に使用。
4. `below_cruise_days_last_20`
   - 長期的な弱い構造の判断に使用。
5. `rolling_strength_rank`
   - ローリング相対強度スコア。
6. `top_tier_locked`
   - Top Tier Lock がトリガーされているか。
7. `promotion_capped`
   - 弱資産昇格上限がトリガーされているか。

---

### 4.2 AssetStrengthDecision

資産状態機の入力前に生成される中間結果：

```rust
pub struct AssetStrengthDecision {
    pub symbol: String,
    pub raw_score: f64,
    pub memory_score: f64,
    pub adjusted_score: f64,
    pub min_state: Option<AssetState>,
    pub max_state: Option<AssetState>,
    pub reasons: Vec<String>,
}
```

用途：

1. 最終的なランキングに使用。
2. 状態の上下限クリッピング（切り詰め）に使用。
3. レポートおよびデバッグ用の説明に使用。

---

### 4.3 接続位置の推奨

このレイヤーを `ActionMatrix` に詰め込まないようにしてください。

より合理的な配置場所：

1. `features / asset feature aggregation`
2. `asset_state.rs` の直前
3. または `asset_state.rs` 内部の独立したヘルパーとして

推奨インターフェースの方向性：

1. `compute_asset_strength_memory(...)`
2. `apply_strength_memory(...)`
3. `clamp_asset_state_by_memory(...)`

---

## 5. インターフェース案

### 5.1 Memory の計算

```rust
pub fn compute_asset_strength_memory(
    symbol: &str,
    history: &[HistoricalAssetSnapshot],
) -> AssetStrengthMemory
```

入力：

1. symbol
2. 直近 10〜20 日間の資産履歴スナップショット

出力：

1. 当該資産の相対強度記憶構造体

---

### 5.2 Memory Decision の生成

```rust
pub fn build_asset_strength_decision(
    current: &AssetFeatures,
    memory: &AssetStrengthMemory,
) -> AssetStrengthDecision
```

出力：

1. 現在の構造スコア
2. memory 調整スコア
3. 状態の上下限
4. 理由リスト

---

### 5.3 状態のクリッピング (Clamp)

```rust
pub fn clamp_asset_state_with_memory(
    proposed: AssetState,
    decision: &AssetStrengthDecision,
) -> AssetState
```

ロジック：

1. `min_state` にヒットした場合、その状態を下回ることを禁止する。
2. `max_state` にヒットした場合、その状態を上回ることを禁止する。

---

### 5.4 ランキング強化

```rust
pub fn rank_assets_with_memory(
    assets: &[AssetFeatures],
    decisions: &HashMap<String, AssetStrengthDecision>,
) -> Vec<RankedAsset>
```

ロジック：

1. ランキング時に `adjusted_score` を使用する。
2. もはや現在の横断面だけに依存しない。

---

## 6. 開発への実施説明

今回、`MarketRegime` は変更しないでください。
Telegram の構造も動かさないでください。
ましてや、パラメータを調整して資産層のランキング問題を隠蔽しようとしないでください。

### 6.1 実施順序

1. まず `AssetStrengthMemory` データ構造を実装する。
2. 次に `compute_asset_strength_memory()` を実装する。
3. 次に `AssetStrengthDecision` を実装する。
4. 最後に、資産状態のクリッピングとランキングに接続する。

### 6.2 今回の最小納品物

以下の完了が必須です：

1. `Top Tier Lock`
2. `Weak Asset Promotion Cap`
3. `rolling_strength_rank` 最小版
4. 状態の上下限クリッピング
5. 構造化された理由出力

今回行わないこと：

1. 新しい因子の追加。
2. 複雑な機械学習スコアリングの追加。
3. アクションマトリックスの変更。
4. Telegram の大構造の変更。

### 6.3 完了基準

少なくとも以下のテストを補完してください：

1. 持続的な強者が、単日の変動で直接 `OBSERVE` まで落ちないこと。
2. 過去20日間で劣勢だった資産が、直接 `OPTIMAL` まで昇格しないこと。
3. `FORMING`（形成中）の資産が、短期的な見た目の良さだけで直接上位に食い込まないこと。
4. ランキングが純粋な横断面から「現在 ＋ 時間的連続性」へと変化していること。
5. 理由ログにおいて以下が明確に確認できること：
   - `top_tier_locked`
   - `promotion_capped`
   - `memory_adjusted_score`

### 6.4 開発への最終メッセージ

本タスクは市場状態機のさらなる修正ではありません。
資産層に「相対強度記憶」を補完し、強者が短期的な変動で追い出されるのを防ぎ、また弱者が短期的な見た目の改善だけで上位に入るのを防ぐためのものです。
