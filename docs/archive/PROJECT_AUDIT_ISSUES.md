---
author: Ray
---

# プロジェクト監査イシュー (Project Audit Issues)

本ドキュメントは、監査結果を実行可能なイシューチケットに変換したものです。

各イシューは、タスク追跡に直接使用できるように記述されています。

## P0

### P0-1: エンドツーエンドのパイプライン統合テストの追加

**目標**

意思決定パイプライン全体をデグレード（先祖返り）から保護する。

**背景**

プロジェクトにはユニットテストは存在しますが、`Engine::run_daily_pipeline()` が依然として一貫した `DecisionPacket` を生成することを証明する統合テストがありません。

**範囲**

1. 以下の項目を網羅する統合スタイルのテストを追加する：
   - 特徴量抽出 (Feature extraction)
   - 市場レジーム遷移 (Market regime transition)
   - ポートフォリオポリシー派生 (Portfolio policy derivation)
   - 資産状態の計算 (Asset-state computation)
   - アクションマトリックス出力 (Action-matrix output)
2. ライブプロバイダーの代わりに、決定論的なフィクスチャ履歴を使用する。
3. 中間状態だけでなく、最終的な `DecisionPacket` のフィールドを検証する。

**推奨されるファイル**

1. `src/core/engine.rs` 内の新しいテストモジュール、または `tests/pipeline_integration.rs`
2. 必要に応じて `tests/fixtures/` 下の再利用可能なフィクスチャ

**検収基準**

1. 少なくとも 1 つの強気パス（Bullish-path）の統合テストが存在すること。
2. 少なくとも 1 つの防御パス（Defensive-path）の統合テストが存在すること。
3. テストが、最終的な `DecisionPacket.market_regime`、`portfolio_policy`、および選択された `assets[*].action` を検証していること。

**依存関係**

なし

### P0-2: アーカイブパッケージの統合テストの追加

**目標**

実行に成功した際、期待される日次アーカイブパッケージが生成されることを保証する。

**背景**

日次アーカイブは現在、副作用ではなく製品要件となっています。

**範囲**

1. `save_to` でのドライランアーカイブ出力をカバーするテストを追加する。
2. 必要なファイルが作成されることを検証する：
   - `decision_history.jsonl`
   - `decision_packet_[DATE].json`
   - `state_transitions.csv`
   - `state_transitions.jsonl`
   - `execution_gate_log.jsonl`
   - `portfolio_snapshot_[DATE].json`
   - `account_snapshot_[DATE].json`
   - `data_quality_log.jsonl`
   - `[DATE].md`
3. 必須の資産に対して書き込み失敗がエラーとして伝播することを検証する。

**推奨されるファイル**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs) 付近の新しいテストモジュール
2. または `tests/archival_integration.rs`

**検収基準**

1. ドライランパスが完全な日次アーカイブパッケージを作成すること。
2. 書き込み権限の欠如や無効な出力パスが、テスト可能な失敗を引き起こすこと。
3. 必要なアーカイブ資産がサイレントにスキップされないこと。

**依存関係**

P0-1 があると役立つが、必須ではない。

### P0-3: ExecutionGate 境界マトリイステストの追加

**目標**

リスクゲートをエッジケース下でも信頼できるものにする。

**背景**

`ExecutionGate` は現在、以下の項目の厳格なコントロールポイントとなっています：

1. 日次予算 (daily budget)
2. 総エクスポージャー (total exposure)
3. 購買力 (buying power)
4. リスクオーバーレイ (risk overlay)

これを明示的にテストする必要があります。

**範囲**

1. 以下のテストを追加する：
   - 中立的な条件下でのパス
   - `max_daily_budget` によるブロック
   - `global_budget` によるブロック
   - `buying_power` によるブロック
   - 防御的/故障レジーム下での買い側取引のブロック
   - `config_multiplier` とポリシー倍率の正しい処理
2. 以下の両方を検証する：
   - `ExecutionResult.trades`
   - `ExecutionResult.audits`

**推奨されるファイル**

1. [execution_gate.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/execution_gate.rs) 内の新しいテスト

**検収基準**

1. すべてのブロック理由が個別にテストされていること。
2. 少なくとも 1 つのテストで、複数の候補取引が予算を争うケースを検証していること。
3. 取引数だけでなく、監査（Audit）ペイロードが表明（Assert）されていること。

**依存関係**

なし

### P0-4: メインパイプラインからの安全でないランタイム前提の削除

**目標**

隠れたランタイム前提を明示的なバリデーションに置き換える。

**背景**

現在のパイプラインは、取引設定を依然としてハードにアンラップ（unwrap）しています：

[cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):233

これは、実行前にバリデーションが強制されている場合にのみ許容されます。現在、その前提は暗黙的なものです。

**範囲**

1. ランタイムパスから `unwrap()` を削除する。
2. 起動時に明示的な設定バリデーションを追加する。
3. 以下のケースに対して型付けされたエラー、または少なくともコンテキストのあるエラーを返す：
   - `[trading]` セクションの欠落
   - 不正なプロバイダー/取引の組み合わせ
   - 必要な設定がない実行モード

**推奨されるファイル**

1. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs)
2. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)

**検収基準**

1. 取引設定の欠落によるパニックパスが残っていないこと。
2. 無効な設定が読み取り可能なエラーとともに早期に失敗すること。
3. テストが無効な設定ケースをカバーしていること。

**依存関係**

なし

### P0-5: クリーンな Clippy ベースラインの強制

**目標**

「コンパイルが通る」から「保守可能でクリーン」な状態へ移行する。

**背景**

`cargo check` はパスしていますが、`cargo clippy --all-targets --all-features` は依然として回避可能な問題を報告しています。

**範囲**

1. 本質的な品質シグナルである現在の `clippy` の指摘事項を修正する：
   - identity map
   - `clone_on_copy`
   - manual clamp
   - `lines().filter_map(Result::ok)`
   - bool assert comparison
   - フォーマット関連のクリーンアップ
2. ドメイン言語を損なう場合、頭字語（Acronym）の Lint を満たすためだけにビジネス Enum の名前を変更しない。
3. ドメイン名が意図的である場合に限り、対象を絞った `#[allow(...)]` を追加する。

**推奨されるファイル**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)
3. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)
4. [portfolio_policy.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/portfolio_policy.rs)
5. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)

**検収基準**

1. `cargo clippy --all-targets --all-features` がパスすること。
2. 残っている Lint の Allow リストが意図的であり、文書化されていること。

**依存関係**

なし

## P1

### P1-1: cli.rs のスリム化

**目標**

オーケストレーションのホットスポットを削減する。

**背景**

`cli.rs` が依然として多くの責務を持ちすぎています：

1. 設定のルーティング
2. プロバイダーの選択
3. データ取得のファンアウト
4. アーカイブのオーケストレーション
5. 実行コンテキストの組み立て
6. レポートの配信

**範囲**

1. アーカイブのオーケストレーションを専用のサービス/モジュールに抽出する。
2. 実行コンテキストの組み立てを専用のヘルパーまたはモジュールに抽出する。
3. `cli` を以下の項目に集中させる：
   - コマンドのルーティング
   - トップレベルの実行モードのディスパッチ

**推奨されるファイル**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. `src/core/runtime_pipeline.rs` や `src/core/archive_service.rs` などの新しいモジュール

**検収基準**

1. `cli.rs` が実質的に小さくなること。
2. メインパイプラインの責務が名前付きのユニットに分割されること。
3. 動作が変わらないこと。

**依存関係**

P0-1 および P0-2 が先に完了していること。

### P1-2: ActionMatrix API のリファクタリング

**目標**

アクションの派生を拡張しやすく、保守しやすくする。

**背景**

`ActionMatrix::decide()` が現在、あまりにも多くのプリミティブな引数を取っています。

**範囲**

1. 現在の引数リストを、型付けされた入力構造体に置き換える。
2. 市場コンテキスト、資産コンテキスト、および実行設定を個別にグループ化する。
3. 現在の振る舞いを維持する。

**推奨されるファイル**

1. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
2. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)

**検収基準**

1. `ActionMatrix::decide()` が長いプリミティブ引数リストを取らなくなること。
2. 既存の振る舞いがテストでカバーされたままであること。
3. 新しい API により、将来のマトリックス拡張が容易になること。

**依存関係**

P0-1 および P0-3

### P1-3: 特徴量抽出ホットスポットの最適化

**目標**

リプレイ負荷の高いワークロードにおいて、繰り返される計算コストを削減する。

**背景**

特徴量計算において、以下の項目で現在重複した作業が行われています：

1. トレンド期間（trend-age）の派生
2. 長いウィンドウでのパーセンタイル推定

**範囲**

1. トレンドの繰り返し再計算を削減する。
2. 実用的な範囲で、ローリング MA やキャッシュされたシリーズを再利用する。
3. フルウィンドウのスキャンを繰り返さないように、パーセンタイル計算戦略を再検討する。

**推奨されるファイル**

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)

**検収基準**

1. 特徴量抽出が動作的に同等であることを維持すること。
2. 同じサンプルデータセットにおいて、リプレイの実行時間が大幅に改善すること。
3. 新しい実装がテストでカバーされていること。

**依存関係**

P0-1

### P1-4: バックテストのリプレイループの最適化

**目標**

より多くの資産やより長いウィンドウに対して、バックテストのスケーラビリティを向上させる。

**背景**

現在のバックテストは、スケーリングしない方法で履歴のスライシングやスキャンを繰り返しています。

**範囲**

1. 日次スライスの繰り返しクローンを削減する。
2. 有用な場合は、日付ごとにバーをプリインデックス化する。
3. 前方リターンのルックアップやドローダウンウィンドウのための線形スキャンの繰り返しを回避する。

**推奨されるファイル**

1. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)

**検収基準**

1. 同じデータセットにおいて、バックテストの実行時間が大幅に改善すること。
2. 出力が同等であることを維持するか、差異が明示的に説明されていること。
3. サマリー生成にデグレードがないこと。

**依存関係**

P1-3 があると役立つが、必須ではない。

### P1-5: 永続化の読み込みパターンの堅牢化

**目標**

長期間保持されるアーカイブ資産の脆弱性を低減する。

**背景**

一部の永続化パターンは機能していますが、長期的な信頼性には理想的ではありません。

**範囲**

1. 最新パケット読み込み時の JSONL の読み取り動作を改善する。
2. 破損した末尾行の履歴に対して、より明確な破損処理を追加する。
3. 追記型（Append-only）アーカイブファイルの期待値を定義する。

**推奨されるファイル**

1. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)

**検収基準**

1. 最新パケットの読み込みにおいて、破損した末尾を適切に処理できること。
2. JSONL の追記/読み取りセマンティクスが文書化されていること。
3. テストが破損した履歴ケースをカバーしていること。

**依存関係**

P0-2

## P2

### P2-1: Telegram 出力テンプレートのアップグレード

**目標**

Telegram を騒がしくすることなく、意思決定密度を高める。

**背景**

現在の Telegram 出力は簡潔ですが、高シグナルのオペレーターブリーフというよりは、軽量なサマリーに近いものです。

**範囲**

1. メッセージを短く保つ。
2. 情報の階層構造を改善する：
   - 市場状態
   - ポートフォリオモード
   - 上位のアクション可能な資産
   - 防御的な場合の主要な警告
3. アーカイブ Markdown を Telegram よりもリッチに保つ。

**推奨されるファイル**

1. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

**検収基準**

1. Telegram がモバイルで読みやすいままであること。
2. メッセージに明確な状態 + ポリシー + 上位のアクションが含まれていること。
3. アーカイブ Markdown がよりリッチな資産であり続けること。

**依存関係**

P0 の完了

### P2-2: バックテストレポートの深化

**目標**

バックテストの出力を、実際のイテレーションツールにする。

**背景**

現在のサマリーは有用ですが、体系的な戦略調整にはまだ不十分です。

**範囲**

1. レジーム持続期間の統計を追加する。
2. アクションレベルのアトリビューションを追加する。
3. より明確な信頼度バケット（Confidence-bucket）レポートを追加する。
4. 実行結果を時系列で比較するためのサマリー形式を準備する。

**推奨されるファイル**

1. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)
2. `backtest/summary.md` の生成ロジック

**検収基準**

1. バックテストの出力がパラメータイテレーションをサポートしていること。
2. サマリーを実行間で比較しやすくなっていること。
3. アトリビューションが単純な的中率（Hit rate）よりも実用的であること。

**依存関係**

P1-4

### P2-3: データ品質ログの診断への変換

**目標**

受動的なログ記録から、能動的なデータ品質評価へ移行する。

**背景**

`data_quality_log.jsonl` は存在しますが、依然として主に生の監査トレイルに留まっています。

**範囲**

1. 品質スコアリングまたは重要度レベルを追加する。
2. 鮮度が低い、疎である、または不完全な履歴にフラグを立てる。
3. 適切な場合に、アーカイブまたは Telegram のサマリーにデータ品質問題を表面化させる。

**推奨されるファイル**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

**検収基準**

1. データ品質ログが手動検査なしで解釈可能になること。
2. 重大なデータ品質問題がオペレーターに見えるようになること。

**依存関係**

P0-2

### P2-4: 実行観測性の強化

**目標**

実際の運用上の失敗下で、実行パスを監査可能にする。

**背景**

システムは現在ゲートとスナップショットを備えていますが、完全な実行観測性ループはまだありません。

**範囲**

1. 構造化された実行失敗の分類（Taxonomy）を追加する。
2. 注文ステータスの照合（Reconciliation）フックを追加する。
3. 実行の成功とブローカー側の失敗の報告を改善する。

**推奨されるファイル**

1. [trader_agent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/trader_agent.rs)
2. [ledger.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/ledger.rs)
3. [trade/trader.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/trade/trader.rs)
4. `src/adapters/futu/` 下の Futu アダプターモジュール

**検収基準**

1. 注文の失敗が構造的にログ記録されること。
2. ブローカー側の拒否とローカルゲートの拒否が区別できること。
3. 実行結果を長期にわたって監査できること。

**依存関係**

P0-3

## 推奨される実行順序

1. P0-1
2. P0-3
3. P0-2
4. P0-4
5. P0-5
6. P1-1
7. P1-2
8. P1-3
9. P1-4
10. P1-5
11. P2-1
12. P2-2
13. P2-3
14. P2-4

## 完了の定義

この監査イシューリストは、以下の場合に完了とみなされます：

1. `P0` がコード、テスト、および Lint においてグリーンである。
2. `P1` が主要な保守性とリプレイのボトルネックを解消している。
3. `P2` がコアパイプラインを不安定にすることなく、オペレーターの品質を向上させている。
