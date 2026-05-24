---
author: Ray
title: Sentinel 状態機シグナル整合性タスクリスト
description: Sentinel 状態機シグナル整合性タスクリスト に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-signal-alignment-task
---

# Sentinel 状態機シグナル整合性タスクリスト

## 1. 目的

本タスクリストは、Sentinel の「市場状態」、「表示文言」、「執行アクション」の間に存在するセマンティック（意味論的）なズレを修正するためのものです。

現在のシステムは、以下の機能をすでに備えています：

1. 市場層の `InertiaLayer`（慣性層）。
2. 個別銘柄層の `Relative Strength Memory`。
3. `Promotion Cap`（昇格制限）。
4. `Upgrade / Downgrade Friction`（昇降格摩擦）。

しかし、直近の出力において以下のような核心的な問題が露呈しました：

1. レポート層に `IGNITION + Stability 0 + 複数の OPTIMAL` と表示されている。
2. アクション層が依然として「加筆 (ADD/ACCUMULATE)」を出力している。
3. ユーザーはこれを「有力な候補資産の観察期」と理解すべきであり、「執行可能な加筆期」と理解すべきではない。

したがって、今回のタスクの目標は、選股ロジックのさらなる強化ではなく、以下の整合性（アライメント）を完了させることです：

1. **状態セマンティクスのアライメント**：`Age` / `Stability` の真の意味を全層で一致させる。
2. **表示セマンティクスのアライメント**：レポートにおいて、初期の候補シグナルを過大評価しないようにする。
3. **執行セマンティクスのアライメント**：脆弱な始動期（Fragile Ignition）において、過度に攻撃的なアクション提案を出力しないようにする。

---

## 2. 現状の問題定義

### 2.1 Stability (安定度) の単位不統一

現在、`stability_score` は特徴量層では `0..1` で計算されていますが、一部のロジックや文言では `0..100` として扱われています。

確認された現状：

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs) 内：
   - `stability_score = (stability_structural / 50.0) * trend_maturity`
   - 結果は `0..1`
2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs) 内：
   - `if features.stability_score * 100.0 < 10.0`
   - ロジック内で一時的にパーセント表示に変換して判断している。
3. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs) 内：
   - `"{:.0}"` を用いて `stability_score` を直接プリントしている。
   - その結果、初期段階では頻繁に `0` と表示されてしまう。

結果：

1. ユーザーは `Stability 0` を「全く安定性がない」と誤解する。
2. 実際には、単に「パーセント表示への変換漏れ」である場合が多い。

### 2.2 Regime Age の二重インクリメントのリスク

現在、`regime_age` は状態機内で2回インクリメントされる可能性があります：

1. `transition()` が先に `current_potential_age = prev_age + 1` を計算。
2. `compute_next_state()` がさらにデフォルトで `next_age = regime_age + 1` と計算。

これにより、2つの問題が発生します：

1. ライフサイクル判定において Off-by-one エラーが発生しやすくなる。
2. 外部から見える `Age` が、実際の状態滞在日数と一致しなくなる可能性がある。

結果：

1. 外部から `Age` が止まって見える場合、問題は履歴チェーン（Historical Chain）にある可能性がある。
2. 外部から `Age` が飛び跳ねて見える場合、問題は状態機の内部ロジックにある可能性がある。

いずれの場合も、「システムに投入された時間」に対する信頼性を損ないます。

### 2.3 IGNITION 脆弱期における攻撃的アクションの出力

現在の動作マトリックスでは：

1. `IGNITION + OPTIMAL -> ACCUMULATE`

となっており、以下の矛盾が同時に発生します：

1. 市場層は明らかに脆弱な始動期（Fragile Ignition）にある。
2. 個別銘柄層は、単なる初期の有力候補に過ぎない。
3. それにもかかわらず、レポート層では「加筆エリア / Top Actions」として展示される。

これは、現在の戦略の真の意図に合致しません。

現在の真の戦略意図は以下の通りであるべきです：

1. `IGNITION + 低安定度` の場合、
2. すべての `OPTIMAL` は「有力候補」とみなす。
3. 振る舞いとしては、観察または小規模な追跡のみを許可し、能動的な加筆は許可しない。

### 2.4 レポートにおける「候補 vs 確定」タグの欠如

現在、Telegram やレポート層には以下が表示されていますが：

1. `Confidence`
2. `Stability`
3. `Regime Age`

以下の重要なセマンティクスが明示的に表現されていません：

1. 現在は「候補リストの生成」段階である。
2. 「確定した機会の成立」ではない。

その結果：

1. 文言は控えめに見えるが、
2. 構造的には依然として取引提案（Trade Recommendation）のように見えてしまう。

---

## 3. 修正目標

本フェーズの完了後、システムは以下の振る舞いを満たさなければなりません：

### 3.1 Stability セマンティクスの統一

以下のいずれかを選択し、システム全体で一貫性を保ちます：

1. すべて `0..100` に変更する。
2. すべて `0..1` のまま保持し、展示時にのみ明示的にパーセントに変換する。

承認要件：

1. 状態機のしきい値判断とレポート表示で同一のセマンティクスを使用すること。
2. フォーマットの問題で `Stability 0` と誤報されないこと。

### 3.2 Age の単一インクリメントへの修正

`regime_age` は、必ず1箇所でのみ1回インクリメントされるようにします。

承認要件：

1. 状態遷移が発生しない場合、毎日 `+1` されること。
2. ソフト降格（Soft Downgrade）時はルールに従ってロールバックすること。
3. ハードリセット時のみ `1` に戻ること。
4. 「内部での二重跳び」や「理由のない停止」が発生しないこと。

### 3.3 IGNITION 脆弱期におけるアクション降格

以下の明示的な執行ルールを追加します：

1. `IGNITION && Stability < しきい値` の場合、
2. `ACCUMULATE` を禁止する。
3. `OBSERVE` または `HOLD` のみを許可する。

推奨されるしきい値：

1. 現在のリセット / 脆弱性（Fragility）セマンティクスと一致させる。
2. 当面は `stability < 10`（百分制）を使用。

このルールは、文言による弱体化だけでなく、製品としてのセマンティクスとして優先的に表現されるべきです。

### 3.4 レポート出力への「候補」タグの追加

脆弱な始動期において、レポートは個別銘柄の強いシグナルを「確定」ではなく「候補」としてマークしなければなりません。

推奨される出力方法（少なくとも1つを実装）：

1. Top Actions の理由欄に `有力候補、継続性の確認待ち` を追記。
2. Signals エリアに統一された診断メッセージを追記。
3. Tactical Summary において「加筆エリア」を「候補エリア」に変更。

承認要件：

1. ユーザーが一目で「候補」と「確定」を区別できること。
2. 「Fragile + 加筆提案」という認知的な不整合を解消すること。

---

## 4. 開発タスクの分解

### P0-1 Stability 量綱（スケール）の統一

対象ファイル：

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)
2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
3. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
4. [telemetry.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/telemetry.rs)

任務要件：

1. `stability_score` の内部単位を明確に定義する。
2. しきい値判定、レポート表示、telemetry 出力を統一する。
3. ラベル関数と数値範囲の対応関係を修正する。

推奨される補完事項：

1. `stability_score` に単位を明記したコメントを追加する。

### P0-2 Regime Age 推進ロジックの修正

対象ファイル：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
3. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)

任務要件：

1. `prev_age` が状態機に入った後の唯一のインクリメント・ポイントを確認し、確定させる。
2. 二重インクリメントを排除する。
3. 昇格しきい値の判定と、最終的な保存値が一致することを保証する。
4. 必要に応じて、以下の監査用フィールドを追加して説明性を高める：
   - `evaluated_age`
   - `next_age`

### P0-3 IGNITION 脆弱期における執行ゲートの追加

対象ファイル：

1. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
2. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
3. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

任務要件：

1. `IGNITION + Fragile` 条件下で、元の `ACCUMULATE` をオーバーライド（上書き）する。
2. `OPTIMAL` の振る舞いを以下に降格させる：
   - `OBSERVE`
   - または `HOLD`
3. 理由欄に以下を明示的に記載する：
   - `Candidate only`
   - `Execution suppressed in fragile ignition`

注意：

1. これは製品戦略レイヤーにおける明示的な制約です。
2. 「Execution Disabled」実行モードのみに頼って問題を隠蔽してはなりません。

### P1-1 報告層への「有力候補」診断メッセージの追加

対象ファイル：

1. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
2. [report_ui_tests.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report_ui_tests.rs)

任務要件：

1. `IGNITION + Fragile` 時に、統一された定型文を追加する。
2. 文言には以下を明記する：
   - 現在は買い点の確定ではない。
   - 現在は主要銘柄の選別期である。
3. 成熟段階の文言には影響を与えないようにする。

### P1-2 履歴チェーンの調査とテストによる保護

対象ファイル：

1. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)
2. [pipeline_integration.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/tests/pipeline_integration.rs)
3. 必要に応じて追加：
   - `backtest` 関連
   - telemetry 関連テスト

任務要件：

1. 履歴パケットが時系列順にロードされ、age/memory の連続計算に使用されていることを確認する。
2. 2日連続、3日連続といったシナリオに対して統合テストを追加する。
3. 以下を保証する：
   - `Age` が連続してインクリメントされること。
   - `Stability` が連続して変化すること。
   - 候補資産が脆弱期に直接加筆アクションをトリガーしないこと。

---

## 5. 承認基準

本タスク完了後、以下のシナリオが成立しなければなりません：

### A. 脆弱な始動（Fragile Ignition）の初日

入力特徴：

1. 市場が `IGNITION` に突入。
2. `Stability` が非常に低い。
3. 複数の資産が `OPTIMAL` 状態。

期待される出力：

1. レポートに「候補観察期」であることが明示される。
2. Top Actions が積極的な加筆として表現されない。
3. `OPTIMAL` 資産は「有力候補」としてのみ表示される。

### B. 脆弱な始動が2〜3日連続した場合

入力特徴：

1. 候補資産リストが継続して存在。
2. `Age` が毎日インクリメントされる。
3. `Stability` は徐々に向上しているが、しきい値を超えていない。

期待される出力：

1. `Age` が連続してインクリメントされ、日のスキップやフリーズが発生しない。
2. アクションは引き続き控えめに維持される。
3. レポートで「継続性の確認待ち」が強調される。

### C. 始動期から確定期への移行

入力特徴：

1. `IGNITION -> NEWBORN` またはそれ以上の段階へ。
2. `Stability` がしきい値を通過。
3. 有力候補が継続して保持されている。

期待される出力：

1. 個別銘柄の通常のアクション・マッピング（`ACCUMULATE` / `HOLD`）が回復する。
2. レポートの文言が「候補」から「確定」へ切り替わる。
3. 成熟段階の既存ロジックに影響を与えない。

### D. 通常の降格とハード・リセット

期待される出力：

1. 通常の降格では `Age` が `1` にリセットされない。
2. reset gate を通過した場合のみ、ハードリセットが許可される。
3. レポートと telemetry 内の `Age`、`Stability` の値に一貫性があり、説明可能であること。

---

## 6. テストリスト

少なくとも以下のテストを新規追加または修正してください：

1. `stability_score` の単位の一貫性テスト。
2. `report` 展示数値と内部数値の一貫性テスト。
3. `regime_age` の正常なインクリメント・テスト。
4. `regime_age` のソフト・ロールバック・テスト。
5. `regime_age` のハード・リセット・テスト。
6. `IGNITION + fragile + OPTIMAL` において `ACCUMULATE` が出力されないことの確認。
7. `IGNITION + fragile` におけるレポートの「有力候補」ヒント出力の確認。
8. `NEWBORN` 以降の段階における通常のアクション・マッピングの回復確認。

推奨される配置場所：

1. [pipeline_integration.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/tests/pipeline_integration.rs)
2. [report_ui_tests.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report_ui_tests.rs)
3. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
4. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)

---

## 7. 非目標 (Out of Scope)

本フェーズでは以下の事項は扱いません：

1. 銘柄母集団（Universe）の変更。
2. Relative Strength Memory のスコアリング公式の書き換え。
3. `OPTIMAL / CRUISE / PULLBACK` の資産層分類基準の再定義。
4. 実際の取引執行ロジックの最適化。
5. Telegram インフラや外部通知チャネルの改造。

本フェーズの重点はただ一つです：

**「状態機、レポート、アクションの三者に、同じ言葉を喋らせる」** こと。

---

## 8. 推奨される実施順序

開発は以下の順序でコミットを提出することを推奨します：

1. P0-1 `Stability` スケールの統一。
2. P0-2 `Age` インクリメントの修正。
3. P0-3 `IGNITION Fragile` 時のアクションゲートの実装。
4. P1-1 レポートの候補タグ実装。
5. P1-2 統合テストと履歴チェーンの保護。

数値セマンティクスの修正と製品の振る舞いの修正を同じコミットに混ぜないよう、各項目を個別に提出することを推奨します。

---

## 9. 完了定義

以下の条件がすべて満たされたとき、本タスクは完了とみなされます：

1. 開発が上記の P0 項目をすべて完了している。
2. 関連するテストがすべてパスしている。
3. 新しいレポートにおいて、以下の組み合わせが発生しなくなる：
   - `Fragile + 複数の OPTIMAL + 加筆提案`
4. ユーザーが以下の2つを安定して区別できるようになる：
   - 有力な候補（Candidate）
   - 確定した強者（Confirmed）
5. `Age` と `Stability` が、数日間にわたる連続した出力において説明可能かつ追跡可能である。
