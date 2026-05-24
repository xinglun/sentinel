---
author: Ray
title: DDD Clean Architecture 移行設計
description: Sentinel を DDD と Clean Architecture に段階移行するための依存方向、境界、検証ルール。
key: ddd-clean-architecture-migration
---

# DDD Clean Architecture 移行設計

この文書は Sentinel を長期的に健全に拡張するための target architecture を定義する。
現在の policy kernel は `src/features/radar/application/policy/**` と `src/features/radar/application/engine.rs` に収める。新規実装と段階移行はこの文書の依存方向に従う。

## 目的

- 業務概念を Domain に閉じ込める。
- CLI、Telegram、外部 API、永続化を Domain から分離する。
- AI Agent が変更してよい範囲を機械的に検証できる状態にする。
- 既存挙動を壊さず、feature 単位で移行できる architecture seam を作る。

## Target Feature Structure

```text
src/features/<feature>/domain          feature 固有の業務概念、値オブジェクト、エンティティ、ドメインサービス
src/features/<feature>/application     ユースケース、ポート、トランザクション境界、アプリケーションサービス
src/features/<feature>/interface       CLI / report / Telegram などの入出力変換
src/features/<feature>/infrastructure  外部 API、永続化、通知、時計、ファイルシステムなどの実装
src/features/<feature>/acl             外部 adapter / raw protocol と feature 内 model の防腐層
src/features/shared/domain             複数 feature で共有する domain primitive
src/features/radar/application/policy  既存 policy kernel を含む radar application policy
```

## Dependency Direction

依存方向は次だけを許可する。

```text
feature interface -> feature application -> feature domain
feature infrastructure -> feature application -> feature domain
feature infrastructure -> feature acl -> external adapter
feature domain -> features/shared/domain
feature domain -> same feature domain
```

Domain は最内層であり、次を参照してはならない。

- CLI
- report / Telegram rendering
- config loading
- filesystem / network / external API
- infrastructure adapter
- legacy presentation implementation

Application は Domain を操作し、外部との接点は port trait で表現する。Infrastructure はその port を実装する。

## Bounded Contexts

Sentinel の主要 bounded context は次とする。

| Context | Responsibility |
| --- | --- |
| Market Observation | price、breadth、regime、breakout の観測 |
| Decision Policy | Gate、NO TRADE、risk policy、position policy |
| Evidence | substantive evidence、hypothesis evidence、source records |
| Reporting | Markdown、Telegram、audit daily、weekly review |
| Calibration | cognitive yield、thesis registry、daily calibration |
| Automation | CI、scheduled radar、run status、notification lifecycle |

## Legacy Migration Rule

root-level legacy layer は再導入しない。代わりに次の規則で縮小する。

1. 新規 domain concept は `src/features/<feature>/domain/**` に作る。
2. 複数 feature で共有する primitive は `src/features/shared/domain/**` に作る。
3. 新規 use case は `src/features/<feature>/application/**` に作る。
4. 新規 external adapter 連携は `src/features/<feature>/acl/**` または `src/features/<feature>/infrastructure/**` に作る。
5. 新規 output formatting は `src/features/<feature>/interface/**` に作る。
6. legacy module を変更する場合は、Work Item に「なぜまだ legacy 側で変更するか」を記録する。
7. 移行済み concept は legacy 側へ逆流させない。

## Anti-Corruption Boundary

外部 source 由来の data は Domain に直接入れない。

- Finnhub / SEC / web HTML は infrastructure DTO として受ける。
- Application port で domain input へ変換する。
- Domain は source URL、HTTP status、raw JSON などを直接扱わない。

## Display-Only Layers

Strategic Context、Hypothesis Layer、Cognitive Yield、Macro Gravity などの表示専用 layer は、Gate や execution に依存させない。
表示専用 layer が必要な data は Application で ViewModel input として組み立て、Domain policy を上書きしてはならない。

## Radar Orchestration Boundary

Radar の段階移行では、CLI から次の policy を Application へ移した。

- data acquisition の成功 / 失敗集約
- pipeline body へ入るかどうかの判定
- decision history を保存するかどうかの判定
- data quality status の導出
- run context の `save_dir`、`date`、`timestamp`、初期 `RunOutcome` 生成
- diagnostic packet、decision outcome、state machine summary、persistence payload の組み立て

CLI はまだ provider 呼び出し、legacy `Engine` 実行、report rendering、notification dispatch を保持する。
これは移行期間の adapter / composition root として許容するが、新しい orchestration policy は `src/features/radar/application/radar.rs` に追加する。

次の領域は今回の migration checkpoint では変更しない。

- `Engine::run_daily_pipeline`
- `PresentationAssembler`
- Telegram / Markdown rendering
- `PersistenceLayer` の実装
- market data provider trait

この境界により、Radar は Big Bang rewrite ではなく、Application use case を厚くしながら CLI を薄くする方向へ進める。

### Radar Migration Checkpoint

現時点の Radar は、Application layer が orchestration policy を保持し、CLI が composition root として外部依存を接続する状態で収束する。

Application layer に移行済みのもの:

- fetch result の成功 / 失敗集約
- prepared data、pipeline plan、history persistence 判定
- run context と初期 run status metadata
- diagnostic / decision outcome / persistence payload builder
- market data 取得、decision history 永続化、notification dispatch の port contract

CLI に残すもの:

- config loading
- market data provider の非同期呼び出し
- `PersistenceLayer`、`Ledger`、`EvidenceStore` の生成
- legacy `Engine` 実行
- report rendering と Telegram dispatch

次に進める場合は、`src/features/<feature>/application` から `crate::data`、root compatibility layer、`crate::adapters` へ依存させず、port trait を先に定義してから infrastructure 実装を接続する。
この rule は `make test-architecture-boundaries` の regression test で固定する。
port contract は `RadarMarketDataPort`、`RadarDecisionHistoryPort`、`RadarNotificationPort` として application layer に置き、実装接続は別 Work Item で行う。

## Architecture Guard

`make check-architecture` は新規 target directories の依存違反を検出する。
root-level `src/core/**` は廃止済みであり、target directories から legacy presentation / CLI / adapter へ依存することは禁止する。

## Definition of Done

DDD / Clean Architecture 移行 task は次を満たす。

- 変更対象の bounded context が明記されている。
- Domain / Application / Interface / Infrastructure の責務が分離されている。
- Domain は IO、config、report、CLI に依存していない。
- `make fmt-check`、`make check-architecture`、`make test-architecture-boundaries`、`make quality` が通る。
- 既存 report / Telegram / audit output の契約が必要に応じて更新されている。
