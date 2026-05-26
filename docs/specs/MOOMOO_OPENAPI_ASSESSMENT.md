---
author: Ray
title: Moomoo OpenAPI アセスメント (Moomoo OpenAPI Assessment)
description: Moomoo OpenAPI アセスメント (Moomoo OpenAPI Assessment) に関する Sentinel の設計・運用情報。
key: docs-specs-moomoo-openapi-assessment
---

# Moomoo OpenAPI アセスメント (Moomoo OpenAPI Assessment)

## 1. 目的

本ドキュメントは、moomoo OpenAPI が Sentinel にとってもつ実際の意義、現在の公式機能の境界、および現在のプロジェクトにおける実装の完了度を定義します。

本ファイルは、以下の3つの問いに答えるために用意されています：

1. 公式機能が Sentinel の現在の方向性をサポートしているか。
2. 現在のコードがどの程度実装されているか。
3. 「安定した本番取引接続レイヤー」を構築するために何が不足しているか。

## 2. 主なソース (Primary Sources)

1. [OpenAPI Introduction](https://openapi.moomoo.com/moomoo-api-doc/en/intro/intro.html)
2. [Authorities and Limitations](https://openapi.moomoo.com/futu-api-doc/en/intro/authority.html)
3. [Place Orders](https://openapi.moomoo.com/moomoo-api-doc/en/trade/place-order.html)
4. [Subscribe and Unsubscribe](https://openapi.moomoo.com/moomoo-api-doc/en/quote/sub.html)

## 3. ハイレベルな結論 (High-Level Conclusion)

結論は明確です：

1. moomoo/OpenD を使用して米国株の相場取得および取引を行う Sentinel の方向性は成立しています。
2. 現在のプロジェクトにおいて、最もコアとなる取引の骨組みはすでに実装されています。
3. ただし、現時点では「コア経路の開通済み」と定義すべきであり、「すべての本番級ブローカー統合の強化（Hardening）が完了した」わけではありません。

## 4. 公式機能のサマリー

### 4.1 アーキテクチャ

公式 OpenAPI は2つの部分で構成されています：

1. `OpenD`
   - ローカルまたはクラウドで実行されるゲートウェイプロセス。
   - TCP を介してプロトコルインターフェースを公開。
2. `moomoo API`
   - 公式 SDK。
   - Python / Java / C# / C++ / JavaScript をサポート。
   - 非公式言語でもプロトコルを直接叩くことで対応可能。

これは Sentinel の現在の `FutuClient -> OpenD -> moomoo` アーキテクチャと一致しています。

### 4.2 機能範囲

OpenAPI の2大機能：

1. `Quotation` (相場)
   - リアルタイムサブスクリプション。
   - スナップショット。
   - 履歴 K 線（チャートデータ）。
   - Tick / Order Book など。
2. `Trading` (取引)
   - Paper Trading (デモ取引)。
   - Live Trading (本番取引)。

### 4.3 Sentinel に関連する市場範囲

Sentinel の現在の目標において最も重要な結論：

1. 米国株 / ETF：相場、デモ取引、本番取引をサポート。
2. 米国株オプション：サポート。
3. 日本株：現在、moomoo ユーザーに対しては未サポート。

したがって：

1. Sentinel が現在米国株を核心目標としているのは正しい選択です。
2. 現在の接続状況を「日本株の自動取引をサポート済み」と誤解して拡張すべきではありません。

## 5. 重要な公式制約事項

### 5.1 アカウントと権限

公式の制限：

1. 対応する市場の取引業務アカウントを事前に開設する必要があります。
2. 相場権限と市場権限は、最初からすべて開放されているわけではありません。
3. 市場やデータタイプごとに、対応する権限（Authority）が必要です。

プロジェクト上の意義：

1. コードがつながっていることと、アカウントが使用可能であることは別物です。
2. 本番稼働前に、必ずアカウント/権限の Preflight（事前チェック）を行う必要があります。

### 5.2 レート制限 (Rate Limits)

公式には、取引インターフェースに頻度制限が存在することが明記されています。

`Place Order` を例にとると：

1. 同一 `acc_id` 下で 30秒間に最大 15回のリクエストまで。
2. 連続する2回のリクエスト間隔は 0.02秒以上である必要があります。

プロジェクト上の意義：

1. 日次/低頻度戦略においては、現時点では大きな問題にはなりません。
2. 将来的に一括発注やイベント駆動型モードを導入する場合、レート制限保護を追加する必要があります。

### 5.3 クォータ (Quotas)

公式の制限：

1. リアルタイムサブスクリプションにクォータ（割り当て制限）があります。
2. 履歴 K 線取得にもクォータがあります。

プロジェクト上の意義：

1. watchlist の拡大や、履歴データの高頻度な重複取得を行う際は、残りのクォータに注意する必要があります。
2. 将来的にサブスクリプションインターフェースを有効化する場合、クォータを認識（Quota awareness）する機能が必要です。

### 5.4 取引セッションの制約

公式の制限：

1. Live アカウントでの取引前に `unlock`（ロック解除）が必要です。
2. Paper trading では `unlock` は不要です。
3. 米国株 24時間取引には注文タイプの制限があります。

プロジェクト上の意義：

1. Sentinel の現在の `ExecutionMode + trd_env + unlock_trade` の設計方向は正しいです。
2. プレマーケット/アフターマーケット/夜間取引に拡張する場合、現在の注文ロジックをデフォルトとして使い続けることはできません。

## 6. Sentinel の実装状況

### 6.1 実装済み

現在のプロジェクトには以下の機能が備わっています：

1. OpenD TCP 接続。
2. 履歴 K 線取得。
3. 取引ロック解除と権限の事前チェック (P1-2)。
4. 資金照会と購買力検証。
5. 注文の全ライフサイクルのクローズドループ化 (P1-1: Filled/Partial など)。
6. デモ/本番の切り替え。
7. 注文取消（キャンセル）インターフェースと二次確認 (P2-2)。
8. 持分照会とカウンター照合 (P2-3: Authoritative Reconciliation)。
9. 失敗セマンティクスの構造化分類 (P1-3)。
10. 実行監査と `run_status_[DATE].json`。

対応するコード：

1. `src/adapters/futu/client.rs`
2. `src/adapters/futu/provider.rs`
3. `src/adapters/futu/trader.rs`
4. `src/cli.rs`
5. `src/features/radar/application/execution_gate.rs`
6. `src/features/trading/infrastructure/trader_agent.rs`

### 6.2 未実装（または不完全）

以下の能力は、本番のメインチェーンにはまだ導入されていません：

1. サブスクリプション方式のリアルタイム相場メインチェーン (Qot_Sub)。
2. 歩み値（Tick）/ 板情報データストリーム。
3. 全自動のポジション修正（現在は乖離を発見し遮断するのみで、自動反対売買による修正は未サポート）。

## 7. 機能マトリックス (Capability Matrix)

| 機能 | 公式サポート | Sentinel 状態 | 評価 |
| --- | --- | --- | --- |
| OpenD ゲートウェイ | あり | 実装済み | Ready |
| 履歴日次 K 線 | あり | 実装済み | Ready |
| アカウント資金 | あり | 実装済み | Ready |
| 取引ロック解除 | あり | 実装済み | Ready |
| 注文執行 (Place order) | あり | 実装済み | Ready |
| デモ/本番切り替え | あり | 実装済み | Ready |
| 相場サブスクリプション | あり | メインパス未導入 | Pending |
| 板情報 / 歩み値ストリーム | あり | メインパス未導入 | Pending |
| 注文状態の照合 | あり | 実装済み | Ready |
| ポジションの照合 | あり | 実装済み | Ready |
| 注文変更/取消 | あり | 実装済み | Ready |
| 権限 Preflight | 必要 | 実装済み | Ready |
| レート制限対応 | 必要 | 実装済み (1秒) | Ready (低頻度) |
| クォータ認識 | 必要 | 実装済み (Preflight)| Ready |

## 8. 製品の境界 (Product Boundary)

現在、外部に対して成立しうる製品の境界は以下の通りです：

1. moomoo/OpenD に基づく米国株の日次観測。
2. moomoo/OpenD に基づく米国株のデモ取引。
3. moomoo/OpenD に基づく米国株の低頻度/日次本番執行（持分検証と監査のクローズドループを完備）。

現在、外部に対して以下のことを公表すべきではありません：

1. 日本株の自動取引をサポート済みであること。
2. 高頻度取引をサポート済みであること。
3. リアルタイムサブスクリプション（Qot_Sub）駆動の戦略が完了していること（現在は依然として RADAR ポーリングモードです）。

## 9. エンジニアリング上の意思決定

現在、moomoo/OpenAPI 統合を以下のように定義することを推奨します：

1. `Core execution layer hardened` (コア実行層の強化済み)
2. `Initial production-grade integration completed for low-frequency trading` (低頻度取引向けの初期プロダクト級統合の完了)

これは以下を意味します：

1. 接続レイヤーが十分な防御性（リスク管理ゲート、照合、取消の二次確認）を備えていること。
2. 現行のアーキテクチャで日次/低頻度規模の本番運用をサポート可能であること。
3. 将来的に秒単位/高頻度に切り替える場合は、P3 段階のリアルタイムサブスクリプションアーキテクチャへのアップグレードを開始する必要があること。
