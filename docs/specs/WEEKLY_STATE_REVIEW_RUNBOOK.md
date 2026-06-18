---
author: Ray
title: 週間状態レビュー・ランブック
description: 週間レビューと audit_daily 監査運用の標準手順を定義するランブック。
key: weekly-state-review-runbook
---

# 週間状態レビュー・ランブック (Weekly State Review Runbook)

## 1. 目的

本ドキュメントは、`V1.3` 観察フェーズにおける標準的な週間復盤（レビュー）プロセスを定義します。

目標は、戦略的な結論を自動生成することではなく、復盤を以下の2つのレイヤーに分けることです：

1. **CI による自動処理**: 原始データの出力と週間集計ドラフトの生成。
2. **人間による判断**: 異常値の解釈と、次週以降のアクションの決定。

一言で言えば：
**CI が統計を担当し、人間が判断を担当します。**

---

## 2. 復盤のタイミング

現在のワークフローのスケジュール：

1. `daily_radar.yml`
   - 月曜日〜金曜日 `23:30 JST`

2. `weekly_backtest.yml`
   - 土曜日 `01:00 JST`

推奨されるレビュー時間：

1. **毎週土曜日 `09:00 - 10:00 JST`**
2. より保守的に行う場合は、**毎週土曜日 `10:00 JST` 以降** を推奨します。

理由：
- 金曜日の最終デイリーデータがすでに保存されている。
- 土曜日の週間バックテストが通常完了している。
- `data` ブランチ内の週間成果物の同期が概ね完了している。

---

## 3. データソース

### 3.1 標準ソース

標準的な週間レビューでは、`data` ブランチ内の成果物を直接使用します。

確認すべきファイル：

#### デイリー観察
1. `reports/run_status_[DATE].json`
2. `reports/decision_packet_[DATE].json`
3. `reports/[DATE].md`

#### 週間集計
1. `reports/weekly_state_metrics.json`
2. `reports/weekly_state_review_auto.md`
3. `backtest/state_machine_metrics_latest.json`
4. `backtest/state_machine_metrics_latest.md`
5. `backtest/summary_latest.md`

### 3.2 重要な原則

`data` ブランチは「結果」を保持するブランチであり、「実行」を行うブランチではありません。

したがって：
1. `data` ブランチ上で `cargo run -- review` を**実行しないでください**。
2. `data` ブランチでは、生成済みの結果ファイルを読み取るだけに留めてください。

---

## 4. 2つの操作スキーム

### 4.1 スキーム A：標準週間レビュー

これがデフォルトのスキームです。

**適用シーン：**
- 定期的な週間レビューを行う場合。
- CI が正常に動作している場合。
- `data` ブランチに最新の成果物が存在する場合。

**コマンドの実行：**
- **不要です。**

**操作手順：**
1. 今週の `daily_radar` と `weekly_backtest` の完了を待ちます。
2. `data` ブランチ内の以下のファイルを開きます：
   - `reports/weekly_state_metrics.json`
   - `reports/weekly_state_review_auto.md`
   - `backtest/state_machine_metrics_latest.md`
3. これらのファイルに基づき、手動でレビューを記入します。

### 4.2 スキーム B：コードブランチでの手動再計算

これはデバッグ用のスキームであり、標準的なレビュー手順ではありません。

**適用シーン：**
- CI が `weekly_state_metrics.json` を出力しなかった場合。
- `review` 集計ロジックを検証する必要がある場合。
- ローカル環境で週間の集計を再計算したい場合。

**実行の前提：**
- コードのワークスペース内であること。
- `main` / `develop` または機能ブランチのワークスペースであること。
- ワークスペース内に以下が存在すること：
  - `Cargo.toml`
  - `src/`
  - `reports/`

**コマンド：**
```bash
cargo run -- review
```

**実行結果：**
1. `reports/weekly_state_metrics.json` が生成されます。
2. `reports/weekly_state_review_auto.md` が生成されます。

**注意：**
- これは `data` ブランチで実行するものではありません。
- コード側のデバッグや補完手段として使用してください。

### 4.3 スキーム C：日次監査サマリー（audit_daily）

これは**分析エンジン**ではなく、`state_transitions.jsonl` に対する**監査サマライザ**です。  
新しい売買シグナルを生成せず、既存の遷移ログを監査可能な形で要約します。

**回答する固定問い（5項目）:**
1. なぜ本日も `NO TRADE`（または `READY`）なのか。
2. 昨日比で構造変化があったか（市場状態 / risk overlay / 主線状態）。
3. breakout は新規・継続・消失のどれか。
4. `NO TRADE` は連続セグメントか、途中で reset しているか。
5. gate を最も阻害している条件は何か（Top 3）。

`NO TRADE` 時は監査レイヤーを以下で解釈します：
1. `初級（シグナルなし）`
2. `偵察（シグナル未検証）`

偵察中は、`rules.market_state_engine.scout_abort_days` 日以内に breakout が 2 銘柄以上へ拡散しない場合、偵察は自動 reset されます。

**コマンド（コードワークスペース）:**
```bash
cargo run -- audit_daily
```

期間指定:
```bash
cargo run -- audit_daily --days 30
```

日付指定（YYYY-MM-DD）:
```bash
cargo run -- audit_daily --date 2026-04-22 --days 30
```

エイリアス:
```bash
cargo run -- transition_audit_summary --days 30
```

**運用上の注意:**
1. 目的は「判断の追加」ではなく「判断の監査」です。
2. まず既存成果物の読み取りを優先し、必要時のみ実行します。
3. P/L 連動、スコアリング、自動評価の拡張はこのフェーズでは行いません。
4. 連続セグメントは**ログ連続ベース**で計算し、週末は自動連結します。
5. `--date` / `--days` が不正な場合、デフォルトへ黙ってフォールバックせずエラー終了します。

**多言語出力（`output.language`）:**
1. `zh-cn`
2. `en-us`
3. `ja-jp`

```toml
[output]
language = "en-us"
```

`weekly_state_review_auto.md` も `output.language` に従って固定 label と境界文を出力する。`weekly_state_metrics.json` の key は machine-readable contract として英語のまま維持し、表示言語の切替対象にしない。

**先頭セクションの出力差分例:**

```text
# zh-cn
# Audit Daily (2026-04-22)
1. Gate 摘要
```

```text
# en-us
# Audit Daily (2026-04-22)
1. Gate Summary
```

```text
# ja-jp
# Audit Daily (2026-04-22)
1. Gate サマリー
```

---

## 5. 毎週の標準ステップ

### 5.1 ステップ 1：CI 完了の確認

今週の以下のワークフローがすべて成功していることを確認してください：
1. `daily_radar.yml`
2. `weekly_backtest.yml`

いずれかが失敗している場合：
1. まず CI の失敗原因を解決してください。
2. 正式なレビューには進まないでください。

### 5.2 ステップ 2：`data` ブランチの成果物確認

少なくとも以下のファイルが存在することを確認してください：
1. `reports/weekly_state_metrics.json`
2. `reports/weekly_state_review_auto.md`
3. `backtest/state_machine_metrics_latest.md`

欠落している場合：
1. ワークフローの失敗を調査してください。
2. 必要に応じて、開発者がコードブランチで `cargo run -- review` を実行して補完してください。

### 5.3 ステップ 3：自動集計の確認

以下の順序で確認します：
1. `reports/weekly_state_metrics.json`
2. `reports/weekly_state_review_auto.md`
3. `backtest/state_machine_metrics_latest.md`

読み方の推奨順序：
1. まず `weekly_totals` (週間合計) を見ます。
2. 次に `daily_summaries` (日次要約) を見ます。
3. 自動ドラフト内の「異常日」を確認します。
4. `weekly_state_metrics.json -> latest_context` で、直近の戦略・マクロ・認知校正コンテキストを確認します。

`latest_context` は以下の読み取り専用スナップショットを保持します：
1. `trend_breadth_mode`
2. `market_cycle_position`
3. `holding_efficiency`
4. `macro_gravity`
5. `strategic_context`
6. `cognitive_calibration`

`weekly_state_metrics.json` は、状態機械の週次監査用に以下も保持します：
1. `weekly_totals`
2. `daily_summaries`

`weekly_state_review_auto.md` には以下の 3 セクションが追加されます：
1. `State Machine Weekly Totals`
2. `Daily State Machine Timeline`
3. `Strategic Context Snapshot`
4. `Macro Gravity Snapshot`
5. `Cognitive Calibration Snapshot`

これらは状態理解の補助であり、スコア、推奨、売買判断を生成しません。

`daily-calibration` は日中または手動確認用の表示 report であり、全文を日次 artifact として保存することは標準ではありません。長期校正は `weekly_state_metrics.json` と `weekly_state_review_auto.md` の週次粒度で確認します。

### 5.4 ステップ 4：手動レビューファイルの生成

テンプレートファイルを直接編集しないでください。

**テンプレート原典:**
- [weekly_state_review.md](../templates/weekly_state_review.md)

毎週、実際の日付を付けた新しいレビューファイルを作成することを推奨します：
- 例：`reports/weekly_state_review_2026-03-21.md`

---

## 6. 何を記入すべきか

### 6.1 Weekly Totals (週間合計)
`reports/weekly_state_metrics.json -> weekly_totals` から以下を記入します：
- `reset_confirmed_total`
- `reset_blocked_total`
- `soft_reset_total`
- `duration_lock_total`
- `defensive_override_total`
- `core_breakdown_total`
- `reconciliation_mismatch_total`

### 6.2 Daily Timeline (日次タイムライン)
`daily_summaries` から各営業日について記入します：
- `from_state -> to_state`
- `reset_confirmed / reset_blocked`
- `soft_reset_applied`
- `duration_locked`
- `defensive_override`
- `reconciliation_mismatch_count`

### 6.3 主要な観察と異常 (Key Observations & Anomalies)
ここが手動記入の重点です。少なくとも以下に回答してください：
1. 今週、異常なリセット（Reset）は発生したか？
2. 大量の `blocked reset`（ブロックされたリセット）は発生したか？
3. 状態の「揺れ（ジッタ）」が多すぎないか？
4. 防御的オーバーライド（Defensive Override）の頻度は適切か？
5. 照合の不一致（Reconciliation Mismatch）が異常に蓄積していないか？

### 6.4 ロジックへのフィードバック
重点的な判断項目：
1. 資産の回復が速すぎないか？
2. 資産の回復が遅すぎないか？
3. 過度な防御になっていないか？
4. 「リセットすべきなのにリセットされなかった」ケースはないか？
5. 「リセットすべきでないのにリセットされた」ケースはないか？

### 6.5 推奨される調整
この項目は毎週必須ではありません。
`2〜4` 週間継続して観察した後に、`V1.4` でのパラメータ収束を行うかどうかを決定します。

---

## 6.6 毎週チェックすべき 5 つの問い

レビューが際限なく発散するのを防ぐため、手動レビューでは以下の 5 つの問いに回答することを基本とします。

1. **「内部的には整合しているが、実態と乖離している」ケースはあったか？**
   - 重点確認：市場状態の説明はメッセージとして筋が通っているが、資産の順位が歴史的な直感に反していないか？Telegram の文章は流暢だが、現実の市場構造とかけ離れていないか？

2. **「継続的な強気銘柄」が短期的な要因で不当に除外されていないか？**
   - 重点確認：`NVDA / GOOG / SPY` のような継続的に強い資産が、単日の押し目（Pullback）だけでコア領域から脱落していないか？直接 `OBSERVE` に降格されていないか？

3. **「歴史的な弱気銘柄」が短期的な見栄えの良さだけで急速に浮上していないか？**
   - 重点確認：かつて長期的に弱かった銘柄が、1〜2日で `OPTIMAL` に急浮上していないか？「見栄えが少し綺麗になっただけ」で長期的な強気銘柄を追い抜いていないか？

4. **`Top Actions` は安定したか？（ただし、硬直していないか？）**
   - 確認：頻繁に入れ替わらなくなったか？一方で、明らかに交代が必要な局面でも同じ銘柄が居座り続けていないか？新しい機会が長期的に入ってこられなくなっていないか？

5. **現在の問題は「過敏」か、それとも「鈍感」か？**
   - **過敏:** リセットが多すぎる、強気銘柄がすぐ脱落する、文案が頻繁に「試行」に戻る。
   - **鈍感:** 交代すべき銘柄が交代しない、明らかに弱体化した銘柄が高い評価を維持している、`Top Actions` が硬直している。

---

## 6.7 最終的な結論（一言）

推奨されるフォーマット：
- 「今週は新たな『乖離ケース』は見られず、システムは安定している。継続観察。」
- 「強気銘柄の保護は正常だが、弱気銘柄の浮上が依然として速い。継続観察。」
- 「Top Actions の安定性は明らかに向上したが、軽微な硬直の兆候がある。もう一週間観察が必要。」

---

## 7. 自動化の境界

### 7.1 自動化済み
- 直近 7 日間の `run_status` スキャン。
- `weekly_totals` の集計。
- `daily_summaries` の生成。
- `weekly_state_metrics.json` の生成。
- `weekly_state_review_auto.md` の生成。
- 成果物の `data` ブランチへの同期。

### 7.2 手動（人間）の領域
- 異常日に対する業務上の解釈。
- システムが過敏か鈍感かの判断。
- `V1.4` への移行判断。
- パラメータ収束の要否。

### 7.3 明確に自動化しない範囲
- パラメータ調整の自動提案。
- 「過敏」等の主観的判断。
- 最終的な業務結論の執筆。

---

## 8. 標準的なレビューの運用基準

1. **デフォルトでは「スキーム A」を使用します。**
2. **原則として `cargo run -- review` を手動で実行しません。**
3. **原則として `data` ブランチの結果を直接読み取ります。**

スキーム B を使用するのは以下の場合のみです：
- CI が成果物を出力しなかった場合。
- 集計ロジック自体を検証・修正する場合。
- ローカルで補完計算が必要な場合。

---

## 9. 開発者への要件

V1.3 観察フェーズにおいて開発者が担保すべき事項：
1. `daily_radar.yml` と `weekly_backtest.yml` が正常に `data` ブランチへデータをプッシュすること。
2. `cargo run -- review` がコードブランチのワークスペースで利用可能であること。
3. 自動ドラフトと JSON の集計結果が一致していること。

**やってはいけないこと：**
1. 状態マシンのロジックを独断で変更し続けること。
2. 戦略的な結論を自動生成しようとすること。
3. 人間の判断を無理に CI に組み込もうとすること。

---

## 10. まとめ

標準的な週間レビューフローは以下の通りです：
1. **CI** がデータとドラフトを自動出力する。
2. **あなた（人間）** が毎週土曜日の午前中に `data` ブランチの結果を確認する。
3. **あなた（人間）** が異常の解釈と次週の判断を手動で記入する。

デフォルトでは、**`cargo run -- review` を手動で実行する必要はありません。**
