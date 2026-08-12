---
author: Ray
title: 价量资格审计遗漏修复设计
description: 修复降级数据、供给上下文缺失和事件基线真实性的边界遗漏。
key: price-volume-review-findings-design
---

# 价量资格审计遗漏修复设计

## 目标

本 Work Item 只修复上一轮审计发现，不改变交易边界和既有价量经济定义。可恢复的数据质量降级应继续支持观察；不可恢复的数据才输出 `UNAVAILABLE`。

## 设计

1. 将“是否能计算指标”和“数据质量标签”分开。只要当前 bar、比较样本和最小有效 OHLCV 数量满足要求，`DEGRADED` 仍可进入既有分类，并由 `PARTIAL`/`CANDIDATE` 生命周期限制确认能力。API 429、完全缺失 volume、不可比较 corporate action 和无法构成比较的 data gap 继续 fail closed。
2. Supply Context 缺失时保留价量结构观察，新增/使用明确的候选表达；`SupplyAbsorption` 固定为 `None` 或 `Unavailable`，并在报告显示上下文缺失原因。不得从 ticker 推断供给事件。
3. 事件基线只接受至少一个真实事件后有效 session；事件日期在样本之后或事件后样本不足时，不能继续使用 `POST_*` 标签。若可用历史满足普通观察，降级为 `AVAILABLE_HISTORY`；否则输出结构化不可用原因。
4. `total_trading_days` 与 bar 样本的职责明确：成熟度由有效 bar 证据决定，传入字段不再无效闲置；通过测试固定实际选择规则。

## 验证

新增逐项测试覆盖 IPO 3/7/15、Lock-up 1/3/5、Earnings 1/3、降级数据、429、volume gap、Supply Context 缺失、事件样本不足、反过拟合和观察边界。报告、legacy serde、计划与 Cockpit 生命周期通过 Make gate 验证。
