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

Codex または Antigravity 上の AI Agent が code、test、docs、CI、`.ai`、`skills`、`Makefile`、`AGENTS.md`、`GEMINI.md` を変更する場合、インストール済み `ai-cockpit` を作業入口として扱う。Work Item 化の基準は AI が関与したかではなく、repo diff と review / audit の必要性で判断する。

必須手順:

1. `.ai/cockpit/README.md` と `ai-cockpit inspect --repo .` で Runtime、repository identity、状態を確認する。
2. 現在 task の `.ai/work-items/active/<task>.contract.json` を確認する。存在しない場合は `ai-cockpit work-item new --repo . --id <task> --mode code` で作成する。
3. `ai-cockpit start` で intent、goal、scope、outOfScope、authority、acceptance、verification を確定する。
4. Work Item Contract の `mode`、`unknowns`、`notCodable`、`scope`、`outOfScope`、`acceptanceCriteria`、`verification`、`agentCapability`、`executionDecision` を確認する。
5. `notCodable: true`、unknowns、`executionDecision` の停止値、または検証不能が残る場合は production code を変更しない。
6. `ai-cockpit preflight` の結果を確認し、`ai-cockpit checkpoint` を編集前に実行する。高リスク・破壊的変更では authority、approval evidence、scenario coverage を Contract に記録する。
7. scope に含まれる範囲だけを変更し、既存の project code、data、evidence、履歴 Work Item を無断で削除しない。
8. 完了時は `ai-cockpit verify` で Contract の required `verification[].check` を `make` 経由で実行し、`finish`、`archive` を順に実行する。
9. required check が失敗した状態で `ready_for_review` と報告しない。残余 risk がある場合は Summary の `residualRisks` と `expectedReviewFocus` に分けて記録する。
10. `close` は人間の最終判断、authority source、evidence ref が必要な場合だけ実行する。

### 5.1 Work Item 境界 checklist

新しい Work Item を実装する前に、次の境界を Contract の scope、acceptance、sources、verification に明記する。

- Gate / execution / trader / action matrix / position sizing に影響するか。影響しない場合は表示・監査専用と明記する。
- Telegram、Markdown、CLI、audit daily、weekly review のどこに表示するか。
- data branch、reports、JSONL、snapshot、weekly metrics のどこへ保存するか。
- zh / en / ja の i18n、snapshot、contract test が必要か。
- fact、manual observation、hypothesis、fixture、local cache を区別する。
- 新しい検証や運用入口は根 `Makefile` の `make` target として提供する。
- Agent が実装・検証できるか、人間判断が必要かを `agentCapability` に明記する。
- review で追加論点が出る場合は `preReviewWarnings` と Summary の `expectedReviewFocus` に記録する。
- 重要判断を会話文脈だけに残さず、Contract、Summary、checkpoint evidence に戻す。

作業後の Summary には、code / test / docs / i18n / report output / data or weekly record / Make or CI guard の確認結果を残す。未確認項目は未確認として書き、required checks 通過後も残る risk は `residualRisks` に分ける。User correction が発生した場合は `userCorrectionSolidification` に固化先を記録する。

標準 command:

```bash
ai-cockpit inspect --repo .
ai-cockpit status --repo .
ai-cockpit doctor --repo .
ai-cockpit compatibility --repo .
ai-cockpit preflight --repo . --contract .ai/work-items/active/<task>.contract.json
ai-cockpit checkpoint --repo . --id <task>
ai-cockpit verify --repo . --work-item <task> --command make --args quality
ai-cockpit finish --repo . --id <task>
ai-cockpit archive --repo . --id <task>
make fmt-check
make test
make clippy
make quality
```

`verification` は installed Contract の `check` 宣言を使い、裸の `python3 scripts/ai_*.py` や旧 `make ai-*` を新しい運用手順に追加しない。Contract の scope、outOfScope、guard、backtrack、status の判断はインストール版 Runtime の protocol と repository-local 設定に従う。
## 6. 核心原則

ユーザーとの対話は中国語、repository 内の手書き comment と document 本文は日本語、identifier は英語、metadata と履歴は front matter と Git に分離する。
<!-- AI_COCKPIT_ADAPTER_BEGIN provider=codex adapterVersion=1 repositoryId=sha256:77b40b1960f2a5251724cfa3b5591e58f15e1439153e8e6cb2610232a709f350 -->

This repository is attached to AI Cockpit.

Canonical interface: .ai/agent-interface.json
Read .ai/README.md before acting; read .ai/glossary.md for the repository-local Agent route and vocabulary.

Use the installed shared Rust Runtime as the repository-governance interface.
Every repository-bound command must include an explicit --repo <path>.
Prefer MCP when available; CLI remains the fallback. Do not infer AI Cockpit state from this file. Query the Runtime for current governance state.

Before editing, query inspect, status, doctor, and agent doctor. Use one bounded Work Item, branch, and worktree. Keep all edits inside the Contract scope; amend and re-run preflight before expanding it.

Contract first: intent, scope, outOfScope, sources, unknowns, acceptance criteria, verification, and authority are human-owned. For code mode, unresolved unknowns or notCodable conditions stop implementation. Do not invent intent, approval, evidence, or completion.

A preflight result of not_ready or needs_human_confirmation is a mandatory human pause. Show the humanDecisionRequest and resume condition; a successful command or yellow result is not authorization.

For authorized changes use: start or work-item new → preflight → checkpoint → verify → finish → archive → close. Keep the Summary current with changed paths and reasons, sources, verification commands/results, guideline compliance, unknowns, risk, generated/destructive changes, and observed issues.

Before archive, present a visible human Outcome with 🟢/🟡/🔴, facts, unknowns, evidence, human decision, and next action. A raw MCP record or folded-only output is not a human handoff. Close only after the merged PR, archive, decision, default-branch synchronization, clean worktrees, and exact branch removal are verified.

Canonical delivery order is latest remote default base → dedicated branch/worktree → implement → finish/archive → push → reviewed PR → merge → close → synchronize and clean. Never merge a feature branch into local main before PR review, delete its branch before merge, or let a provider auto-delete it to bypass finalization. If a remote step fails, preserve the retry checkout and identity until recovery is complete.

A terminal green Outcome is the Rust equivalent of status=completed plus humanStatusColor=green: it requires state=Verified, decisionState=green, current Contract/Summary/evidence bindings, and direct human-visible delivery. Include issue count, blockers/stopping reason, resolved issues, risks, unknowns, verification, impact, human decision, and next action; every factual claim needs evidence, and unproven benefit is an inference.

When a defect is found in the current Work Item, repair it there by amending and revalidating its Contract before opening another Work Item or Issue. A successor is allowed only for a genuinely different scope, authority, or base, an independent compatible change, an unsafe in-scope repair, immutable failed delivery, or explicit human direction.

Never edit global Agent or MCP configuration, secrets, or credentials. Do not copy V1 runtime code, Python modules, Make commands, installers, or schemas into this repository.

<!-- AI_COCKPIT_ADAPTER_END -->
