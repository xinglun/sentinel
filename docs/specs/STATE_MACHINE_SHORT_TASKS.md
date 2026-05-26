---
author: Ray
title: Sentinel 状態機改造ショートタスクリスト
description: Sentinel 状態機改造ショートタスクリスト に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-short-tasks
---

# Sentinel 状態機改造ショートタスクリスト

## P0 (最優先)

### P0-1 InertiaLayer (慣性層) の実装

目標：

1. 生のシグナルと状態判定の間に「慣性層」を追加する。
2. `reset gate`、`downgrade gate`、`duration lock`、`defensive override` を集中実装する。

修正範囲：

1. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/market_regime.rs)
2. 関連する設定のロードパス。

承認基準：

1. 防御的でない通常の押し目において、直接 `IGNITION` へリセットされないこと。
2. `ANY -> DEFENSIVE` は引き続き最高優先度を維持すること。

### P0-2 Reset Gate (リセット・ゲート) の固め

目標：

以下のすべての条件を満たした場合にのみ、`IGNITION` へのリセットを許可する：

1. `TrendDominant == false`
2. `Structural < 25`
3. `Stability < 10` が3日間連続。
4. `Flow <= 0`
5. `CoreAssetsBreakdown == true`

承認基準：

1. `ESTABLISHED -> IGNITION` が稀であり、かつ説明可能なイベントであること。
2. `market_regime.reasons` にリセットまたはリセット・ブロックの理由が記録されること。

### P0-3 段階的降格 (Step Downgrade)

目標：

通常のライフサイクル降格は最大1段階に制限する：

1. `CONFIRMED -> ESTABLISHED`
2. `ESTABLISHED -> EARLY_CONFIRMATION`
3. `EARLY_CONFIRMATION -> NEWBORN`

禁止事項：

1. `ESTABLISHED -> IGNITION`
2. `CONFIRMED -> NEWBORN`

例外：

1. `ANY -> DEFENSIVE`

承認基準：

1. 通常の押し目では、1段階ずつの降格が行われること。
2. 構造的破壊時には、依然として直接 `DEFENSIVE` に突入できること。

## P1

### P1-1 Duration Lock (期間ロック)

目標：

1. 昇格とリセットをタイムロック（時間による保護）で保護する。
2. 防御的な降格は阻害しない。

推奨される実装：

1. `min_upgrade_duration`
2. `soft_downgrade_lock`
3. `hard_defensive_override`

承認基準：

1. 「今日リセットして明日復帰する」といったチャタリング（ジッタ）が発生しないこと。
2. チャタリング防止が、リスクに対する鈍感さに繋がらないこと。

### P1-2 Core Assets (コア資産) の構成化

目標：

1. `core_assets` をコード内の定数から設定ファイル（Config）へ引き上げる。
2. `CoreAssetsBreakdown` の判定を、シンボルのハードコードではなく設定に依存させる。

承認基準：

1. `core_assets` が設定ファイルで定義可能であること。
2. `reset gate` が設定されたコア資産集合を使用していること。

### P1-3 State Transition Log (状態遷移ログ)

目標：

状態の変化と、それを阻害（ブロック）した理由を構造化して記録する。

最小限のフィールド：

1. `from` (遷移前)
2. `to` (遷移後)
3. `blocked_reset` (リセットがブロックされたか)
4. `core_assets_breakdown` (コア資産の崩壊フラグ)
5. `reasons` (理由)

承認基準：

1. 「なぜ降格しなかったのか / なぜリセットされなかったのか / なぜ防御に入ったのか」を特定できること。

## P2

### P2-1 個別銘柄の復帰しきい値

目標：

個別銘柄の復帰パスを強制する：

`DEFEND -> CAUTION -> CRUISE -> OPTIMAL`

承認基準：

1. `DEFEND -> OPTIMAL` のような飛び級を許可しないこと。
2. 各ステップでクールダウン期間を経過すること。

### P2-2 Historical Penalty (歴史的ペナルティ)

目標：

直近20日以内に `DEFEND` 状態だった資産：

1. デフォルトの `max_state = CRUISE` とする。
2. 追加の構造確認を満たした後にのみ、制限を解除する。

承認基準：

1. 弱い資産が単日で「潔白」になれないこと。

### P2-3 FORMING (形成中) 資産の保護

目標：

`FORMING` 資産が、市場のリセットや市場環境の改善によって自動的に強気状態へ引き上げられないようにする。

承認基準：

1. `FORMING` は、個別の構造成熟条件を満たした後にのみ、通常の状態機に突入すること。

## デリバリー（実施）順序

1. `P0-1`
2. `P0-2`
3. `P0-3`
4. `P1-1`
5. `P1-2`
6. `P1-3`
7. `P2-1`
8. `P2-2`
9. `P2-3`
