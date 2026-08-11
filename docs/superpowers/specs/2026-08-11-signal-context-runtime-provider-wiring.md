---
author: Ray
title: Signal Context 構造化来源运行时接线设计
description: 将可追溯的 SignalContextV1 JSON 来源接入 Observation / Interpretation read model。
key: signal-context-runtime-provider-wiring
---

# Signal Context 構造化来源运行时接线

## 目的

上一阶段已经建立六类 Context 的 v1 schema 与 fixture gate，但生产 read model 仍只从官方宏观日历构造 Context。本设计通过现有环境配置入口读取结构化 `SignalContextV1`，让企业、地缘政治、商品、利率/信用和市场结构事实能够进入同一聚合路径。

## 输入契约

- 环境变量：`SENTINEL_SIGNAL_CONTEXT_JSON_PATH`。
- 文件内容：`SignalContextV1` JSON。
- `market_date` 必须等于当前美股交易日。
- HIGH / MEDIUM item 必须包含非空 EvidenceRecord。
- `EXPIRED` item、日期不匹配、解析失败或文件缺失不得进入当日 Primary Context。

## Fail-closed 规则

读取失败时保留现有宏观日历事实，但外部六类来源不宣称 HEALTHY；不得将失败降级为 LOW 或无事件结论。结构化来源被成功读取后，由 v1 聚合器按信息量、市场相关性、证据质量与类别优先级确定 Primary/Secondary。

## 边界

该接线只改变 Observation / Interpretation。`decision_weight`、`trade_signal`、Gate、Execution、Trader、Action Matrix、Price-Volume、Supply、Gravity 与 Expectation 均不读取或修改外部 Context。

## 验收证据

运行时读取、日期/生命周期过滤、8/7 Payroll、8/10 地缘政治/原油、失败源和交易边界均由 Rust 测试、fixture consistency gate 与 Cockpit required checks 验证。
