---
author: Ray
title: Signal Context 表示・観測整合性 実装計画
description: Decision transition、候補表示、Primary Event binding、DAY_ONLY 精度、AI Policy 観測を bounded Work Item として順序付ける。
key: signal-context-presentation-observation-plan
---

# Signal Context 表示・観測整合性 実装計画

## 方針

この計画は、既存の Decision、Gate、Execution、Trader、Action Matrix、Position Sizing、Temporal Alignment invariant を変更せず、表示 read model と observation-only の構造化データだけを更新する。Geopolitical lexical integrity は既に develop に統合済みのため、重複実装しない。

共通の acceptance invariant は次の通りとする。

```text
表示または observation の事実
= 構造化された入力
+ 既存の provenance
+ fail-closed な欠損処理
```

Signal Context の既存境界は維持する。

```text
decision_weight = 0
trade_signal = false
Gate / Execution / Trader / Position Sizing への接続なし
```

## 実施順序

### 1. Decision transition 表示整合性

`transition_evidence_read_model` を表示事実の SSOT とし、Markdown、Telegram HTML、監査表示が同じ change predicate と localized value を消費するようにする。内部 enum の debug 表示や raw blocker の直接表示は許可しない。判定値、閾値、Decision packet は変更しない。

### 2. Risk / Probe candidate label integrity

リスク overlay による阻断、または eligibility が false の候補に、実行可能性を示す候補ラベルを付けない。eligibility、Gate、NO TRADE、Probe、取引動作は変更せず、PresentationPacket の candidate label と focused report test だけを更新する。

### 3. Geopolitical lexical integrity

完了済みの `matches_geopolitical_keyword` とその回帰テストを再利用する。新しい Work Item、別 provider の lexical 判定、renderer 内の文字列判定は追加しない。

### 4a. Primary Event scoped binding

MarketReaction の表示対象を Primary Event の event_id に紐付ける。全 temporal-eligible binding の union を使用せず、Primary Event がない場合や event_id が不正な場合は market reaction を表示しない。Temporal eligibility の計算規則自体は変更しない。

### 4b. DAY_ONLY observation precision

timestamp が日付精度だけの観測を `DAY_ONLY` として構造化し、時刻精度のある binding と区別する。日付しかない事実に時刻を推測せず、既存の temporal eligibility を fail-closed に保つ。表示と audit のみを更新する。

### 5a. AI Policy frontier pacing observation

AI Policy frontier pacing を、source、source_url、source_published_at、headline、provider を保持する observation-only record として追加する。Phase 1 は構造化 observation と表示までに限定し、hypothesis confirmation、linkage lifecycle、ranking、Decision 接続は行わない。

### 5b. Hypothesis / linkage lifecycle（後続）

5a の record と provenance が develop に統合された後に、別 Work Item として lifecycle を設計する。Phase 1 に仮説確認の自動判定を混在させない。

## 検証

各 Work Item は専用 branch、専用 worktree、専用 Contract、専用 PR とする。編集前に Runtime の preflight と checkpoint を通し、TDD の RED → GREEN を行う。最低限、対象 focused test、`make fmt-check`、`make test`、`make clippy`、`make quality` を Runtime receipt とともに記録する。最後に PR merge、develop 同期、main 同期、Work Item close、branch/worktree cleanup を事実確認する。
