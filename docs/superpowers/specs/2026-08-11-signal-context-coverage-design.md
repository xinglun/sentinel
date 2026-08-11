---
author: Ray
title: Signal Context 事实覆盖修复设计
description: 将 Signal Context 从单一宏观日历检测扩展为可追溯的六类市场信息观察层。
key: signal-context-coverage-design
---

# Signal Context 事实覆盖修复设计

## 目标与边界

本 Work Item 修复 Signal Context 将官方宏观日历错误等同于整体市场信息环境的问题。输出只描述事实、证据、市场反应和解释，不改变 `decision_weight`、`trade_signal`、Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing、Price-Volume 或 Supply Layer。

## 统一模型

以 `SignalContextAssessment` 为报告投影，以 `SignalContextSnapshot` 为唯一事实契约。快照包含 `market_date`、六类 Context、`primary_context`、`secondary_contexts`、`overall_information_content`、`context_quality`、六类 `source_status`、`overall_coverage` 和固定为 0 的 `decision_weight`。每个事件包含 `EvidenceRecord`、`MarketReaction`、UTC 与 America/New_York 时间、生命周期和新鲜度。

事件事实与市场反应分开存储。解释只使用 `consistent with`、`likely contributed` 或 `market reaction suggests` 等弱因果措辞；没有 Reaction Evidence 时不得写成因果结论。

## 覆盖与判定

六类 source status 使用 `HEALTHY`、`PARTIAL`、`DEGRADED`、`UNAVAILABLE`。全部健康时 Overall Coverage 为 `HEALTHY`；任一部分成功且无不可用关键组时为 `PARTIAL`；存在退化或不可用组时按真值表降级。已有 HIGH / MEDIUM 证据仍可保留，但非 HEALTHY coverage 时 Context Quality 不得为 HIGH，也不得输出绝对的无事件结论。只有六类均扫描完成、覆盖 HEALTHY 且没有 HIGH / MEDIUM 时，才允许 LOW 与完整的 absence wording。

第一版使用确定性阈值：原油日变化绝对值至少 4%，或超过过去 60 个交易日绝对日变化的 95 分位；美国 2Y/10Y 变化至少 10bp，VIX 日变化至少 20%，信用利差至少 15bp；重大公司事件必须同时满足 Watch Universe / 行业领导者条件和显著价格或同业反应。阈值集中在配置常量，测试使用固定 fixture。

## 主上下文与生命周期

Primary 按 Information Content、Market Relevance、Evidence Quality、Freshness、稳定事件 ID 排序；其余去重后进入 Secondary。事件所属交易日始终由 `America/New_York` 计算，不使用日本日期。生命周期为 `UPCOMING`、`RELEASED`、`ACTIVE_REPRICING`、`AFTERMATH`、`EXPIRED`；跨日事件只能在有持续 Reaction Evidence 时作为 AFTERMATH。

## 证据与故障

所有 HIGH / MEDIUM 事件必须绑定 `EvidenceRecord { source, source_url, timestamp, event_type, subject, importance }`。429、源失败、来源不完整或扫描未完成会被保留在 source diagnostics，并阻止 LOW 或绝对 absence wording。六类来源在 v1 使用当前可追溯的本地/结构化适配器；无法提供结构化证据的类别明确标为 UNAVAILABLE，不由 AI 自由猜测事实。

## 输出与验证

日报、Markdown、Telegram 和现有 weekly traceability 只消费同一 read model，并显示 Coverage、Primary / Secondary、Observed Market Reaction、Interpretation 与 `Decision Weight: 0%`。增加 consistency check，覆盖 CPI、Payroll、FOMC、GDP、Fed speech、财报、地缘政治、商品、利率/信用、VIX/行业轮动、无事件、429、部分失败、时区、周末、假日和盘前盘后发布时间。8/7 与 8/10 固定为回归 fixture。
