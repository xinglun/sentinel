---
author: Ray
title: AI ガバナンス用語集
description: Sentinel の AI Work Item と Preflight Review で使う用語の短い定義。
key: ai-glossary
---

# AI ガバナンス用語集

この文書は、Sentinel の AI Work Item と Cockpit で使う最低限の用語をまとめたものです。

## 用語

| 用語 | 定義 |
|---|---|
| Preflight Review | 実装前に Work Item Contract の既存 evidence から導出する readiness の表示。 |
| Preflight Pause Rule | `needs_human_confirmation` または `not_ready` のとき、エージェントがユーザーへ review を報告してから実装を続ける規則。 |
| Evidence over Self-Declaration | readiness を AI の自己申告ではなく、既存 Contract evidence から導く原則。 |
| Current Status | reviewer 向けに Contract、Summary、Preflight Review、required checks を圧縮して表示する Cockpit の状態表示。 |
| Gate | policy が明示した場合だけ有効になる阻止規則。デフォルトでは advisory として扱う。 |
