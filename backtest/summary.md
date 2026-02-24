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

## 3. Potential Energy Forward Returns (Median 20d/60d Index Returns)
- **High Tension (Potential >= 2.0)**: +20d = `+1.75%`, +60d = `+2.19%`
- **Low Tension (Potential <= 1.0)**: +20d = `+1.71%`, +60d = `+4.48%`

## 4. State Transition Flow
**FROM fear_1** (50 transitions):
  - `→ pullback`: 54.0%
  - `→ fear_downtrend`: 26.0%
  - `→ CAUTION`: 18.0%
**FROM overheat_2** (72 transitions):
  - `→ overheat_1`: 98.6%
  - `→ cruise`: 1.4%
**FROM DEFEND** (85 transitions):
  - `→ fear_downtrend`: 58.8%
  - `→ CAUTION`: 14.1%
  - `→ optimal`: 10.6%
**FROM optimal** (428 transitions):
  - `→ cruise`: 61.0%
  - `→ pullback`: 33.6%
  - `→ CAUTION`: 3.5%
**FROM CAUTION** (63 transitions):
  - `→ fear_downtrend`: 31.7%
  - `→ cruise`: 19.0%
  - `→ optimal`: 15.9%
**FROM pullback** (177 transitions):
  - `→ optimal`: 62.1%
  - `→ fear_1`: 23.7%
  - `→ CAUTION`: 6.8%
**FROM fear_downtrend** (84 transitions):
  - `→ DEFEND`: 81.0%
  - `→ CAUTION`: 19.0%
**FROM cruise** (501 transitions):
  - `→ optimal`: 58.9%
  - `→ overheat_1`: 39.5%
  - `→ pullback`: 0.8%
**FROM overheat_1** (285 transitions):
  - `→ cruise`: 77.2%
  - `→ overheat_2`: 22.1%
  - `→ optimal`: 0.7%
