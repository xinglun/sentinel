---
author: Ray
---

# 多端表示セマンティック標準 (DISPLAY_SEMANTICS_STANDARD.md)

本仕様は、Sentinel の意思決定パッケージにおける異なる表示端（Telegram, CLI, Web/App）での一貫したレンダリング原則を定義します。

## 1. 表示意図のマッピング (DisplayIntent Mapping)

| 表示意図 (DisplayIntent) | ビジネスコンテキスト | Telegram セマンティクス | Web/App セマンティクス |
| :--- | :--- | :--- | :--- |
| **ADD** | ポジションの構築または買い増しを推奨 | 買い増し | 🟢 緑のプラス / BUY |
| **HOLD** | 継続保有を推奨 | 保持 | 🔵 青のドット / HOLD |
| **OBSERVE** | 観察エリアにあり、ポジションなし | 観察 | ⚪️ 灰白色のドット / WATCH |
| **TRIM** | 減配（一部売却）を推奨 | 減配 | 🟠 オレンジのマイナス / REDUCE |
| **EXIT** | 清算を推奨 | 清算 | 🔴 赤のクロス / EXIT |

## 2. 表示ラベル仕様 (Strategic Tags)

すべてのレンダリング端は、`DisplayContext` の事実に基づいて以下のタグを自動的に付与する必要があります：

### [Core] (コア持分)
- **判定ロジック**: `has_position == true && is_core_holding == true`
- **目的**: 長期保有の自信を強化します。
- **表示推奨**: 目立つスタイル、または金色/ハイライト色。

### [Candidate] (重点候補)
- **判定ロジック**: `has_position == false && is_candidate_only == true`
- **目的**: 現在重点的に注目している非持分銘柄を明確にします。
- **表示推奨**: 点線枠、または淡色のハイライト。

### [Blocked] (ブロック済み)
- **判定ロジック**: `is_candidate_only == true && participation_ready == false`
- **目的**: 「一見良さそうに見える」にもかかわらず、システムが ADD を命じていない理由を説明します。
- **表示推奨**: ロックアイコン、またはグレーの斜線。

## 3. バケット原則 (Standard Buckets)

多端で一律に `DisplayIntent` をバケットの主軸として採用します：
1. **Top Actions**: すべての `ADD`, `TRIM`, `EXIT` 意図、および重要な `HOLD` の変更。
2. **保有エリア (Holdings)**: `DisplayIntent::HOLD` かつ `has_position == true`。
3. **観察エリア (Watchlist)**: `DisplayIntent::OBSERVE`。
4. **リスク/機会エリア**: 特定のルールに基づいて表示エリアにマッピングされる追加の注釈。

## 4. 優先順位の競合処理

1. **撤退優先**: `DisplayIntent::EXIT` は最高の表示ウェイトを持ちます。
2. **ブロック優先**: もし `participation_ready == false` ならば、いかなる買いの提案も「待機」に変換されるか、明示的に `[Blocked]` タグを付与します。
3. **診断優先**: `exit_decision` 内で明確な `Protection` がトリガーされている場合、必ず詳細な理由を表示する必要があります。
