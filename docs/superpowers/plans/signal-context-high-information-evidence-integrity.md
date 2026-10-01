---
author: Ray
title: HIGH information 証拠契約
description: 分類と情報量を分離する観測専用 read model の実装計画。
key: signal-context-high-information-evidence-integrity
---

# HIGH information 証拠契約

既存 event classification / extraction と取引判断は変更しない。read model で HIGH を認めるには正向証拠を必須とする。`decision_weight=0`、`trade_signal=false`。

- [x] 不足証拠の HIGH と無条件 repricing prose を再現する失敗テスト。
- [x] primary event に束縛した SignalInformationEvidence SSOT を実装。
- [x] Route A は timestamp precision、合法な TemporalBinding、source、数値 magnitude / baseline / significance floor、方向、独立 dimension を検証。
- [x] Route B は structured source metadata が event character と surprise / material-new-action を明示する場合だけ許可。単なる formal action label は不十分。
- [x] commentary / analysis / profile / political positioning は Route A なしで HIGH にしない。metadata 不足は UNKNOWN。headline keyword で例外を追加しない。
- [x] 既存 provider は structured reaction を供給しないため None のまま維持する。coverage と prose から数値を捏造しない。
- [x] zh/en/ja と Markdown / Telegram / Archive の同一 evidence rendering、decision invariance を検証。
- [ ] fmt-check / test / clippy / quality、Runtime verify、独立 review、hosted exact head、archive / merge / finalize / close。

Significance floor は証拠に含まれる measurement method と unit を持つ正の数値であり、取引 threshold ではない。timestamp、primary event id、observation identity、観測窓の不整合は fail-closed。単なる受信時刻を source publication の代わりにしない。

accepted proof は検証を通った source record / TemporalBinding のみから構築し、生の evidence は別 collection に保持する。同一 instrument の異なる次元は全て除外し、入力順の6排列を回帰検証する。Archive は SSOT の完全 JSON を併記する。
