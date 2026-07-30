---
author: Ray
title: 市场演化观察设计
description: 将日报从单日快照扩展为可追踪的跨日市场观察，并固定历史覆盖与变化等级语义。
key: market-evolution-observation-design
---

# 市场演化观察设计

## 目标与边界

本轮将日报升级为市场演化观察，使系统能够比较当前日、前一交易日和最近七个交易日的结构变化。新增内容只属于 Observation / Interpretation Layer，`Decision Weight` 固定为 `0%`。

新增内容不得进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing 或 Risk Sizing，也不生成交易信号。

## Leader Persistence

Leadership Snapshot 是当前日和历史日的唯一事实源。输出字段固定包含：

- `current_leadership_streak_days`
- `observed_leadership_days_in_lookback`
- `breakout_continuity_days`
- `history_coverage`
- `first_observed_at`
- `previous_leader`
- `leadership_score`
- `leadership_state`

`current_leadership_streak_days` 只统计连续担任 Composite Leader（综合排名第一）的交易日，不与 `breakout_continuity_days` 混用；Composite Leader 不等同于单资产突破 Leader。

状态规则固定如下：`NEW` 为 1 日，`EARLY` 为 2--3 日，`ESTABLISHED` 为 4--7 日；`DOMINANT` 要求连续领导至少 8 个交易日，且 `breadth_score >= 60`、`relative_strength >= 60`；`FADING` 要求当前仍为 Leader，且 `leadership_score` 或 `relative_strength` 每日至少下降 2 分并连续 3 日，或三日累计下降至少 5 分；冲突或无法形成历史时为 `UNAVAILABLE`。

历史覆盖分为 `COMPLETE`、`PARTIAL`、`UNAVAILABLE`。周末和已知休市日不计入七个交易日窗口。缺少应有交易日快照为 `PARTIAL`；无法形成有效连续历史为 `UNAVAILABLE`。`PARTIAL` 时禁止输出“首次观察”，只有完整窗口确认后才允许该语义。

## Observation Timeline

时间轴按最近七个交易日保存每日结构化记录，至少包含日期、Composite / Secondary Leaders、breadth、concentration、rotation、confidence、market state、supply phase、risk state 与 day type。

主报告只显示压缩摘要；完整记录独立保存为 latest JSON、按日 JSON 与 JSONL，不写入单日 decision packet。重复且无结构变化的内容在主报告中静默压缩为“过去 7 个交易日未出现结构性变化”。

## Market Change Driver

比较维度分为核心维度和辅助维度。变化等级按 `MAJOR > MODERATE > MINOR > NONE` 取最高级别：

- `MAJOR`：Market State、Risk State、Day Type 任一变化，或多个核心维度同时变化。
- `MODERATE`：Composite Leader、Breadth Classification、Supply Phase 任一变化。
- `MINOR`：只发生 confidence / score 变化，或局部排序变化不超过 1 位且分数绝对变化小于 5。
- `NONE`：没有核心维度变化。

输出同时包含 `change_drivers`、`unchanged_dimensions` 与 `summary`，不能只由 confidence 决定。

## Future Supply Queue

主报告显示 Subject、Event Type、Expected Window、Status 与 Source Quality。详情继续读取现有供给观察事实，不改变供给判断进入交易决策的边界。

## 验证策略

领域测试覆盖阈值、连续下降、历史覆盖和 change level 优先级；集成测试覆盖日报输出、三语言 snapshot、独立 archival artifact 与 `decision_weight: 0%`。Cockpit scope、架构边界和 Rust quality gate 作为完成前 required checks。
