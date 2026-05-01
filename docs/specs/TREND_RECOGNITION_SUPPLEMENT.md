---
author: Ray
title: トレンド認識補強設計
description: NO TRADE/READY の意思決定層を維持したまま、トレンド認識能力を補強するための追加設計。
key: trend-recognition-supplement
---

# トレンド認識補強設計

## 目的

本設計は、既存の `NO TRADE / READY` 判定規律を維持しつつ、トレンド形成過程の認識精度を引き上げることを目的とする。  
表示改善のみではなく、シグナル層の識別能力を強化する。

## 設計方針

1. 意思決定層は不変とする。  
`NO TRADE / READY` は引き続き Gate のみで決定する。
2. 識別能力の拡張は Signal Layer に限定する。  
Evidence Layer は説明専用とし、意思決定に逆流させない。
3. 人間判断は実行系へ注入しない。  
外部レビューは可能だが、ランタイム入力には使用しない。

## 層構造

### 1) Decision Layer（既存）

- 役割: 取引許可判定のみ
- 変更: なし

### 2) Signal Layer（本設計の主対象）

- 役割: トレンド形成の識別能力強化
- 変更: 新規特徴量と状態遷移ロジックを追加

### 3) Evidence Layer（既存拡張）

- 役割: 状態遷移証拠の説明
- 変更: 新規識別結果の表示を追加（説明専用）

## 追加シグナル仕様

### diffusion_score（加重拡散スコア）

単純な銘柄数判定を廃止し、品質加重へ置換する。

- 構成例:
  - `leader_confirm_weight * leader_confirmation`
  - `follower_confirm_weight * confirmed_followers`
  - `sector_breadth_weight * sector_breadth`
- 目的:
  - 弱い追随 1 件を過大評価しない
  - 主導銘柄の質と波及の質を区別する

### leader_follow_lag_state（先行・遅行状態）

先行銘柄と追随銘柄の時間差を状態として扱う。

- 新規状態:
  - `LEADER_CONFIRMED_FOLLOWERS_LAGGING`
- 目的:
  - 「主導は成立、追随は遅延」という実市場パターンをノイズ扱いしない
- 制約:
  - この状態でも Gate 未達なら `NO TRADE` を維持

### single_asset_decay（単銘柄減衰）

単銘柄 breakout が N 日以内に拡散しない場合、信号強度を段階的に減衰させる。

- 減衰後の終端:
  - しきい値未満で reset
- 目的:
  - 単発イベントを延命させない

### event_follow_through（イベント後持続性）

決算等イベント日だけでなく、`T+1..T+5` の持続品質を計測する。

- 指標例:
  - 継続リターン
  - 出来高維持
  - 押し目耐性
  - 同セクター共振
- 目的:
  - 一日限りの反応をトレンド成立と誤認しない

## 新規内部状態

`TrendContinuationState` を新設する。

- `NONE`
- `EARLY_LEADER`
- `LEADER_CONFIRMED_FOLLOWERS_LAGGING`
- `BROADENING`
- `MATURE`

用途は監査・説明のみとし、取引許可ロジックへ直接入力しない。

## ハード制約

1. `TrendContinuationState` が `MATURE` でも Gate FAIL の場合は必ず `NO TRADE`。
2. `diffusion_progress = 無（単一資産）` の場合、「トレンド形成済み」表現を禁止。
3. Evidence は Decision に逆流させない（read-only one-way）。

## 出力拡張

### Telegram（状態転移証拠）

以下を追加表示する。

- `トレンド段階`
- `拡散スコア`
- `先行・遅行状態`
- `単銘柄減衰（day k / N）`

### audit_daily

固定テンプレートを追加する。

- `トレンド認識品質: {state}; 拡散スコア {x.xx}; 遅行状態 {lag_state}`

## 実装タスク

1. `transition_log` に識別補強構造体を追加する。  
含む項目: スコア、段階、遅行状態、減衰カウンタ。
2. `engine` のシグナル算出に新規ロジックを実装する。  
既存の実行判断ロジックは変更しない。
3. `presentation_assembler` で Evidence 表示へマッピングする。  
Decision 要約へは反映しない。
4. `report` および `cli audit_daily` に三言語表示を追加する。
5. テストを追加する。

## テスト要件

1. 先行成立・追随遅延ケース  
期待: 遷移状態は過渡を示すが、Gate 未達で `NO TRADE` 維持。
2. 単銘柄未拡散ケース  
期待: 日次減衰後に reset。
3. 三言語スナップショット  
期待: `zh/en/ja` の全文一致を契約化。
4. 契約テスト  
期待: Evidence 強化時でも Gate FAIL なら取引許可は不変。

## 受け入れ条件

1. 意思決定挙動に回帰がないこと。
2. 追加表示が説明専用であること。
3. 品質ゲートを満たすこと。

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```
