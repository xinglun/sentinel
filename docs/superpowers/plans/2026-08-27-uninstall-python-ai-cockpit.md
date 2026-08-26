# Python版AI Cockpitからインストール版Runtimeへの切替え Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax to track progress.

**Goal:** リポジトリに同梱された Python 版 AI Cockpit の実行経路を除去し、インストール済み `ai-cockpit` 0.2.33 を共有 Runtime として接続する。

**Architecture:** リポジトリには Runtime 本体をコピーせず、インストール版 CLI が `.ai/cockpit.toml`、`.ai/project.json`、`.ai/agent-interface.json` および証跡を管理する。インストール版 CLI は独立実行ファイルであり、AI Cockpit lifecycle は Makefile に依存しない。プロジェクト固有の Rust 品質コマンドだけをルート `Makefile` に残す。

**Tech Stack:** Rust、GNU Make、インストール済み Rust 製 `ai-cockpit` CLI、JSON、Markdown。

## 実装タスク

- [x] 旧 Python Work Item を切替え準備用として確定し、旧 Runtime の active 状態を archive する。
- [x] インストール版 CLI でリポジトリを attach し、Runtime の repository state と Codex adapter を生成する。
- [x] AGENTS.md、GEMINI.md、リポジトリ内 Cockpit Skill をインストール版 CLI の lifecycle に合わせ、ルート Makefile はプロジェクト品質用途だけに整理する。
- [x] 旧 Python Runtime の Makefile、runtime metadata、installer-managed helper、旧 AI 専用 script/test を明示的に削除する。プロジェクト固有コード、履歴、証跡、宣言は保持する。
- [x] 新 Runtime 管理下の Work Item を使って scope、boundary、verification を記録し、Rust 品質 gate と CLI の inspect/status/doctor/compatibility/verify を実行する。
- [x] Work Item Summary を更新し、残余 risk と review focus を記録して archive する。

削除対象のうち `scripts/ai_check_test_weakening.py` は、インストール版 CLI が test-like file の削除を test integrity violation として fail-closed に扱うため、内容を変更せず保持した。この helper はルート Makefile や Runtime lifecycle からは参照されず、旧 Runtime の binding ではない。プロジェクト固有の `scripts/ai_test_*.py` も保持した。

## 境界条件

- Gate、execution、trader、action matrix、position sizing、report output、data branch、weekly metrics の挙動は変更しない。
- `.ai/work-items/archive/**`、既存の evidence、knowledge、project declaration は履歴・プロジェクトデータとして保持する。
- 全局 Python、全局 CLI、ユーザーの他リポジトリ、外部サービス設定は変更しない。
- 破壊的削除は manifest の分類をそのまま信頼せず、対象を実査した明示リストに限定する。

## 検証

- [x] `ai-cockpit --version` が 0.2.33 を返す。
- [x] `ai-cockpit inspect/status/doctor/compatibility` が対象リポジトリを installed Runtime として認識する。profile status は human confirmation 未実施のため `calibration_required` を残す。
- [x] `make fmt-check`、`make test`、`make clippy` が成功する。
- [x] `make quality` が Python Runtime script を参照せず成功する。
- [x] `rg` で旧 Python Runtime の Makefile 呼出しと runtime metadata が残っていないことを確認する。

## 残余

- `ai-cockpit doctor` と `compatibility` は installed Runtime として green / `COMPATIBLE` である。
- `ai-cockpit status` は profile の人間確認が未実施のため `calibration_required` である。これは Runtime incompatibility ではない。
- 変更はローカル branch に留め、commit、push、pull request は実施していない。
