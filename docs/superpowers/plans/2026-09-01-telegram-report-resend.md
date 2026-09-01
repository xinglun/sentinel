---
author: Ray
title: 既存日報の Telegram 再送実装計画
description: 同日再計算を避け、data ブランチの正式日報を Actions から安全に再送する計画。
key: telegram-report-resend
---

# Telegram 再送実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `workflow_dispatch` の resend モードで data ブランチに保存済みの当日日報を再送し、同日スナップショットの再計算・上書きを発生させない。

**Architecture:** 通常の観測・意思決定・履歴書き込み経路と再送経路を workflow の入力で分離する。再送経路は復元済み `reports/<JST日付>.md` と成功済み `run_status_<JST日付>.json` を検証し、Telegram API に plain text の分割メッセージとして送信する。送信結果だけを `notification_resend` として同じ run status に追記し、data worktree の既存 write-back で監査記録を保存する。

**Tech Stack:** GitHub Actions YAML、Bash、Python 標準ライブラリ、Rust workflow 契約テスト、AI Cockpit Work Item。

**Spec:** `.ai/work-items/active/telegram-report-recovery.contract.json`

## Global Constraints

- 取引、Gate、Execution、Trader、Action Matrix、Position Sizing の意味論を変更しない。
- resend は同日再計算を実行せず、data ブランチの正式日報だけを入力にする。
- Telegram token と chat id の値をログ、Contract、Summary、テスト出力へ記録しない。
- 新規 workflow は非成功 HTTP 応答または Telegram の `ok != true` を失敗として扱う。
- 検証は `make fmt-check`、`make test`、`make clippy`、`make quality` を実行する。

---

### Task 1: 再送 workflow 契約を先に追加する

**Files:**
- Modify: `tests/daily_radar_workflow_integration.rs`
- Test: `tests/daily_radar_workflow_integration.rs::daily_radar_manual_resend_reuses_archived_report_and_has_valid_shell_syntax`

**Interfaces:**
- Consumes: workflow の `workflow_dispatch` 入力と `Resend Existing Daily Report` step 名。
- Produces: 再送 mode、保存済み日報、run status、Telegram API 検証を要求する契約テスト。

- [x] **Step 1: Write the failing test**

  `daily_radar_workflow_integration.rs` に次の条件を検証するテストを置く。

  - workflow に `type: choice` と `resend` が存在する。
  - `Resend Existing Daily Report` step を抽出できる。
  - 抽出した shell が `bash -n` を通る。
  - script が `reports/${DATE_JST}.md`、`run_status_${DATE_JST}.json`、`api.telegram.org`、Telegram の `ok` 検証を含む。

- [x] **Step 2: Run test to verify it fails**

  Run: `cargo test --test daily_radar_workflow_integration daily_radar_manual_resend_reuses_archived_report_and_has_valid_shell_syntax -- --exact`

  Expected: `workflow step is missing` で失敗する。

### Task 2: 保存済み日報の安全な再送経路を実装する

**Files:**
- Modify: `.github/workflows/daily_radar.yml`

**Interfaces:**
- Consumes: `workflow_dispatch.inputs.mode`、復元済み data branch の `reports/<JST日付>.md` と `run_status_<JST日付>.json`、`TELEGRAM_BOT_TOKEN`、`TELEGRAM_CHAT_ID`。
- Produces: `Resend Existing Daily Report` step、分割送信と `notification_resend` 監査フィールド。

- [x] **Step 1: Add the manual mode selector**

  `workflow_dispatch` に必須 choice input `mode` を追加し、選択肢を `generate` と `resend`、既定値を `generate` とする。schedule は既存の generate 経路を使う。

- [x] **Step 2: Skip recalculation when resending**

  evidence 収集と `Run Sentinel Radar` に `if: github.event_name != 'workflow_dispatch' || inputs.mode != 'resend'` を付ける。再送 step は `if: github.event_name == 'workflow_dispatch' && inputs.mode == 'resend'` とし、通常の生成 step と同じ Telegram secrets を受け取る。

- [x] **Step 3: Validate archived inputs before sending**

  再送 script で JST 日付を決め、日報と run status が空でないこと、run status の `decisioning` が succeeded であることを Python で検証する。検証失敗時は `set -euo pipefail` で終了し、`Notify on Failure` を起動する。

- [x] **Step 4: Send bounded plain-text chunks and record the result**

  Python 標準ライブラリで日報を UTF-8 文字単位の 3800 文字以下に分割し、Telegram `sendMessage` を順番に呼ぶ。各応答の HTTP status と JSON `ok == true` を検証する。全 chunk 成功後だけ `run_status_<JST日付>.json` の `notification_resend` に status、source、chunk 数、UTC timestamp を追記し、`REPORT_DATE_JST` を export する。

- [x] **Step 5: Run the focused contract test**

  Run: `cargo test --test daily_radar_workflow_integration daily_radar_manual_resend_reuses_archived_report_and_has_valid_shell_syntax -- --exact`

  Expected: PASS。

### Task 3: 全体検証と main 上の遠隔実行

**Files:**
- Modify: `.ai/work-items/active/telegram-report-recovery.contract.json`
- Modify: `.ai/work-items/active/telegram-report-recovery.summary.json`
- Modify: `.ai/evidence/telegram-report-recovery.verification.json`

**Interfaces:**
- Consumes: focused test、Rust quality gates、PR checks、GitHub Actions run output。
- Produces: fresh verification evidence、merged `main` commit、resend run with successful notification status。

- [x] **Step 1: Run repository checks through Make**

  Run: `make fmt-check && make test && make clippy && make quality`

  Expected: all commands exit 0; no secret values appear in output。

- [ ] **Step 2: Commit and push the dedicated branch**

  Run: `git add .github/workflows/daily_radar.yml tests/daily_radar_workflow_integration.rs .ai docs/superpowers/plans/2026-09-01-telegram-report-resend.md && git commit -m "fix: 既存日報の Telegram 再送を追加" && git push -u origin codex/telegram-report-resend-v2`

  Expected: push succeeds and PR base is `main`。

- [ ] **Step 3: Merge only after PR checks pass**

  Confirm PR checks pass, merge the PR with the `xinglun` account, fetch `origin/main`, and confirm the merged commit contains the workflow input and resend step。

- [ ] **Step 4: Execute the resend on main**

  Run: `gh workflow run daily_radar.yml --repo xinglun/sentinel --ref main -f mode=resend`

  Expected: `Resend Existing Daily Report` and `Freshness Gate and Output Validation` succeed; `run_status_<JST日付>.json` contains `notification_resend.status=succeeded`; the data branch write-back verification succeeds。

- [ ] **Step 5: Verify delivery evidence**

  Inspect the run log without printing secrets. Confirm Telegram API responses were `ok=true`, the normal report was not regenerated, and the run completed on the merged `main` SHA。
