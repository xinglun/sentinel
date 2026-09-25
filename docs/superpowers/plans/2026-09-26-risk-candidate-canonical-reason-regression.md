---
author: Ray
title: Candidate canonical risk reason 回帰修正の実装計画
description: 最終 Leadership と候補表示の risk reason を一致させる限定修正と受け入れ検証。
key: risk-candidate-canonical-reason-regression-plan
---

# Risk Candidate Canonical Reason Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 最終 Leadership が `LEADERLESS / FRAGMENTED` と確定した後も、Candidate / Watchlist が canonical risk presentation reason を表示し、未保有かつ eligible でない候補へ保有を促す文言を出さない。

**Architecture:** `PresentationAssembler` が最終 Leadership reconciliation 後に Candidate ViewModel を同じ packet の canonical risk ViewModel と同期する。risk fact が見つからない場合は Candidate の具体的な理由を出さず、`report.rs` は Candidate label と reason を分けて表示し、重複 label を省く。

**Tech Stack:** Rust、Cargo、Make quality gates、AI Cockpit Runtime。

**Spec:** `/Users/sei-rinn/.codex/attachments/9e0d94f9-d149-45b6-946b-15aeb0e77a20/貼り付けたテキスト.txt`

## Global Constraints

- 変更は presentation integrity と regression coverage に限定し、Decision、Gate、Probe、Eligibility、Action Matrix、Risk Adjustment、Exit State、Position Sizing の判定を変えない。
- Leaderless の表示は既存の canonical risk mapping を再利用し、新しい翻訳や二つ目の canonicalizer を作らない。
- Renderer 内で Leadership 状態や raw reason を意味変換しない。
- `decision_weight=0` と `trade_signal=false` の observation boundary を維持する。
- 手書きコメントと文書本文は日本語で記述し、既存の i18n 文言を再利用する。

## Review Focus

- 初期 packet には候補 tier があるが、後続 Leadership snapshot は leaderless と確定する場合。
- `RECOVERY_WATCH` の StrengthLoss reason を後段 reconciliation が失わないこと。
- Candidate と同一 symbol の canonical risk ViewModel が欠落または不正な場合に raw reason を出さないこと。
- Confirmed leader の下で有効な StrengthLoss 表示を一律に置換しないこと。
- position がなく eligible でない巡航候補を、Ready / Probe 状態でも observation-only として表示すること。

## File Structure

- `src/features/radar/interface/presentation_assembler.rs`: 最終 Leadership に応じた canonical Candidate diagnostic と observation-only wording。
- `src/features/radar/interface/report.rs`: Candidate block で既に表示した label の重複を省く。
- `src/features/radar/interface/report_ui_tests.rs`: production-like assembly、reconciliation、report surface の回帰。
- `docs/superpowers/plans/2026-09-26-risk-candidate-canonical-reason-regression.md`: この計画。

## Task 1: Candidate presentation を canonical risk fact に束ねる

**Interfaces:** `PresentationAssembler::assemble` が生成する `top_actions` と `risk_opportunities` を、`reconcile_tactical_leadership_display` が最終 Leadership を受けて整合させる。`generate_refined_report` は完成済みの Presentation ViewModel から Markdown、Telegram HTML、Archive Markdown を生成する。

**Steps:**

- [ ] `report_ui_tests.rs` に leaderless StrengthLoss candidate の production-like fixture を追加する。初期 packet の tier candidate が assemble を通過した後、Leadership snapshot を `LEADERLESS / FRAGMENTED` にし、Markdown / Telegram HTML / Archive Markdown で古い語彙が漏れることを確認する。
- [ ] 同じテスト経路へ RECOVERY_WATCH、zh-CN / en-US / ja-JP、canonical risk fact 欠落、confirmed leader の control case を加え、未修正コードで期待どおり fail することを確認する。
- [ ] 未保有・eligible=false・巡航候補の Ready / Probe presentation を組み立て、三言語の既存 observation wording が保有を示唆しないことを確認する。
- [ ] PresentationAssembler で同一 symbol の canonical risk reason を Candidate diagnostic に適用する。risk fact が欠落または不正なら raw diagnostic を保持せず、具体的な理由を非表示にする。
- [ ] position がなく candidate eligibility もない巡航資産には既存 observation-only wording を適用し、他の exit/risk fact は維持する。
- [ ] `report.rs` で Candidate の一次 label と同じ文字列の tag を二回表示しない。
- [ ] regression test を再実行し、Risk Summary / Portfolio Risk の既存表示、execution-window / participation / eligible count、Markdown / Telegram HTML / Archive Markdown を確認する。
- [ ] Contract に宣言された全ての Rust と Make verification を実行する。

**Expected:** Candidate / Watchlist の visible risk reason は同一 symbol の canonical risk ViewModel と一致する。leaderless raw StrengthLoss 文言は三言語・三 surface に存在せず、RECOVERY_WATCH は既存 canonical wording のまま。confirmed leader の raw wording、決定出力、観測境界は変わらない。

## Verification

- `cargo test risk --lib`
- `cargo test candidate --lib`
- `cargo test report_ui --lib`
- `make fmt-check`
- `make test`
- `make clippy`
- `make quality`
