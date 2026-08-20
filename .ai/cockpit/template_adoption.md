---
author: Ray
title: AI Cockpit Template 採用メモ
description: ai-cockpit-template から Sentinel Cockpit へ反映する経験と保留事項。
key: ai-cockpit-template-adoption
---

# AI Cockpit Template 採用メモ

このメモは `https://github.com/spirex-ds-dev/ai-cockpit-template` の source commit `e5acb677da6621004d96f0ef353c58fe8d3acfbf`（default branch: `main`）を基準に、Sentinel の `develop`（base commit: `fb40532e5f893c5baf59d9afbfae2669a0ad1cac`）へ反映した結果を記録する。目的は template runtime を無条件に上書きすることではなく、Sentinel の既存 guard、Work Item evidence、Rust quality gate を保ったまま、installer の機能清算と upgrade / rollback の入口を揃えることである。

## 採用する原則

| 原則 | Sentinel での扱い |
|---|---|
| 目的を先に書く | Contract の `problemStatement` と `intent.problem` で、何を解決する作業かを明示する。製品文脈がない場合は推測しない。 |
| field 順序を固定する | Contract と Summary の field 順序は review 可読性の約束として固定し、無意味な diff を避ける。 |
| scope を実装前に固定する | `scope` は実装後の変更一覧ではなく、編集許可境界として扱う。`outOfScope` には production code、CI、Makefile などを明示できる。 |
| schema より文書を先に固める | 新しい field や guard を追加する前に、review model と運用判断を文書化する。 |
| status は判断信号へ圧縮する | `current_status.md` は Summary の複製ではなく、blocking reason、required checks、changed files、next action を短く示す。 |
| upgrade は active Work Item と分離する | Cockpit runtime や schema の更新は専用 Work Item で行い、Contract、Summary、checkpoint、archive を同じライフサイクルで残す。 |
| Make entrypoint を維持する | Sentinel では `verification[].command` を `make ...` に限定する。template の `verification[].check` 方式は直接持ち込まない。 |

## 既に Sentinel にある機能

| template の概念 | Sentinel の現状 |
|---|---|
| Scope Guard | `make check-ai-scope CONTRACT=...` と `scripts/ai_check_scope.py` で管理している。 |
| File ownership / boundary | `.ai/guards/file_ownership.yaml` と `.ai/guards/file_boundary.yaml` を `make check-ai-guards` で検証している。 |
| Backtrack Guard | `make check-ai-backtrack CONTRACT=... SUMMARY=...` で test、snapshot、i18n、Work Item evidence の無宣言削除を検出する。 |
| Coverage Guard | `make check-ai-coverage-guard` で production Rust code 変更に test 変更証跡を要求する。 |
| Scenario Coverage Guard | `make check-ai-scenario-coverage` で risk 域の verified / unverified / not_applicable を確認する。 |
| Preflight Review | `make ai-preflight`、`make generate-ai-preflight-review`、`make check-ai-preflight-review` で Contract evidence から readiness を導出し、implementation 前の pause を促す。 |
| Status Consistency | `make generate-cockpit-status` と `make check-ai-status`、`make check-ai-status-consistency` で active / no-active 状態を確認する。 |
| Finish Flow | `make ai-finish TASK=...` が required checks 成功時だけ archive する。 |
| Rust quality gate | `make fmt-check`、`make test`、`make clippy` を commit 前 gate として扱う。 |

## 今回の反映結果

| 対象 | 結果 |
|---|---|
| installer catalog | 13 stack、115 script を `scripts/ai_installer_catalog.json` に固定し、`make check-ai-installer-catalog` で各 item の file existence と import を逐項検証する。 |
| runtime script | catalog にある 18 件の Sentinel 固有 core script は保持し、残り 97 件と Make entrypoint の補助 script を追加した。core の `verification[].command` 契約は変更していない。 |
| governance surface | calibration、guard、policy、project、quality、schema、trust の新規 managed file を追加し、既存の同名 file、archive、active evidence、external handoff、recovery receipt、glossary は保持した。 |
| installation facts | `.ai/cockpit/version.json` と `.ai/install/**` を追加し、source commit、distribution version、managed region、rollback baseline を検証可能にした。 |
| default branch | root `Makefile`、`Makefile.ai`、`templates/make/Makefile.ai` の `BASE_BRANCH` default を `develop` に固定した。 |
| business surface | Gate、execution、trader、action matrix、position sizing、Telegram、業務 report、data branch、weekly metrics は変更していない。 |

## Sentinel へ直接持ち込まないもの

| template の要素 | 判断 |
|---|---|
| `verification[].check` ID 方式 | 現行 Sentinel は `verification[].command` と `make` 入口を hard gate にしているため、schema migration なしに混在させない。 |
| managed installer による無条件の core 上書き | Sentinel には repository 固有の guard、AI Cockpit policy、Rust / data branch 境界があるため、installer の一括上書きは使わず、catalog parity と bounded sync で反映する。 |
| template の Contract schema の直接置換 | Sentinel は `verification[].command` と `make` 入口を既存の hard gate として保持し、互換 helper を追加して新 runtime を接続する。 |
| 汎用 cross-language examples | 13 stack の catalog は installer surface として保持するが、Sentinel の project quality command は Rust 用 stack preset を正とする。 |
| template の governance compression 表示面 | 現行 `current_status.md` に Preflight Review の Status / Recommendation / Decision Drivers / Pause Rule を追加し、reviewer visibility と pre-implementation pause を両立する。 |

## 後続候補

| 候補 | 進める条件 |
|---|---|
| Contract / Summary field reference の Sentinel 版 | Contract review で field 意味の解釈ずれが繰り返し起きた場合に、`.ai/cockpit/` または `.ai/README.md` へ追加する。 |
| Status の reviewer guide | `current_status.md` を読む人が増え、ready / risk / blocked の判断基準を短文化する必要が出た場合に追加する。 |
| Scenario coverage | 中高リスク Work Item で未検証 scenario と residual risk を分ける必要が明確になった場合に Summary schema と guard を拡張する。現在は軽量版として採用済み。 |
| Upgrade playbook | 今回の Contract、`.ai/install/**`、`template_feature_parity.json`、`make check-ai-installer-catalog` を基準に、次回更新時も source commit と全 catalog item の差分を記録する。 |

## 運用判断

Sentinel の Cockpit は、template の汎用性よりも repository 固有の安全境界を優先する。特に Gate、execution、trader、action matrix、position sizing、data branch、report output に影響する変更は、template の一般例ではなく Sentinel の Work Item Contract、`.ai/guards/**`、Make target を正とする。

AI Agent は template 由来の便利な field や script を見つけても、既存 Contract schema と guard が許すまで production code や governance runtime へ直接追加しない。必要な場合は、先に Contract に scope、acceptance、verification、agentCapability、残余 risk を書き、`executionDecision` を `continue` にできる状態へ更新する。Scenario Coverage は test list ではなく、risk 域の検証場面として Summary に残す。
