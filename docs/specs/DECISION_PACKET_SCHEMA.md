---
author: Ray
title: Sentinel 意思決定パッケージ仕様 (DECISION_PACKET_SCHEMA.md)
description: Sentinel 意思決定パッケージ仕様 (DECISION_PACKET_SCHEMA.md) に関する Sentinel の設計・運用情報。
key: docs-specs-decision-packet-schema
---

# Sentinel 意思決定パッケージ仕様 (DECISION_PACKET_SCHEMA.md)

## 1. 構造の概要

`decision_packet.json` はシステムの唯一の主要な成果物であり、市場、ポートフォリオ、資産のすべての意思決定情報を含んでいます。Telegram レンダラーなどは、この JSON からデータを取得しなければなりません。

## 2. フィールド定義 (v1.0 ドラフト)

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
      "position_intent": "HOLD",
      "exit_decision": {
        "position_intent": "HOLD",
        "asset_exit_state": "None",
        "reasons": []
      },
      "reasons": ["trend intact"],
      "metrics": {
          "deviation": 5.2,
          "z_score": 1.2,
          "slope": 0.8
      },
      "history_metrics": {
          "previous_state": "OPTIMAL",
          "state_streak": 5,
          "top_tier_streak": 3,
          "out_of_top_tier_streak": 0
      },
      "display_context": {
          "has_position": true,
          "is_core_holding": true,
          "is_candidate_only": false,
          "is_top_tier": true,
          "participation_ready": true
      },
      "display_intent": "HOLD"
    }
  ],
  "participation": {
    "participation_ready": false,
    "stability_ready": true,
    "core_tier_streak_ready": false,
    "core_tier_streak": 1,
    "reasons": ["Core Tier streak < 3"]
  },
  "participation_changed": false,
  "top_tier_symbols": ["AAPL", "MSFT", "NVDA"],
  "telegram": {
    "headline": "Market State: ESTABLISHED",
    "summary": "Hold core leaders, buy controlled pullbacks, no chasing."
  }
}
```

## 3. 消費の原則

1. **不可逆性**: レンダリングレイヤーは、`metrics` に基づいて `action` を再推論してはなりません。
2. **テキストの安定性**: `telegram.headline` と `telegram.summary` は意思決定エンジンのロジックによってあらかじめ設定されるべきであり、一貫性を保証します。
3. **マシンリーダブル**: このファイルは `history/` ディレクトリに保存され、バックテストと監査の基礎として使用されるべきです。
