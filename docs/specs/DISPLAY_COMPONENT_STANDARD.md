---
author: Ray
title: 多端表示コンポーネント・セマンティック標準 (DISPLAY_COMPONENT_STANDARD.md)
description: 多端表示コンポーネント・セマンティック標準 (DISPLAY_COMPONENT_STANDARD.md) に関する Sentinel の設計・運用情報。
key: docs-specs-display-component-standard
---

# 多端表示コンポーネント・セマンティック標準 (DISPLAY_COMPONENT_STANDARD.md)

本仕様は、Sentinel の意思決定パッケージがどのように高レベルの UI コンポーネント・セマンティクスに変換されるかを定義し、クロスプラットフォーム（Telegram, CLI, Web/App）での表現の一貫性を確保します。

## 1. Top Action コンポーネント・セマンティクス

Top Action はシステムで最も重要な意思決定の出力であり、コア資産に対する即時の提案を反映します。

### ViewModel 構造 (簡略版)
- **`title`**: 銘柄コード (例: "NVDA")。
- **`primary_label`**: 主アクション文言 (例: "買い増し", "保持", "清算")。
- **`indicator`**: 状態アイコン (例: "🟢", "◎", "🔴")。
- **`secondary_area`**: サブ情報（変更タグ、最高優先度の Context Tag、および診断理由を表示）。

### 表現マッピングの原則
- **階層化 (Hierarchy)**:
    - **メイン行 (Minimal)**: `{Symbol} {Label} {Icon} {State}` のみを含め、ファーストビューの重心が「意思決定の結論」にあることを保証します。
    - **サブ行 (Decluttered)**: `({変更·タグ}) | {診断理由}` の形式を採用します。
    - **詳細行 (Optional)**: `└ {Reason}` 形式で詳細な説明を表示します。
- **アイコンのマッピング**:
    - ADD -> 🟢
    - HOLD -> ◎
    - OBSERVE -> △
    - TRIM -> 🟠
    - EXIT -> 🔴
- **タグの優先順位**: `Blocked` > `Core` > `Candidate` (最高位のタグを1つのみ保持)。

## 2. Tactical Summary (戦術パーティション) コンポーネント・セマンティクス

すべての監視資産を「意図バケット」ごとに分類するために使用されます。

### ViewModel 構造
- **`bucket_id`**: 列挙型 (ACCUMULATE, HOLDINGS, WATCHLIST, ACTIONS)。
- **`display_name`**: 製品化された名称 (例: "買い増しエリア", "保有エリア", "観察エリア", "収縮エリア")。
- **`items`**: 銘柄コードを含むリスト。

### 承認とソート
- **買い増しエリア**: `DisplayIntent::ADD`。
- **保有エリア**: `DisplayIntent::HOLD`。
- **観察エリア**: すべての `DisplayIntent::OBSERVE` 行動。
- **収縮エリア**: すべての減配/撤退（TRIM, EXIT）行動。

## 3. Risk & Opportunity (リスクと機会) コンポーネント・セマンティクス

システムが診断を通じて発見した極端な状況を抽出します。統一された「銘柄 + トリガーワード」の形式を採用します。

## 4. Monitoring Signals (監視シグナル) コンポーネント・セマンティクス

レポートの監視セクションは完全に日本語化し、下層のエンジニアリング用語を露出させないようにします。

- **Confidence** -> **信頼指数** (高/中/低)
- **Stability** -> **安定性** (安定/不安定/脆弱)
- **Participation** -> **参加状態** (準備完了/未完了)
- **Streak** -> **連続性** (例: "3日連続")
- **Regime Age** -> **周期の長さ**
- **Flow** -> **資金流向**

## 5. 変更メンテナンス原則 (Maintenance Principles)

「実装は変わったが契約が変わっていない」という回帰を防ぐため、表示セマンティクスに関わる修正は必ず **「三位一体同期 (Trinity Sync)」** に従う必要があります：

> [!IMPORTANT]
> 1. **コードの同期**: `DisplayAdapter` (`display.rs`) の ViewModel 変換ロジックを更新します。
> 2. **アサーションの同期**: `report_ui_tests.rs` 内のすべての関連する文字列アサーションを修正します。
> 3. **仕様の同期**: 本コア仕様 (`DISPLAY_COMPONENT_STANDARD.md`) 内のフォーマット定義を更新します。

---
**レンダリング記号を変更しながら古いテストアサーションを保持することは禁止されています。これは、標準化されたセマンティクスの事実上のドリフトを引き起こします。**
