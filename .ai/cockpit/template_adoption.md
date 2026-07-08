---
author: Ray
title: AI Cockpit Template 採用メモ
description: ai-cockpit-template から Sentinel Cockpit へ反映する経験と保留事項。
key: ai-cockpit-template-adoption
---

# AI Cockpit Template 採用メモ

このメモは `/Users/sei-rinn/dev/workspace_python/ai-cockpit-template` を参照し、Sentinel の現行 Cockpit に取り込むべき経験を整理する。目的は template runtime の機械的な移植ではなく、Sentinel の既存 guard と Work Item 運用を壊さずに、review 可能性を上げる判断基準を残すことである。

## 採用する原則

| 原則 | Sentinel での扱い |
|---|---|
| 目的を先に書く | Contract の `problemStatement` と `intent.problem` で、何を解決する作業かを明示する。製品文脈がない場合は推測しない。 |
| field 順序を固定する | Contract と Summary の field 順序は review 可読性の約束として固定し、無意味な diff を避ける。 |
| scope を実装前に固定する | `scope` は実装後の変更一覧ではなく、編集許可境界として扱う。`outOfScope` には production code、CI、Makefile などを明示できる。 |
| schema より文書を先に固める | 新しい field や guard を追加する前に、review model と運用判断を文書化する。 |
| status は判断信号へ圧縮する | `current_status.md` は Summary の複製ではなく、blocking reason、required checks、changed files、next action を短く示す。 |
| upgrade は active Work Item と分離する | Cockpit runtime や schema を更新する場合、active Work Item がない状態で別 task として扱う。 |
| Make entrypoint を維持する | Sentinel では `verification[].command` を `make ...` に限定する。template の `verification[].check` 方式は直接持ち込まない。 |

## 既に Sentinel にある機能

| template の概念 | Sentinel の現状 |
|---|---|
| Scope Guard | `make check-ai-scope CONTRACT=...` と `scripts/ai_check_scope.py` で管理している。 |
| File ownership / boundary | `.ai/guards/file_ownership.yaml` と `.ai/guards/file_boundary.yaml` を `make check-ai-guards` で検証している。 |
| Backtrack Guard | `make check-ai-backtrack CONTRACT=... SUMMARY=...` で test、snapshot、i18n、Work Item evidence の無宣言削除を検出する。 |
| Coverage Guard | `make check-ai-coverage-guard` で production Rust code 変更に test 変更証跡を要求する。 |
| Scenario Coverage Guard | `make check-ai-scenario-coverage` で risk 域の verified / unverified / not_applicable を確認する。 |
| Status Consistency | `make generate-cockpit-status` と `make check-ai-status`、`make check-ai-status-consistency` で active / no-active 状態を確認する。 |
| Finish Flow | `make ai-finish TASK=...` が required checks 成功時だけ archive する。 |
| Rust quality gate | `make fmt-check`、`make test`、`make clippy` を commit 前 gate として扱う。 |

## Sentinel へ直接持ち込まないもの

| template の要素 | 判断 |
|---|---|
| `verification[].check` ID 方式 | 現行 Sentinel は `verification[].command` と `make` 入口を hard gate にしているため、schema migration なしに混在させない。 |
| managed installer による上書き upgrade | Sentinel には repository 固有の guard、AI Cockpit policy、Rust / data branch 境界があるため、installer の一括上書きは採用しない。 |
| 汎用 cross-language examples | Sentinel は Rust repository として運用するため、他言語 example は設計参考に留める。 |
| template の governance compression 表示面 | 現行 `current_status.md` の簡潔な状態表示を優先し、Recommendation / Decision Drivers の追加は別 Work Item で検討する。 |

## 後続候補

| 候補 | 進める条件 |
|---|---|
| Contract / Summary field reference の Sentinel 版 | Contract review で field 意味の解釈ずれが繰り返し起きた場合に、`.ai/cockpit/` または `.ai/README.md` へ追加する。 |
| Status の reviewer guide | `current_status.md` を読む人が増え、ready / risk / blocked の判断基準を短文化する必要が出た場合に追加する。 |
| Scenario coverage | 中高リスク Work Item で未検証 scenario と residual risk を分ける必要が明確になった場合に Summary schema と guard を拡張する。現在は軽量版として採用済み。 |
| Upgrade playbook | Cockpit runtime、script、schema をまとめて更新する前に、active Work Item 不在、backup、rollback、PR audit の手順を別 task で定義する。 |

## 運用判断

Sentinel の Cockpit は、template の汎用性よりも repository 固有の安全境界を優先する。特に Gate、execution、trader、action matrix、position sizing、data branch、report output に影響する変更は、template の一般例ではなく Sentinel の Work Item Contract、`.ai/guards/**`、Make target を正とする。

AI Agent は template 由来の便利な field や script を見つけても、既存 Contract schema と guard が許すまで production code や governance runtime へ直接追加しない。必要な場合は、先に Contract に scope、acceptance、verification、agentCapability、残余 risk を書き、`executionDecision` を `continue` にできる状態へ更新する。Scenario Coverage は test list ではなく、risk 域の検証場面として Summary に残す。
