# Sentinel 状态机下一阶段路线图

## 1. 总体目标

V1.0 - V1.2 已完成：

1. 惯性层落地
2. 判定原语配置化
3. 结构化审计
4. 验证与可观测性闭环

下一阶段不再以“继续扩规则”为主，而是进入：

1. 运行验证
2. 回测对比
3. 运维告警

一句话：

**先观察它，再量化它，再收敛它。**

---

## 2. V1.3 实盘观察期

### 2.1 目标

验证状态机在真实日常运行中的稳定性，而不是只在测试和回测中成立。

### 2.2 观察周期

建议：

1. 连续观察 `2-4` 周

### 2.3 每日记录项

必须追踪：

1. `reset_count`
2. `blocked_reset_count`
3. `state_flip_count`
4. `soft_reset_count`
5. `defensive_override_count`
6. `reconciliation_mismatch_count`
7. `broker_preflight_failure_count`

### 2.4 每周复盘问题

每周至少回答：

1. 哪些状态迁移是合理的
2. 哪些迁移过于敏感
3. 哪些资产恢复过快
4. 哪些资产恢复过慢
5. 是否存在过度防御

### 2.5 产出

建议新增：

1. `weekly_state_review.md`
2. 状态机每周摘要表

### 2.6 验收标准

1. 能连续输出 2 周以上运行观察记录
2. 能定位最常见的 3 类异常迁移模式

---

## 3. V1.4 回测对比与参数收敛

### 3.1 目标

回答核心问题：

**V1.1/V1.2 比旧状态机到底改好了什么。**

### 3.2 必做对比项

旧版 vs 新版至少比较：

1. `reset_count`
2. `blocked_reset_count`
3. `state_flip_count_5d`
4. `defensive_override_count`
5. 最大回撤
6. 状态持续时间
7. 趋势存活率

### 3.3 参数收敛原则

顺序必须固定：

1. 先对比
2. 再分析
3. 最后才微调参数

禁止：

1. 先改参数再找理由

### 3.4 建议可调参数

只允许评估这些：

1. `min_state_duration`
2. `trend_dominant_min_confidence`
3. `core_breakdown_k`
4. `core_breakdown_avg_deviation`
5. `core_breakdown_breadth_floor`

### 3.5 产出

建议新增：

1. `state_machine_comparison.md`
2. `state_machine_parameter_review.md`

### 3.6 验收标准

1. 能给出旧版 vs 新版的量化差异
2. 参数调整必须基于对比数据而不是主观直觉

---

## 4. V1.5 运维告警层

### 4.1 目标

让状态机从“能运行”升级成“能长期盯盘和巡检”的系统。

### 4.2 优先告警事件

建议优先接入：

1. `reset confirmed`
2. `blocked reset`
3. `defensive override`
4. `reconciliation failed`
5. `broker preflight failed`

### 4.3 巡检入口

建议统一巡检来源：

1. `run_status_[DATE].json`
2. `decision_packet_[DATE].json`
3. `state_machine_metrics.json`

### 4.4 产出

建议新增：

1. `operations_checklist.md`
2. 每周健康摘要
3. 告警事件统计表

### 4.5 验收标准

1. 能主动提示高风险状态机事件
2. 能支持最小化人工巡检

---

## 5. 推荐执行顺序

### 第一优先级

`V1.3 实盘观察期`

原因：

1. 先确认真实运行是否稳定
2. 不要在缺观察数据时继续改逻辑

### 第二优先级

`V1.4 回测对比与参数收敛`

原因：

1. 参数必须建立在比较结果上

### 第三优先级

`V1.5 运维告警层`

原因：

1. 当状态机稳定后，告警体系的价值最大

---

## 6. 给开发的要求

下一阶段不要继续扩展状态机规则。  
只允许围绕以下问题工作：

1. 它在真实运行中是否稳定
2. 它相对旧版是否更好
3. 它是否足够可监控

---

## 7. 一句话结论

下一阶段不是继续发明规则。  
下一阶段是：

**观察它、量化它、再收敛它。**
