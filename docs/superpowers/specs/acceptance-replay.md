---
author: Ray
title: Sentinel acceptance-replay 仕様
description: 不変な過去入力を現在コードで再計算し、canonical state を変更せずに受入検証する実行契約
key: sentinel-acceptance-replay-spec
---

# Sentinel acceptance-replay 仕様

## 目的

`acceptance-replay` は、正式な `generate` と保存済み payload の `resend` に加わる第三の実行意味論である。指定日の immutable input manifest を読み、現在のコードで新しい計算を行い、隔離された出力と machine-readable verification receipt を生成する。Telegram 通知は明示的に要求された場合だけ許可する。

この入口は、Signal Context の lexical 修正、renderer 修正、timezone 修正などを、正式 snapshot や data branch を書き換えずに受入検証するための production-code acceptance harness とする。

## 実行意味論

| mode | 計算 | canonical snapshot | data branch / freshness | 通知入力 |
| --- | --- | --- | --- | --- |
| `generate` | 新規計算 | 既存の正式経路で保存 | 既存の正式経路で更新 | generated report |
| `resend` | 再計算しない | 書き込まない | 書き込まない | archived payload |
| `acceptance-replay` | immutable input から新規計算 | 常に禁止 | 常に禁止 | 新規 isolated generated payload のみ、任意 |

`acceptance-replay` は publishable ではない。`generate --force`、`generate --ignore-snapshot-conflict`、snapshot の削除、canonical file の一時リネームによる回避は実装しない。

CLI は次の引数を必須とする。

```text
stock-sentinel acceptance-replay \
  --date <YYYY-MM-DD> \
  --revision <SHA> \
  --input-manifest <PATH> \
  --output-dir <ISOLATED-DIR>
```

相対 `--output-dir` は manifest の親ディレクトリから解決する。既存ディレクトリが空でない場合、または canonical state の内側を指す場合は停止する。

## 不変条件

次の条件をすべて満たさない限り、geopolitical classification の受入結果を成功としない。

```text
Geopolitical classification
= lexically valid
AND contextually plausible
AND evidence-backed
```

`Warsh != war` を固定する。Warsh の headline は geopolitical context を生成せず、`War in Iran ... missile strike` は有効な positive とする。workers strike、option strike、company attacks rising costs などの曖昧な表現は fail-closed とする。

入力 manifest は report date、入力 evidence digest、snapshot/input revision を持ち、実行時に対象日付と digest を厳密に照合する。manifest がない、壊れている、日付が違う、digest が違う、または encoding が扱えない場合は `ACCEPTANCE_REPLAY_INPUT_UNAVAILABLE` 相当で停止する。current provider response、latest snapshot、別日付の入力への fallback はしない。

## 出力と provenance

出力ディレクトリは毎回新規の isolated directory とし、canonical path の祖先外に置く。receipt には少なくとも次を記録する。

```text
report_lifecycle.mode = GENERATED
report_lifecycle.run_purpose = ACCEPTANCE_REPLAY
report_lifecycle.publication_scope = ISOLATED
report_lifecycle.canonical_write = false
execution_git_commit_sha = <実行 revision>
input_manifest_digest = <immutable manifest digest>
canonical_snapshot_before = <digest>
canonical_snapshot_after = <digest>
canonical_state_unchanged = true
notification.source = generated_report_payload | none
```

`canonical_snapshot_before == canonical_snapshot_after` を要求する。Decision、Gate、Action Matrix、Execution、Trader、Position Sizing の projection digest も replay 前後で一致させ、`decision_weight=0`、`trade_signal=false`、`gate_effect=none` を確認する。

## 構造境界

入力検証、隔離出力、provenance、verification manifest の責務を一つの application/interface boundary に集約する。既存 provider、renderer、Decision、TemporalBinding、MarketObservation に lexical 判定を複製しない。renderer は classification を行わず、既存の generate と resend は同じ意味論を維持する。

`acceptance-replay` の runner は canonical persistence capability を受け取らない設計とし、通常 runner の side effectful publish path を mode 条件の分岐で再利用しない。必要な共通計算は pure projection または明示的な isolated sink に切り出す。

## 検証 manifest

verification manifest は次を機械的に検証する。

- Warsh negative と real geopolitical positive。
- empty、malformed、missing、mismatched immutable input の fail-closed。
- provenance の `GENERATED + ACCEPTANCE_REPLAY + ISOLATED`。
- canonical before/after digest の一致。
- Decision/Gate/Position Sizing 等の projection digest の一致。
- fresh generated payload を使う optional notification と archived resend の分離。

manifest の結果は各 assertion の `PASS` / `FAIL` と receipt digest を持つ。失敗時に report を canonical artifact として promote しない。

## 非対象

この Work Item は lexical classification と隔離 replay の検証だけを扱う。Decision、Gate、RS、Leadership、Action Matrix、Execution、Trader、Position Sizing、Temporal Alignment、MarketObservation、threshold、provider credential、canonical data、正式 report archive の意味論は変更しない。
