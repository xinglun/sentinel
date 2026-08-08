---
author: Ray
title: Price-Volume ATR Normalization 設計
description: Price-Volume の ATR-normalized move を true-range average で正しく算出する。
key: price-volume-atr-normalization-design
---

# Price-Volume ATR Normalization 設計

直近 14 bar の true range 平均を ATR とし、当日の absolute body を ATR で除算する。ACCUMULATION の限定 downside は 1.0 ATR 以下を確認する。これは Observation Layer のみであり、取引経路は変更しない。
