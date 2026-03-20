# Sentinel 决策包规范 (DECISION_PACKET_SCHEMA.md)

## 1. 结构概述

`decision_packet.json` 是系统的唯一主产物，包含了市场、组合、资产的全量决策信息。Telegram 渲染器必须从该 JSON 中拉取数据。

## 2. 字段定义 (v1.0 草案)

```json
{
  "date": "2026-03-19",
  "market_regime": {
    "market_state": "ESTABLISHED",
    "lifecycle_state": "ESTABLISHED",
    "risk_overlay": "NORMAL",
    "scores": {
      "confidence": 80.58,
      "stability": 30.0,
      "structural": 33.6,
      "maturity": 45.0,
      "flow_acceleration": 0.0
    },
    "transition": {
      "from": "EARLY_CONFIRMATION",
      "to": "ESTABLISHED",
      "changed": true,
      "reasons": [
        "stability crossed 25",
        "maturity crossed 35"
      ]
    }
  },
  "portfolio_policy": {
    "target_exposure_min": 0.60,
    "target_exposure_max": 0.80,
    "allow_chase": false,
    "allow_pullback_buy": true,
    "allow_new_risk": true,
    "risk_assets_mode": "DEFEND"
  },
  "assets": [
    {
      "symbol": "NVDA",
      "name": "Nvidia",
      "asset_state": "OPTIMAL",
      "action": "HOLD",
      "reasons": ["trend intact"],
      "metrics": {
          "deviation": 5.2,
          "z_score": 1.2,
          "slope": 0.8
      }
    }
  ],
  "telegram": {
    "headline": "Market State: ESTABLISHED",
    "summary": "Hold core leaders, buy controlled pullbacks, no chasing."
  }
}
```

## 3. 消费原则

1. **不可回退**: 渲染层禁止根据 `metrics` 重新推断 `action`。
2. **文本稳定性**: `telegram.headline` 和 `telegram.summary` 应当由决策引擎逻辑预设，确保一致性。
3. **机器可读**: 该文件应当存储在 `history/` 目录下，作为回测和审计的基础。
