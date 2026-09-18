---
author: Ray
title: AI Policy frontier pacing 仮説・linkage ライフサイクル
description: AI Policy observation に対する明示的な仮説・linkage 状態を observation-only で保持する。
key: ai-policy-frontier-pacing-hypothesis-linkage
---

# AI Policy frontier pacing 仮説・linkage ライフサイクル

## 目的

5a の provenance 付き observation に対して、仮説と event linkage の状態を構造化して保持する。状態は外部入力で明示されたものだけを表示し、自動確認、自動拒否、自動遷移、ranking、Decision、Gate、取引処理は実装しない。

## 設計

1. `AiPolicyFrontierPacingLinkageStatus` は `PROPOSED`、`CONFIRMED_BY_HUMAN`、`REJECTED_BY_HUMAN`、`EXPIRED`、`UNAVAILABLE` を持つ。
2. linkage は `hypothesis_id`、`observation_id`、status、任意の `linked_event_id`、provenance-backed evidence、任意の human decision 時刻/source を保持する。
3. 必須 ID、observation 参照、evidence、RFC3339 時刻、human decision metadata が欠けた記録は fail-closed で破棄する。
4. `CONFIRMED_BY_HUMAN` は linked event と human decision metadata を要求する。`PROPOSED` は confirmation と解釈しない。
5. SignalContextV1、Markdown/Telegram HTML、weekly audit JSON/text は observation-only の独立領域として同じ状態を表示する。

## TDD と検証

- valid proposed linkage が保持されること。
- confirmed linkage が明示的な linked event、decision source、decision time、evidence なしでは保持されないこと。
- observation、hypothesis、linkage のいずれかが不完全な場合に fail-closed となること。
- report に状態が表示されても `BUY`、`SELL`、Decision、Gate、Position Sizing に影響しないこと。
- `make fmt-check`、`make test`、`make clippy`、`make check-signal-context-consistency`、`make quality` を Runtime evidence に記録する。
