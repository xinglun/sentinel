---
author: Ray
title: Sentinel Observation Layer 修复設計
description: 修复事件事实、短历史价量观察、相对强度与 Leader 缺失语义，同时保持交易边界不变。
key: observation-layer-repair-design
---

# Sentinel Observation Layer 修复設計

## 目标

增强 Sentinel 识别现实市场事实的能力，不提高交易激进程度。事件发现与事件实际值分离；短历史资产允许基于局部证据形成观察假设；当前相对强度与结构性 Leader 分离；Leader 缺失持续时间可审计。

## 边界

所有新增或调整字段属于 Observation Layer，固定 `decision_weight = 0%`、`trade_signal = false`、`gate_effect = none`、`execution_effect = none`、`position_sizing_effect = none`。不修改 Gate、Execution、Trader、Action Matrix、Position Sizing、Breadth、Market State threshold、Confidence threshold、Leader qualification 或 Breakout qualification。

## 架构

Signal Context 使用 `EventDiscovery` 保存已知事件与发布时间，使用 `EventObservation` 保存 expected/actual/surprise 和数据状态；事件生命周期由 discovery 时间与 observation 状态推导。Coverage 独立记录六类来源状态，数据缺失只降低质量，不删除已知事件。

Price-Volume 将数据资格、基线选择、结构假设、生命周期和观察持续天数作为独立概念。`PARTIAL` 允许至少五个连续有效交易日；RVOL 输出对应基线；Supply Context 只读消费 Supply Layer 事实，并显式报告缺失原因。

Current Relative Strength 使用 1d/5d 相对 SPY、价格位置和成交量参与度产生有限状态，不进入 Leader 或交易决策链。Leader 为 none 时使用 ABSENT 与缺失持续天数，不复用资产 Leader 专用字段。

## 验收与回归

固定 CPI 2026-08-12 的 UPCOMING、RELEASED、actual unavailable 三态；覆盖无事件、部分来源失败、短历史 IPO/Lockup、财报后重估、结构不可用与观察持续、累计/耗尽上涨候选、Leader 缺失 1/6 天、相对强度但无 Leader、强相对资产仍 NO TRADE、同日重跑。所有情形都必须保留 Observation-only boundary。
