---
author: Ray
title: AI Cockpit Rules Template
description: Sentinel の AI Cockpit 規則を他リポジトリへ移植するための参照テンプレート。
key: template-ai-cockpit-rules
---

# AI Cockpit Rules Template

このファイルは、AI Cockpit の運用規則をテンプレートとして再利用するための参照です。

## 必須ワークフロー

1. `.ai/cockpit/README.md` または採用先の同等文書で状態定義を確認する。
2. active Work Item Contract を確認する。
3. `mode: code` の場合は、`unknowns`、`notCodable`、`scope`、`outOfScope`、`acceptance`、`verification` を実装前に確定する。
4. `make ai-preflight` を実行し、Preflight Review を表示する。
5. `needs_human_confirmation` または `not_ready` の場合は、レビューをユーザーへ報告してから実装を続ける。
6. 完了時は Summary を更新し、`make ai-finish` で archive する。

## 運用原則

- Preflight Review は reviewer visibility ではなく、実装前の pause を補助する。
- readiness は AI の自己申告ではなく、Contract evidence から導出する。
- すべての verification は `make` entrypoint 経由で記録する。
- Markdown 文書は日本語本文と front matter を維持する。
