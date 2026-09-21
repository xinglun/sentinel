---
author: Ray
title: Sentinel acceptance-replay 実装计划
description: immutable input、isolated computation、provenance、verification receipt を段階的に実装する計画
key: sentinel-acceptance-replay-plan
---

# Sentinel acceptance-replay 実装計画

## 進め方

1. Contract、既存の generate/resend runner、run status、CLI、workflow を再確認し、変更境界を固定する。
2. TDD で focused acceptance tests を先に追加し、Warsh negative、missile strike positive、ambiguous strike/attack negative、manifest fail-closed、canonical hash 不変、decision projection 不変を red で確認する。
3. immutable input manifest と isolated output contract を追加する。入力は report date、revision、digest が一致しなければ受け付けない。
4. side-effectful canonical runner と分離した acceptance-replay application/interface path を追加する。replay path は canonical snapshot/data/freshness/promotion capability を持たない。
5. `GENERATED` lifecycle と `ACCEPTANCE_REPLAY` purpose、`ISOLATED` publication scope、execution revision、input digest、before/after digest を run status に投影する。
6. `sentinel acceptance-replay` CLI と専用 workflow dispatch を追加する。既存 daily workflow の `generate` と `resend` 分岐は変更しない。
7. verification manifest と machine-readable receipt を生成し、optional notification は isolated generated payload だけを入力にできるようにする。
8. focused tests、workspace tests、fmt、clippy、quality、workflow contract checks を実行し、Contract Summary の scenario coverage と evidence を更新する。

## 検証順序

- `cargo test acceptance_replay`
- `cargo test --workspace`
- `make fmt-check`
- `make test`
- `make clippy`
- `make quality`
- acceptance-replay の isolated fixture 実行（canonical before/after digest、receipt、Warsh assertion）
- Runtime `verify`、`finish`、`archive` と review/PR/merge 後の finalization/close

## 完了条件

実行結果に新しい generated isolated payload と verification receipt があり、同じ report date の canonical snapshot、data branch、freshness marker、Decision、Gate、Position Sizing が変化していないことを直接確認できる。Warsh negative と実戦的 geopolitical positive が同一の current code path で PASS し、入力不在または不一致は fail-closed になる。
