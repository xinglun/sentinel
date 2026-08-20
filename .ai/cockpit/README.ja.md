---
author: Ray
title: "AI Cockpit"
description: AI Cockpit ワークスペース概要とワークフローガイド（日本語）。
keywords:
  - ai-cockpit
  - work-item-contract
  - scope-guard
  - change-summary
  - cockpit-status
---

# AI Cockpit

[English](README.md)

AI Cockpit は、エージェント型開発のための協調エンジニアリング環境です。Codex、Gemini、Claude、Cursor、Antigravity などのコーディングエージェントに、ファイル変更前の共通運用契約を提供します。

コックピットは言語非依存です。明示的なスコープ、委譲されたチェック、レビュー証跡、監査可能なタスク記録を通じて AI Change Governance を提供し、Makefile はプロジェクト固有のチェックを各リポジトリでカスタマイズ可能なコマンドへ委譲します。

## コアファイル

- `checks.yaml`: チェックカタログとプロジェクト固有コマンドの選択指針。
- `current_status.md`: アクティブ Work Item の生成済みステータスビュー。
- `.ai/work-items/active/*.contract.json`: 作業開始前のタスク境界。
- `.ai/work-items/active/*.summary.json`: 完了前の変更レポート。
- `.ai/guards/*.yaml`: ファイル所有権、境界、スコープ、バックトラック、カバレッジルール。

## フロー

### リポジトリの役割とレビュー単位
既定のレビュー単位は「1 Work Item、1 専用作業ブランチ、1 Pull/Merge Request」です。無関係な Work Item を同じブランチや PR に混在させないでください。
ブランチの起点はリポジトリの役割で決まります。

- テンプレートリポジトリでは、最新の `origin/main` から保守ブランチを作成する。
- 導入先プロジェクトでは、そのプロジェクト自身のリモート既定ブランチの最新コミットから作成する。`origin/main` を仮定せず、リモート名とブランチ名を確認し、Work Item に `baseRemote`、`baseBranch`、`baseCommit` を記録する。
インストールとアップグレードの変更履歴は導入先プロジェクト側に属します。移動するテンプレート作業ブランチではなく、公開済みテンプレートのリリースタグを使用してください。PR のマージ後は、明示的な復旧例外を除き、リモートとローカルの作業ブランチを削除します。

### ライフサイクルのクローズ

Work Item を archive し、対応する PR が merge された後に
`make ai-close-work-item TASK=<task>` を実行します。local Work Item Head と
merge 済み PR Head SHA を束縛し、base を同期・検証し、remote Work Item
branch の不在を証明してから local の再試行 identity を削除します。remote
削除が失敗した場合は Work Item checkout を保持または復元します。
`ready_on_base` は実行元 worktree が clean な同期済み base 上にある場合だけ
報告します。別の worktree が base を所有する場合は
`closed_but_current_worktree_detached` を報告し、クローズ済みでも実行元では
次の Work Item を開始できないことを明示します。

Contract が未公開 commit では実行できない hosted verification を明示的に
要求する場合に限り、Finish 前の測定 stage を利用できます。実装とローカル検証を
完了し、人間の明示的な承認を得てローカル snapshot commit を作成した後、次を
実行します。

```text
make ai-prepare-hosted-verification-snapshot \
  CONTRACT=.ai/work-items/active/<task>.contract.json
```

このコマンドは、clean で commit 済みの専用 branch、active な v2 Contract、
未完了の登録済み hosted evidence、正しい Contract baseline、成功したローカル
`make quality` session を要求します。`target/` に commit、tree、branch、base、
Contract/Summary digest を束縛した receipt を出力しますが、commit、push、PR
作成、merge、release、archive、close、branch 変更は実行しません。receipt が
示す次の適格な操作は、その正確な branch を hosted 測定のためだけに push する
ことですが、receipt 自体は人間の承認を提供しません。
release/publication intent、archive 済み Work Item、完了済み hosted
evidence、dirty/detached state、base branch、quality failure では fail closed
です。hosted 結果を active Summary に記録した後、通常の
`ai-finish`/archive、final push、PR、merge、`ai-close-work-item`、cleanup に
戻ります。

`ai-close-work-item` は worktree を考慮します。base branch が別の
worktree で checkout 済みの場合、その worktree で base を検証・同期し、
remote Work Item branch の不在を証明してから、実行元 Work Item worktree を
detached にして local branch を削除します。local 削除が失敗した場合は、可能な
限り Work Item checkout を復元します。base worktree 自体は削除せず、次の
Work Item はコマンドが表示するその path から開始します。過去の archive 証跡は
保持します。

停止中の Work Item を corrective predecessor のクローズ後に再開する場合は、専用
branch を取得済みの最新 remote default branch へ rebase し、
`predecessorWorkItem` を完了済み closure 証跡へ更新してから、次を実行します。

```text
make ai-resume-work-item \
  CONTRACT=.ai/work-items/active/<task>.contract.json \
  BASE_REMOTE=<remote> BASE_BRANCH=<default-branch>
```

このコマンドは元の Start Receipt を変更せず、source-bound な
`resumeHistory` transition を追加してから Contract の baseline を進めます。旧
base が正確な predecessor merge の祖先であること、現在の branch が開始時と同じ
専用 Work Item branch であること、predecessor の archive manifest と closure
facts が有効であることを確認できない場合は fail closed です。Start Receipt、
`baseCommit`、`resumeHistory` を手で編集してはいけません。再開後は Preflight
をやり直し、古くなった検証をすべて再実行します。

完了済み corrective predecessor がないまま active な専用 Work Item を最新 remote
default branch へ rebase する必要がある場合、手作業の rebase は禁止です。先に
fetch してから次を実行します。

```text
make ai-synchronize-work-item \
  CONTRACT=.ai/work-items/active/<task>.contract.json \
  BASE_REMOTE=<remote> BASE_BRANCH=<default-branch> \
  TARGET_ROOT=<target-worktree-root>
```

tracking ref が stale なら fail closed です。この command は専用 branch、active
Contract/Summary、immutable Start Receipt を検証します。clean な worktree は直接
rebase します。dirty な active Work Item は Contract の明示的な
`synchronizationCheckpoint` authorization と、全 dirty path の Contract ownership が
ある場合だけ、recorded local checkpoint を作ってから rebase できます。checkpoint
identity、path、old/new base は digest-bound `synchronizationHistory` に記録され、
既存 verification は無効化されます。push、force-push、PR、provider、archive、Start
Receipt の変更権限はありません。conflict は自動 abort し、recoverable checkpoint は
synchronization 成功を主張しません。Finish 前に Preflight と必要な全 check を再実行します。

対象 checkout 内から実行する場合の `TARGET_ROOT` は optional です。別の caller が
worktree を統制する場合は必須であり、command は Contract、Summary、Git facts と許可された
すべての evidence write をその target root 内だけで解決します。caller 側の active Work Item
evidence へ fallback しません。

1. `make ai-start TASK=<task> TITLE="..." MODE=code` で Work Item を作成する。
2. Contract の `scope`、`sources`、`acceptance`、`verification`、リスク評価、エージェント能力、実行判断を明確にする。
3. 宣言したスコープ内のみ実装する。
4. Summary に変更ファイル、チェック結果、リスク、レビュー準備、境界チェック、既知ギャップ、破壊的変更を記録する。
5. `make ai-finish TASK=<task> REPORT_LANGUAGE=<conversation-locale>` を実行する。
6. 生成されたステータスとアーカイブ済み Contract/Summary をレビューする。

前置フローで導入準備状況を先に確認したい場合は `make ai-preflight` を実行してください。
このターゲットは Preflight Review を生成してから検証します。既定の enforced policy では、`needs_human_confirmation`、`human_decision_recorded`、`not_ready` は失敗になります。互換動作が必要なリポジトリだけが `profile: advisory`、`gateEnabled: false`、`blockedStatuses: []` を明示して advisory policy を選択できます。advisory mode は正式な Trust Layer の証明ではありません。
`make generate-ai-preflight-review` は検証を行わずにレポートだけ生成したい場合に使えます。
`make check-ai-preflight-review` は生成済みレポートの構造を検証し、policy が有効な場合のみ gate として動作します。

V2.6.5 では Preflight Review を追加します。原則は **Evidence over Self-Declaration** で、実装可否は AI の自信ではなく Contract の証拠から派生させます。`make ai-start TASK=<task> TITLE="..." MODE=code` と `make ai-preflight` は実装前にこのレビューを表示します。既定は enforced profile であり、`needs_human_confirmation`、`human_decision_recorded`、`not_ready` の場合、ガバナンス経路を停止します。

中高リスクの code Work Item で実装後にしか実行できない必須シナリオは、Contract に空でない期待結果と空でない `verificationPlan` の両方が記録されている場合だけ、実装開始の準備完了と判定できます。このとき Preflight 証拠が示すのは「検証予定」であり「検証完了」ではありません。この遷移が許可するのは実装だけです。Summary の Scenario Coverage Guard と `ai-finish` は実行済み証拠を引き続き必須とし、必須シナリオが `unverified` の間は fail closed を維持します。

`notCodable: true`、`executionDecision.status` が `block` / `defer` / `needs_human_decision`、または実装不可・検証不可・人間判断要求を示す `agentCapability` は、直接 `not_ready` を導く明示的ブロッカーです。

`unknowns` や `notCodable` は失敗ではなく有効な出力です。Summary は監査記録であると同時に協働の引き継ぎです。checkpoint は長いタスクのドリフトを防ぐための環境支援であり、単なる遵守項目ではありません。

`current_status.md` は生成物です。手編集しないでください。

`make ai-lifecycle-facts` は、決定的で読み取り専用のライフサイクル事実を JSON として出力します。Bootstrap、Calibration、Governed Development、No Active Work Item とアクティブ Work Item 数を示します。`readiness` と `enterpriseAssurance` は `not_claimed`、プロバイダー資産と外部エンタープライズ保証は `not_run` のままです。

Complexity Policy の変更も同じ境界に従います。提案は policy state が明示的に `confirmed` となり、レビュー証拠が揃うまで有効化しません。予算増加には返済記録を必須とし、記録の欠落や古さは Allow ではなく blocking として扱います。

## 初回導入のショートカット

インストール後の設定は、個別コマンドの羅列ではなく次の 3 フェーズに整理できます。

```sh
make ai-onboard              # 環境 → キャリブレーション → 導入準備
make ai-onboard PHASE=1      # 環境確認のみ
make ai-onboard PHASE=2      # キャリブレーションのみ
make ai-onboard PHASE=3      # 導入準備のみ
```

詳細なチェックリストは [導入準備ガイド](adoption.ja.md) を参照してください。

## ライフサイクルチェック

`make ai-start` は新しい skeleton 作成前にライフサイクル preflight を実行します。アクティブ Contract/Summary が不整合、複数 Work Item が同時アクティブ、`current_status.md` が実状態と不一致の場合は開始を拒否します。
`MODE=code` ではさらに `make ai-preflight` を実行し、実装開始前に Preflight Review を表示します。`needs_human_confirmation`、`human_decision_recorded`、`not_ready` の場合、エージェントはここで停止し、レビュー内容をユーザーへ報告します。新しく再計算された `ready` の証拠が得られるまで、実装または finish へ進めません。

Cockpit Status はレビュアー向けに Preflight Review を見えるままに保ちますが、実装前の pause の代わりにはなりません。

`current_status.md` を生成または検証した後、Work Item を完了せずにライフサイクル状態だけ確認する場合は `make check-ai-status-consistency` を実行します。

### blocked Work Item の successor route

Work Item に red の active `blocked` Outcome があり、是正 successor または quarantine についてユーザー権限が記録されている場合、receipt を手作業で作成せず、次のガバナンス済みコマンドを使用します。

```sh
make ai-transition-to-successor \
  PREDECESSOR_TASK=<blocked-task> \
  SUCCESSOR_TASK=<distinct-successor-task> \
  SUCCESSOR_BRANCH=codex/<distinct-successor-task> \
  SUCCESSOR_BASE=<40-character-base-sha> \
  ISSUE=https://github.com/spirex-ds-dev/ai-cockpit-template/issues/<number> \
  AUTHORITY='<recorded human authority>' \
  MODE=quarantined \
  REASON='<specific corrective reason>'
```

このコマンドは、blocked Outcome とその digest、相互に異なる identity、リポジトリ Issue、authority、reason、mode、receipt path を検証してから、唯一の束縛済み successor receipt を書き込みます。Status と doctor は有効な route を yellow と表示しますが、predecessor は独立に解消されるまで red / blocked のままです。receipt は archive、merge、release、branch 削除、provider mutation、または predecessor evidence の書き換えを許可しません。

### active Work Item の retry と successor の境界

Contract と scope が同じ delivery を表したままで、修正が active な schema/evidence
（例: `before_finish` checkpoint や Summary evidence field の不足）に限られる場合は、
同じ Work Item 内で retry します。すべての blocked Outcome を保持し、修正した evidence を
append して必要な check を再実行します。この場合に別 Issue や successor を作成してはいけません。

changed base から delivery を再開する場合、active Contract/scope が無効化された場合、または
immutable な failed-delivery evidence を独立に再 delivery する場合にだけ、上記の
governed successor/quarantine route を使用します。いずれの場合も predecessor Outcome の
書き換えは許可されません。

`ai-finish` は失敗した checkpoint に対応する正規の recovery を表示します。`before_finish`
record がない場合は `make ai-checkpoint CONTRACT=<contract> SUMMARY=<summary> STAGE=before_finish`、
immutable な `before_edit` Contract binding が stale の場合は append-only の
`make ai-revalidate-contract-amendment` を使います。どちらも validation や Outcome emission を
bypass しません。

アクティブ Work Item が 0 件、または 1 組の Contract/Summary ペアだけの場合、`make repair-ai-status` で `current_status.md` を再生成できます。不整合ファイルや複数アクティブ Work Item の修復は含みません。

archive 後の状態は `no_active_work_item` です。これは worktree が clean である意味ではなく、no-active status はファイル一覧を保存しません。最初の archive bundle commit 前に限り、現在変更中の同一 Work Item の Contract、Summary、manifest、index 更新、Start Receipt が揃い、manifest が正確な archive pair を束縛し、すべての live path が archived Summary の `changedFiles` に記録されている場合だけ、同じ transaction として扱います。Summary にない path、無関係な変更、孤立した receipt、履歴にしか存在しない pair、不完全な pair、不正な Summary、manifest の不一致は引き続き fail closed です。完全な archive bundle を先に commit し、アーカイブ証跡と完全 PR diff の所有権を `make check-ai-pr AI_BASE_COMMIT=<merge-base>` で検証します。`repair-ai-status` は有効な 0 件または 1 組の active 状態で stale な Status 表示を再生成できますが、live change の所有権は作れません。archive transaction 外の path が報告された場合は repair を繰り返さず、変更を戻すか Work Item を作成・再開します。

`make check-ai-diff-ownership` は早期の読み取り専用 Preview です。`AI_BASE_COMMIT` なしでは未追跡ファイルを含むローカル diff を、指定時には PR diff を検査し、PR audit と同じく今回追加された archive pair だけを使用します。PR audit は重複する archive claim を決定的に解決し、最後に有効だった archive pair を採用します。`make ai-pre-merge AI_BASE_COMMIT=<merge-base>` は品質、lifecycle、Preview、最終 PR audit を順に表示し、いずれかが失敗すれば merge 不可です。

## エージェントリスク制御

AI Cockpit はプロンプト指示をガイダンスとして扱い、強制力とはみなしません。リポジトリの安全性は、実際の Work Item と diff を検査するハードゲートから得られます。

既定テンプレートは 3 つの一般的なエージェントリスクを次の制御へ対応付けます。

- プロンプトは助言にすぎない: `make check-ai-agent-risk` が Contract の必須 AI ゲートが verification に含まれ、Summary で passed であることを検証する。
- 作業中のドリフト: `make ai-checkpoint` がスコープ、スコープ外ファイル、unknowns、acceptance、必須チェック状態、レビュー注視点、次アクション、checkpoint メタデータを表示する。
- 不確実性の過大主張: Contract 検証と Agent Risk Guard が unknowns または `notCodable` 状態で非 coding の execution decision を要求する。

Contract の `checkpointPolicy.requiredBeforeFinish` が true の場合、完了前に Summary の `checkpointEvidence` に checkpoint 使用を記録する。

次の概念は分けて扱う:

- `unknowns`: 未解決の事実や設計上の疑問。
- `scenarioCoverage`: verified / unverified / not_applicable で表す既知シナリオ。
- `residualRisks`: 実装後も reviewer が受け入れる残存リスク。
- `followUps`: 現在の Work Item では解決しないが追跡が必要な後続作業。
- `unverifiedScenarios`: 検証未完了のシナリオ名。

## Governance Compression

V2.5 では、Repository Truth が確立された後にもう 1 層が追加されます。V2.6 ではそこに Scenario Coverage が追加されます。

```text
Summary (Repository Truth) → Cockpit (Governance Compression) → Human Decision
```

Cockpit は Summary を複製しません。Contract、Summary、verification の証拠を圧縮して、レビュー担当者や保守者が判断しやすい状態を示します。

`current_status.md` は次の項目を表します。

- `Recommendation`
- `Governance Signals`
- `Evidence`
- `Decision Drivers`

これらの項目は説明可能で保守的であるべきです。証拠が欠けている場合、それを楽観的な結果に置き換えてはいけません。

V2.6 では、中高リスク Work Item 向けに通用的な `Scenario Coverage` 信号が追加されます。`complete`、`incomplete`、`not_required`、`unknown` を区別しますが、release/auth/installer などのシナリオ集を Core に埋め込みません。シナリオ内容は Work Item が保持し、Cockpit は証拠をレビュー向けに圧縮するだけです。

## レビュー準備

Contract の readiness フィールドは、コーディング開始前にエージェントが実装と検証を実行できるかを記録します。Summary の readiness フィールドは残留リスク、期待レビュー注視点、境界チェック、ユーザー修正、既知ギャップ、未検証の主張を記録します。

このテンプレートを他リポジトリへコピーする場合、これらのフィールドは言語中立に保つ。

`.ai/guards/ai_review_policy.yaml` で宣言されたガバナンス機微パスについては `make check-ai-review-policy SUMMARY=<summary.json>` を実行する。このチェックは報告のみで、Summary に `reviewReadiness.expectedReviewFocus` があるかを記録する。

アーカイブ後、PR CI は `make check-ai-pr AI_BASE_COMMIT=<merge-base>` を実行する。インストール済み配布物にはこのターゲットと検証器が含まれる。PR diff 全体の非免除パスは、ちょうど 1 つの変更済みアーカイブペアによって所有されなければならない。Contract の scope 内かつ outOfScope 外であり、対応 Summary に報告されていること。

PR 証跡には Contract version 2 が必要である。version 1 はレガシー読み取り専用で、新規 PR 証跡として導入できない。Contract の承認フィールドは自己申告記録であり、人間 ID の証明ではない。信頼できる承認には保護されたプラットフォームレビューを使い、ガバナンス PR チェックとは独立してプロジェクトテストを実行する。

Summary は Repository Truth、Cockpit は Human Decision State です。Cockpit は事実を増やさず、レビュー可否、ブロック、調査要否を判断するための圧縮された信号だけを示します。
