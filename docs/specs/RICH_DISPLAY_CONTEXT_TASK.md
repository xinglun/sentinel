---
author: Ray
title: Sentinel リッチ展示コンテキスト (Rich Display Context) タスクリスト
description: Sentinel リッチ展示コンテキスト (Rich Display Context) タスクリスト に関する Sentinel の設計・運用情報。
key: docs-specs-rich-display-context-task
---

# Sentinel リッチ展示コンテキスト (Rich Display Context) タスクリスト

## 1. 目標

本タスクは、既存の `DisplayAdapter` をベースに、展示セマンティクス（表示上の意味論）のさらなる精細化を推進することを目的としています。

現在のシステムで完了している事項：

1. `display_intent` と旧 `AssetAction` のデカップリング。
2. `HOLD / OBSERVE` が `has_position`（ポジションの有無）によって駆動される。
3. Telegram / CLI / Tactical Summary が同一の展示プリミティブを共有している。

しかし、現在の展示コンテキストは依然として粗い状態です：

1. `has_position` しか情報がない。
2. 以下の状態を表現するには不十分：
   - ポジションはあるが、すでに勢いが衰えている（脱落）。
   - ポジションはないが、優先度の高い候補である。
   - コアな持ち分（Core Holding） vs 周辺的な持ち分（Edge Holding）。
   - 市場が Ready でない状態での候補資産。

本フェーズの目標は以下の通りです：

**展示セマンティクスを「単一の事実駆動」から「よりリッチな展示コンテキスト (Richer Display Context) 駆動」へとアップグレードする。**

---

## 2. なぜこの層が必要なのか

現状：

1. `has_position = true` -> 多くの場合 `HOLD` と表示。
2. `has_position = false` -> 多くの場合 `OBSERVE` と表示。

これは旧バージョンより大幅に改善されていますが、まだ不十分です。

なぜなら、実際の展示セマンティクスには、少なくとも4つの異なる対象が存在するからです：

1. コアな持ち分 (Core Holding)
2. コアではないが、依然として保持している銘柄 (Non-Core Holding)
3. ポジションのない候補 (Candidate Only)
4. すでに勢いが衰え、戦線を縮小すべき銘柄 (Laggard)

もし展示層がこれらの状態を区別できない場合：

1. ユーザーが見る UI が単調（フラット）になる。
2. 説明力が低下する。
3. 将来的な Web / App などのマルチデバイス展開において、統一した表現が難しくなる。

---

## 3. 設計目標

より明確な展示コンテキストレイヤーを新規追加します。例：

```rust
pub struct DisplayContext {
    pub has_position: bool,
    pub is_core_holding: bool,
    pub is_candidate_only: bool,
    pub is_top_tier: bool,
    pub participation_ready: bool,
}
```

最低限必要な要件：

1. `has_position`
2. `is_core_holding`
3. `is_candidate_only`

推奨される補完項目：

1. `is_top_tier`
2. `participation_ready`

---

## 4. 核心となるセマンティクスの定義

### 4.1 Core Holding (コアな持ち分)

定義の推奨案：

1. 現在ポジションを保有している。
2. かつ、依然として Top Tier / コア持ち分集合に含まれている。

### 4.2 Candidate Only (候補のみ)

定義の推奨案：

1. 現在ポジションを保有していない。
2. しかし、依然として候補のコア集合に含まれている。
3. 主に `OBSERVE` と表示するために使用される。

### 4.3 Non-Core Holding (非コアな持ち分)

定義の推奨案：

1. 現在ポジションを保有している。
2. しかし、すでにコア集合からは外れている。

この種の資産は、`Core Holding` と同じようには展示されるべきではありません。

---

## 5. DisplayIntent 第2段階ルール

推奨される初版ルール：

1. `PositionIntent::ADD -> DisplayIntent::ADD`
2. `PositionIntent::TRIM -> DisplayIntent::TRIM`
3. `PositionIntent::EXIT -> DisplayIntent::EXIT`
4. `PositionIntent::HOLD` の場合：
   - `is_core_holding == true` -> `DisplayIntent::HOLD`
   - `is_candidate_only == true` -> `DisplayIntent::OBSERVE`
   - `has_position == true && !is_core_holding` -> 引き続き `HOLD` と表示するが、弱体化タグの付加を許可する。
   - `has_position == false && !is_candidate_only` -> `DisplayIntent::OBSERVE`

注意：

1. 本フェーズでは、これ以上の intent 列挙型の新規追加は行いません。
2. まずはリッチなコンテキストを通じて説明力を向上させます。

---

## 6. 推奨される出力の拡張

資産レベルの意思決定結果に以下を新規追加することを推奨します：

1. `display_context`
2. `display_tags`
3. `display_notes`

例：

```rust
pub struct DisplayContext {
    pub has_position: bool,
    pub is_core_holding: bool,
    pub is_candidate_only: bool,
    pub is_top_tier: bool,
    pub participation_ready: bool,
}
```

```rust
pub display_tags: Vec<String>
```

推奨されるタグの例：

1. `Core Holding` (コア持ち分)
2. `Candidate` (候補)
3. `Non-Core Holding` (周辺持ち分)
4. `Participation Blocked` (参加制限中)

---

## 7. 推奨されるコードの配置

引き続き以下を使用します：

1. [display.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/display.rs)

そして以下の場所で接続します：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/application/engine.rs)
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/report.rs)
3. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/decision.rs)

### 7.1 Engine の責務

1. `DisplayContext` の生成。
2. `display_intent` の生成。
3. オプションの展示タグの生成。

### 7.2 Report の責務

1. `display_intent` の消費。
2. オプションで `display_tags` を表示。
3. 展示コンテキストの独自推論を停止。

---

## 8. 開発タスクの分解

### P0-1 DisplayContext の新規追加

任務要件：

1. `DisplayContext` を定義。
2. 資産レベルの意思決定構造体に永続化。
3. 過去との互換性のために `serde(default)` を追加。

### P0-2 Engine におけるリッチな展示コンテキストの生成

任務要件：

1. ポジション、Top Tier、Participation 状態に基づいて `DisplayContext` を生成。
2. もはや `has_position` だけに依存しない。

### P0-3 DisplayAdapter による DisplayContext の消費への変更

任務要件：

1. `derive_display_intent(...)` の入力を `DisplayContext` に変更。
2. 単一のブール値のみを受け取るのを停止。

### P1-1 報告層への軽量タグの追加

任務要件：

1. Top Actions または理由欄において、以下のタグをオプションで表示：
   - `Core`
   - `Candidate`
   - `Blocked`
2. 報告書は簡潔に保ち、過剰な情報化を避けること。

### P1-2 UI / 回帰テスト

任務要件：

1. Core Holding シナリオのカバー。
2. Candidate Only シナリオのカバー。
3. Non-Core Holding シナリオのカバー。
4. Participation blocked candidate シナリオのカバー。

---

## 9. テストリスト

少なくとも以下のテストを補完してください：

1. `has_position=true && is_core_holding=true` -> `HOLD`
2. `has_position=false && is_candidate_only=true` -> `OBSERVE`
3. `has_position=true && is_core_holding=false` -> `HOLD` を維持するが、弱体化タグが付与されること。
4. `participation_ready=false && candidate_only=true` -> 引き続き `OBSERVE` と表示され、blocked セマンティクスを伴うこと。
5. Telegram Top Actions / Tactical Summary / 期待・機会セクションで同一のコンテキスト基準を共有していること。

---

## 10. 完了基準

本タスク完了後、システムは以下の条件を満たさなければなりません：

1. `display_intent` が `has_position` だけに依存していないこと。
2. 展示層が、コア持ち分、候補資産、周辺持ち分を区別できていること。
3. 報告書の説明力が向上し、かつ旧 `action` への依存が再導入されていないこと。
4. すべての展示モジュールが引き続き、統一された DisplayAdapter の出力を共有していること。

---

## 11. 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. UI の美化。
2. 執行意図（Execution Intent）のさらなる追加。
3. Web / App コンポーネントの開発。
4. 複雑なタグシステムの構築。

本フェーズで行うのは以下のことのみです：

**展示コンテキストを「単一の事実」から「構造化された事実」へとアップグレードすること。**
