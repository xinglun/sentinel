---
author: Ray
title: 監査済み Archive Repair 設計
description: append-only archive の誤 merge を限定的に復元する PR Guard 例外を定義する。
key: audited-archive-repair-design
---

# 監査済み Archive Repair 設計

通常の archive は append-only とする。例外は新規 archive Contract の `archiveRepair` 宣言を持つ 1 件の `M` だけである。

宣言は対象 path、基線 content の SHA-256、復元元 commit、復元 content の SHA-256、理由を必須とする。Guard は復元元が基線の祖先であること、基線・復元・HEAD の content が宣言どおりであることを確認する。

これにより任意の書換えは拒否し、過去に監査された byte-for-byte content への復元だけを許可する。
