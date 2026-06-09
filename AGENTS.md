---
author: Ray
title: Global User Rules
description: Codex / Antigravity repository-wide language, documentation, AI Cockpit, and commit rules.
key: global-user-rules
---

# Global User Rules

## 1. 言語

### 1.1 ユーザーとの対話

- AI 対話での説明、質問、回答、議論は**中国語**を使う。

### 1.2 code と repository 内 document

- code comment、`///`、JSDoc、TSDoc などの documentation comment は**日本語**で書く。
- repository 内 Markdown（`.md`、README、設計 document、API document、技術説明など）の本文は**日本語**で書く。
- commit message は**日本語**で書く。
- identifier（variable、function、class、file name など）は英語で書き、project 既存規約に従う。

### 1.3 その他の文字列

- log は project の既存方針に合わせる。方針がない場合は、現状の team style に合わせて日本語または英語を使う。
- end user 向け error message は、product の言語方針がない限り日本語を優先する。

### 1.4 例外

- `src/adapters/futu/protocol/generated/**` などの生成 code は、上流 proto comment と code generator comment を保持してよい。手書き code comment の言語規約には含めない。
- snapshot / fixture の Markdown は、出力契約そのものを表すため、front matter と本文言語規約の対象外にできる。
- `reports/**` と `.aider.chat.history.md` は生成 artifact / local tool history として扱い、正式 document の front matter 規約から除外する。
- Skill など固定 header format を持つ file は、その tool format を優先する。format が許す場合は `author: Ray` を追加する。

## 2. Markdown document: Front Matter を必須にする

### 2.1 適用範囲

新規作成または実質的に書き換える次の file は、本章に従う。

- repository 内 `.md`
- `README` 系 file
- 技術 / 設計 / API 説明などの Markdown document

ただし、CHANGELOG / release note、対外 API version document、Skill 固定 format、snapshot / fixture は、それぞれの形式を優先してよい。

### 2.2 必須 Front Matter

適用対象 document の第一行は `---` とし、YAML front matter を置く。必須 field は次の通り。

```yaml
author: Ray
title: <document title>
description: <short summary>
key: <document-key>
```

Author 情報は front matter の `author: Ray` にだけ置く。本文末尾や本文途中に Author / 作者 / 貢献者 block を重複して書かない。

最小例:

```markdown
---
author: Ray
title: ドキュメントタイトル
description: ドキュメントの要約
key: sample-doc
---

# ドキュメントタイトル

本文は日本語で記述する。
```

### 2.3 本文と metadata の分離

- version と変更履歴は Git と hosting platform の history を SSOT とする。
- 通常 document の本文には、手書き version line、Last Updated / 最終更新 / 更新日、version × date × summary の更新履歴表を置かない。
- release note や対外契約として version 表記が必要な API document は例外とする。

## 3. Commit message

Commit message は Conventional Commits 形式を基本とし、subject は日本語で簡潔に書く。

```bash
git commit -m "fix: ログイン画面のバリデーションを修正"
git commit -m "feat: ユーザープロフィール編集機能を追加"
```

## 4. Rust quality gate

Commit 前に少なくとも次を実行し、通過させる。

```bash
make fmt-check
make test
make clippy
```

`make test` は Clippy を代替しない。`clippy::unnecessary_sort_by` のような lint は `make clippy` でのみ検出されるため、必ず別途実行する。

## 5. AI Cockpit 強制プロトコル

Codex または Antigravity 上の AI Agent が code、test、docs、CI、`.ai`、`skills`、`Makefile`、`AGENTS.md`、`GEMINI.md` を変更する場合、`.ai/cockpit/` を作業入口として扱う。

Work Item 化の基準は AI が関与したかではなく、repo diff と review / audit の必要性があるかで判断する。質問への回答、説明、比較、diff を伴わない臨時調査は Work Item にしない。

次の path や種別を変更する場合、Cockpit / Work Item Contract を必須とする。

- `src/**`、`tests/**` などの code / test
- `docs/**`、`README.md`、`AGENTS.md`、`GEMINI.md` などの設計・運用文書
- `scripts/**`、`Makefile` などの checker / command entrypoint
- `.github/workflows/**` などの CI
- `.ai/guards/**`、`.ai/cockpit/**`、`.ai/work-items/**` などの guard / cockpit / work item
- `skills/**` などの Skill / AI 実行手順

必須手順:

1. `.ai/cockpit/README.md` で状態定義と作業可否を確認する。Cockpit 状態と status 生成の machine-readable SSOT は `.ai/cockpit/status_policy.yaml` とする。
2. 現在 task の `.ai/work-items/active/<task>.contract.json` を確認する。存在しない場合は `make ai-start TASK=<task> TITLE="..." MODE=code` で作成する。template は `.ai/work-items/_templates/work_item_contract.example.json` を参照する。
3. Work Item Contract の `mode`、`unknowns`、`notCodable`、`scope`、`outOfScope`、`acceptance`、`verification` を確認する。
4. `notCodable: true` または `unknowns` が残る場合、production code を変更せず、調査、TODO 整理、または blocker 記録に限定する。
5. Contract の `riskAssessment`、`agentCapability`、`executionDecision`、`preReviewWarnings` を確認する。`executionDecision` が `contract_update_required`、`blocked`、`downgraded_to_investigation` の場合、production code を変更せず Contract 更新、調査、TODO 整理、または blocker 記録に切り替える。
6. coding する場合は `mode: code`、`notCodable: false`、`unknowns: []`、`executionDecision: continue` を確認し、`scope` に含まれる範囲だけを変更する。
7. 作業後は `.ai/work-items/active/<task>.summary.json` を更新し、Contract の required checks を `make` 経由で実行する。
8. 必須 check が失敗した状態で `ready_for_review` と報告しない。required checks が通過しても残余 risk がある場合は、`ready_with_risks` と `expectedReviewFocus` を Summary に記録する。


### 5.1 Work Item 境界 checklist

新しい Work Item を実装する前に、次の境界を Contract の acceptance または sources に明記する。

- Gate / execution / trader / action matrix / position sizing に影響するか。影響しない場合は表示・監査専用と明記する。
- Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。表示しない場合も明記する。
- data branch、reports、JSONL、snapshot、weekly metrics のどこへ保存するか。全文保存と構造化 record を混同しない。
- zh / en / ja の i18n、snapshot、contract test が必要か。単一言語 report に別言語の設定文を混入させない。
- fact、manual observation、hypothesis、fixture、local cache を区別し、Reality Layer と Hypothesis Layer を混ぜない。
- 新しい検証や運用入口は `make` target として提供する。裸の script command を運用手順にしない。
- data branch は data-only branch とし、code tree、AI governance、local temporary cache を持ち込まない。認知校正の長期比較は週次粒度を標準とする。
- Agent が実装できるか、検証できるか、人間判断が必要かを `agentCapability` に明記する。
- review で追加論点が出る可能性がある場合は、`preReviewWarnings` と Summary の `expectedReviewFocus` に先に記録する。
- Contract 内で危険を認識した場合は、`executionDecision` を `contract_update_required`、`blocked`、または `downgraded_to_investigation` にして止める。実装を続けながら後で Summary だけに risk を書かない。

作業後の Summary には、code / test / docs / i18n / report output / data or weekly record / Make or CI guard の確認結果を残す。未確認項目は未確認として書き、完了を過大に表現しない。required checks 通過後も残る risk は `residualRisks` に分け、review で確認すべき観点は `reviewReadiness.expectedReviewFocus` に記録する。

User correction が発生した場合は、修正だけで終えず、同種の回帰を防ぐために Contract、Summary、document、template、guard、skill のどこへ固化するかを `userCorrectionSolidification` に記録する。

標準 command:

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make fmt-check
make check-ai-guards CONTRACT=.ai/work-items/active/<task>.contract.json
make check-architecture-all
make check-ai-backtrack CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-coverage-guard
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status-consistency
make ai-preflight
make quality
```

`make test-ai-guards` は `.ai/**/*.yaml` の parse guard を含み、`make check-ai` と `make quality` から実行される。Work Item 文脈では `CONTRACT` と `SUMMARY` を渡す command を優先し、裸の `make quality` だけで Cockpit status consistency を代替しない。

Contract / Summary の `verification[].command` は `make ...` 形式だけを許可する。`python3 scripts/...`、`cargo ...`、`bash ...`、`git ...` などの裸 command が必要になった場合は、先に Makefile target を追加してから、その `make` target を verification に記録する。

Work Item を完了する時は次を使う。

```bash
make ai-finish TASK=<task>
```

`make ai-finish` は required checks を再実行し、成功した場合だけ Work Item を archive する。

Skill と Cockpit の内容が衝突する場合は、Work Item Contract の `scope`、`outOfScope`、`unknowns`、`notCodable`、`verification` を優先し、必要なら作業を止めて blocker として報告する。

## 6. 核心原則

ユーザーとの対話は中国語、repository 内の手書き comment と document 本文は日本語、identifier は英語、metadata と履歴は front matter と Git に分離する。
