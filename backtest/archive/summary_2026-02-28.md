# 🔭 Backtest Calibration Summary

## 1. Probability Calibration Curve
| Confidence Bucket | Expected Hit Rate | Actual Hit Rate | Signals |
|-------------------|-------------------|-----------------|---------|
| 90-100 | 95% | 60.2% | 8916 |
| 80-90 | 85% | 63.1% | 3682 |
| 70-80 | 75% | 61.8% | 1271 |
| 60-70 | 65% | 52.4% | 166 |

**🌟 Calibration Error**: `29.2%` *(Measures probability scale accuracy, lower is better)*

## 2. Regime Alpha Separation
*This demonstrates the dual-alpha structure of the system. High Confidence represents stable trends, while Low Confidence represents highly-elastic mean reversion opportunities.*

### 📈 Trend Stability Alpha (Confidence >= 80%)
- **Hit Rate**: `61.1%`
- **Avg 20d Forward Return**: `+2.64%`
- *Characteristics: High probability of success, lower elastic magnitude. Suitable for compounding over time.*

### 🧲 Mean Reversion Alpha (Confidence <= 60%)
- **Hit Rate**: `52.4%`
- **Avg 20d Forward Return**: `+4.33%`
- *Characteristics: Lower probability of immediate success, but much higher elastic magnitude on resolution. Suitable for opportunistic accumulation.*

## 3. Regime-Specific Alpha Audit
*Performance decoupled by generated Capital State (20d forward metrics).*

| Capital State | Signals | Hit Rate | Avg 20d Return | Avg 20d Max Drawdown |
|---------------|---------|----------|----------------|----------------------|
| **🟢 optimal** | `4392` | `66.3%` | `+1.93%` | `-3.76%` |
| **🟢 cruise** | `3258` | `64.6%` | `+2.94%` | `-4.80%` |
| **🛑 DEFEND** | `1685` | `47.2%` | `+1.45%` | `-8.15%` |
| **🟡 overheat_1** | `1595` | `59.7%` | `+3.23%` | `-6.19%` |
| **⚠️ CAUTION** | `840` | `59.3%` | `+2.36%` | `-7.23%` |
| **🛑 fear_downtrend** | `812` | `49.0%` | `+2.12%` | `-12.13%` |
| **🟡 pullback** | `655` | `62.7%` | `+4.51%` | `-7.05%` |
| **🟡 overheat_2** | `631` | `62.8%` | `+7.79%` | `-9.57%` |
| **🩸 fear_1** | `167` | `56.3%` | `+4.91%` | `-8.72%` |

## 4. Potential Energy Forward Returns (Median 20d/60d Index Returns)
- **High Tension (Potential >= 2.0)**: +20d = `+1.75%`, +60d = `+2.19%`
- **Low Tension (Potential <= 1.0)**: +20d = `+1.71%`, +60d = `+4.48%`

## 5. State Transition Flow
**FROM DEFEND** (85 transitions):
  - `→ fear_downtrend`: 58.8%
  - `→ CAUTION`: 14.1%
  - `→ optimal`: 10.6%
**FROM CAUTION** (63 transitions):
  - `→ fear_downtrend`: 31.7%
  - `→ cruise`: 19.0%
  - `→ optimal`: 15.9%
**FROM fear_1** (50 transitions):
  - `→ pullback`: 54.0%
  - `→ fear_downtrend`: 26.0%
  - `→ CAUTION`: 18.0%
**FROM overheat_2** (72 transitions):
  - `→ overheat_1`: 98.6%
  - `→ cruise`: 1.4%
**FROM overheat_1** (285 transitions):
  - `→ cruise`: 77.2%
  - `→ overheat_2`: 22.1%
  - `→ optimal`: 0.7%
**FROM optimal** (428 transitions):
  - `→ cruise`: 61.0%
  - `→ pullback`: 33.6%
  - `→ CAUTION`: 3.5%
**FROM fear_downtrend** (84 transitions):
  - `→ DEFEND`: 81.0%
  - `→ CAUTION`: 19.0%
**FROM pullback** (177 transitions):
  - `→ optimal`: 62.1%
  - `→ fear_1`: 23.7%
  - `→ CAUTION`: 6.8%
**FROM cruise** (501 transitions):
  - `→ optimal`: 58.9%
  - `→ overheat_1`: 39.5%
  - `→ pullback`: 0.8%
