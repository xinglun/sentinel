---
author: Ray
title: AI Policy frontier pacing observation 実装計画
description: provenance-backed な observation-only 記録を Signal Context の構造化入力と表示へ追加する。
key: ai-policy-frontier-pacing-observation-plan
---

# 目的

AI Policy frontier pacing を、source、source_url、source_published_at、headline、provider の provenance を保持する構造化 observation として扱う。欠損または不正な provenance は推測せず fail-closed とする。

# 境界

この Work Item は external structured SignalContextV1 の受け渡し、正規化、read model、Markdown/HTML、weekly audit の表示だけを対象とする。provider API、hypothesis confirmation、linkage lifecycle、Primary Context ranking、Decision、Gate、trade_signal、position sizing は変更しない。

# TDD 手順

1. provenance が完全な observation を保持し、欠損または不正な `source_published_at` を破棄するテストを先に追加する。
2. observation を `SignalContextV1` と外部 JSON load/merge に通す最小実装を追加する。
3. report と weekly audit の observation-only 表示を追加し、既存の decision boundary を回帰テストで固定する。
4. focused tests、`make fmt-check`、`make test`、`make clippy`、`make quality`、`make check-signal-context-consistency` を Runtime receipt に記録する。

# Acceptance invariant

```text
AI Policy observation
= complete provenance
+ structured read model
+ observation-only presentation
+ fail-closed invalid input
```

`decision_weight=0` と `trade_signal=false` は維持し、5b の hypothesis/linkage は後続 Work Item とする。
