---
author: Ray
title: Sentinel 状態機慣性強化仕様 (STATE_MACHINE_INERTIA_HARDENING.md)
description: Sentinel 状態機慣性強化仕様 (STATE_MACHINE_INERTIA_HARDENING.md) に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-inertia-hardening
---

# Sentinel 状態機慣性強化仕様 (STATE_MACHINE_INERTIA_HARDENING.md)

## 1. 目的

本仕様書は、以下の問題を修正するために策定されました：

1. 市場状態（Market Regime）が、通常の押し目において過剰に `IGNITION` へリセットされてしまう。
2. `stability` / `regime_age` が、トレンド崩壊以外のシナリオでもゼロクリアされ、トレンドのライフサイクルが歪んでしまう。
3. 個別銘柄の状態（Asset State）に歴史的慣性が欠けており、弱い資産が短期的な反発だけで誤って `OPTIMAL` に復帰してしまう。
4. Telegram やレポート層において、「市場の再起動」と「個別銘柄の強気継続/急転換」が共存するというロジックの不整合が発生している。

本仕様は、既存の [TRANSITION_RULES.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/TRANSITION_RULES.md) および [STATE_DEFINITIONS.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/STATE_DEFINITIONS.md) を補完し、強化するものです。

## 2. 開発用修正スペック

### 2.1 コア目標

状態機は以下の振る舞い制約を満たさなければなりません：

1. トレンドは減衰することはあっても、一夜にしてゼロになることはない。
2. `IGNITION` は「新トレンドの初期始動」のみを意味し、通常の押し目におけるデフォルトの降格先として使用してはならない。
3. `ESTABLISHED` / `EARLY_CONFIRMATION` からの調整は、原則として「再起動 (Restart)」ではなく「降格 (Downgrade)」として表現する。
4. 個別銘柄の状態には復帰しきい値を設ける。過去に弱かった資産が、単日の局所的な改善で直接 `OPTIMAL` に復帰することを禁止する。

### 2.2 実装必須の修正事項

1. 市場状態機への `reset gate`（リセット・ゲート）の追加。
   - 厳格なリセット条件を満たさない限り、`IGNITION` への復帰を禁止する。
2. ライフサイクルへの「段階的降格」ルールの導入。
   - デフォルトでは1段階ずつの降格のみを許可し、一足飛びのゼロリセットを禁止する。
3. 個別銘柄状態への「復帰パス (Recovery Path)」の導入。
   - `DEFEND -> OPTIMAL` のような復帰は、複数ステップに分解しなければならない。
4. 直近の弱気資産に対する「歴史的ペナルティ」の導入。
   - 最近 `DEFEND` / `CAUTION` だった資産は、追加の確認ウィンドウを通過しなければならない。
5. `stability` / `age` のリセットに対する保護。
   - ハードリセット・シナリオ以外では、`regime_age` が `1` に戻ってはならない。

### 2.3 コード配置の推奨

1. 市場状態機：
   - [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
2. 個別銘柄状態とアクションの連動：
   - `asset_state` 関連モジュール
   - [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
3. 報告層の診断出力：
   - [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
   - `market_regime.reasons` に以下の理由を明記すること：
     - なぜ降格したのか
     - なぜリセットされなかったのか（慣性によるブロック）
     - なぜ個別銘柄の復帰が阻止されたのか

### 2.4 承認基準

以下のシナリオがテストをパスしなければなりません：

1. `ESTABLISHED` 状態が、中程度の押し目において `IGNITION` ではなく `EARLY_CONFIRMATION` へ降格すること。
2. `EARLY_CONFIRMATION` が、押し目において `IGNITION` ではなく `NEWBORN` へ降格すること。
3. 厳格なリセット条件を満たした場合のみ、ライフサイクルが `IGNITION` に戻ること。
4. 直近20取引日以内に `DEFEND` が発生した資産は、単日の改善で直接 `OPTIMAL` になってはならない。
5. `DEFEND -> CAUTION -> CRUISE -> OPTIMAL` の復帰パスがユニットテストでカバーされていること。

---

## 3. 具体的な状態遷移ルール

### 3.1 市場状態の昇格ルール

既存の昇格パスを継続使用します：

1. `NONE -> IGNITION`
2. `IGNITION -> NEWBORN`
3. `NEWBORN -> EARLY_CONFIRMATION`
4. `EARLY_CONFIRMATION -> ESTABLISHED`
5. `ESTABLISHED -> CONFIRMED`

昇格ロジックは引き続き「遅い確認（Slow Confirmation）」の原則に従います。これは今回の変更の重点ではありません。

### 3.2 市場状態の降格ルール

デフォルトで段階的降格を採用します：

1. `CONFIRMED -> ESTABLISHED`
2. `ESTABLISHED -> EARLY_CONFIRMATION`
3. `EARLY_CONFIRMATION -> NEWBORN`
4. `NEWBORN -> IGNITION`
5. `ANY -> DEFENSIVE` （ハードリスク・トリガー時のみ）

### 3.3 `DEFENSIVE` への直接突入が許可される条件

以下の条件では、高速脱出の優先順位を維持します：

1. `system_confidence < 50`
2. コア資産群が揃って `DEFEND / CAUTION` に転落
3. `risk_overlay` が `DEFENSIVE / BROKEN` に達した
4. 構造的な破壊が明確であり、通常の押し目ではないと判断される場合

### 3.4 Reset Gate：`IGNITION` への復帰を許可する厳格な条件

`EARLY_CONFIRMATION / ESTABLISHED / CONFIRMED` から `IGNITION` に戻るには、以下のすべてを同時に満たさなければなりません：

1. `TrendDominant == false` または同等の主トレンド判定が失効。
2. `stability_structural < 25`
3. `stability_score < 10` が `3` 日間継続。
4. `flow_acceleration <= 0`
5. コア資産群がもはや上昇構造を維持していない。

いずれかの条件を満たさない場合：

1. リセットを禁止する。
2. 段階的降格のみを執行する。

### 3.5 Age / Stability 保護ルール

1. `reset gate` を通過した場合のみ、`regime_age` を `1` にリセットすることを許可する。
2. 通常の降格時：
   - `regime_age` の継続を許可する。
   - または、ルールに従った「ソフトな後退（Soft Rollback）」を行うが、ゼロにはしない。
3. `stability_score` は、ライフサイクルの降格のみを理由にゼロクリアしてはならない。ただし、以下の場合を除く：
   - `DEFENSIVE` に突入した場合。
   - または、`reset gate` を通過した場合。

### 3.6 推奨される診断タグ

Telegram や監査レポートでの説明を容易にするため、`market_regime.reasons` に標準化されたタグを追加することを推奨します：

1. `DowngradeOnly`
2. `ResetBlockedByInertia`
3. `ResetConfirmed`
4. `CoreStructureStillIntact`
5. `StructuralBreakConfirmed`

---

## 4. 個別銘柄の復帰しきい値ルール

### 4.1 復帰パス (Recovery Path)

以下の「一足飛びの復帰」を禁止します：

1. `DEFEND -> OPTIMAL`
2. `DEFEND -> PULLBACK`
3. `CAUTION -> OPTIMAL`

推奨される復帰パス：

1. `DEFEND -> CAUTION`
2. `CAUTION -> CRUISE`
3. `CRUISE -> PULLBACK / OPTIMAL`

### 4.2 DEFEND 復帰しきい値

`DEFEND -> CAUTION` には、少なくとも以下が必要です：

1. 長期周期の破壊条件が解除されていること。
2. 主要移動平均線の傾きがそれ以上悪化していないこと。
3. `N=3` 日間連続して、再度 `DEFEND` がトリガーされていないこと。

### 4.3 CAUTION 復帰しきい値

`CAUTION -> CRUISE` には、少なくとも以下が必要です：

1. コア引力帯（Gravity Band）への再突入。
2. ボラティリティの収束。
3. `N=3~5` 日間連続した構造の安定。

### 4.4 CRUISE から強気状態への復帰しきい値

`CRUISE -> PULLBACK / OPTIMAL` には、少なくとも以下が必要です：

1. トレンドの傾きが再びプラスに転じていること。
2. Owner/Leash 構造の整合性が回復していること。
3. 直近の `DEFEND` による未解除のペナルティが存在しないこと。

### 4.5 歴史的ペナルティルール

資産が直近 `20` 取引日以内に `DEFEND` 状態だった場合：

1. デフォルトの上限を `CAUTION / CRUISE` にロックする。
2. 追加の復帰ウィンドウを満たした場合のみ、`PULLBACK / OPTIMAL` への突入を許可する。
3. 推奨される追加復帰ウィンドウ：
   - 5日間連続した構造の安定。
   - 長期周期の傾きの回復。
   - 新たな支持線割れイベントが発生していないこと。

### 4.6 FORMING (形成中) 資産の制限

`FORMING` 資産は、市場状態のリセットによって自動的に強気状態へ引き上げられてはなりません。

1. `FORMING` は `FORMING / OBSERVE` を維持しなければならない。
2. 個別に構造の成熟条件を満たした後にのみ、通常の個別銘柄状態機への突入を許可する。

### 4.7 レポート層の一貫性要件

以下の出力は、同一の個別銘柄バケット（Bucket）結果を共有しなければなりません：

1. `Top Actions`
2. `戦術分区 (Tactical Summary)`
3. `リスクと機会`

ある資産が以下のようにマークされている場合：

1. `DEFEND / CAUTION`
   - 同時に「機会（Opportunity）」や「加筆エリア（ADD）」に入ってはならない。
2. `OPTIMAL / PULLBACK`
   - 同一のメッセージ内で「防御エリア（DEFEND）」に分類されてはならない。

---

## 5. テストリスト

以下のテストを新規追加または強化することを推奨します：

1. `ESTABLISHED` からの調整が直接 `IGNITION` にリセットされないこと。
2. `EARLY_CONFIRMATION` からの調整が `NEWBORN` への降格に留まること。
3. すべての `reset gate` 条件を満たした場合にのみ `IGNITION` を許可すること。
4. `DEFEND` 資産が単日の反発で `OPTIMAL` に到達できないこと。
5. `DEFEND` 資産が数日間の修復期間を経て、パス通りに復帰できること。
6. Telegram 出力において、`Top Actions`、`戦術分区`、`リスクと機会` が同一のバケットを使用していること。

---

## 6. 一言ルール (The Golden Rule)

トレンドは減衰しても、一夜にして消え去ることはない。  
状態は降格しても、容易にゼロにリセットされることはない。  
弱い資産は修復できても、瞬時に「潔白」になることはない。
