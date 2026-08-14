---
author: Ray
title: 价量资格审计遗漏修复实施计划
description: 以测试先行为入口修复数据质量、供给上下文和事件基线边界。
key: price-volume-review-findings-plan
---

# 价量资格审计遗漏修复实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox syntax for tracking.

**Goal:** 使可观察的降级数据、缺失供给上下文和事件后基线都保持语义诚实，并补齐回归证据。

**Architecture:** 在现有 price-volume domain 中把 quality gate 与指标可计算性分开；事件 baseline helper 只返回有真实事件后样本的选择；report 和 persistence 继续只投影 assessment。运行层不新增 ticker 特判或交易耦合。

**Tech Stack:** Rust、Serde、Chrono、Cargo unit tests、Makefile AI Cockpit gates。

## Global Constraints

- `decision_weight_percent=0`、`trade_signal=false`、所有 observation effects 为 `None`。
- 不修改 Gate、Trader、Action Matrix、Position Sizing、既有经济定义或 S-29 外部采集。
- repository 内 Markdown 使用日文本文与 front matter；commit subject 使用日文 Conventional Commits。

### Task 1: 先补 domain 失败测试

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Test: 同一文件的 domain tests

- [x] 添加可恢复 DEGRADED 仍输出 PARTIAL/CANDIDATE 的测试。
- [x] 添加无 Supply Context 时保留价量候选、但不产生供给吸收的测试。
- [x] 添加事件日期之后没有足够样本时不使用 `POST_*` 的测试。
- [x] 运行 focused domain tests，确认新增断言先失败。

### Task 2: 实现最小修复

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`

- [x] 让可恢复 `DEGRADED` 经过指标计算，保留 fail-closed 原因。
- [x] 为缺失 Supply Context 保留候选观察并限制 `SupplyAbsorption`。
- [x] 让 event baseline 检查真实事件后样本，不足时退回 `AVAILABLE_HISTORY` 或明确不可用。
- [x] 运行 focused tests，确认新增测试通过且既有分类不变。

### Task 3: 补齐场景与投影测试

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Modify: `src/features/radar/interface/price_volume_structure_report.rs`
- Modify: `src/features/radar/infrastructure/persistence.rs`
- Modify: `src/features/shared/domain/market_data.rs`（仅在需要固定 total trading days 语义时）

- [x] 补齐 IPO 3/7/15、Lock-up 1/3/5、Earnings 1/3 的独立断言。
- [x] 补齐 429、volume gap、Supply Context missing、short squeeze/repair、三日生命周期和 boundary 断言。
- [x] 补齐 report、legacy serde 和事件样本真实性测试。
- [x] 运行 domain、report、persistence focused tests。

### Task 4: 文档、治理与完整验证

**Files:**
- Modify: `.ai/work-items/active/price-volume-review-findings.contract.json`
- Modify: `.ai/work-items/active/price-volume-review-findings.summary.json`
- Modify: `.ai/cockpit/current_status.md` via generator

- [x] 勾选并记录本计划的实际执行结果。
- [x] 更新 Summary 的 scenario coverage、residual risks 和 review focus。
- [x] 运行 Contract 要求的全部 `make` checks、`make quality` 和独立边界审计。
- [x] 运行 `make ai-finish TASK=price-volume-review-findings` 归档并确认无 active Work Item。
