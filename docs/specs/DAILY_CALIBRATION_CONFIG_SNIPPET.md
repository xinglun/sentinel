---
author: Ray
title: 日次認知校正設定スニペット
description: daily-calibration 用の research_attention と asset_thesis 設定例。
key: daily-calibration-config-snippet
---

# 日次認知校正設定スニペット

`config.toml` はローカル secrets / runtime config を含む可能性があるため、AI guard では直接編集を禁止している。以下は `config.toml` の末尾へ手動で追加するための設定スニペットであり、Gate や実行層には接続しない。

```toml
[research_attention.NVDA]
cognitive_yield = "HIGH"
attention_cost = "HIGH"
information_density = "ACTIVE"
reason = "AI インフラ需要、供給制約、粗利率、データセンター投資回収の変化率が高い。"

[research_attention.GOOG]
cognitive_yield = "MEDIUM"
attention_cost = "MODERATE"
information_density = "STABLE"
reason = "AI 収益化と検索防衛の論点は重要だが、長期ロジックは市場にかなり理解されている。"

[research_attention.MSFT]
cognitive_yield = "MEDIUM"
attention_cost = "LOW"
information_density = "STABLE"
reason = "クラウド、Copilot、データセンター投資の実行力は高いが、情報変化率は比較的安定している。"

[research_attention.TSLA]
cognitive_yield = "HIGH"
attention_cost = "HIGH"
information_density = "EXPANDING"
reason = "FSD、Robotaxi、Physical AI、製造自動化の不確実性が高く、認知増分も大きい。"

[research_attention.PLTR]
cognitive_yield = "HIGH"
attention_cost = "MODERATE"
information_density = "EXPANDING"
reason = "Ontology、企業 AI 組織化、AI ガバナンスの進化を観測する価値が高い。"

[research_attention.ISRG]
cognitive_yield = "MEDIUM"
attention_cost = "LOW"
information_density = "STABLE"
reason = "優良企業だが、価格と成長品質のバランスを観測するサンプルとして扱う。"

[research_attention.U]
cognitive_yield = "MEDIUM"
attention_cost = "MODERATE"
information_density = "ACTIVE"
reason = "低位修復後の再評価、実需、収益構造の改善が続くかを観測する。"

[research_attention.SPY]
cognitive_yield = "LOW"
attention_cost = "LOW"
information_density = "STABLE"
reason = "個別テーマではなく市場構造、流動性、指数トレンドの基準線として観測する。"

[asset_thesis.NVDA]
thesis = "AI インフラ需要が継続し、データセンター投資が収益へ転換し続けるかを観測する。"
observation_focus = ["データセンター注文と供給制約の継続性", "粗利率、在庫、次世代 GPU 移行の質", "クラウド各社 Capex と半導体需要の接続"]
invalidation = ["主要クラウドの Capex 減速が継続する", "注文可視性または粗利率が明確に悪化する", "AI インフラ投資の回収可能性が弱まる"]

[asset_thesis.GOOG]
thesis = "AI 商業化が検索、クラウド、広告の利益構造へ定着し、長期収益力を強化するかを観測する。"
observation_focus = ["AI Overviews と検索広告収益の両立", "Google Cloud の成長と利益率", "Gemini / TPU / 内製インフラの投資回収"]
invalidation = ["AI 投資が利益率を継続的に圧迫する", "検索広告の防衛力が低下する", "クラウド成長が明確に鈍化する"]

[asset_thesis.MSFT]
thesis = "Azure、Copilot、企業 AI 導入がデータセンター投資を正当化し続けるかを観測する。"
observation_focus = ["Azure 成長率と AI 寄与", "Copilot の実利用と単価改善", "Capex 増加と営業利益率のバランス"]
invalidation = ["AI 関連 Capex が収益成長に接続しない", "Copilot の導入が期待を下回る", "クラウド成長が構造的に鈍化する"]
time_horizon = "LONG"
materialization_window = "12-36 months"

[asset_thesis.MSFT.narrative_state]
consensus_level = "CROWDED"
skepticism_level = "LOW"
valuation_reflection = "PARTIAL"

[asset_thesis.MSFT.reality_override]
observable_contradiction = true
confidence_decay = true

[asset_thesis.TSLA]
thesis = "EV 企業から Physical AI / 自動運転 / ロボティクス企業へ転換できるかを観測する。"
observation_focus = ["FSD と Robotaxi の商用化進展", "EV 価格競争と粗利率", "Optimus と製造自動化の現実性"]
invalidation = ["FSD / Robotaxi の商用化が遅延し続ける", "EV 事業の利益率低下が止まらない", "Physical AI の証拠が価格期待に追いつかない"]

[asset_thesis.PLTR]
thesis = "企業 AI が単なるチャットではなく、Ontology と業務 OS として組織に定着するかを観測する。"
observation_focus = ["AIP 導入の継続性と商用売上成長", "顧客拡大と既存顧客の利用深度", "AI ガバナンスと業務プロセス統合"]
invalidation = ["売上成長が期待に対して鈍化する", "AIP の導入が実験段階に留まる", "高バリュエーションを支える証拠が不足する"]

[asset_thesis.ISRG]
thesis = "優良企業としての成長品質が、現在の価格と期待を正当化できるかを観測する。"
observation_focus = ["da Vinci 導入台数と手術件数の成長", "消耗品収益と利益率の安定性", "価格が成長品質に対して過度に先行していないか"]
invalidation = ["手術件数または導入台数の成長が鈍化する", "競争または規制で利益率が低下する", "良い会社だが良い価格ではない状態が長期化する"]

[asset_thesis.U]
thesis = "低位修復後の価格再評価が、実需と収益構造改善で継続できるかを観測する。"
observation_focus = ["ゲーム以外の利用拡大", "収益性改善とコスト構造", "修復相場から構造成長へ移れるか"]
invalidation = ["収益改善が一時的に終わる", "主要顧客または開発者エコシステムが弱まる", "価格修復だけで実体が追いつかない"]

[asset_thesis.SPY]
thesis = "個別銘柄ではなく、市場全体の流動性、指数トレンド、リスクオン/オフの基準線として観測する。"
observation_focus = ["指数トレンドと breadth の関係", "Mega-cap leadership と市場全体の乖離", "金利、流動性、リスク許容度"]
invalidation = ["指数が長期トレンドを明確に割り込む", "breadth の悪化がリーダー資産へ波及する", "流動性環境が構造的に悪化する"]
```

## Macro Gravity 追加例

```toml
[macro_gravity]
rate_pressure = "RISING"
real_yield_pressure = "TIGHT"
yield_curve = "FLAT"
credit_stress = "NORMAL"
liquidity = "NEUTRAL"
growth_valuation_impact = "COMPRESSING"
# note は内部メモ用途。ユーザー向けレポートには直接表示しない。
# note = "債券市場は AI / Mega-cap の構造判断ではなく、割引率と時間コストの重力として観測する。"
```
