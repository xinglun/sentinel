---
author: Ray
title: 監査済み Archive Repair 実装計画
description: PR Guard の限定的 archive repair 例外を実装・検証する。
key: audited-archive-repair-plan
---

# 監査済み Archive Repair 実装計画

1. `ai_test_pr_check.py` に正しい historical restore と不一致 restore の失敗テストを追加する。
2. `ai_check_pr.py` が新規 Contract の `archiveRepair` を読み、唯一の archive modification だけを祖先と SHA-256 で検証するようにする。
3. S-28-04 summary を commit `052d12fa` の content へ復元する Contract を archive し、PR Guard を実行する。
4. Cockpit と Rust quality gate を通して archive、PR、merge、branch cleanup を行う。
