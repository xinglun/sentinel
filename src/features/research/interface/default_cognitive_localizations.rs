/// 標準 watchlist の中国語向け認知観測説明を返す。
pub(super) fn research_reason_zh(symbol: &str, original: &str) -> Option<&'static str> {
    match (symbol, original) {
        ("NVDA", "AI インフラ需要、供給制約、粗利率、データセンター投資回収の変化率が高い。") => {
            Some("AI 基础设施需求、供应约束、毛利率与数据中心投资回报的变化率较高。")
        }
        ("GOOG", "AI 収益化と検索防衛の論点は重要だが、長期ロジックは市場にかなり理解されている。") => {
            Some("AI 变现与搜索防御仍然重要，但长期逻辑已被市场较充分理解。")
        }
        ("MSFT", "クラウド、Copilot、データセンター投資の実行力は高いが、情報変化率は比較的安定している。") => {
            Some("云服务、Copilot 与数据中心投资执行力较强，但信息变化率相对稳定。")
        }
        ("TSLA", "FSD、Robotaxi、Physical AI、製造自動化の不確実性が高く、認知増分も大きい。") => {
            Some("FSD、Robotaxi、Physical AI 与制造自动化仍高度不确定，认知增量也较高。")
        }
        ("PLTR", "Ontology、企業 AI 組織化、AI ガバナンスの進化を観測する価値が高い。") => {
            Some("Ontology、企业 AI 组织化与 AI 治理仍在演进，具有持续观察价值。")
        }
        ("ISRG", "優良企業だが、価格と成長品質のバランスを観測するサンプルとして扱う。") => {
            Some("公司质量优秀，适合作为价格与成长质量平衡的观察样本。")
        }
        ("U", "低位修復後の再評価、実需、収益構造の改善が続くかを観測する。") => {
            Some("观察低位修复后的再评价、真实需求与盈利结构改善能否延续。")
        }
        ("SPCX", "宇宙輸送、Starlink、衛星通信、政府契約、AI / compute infrastructure との接続が公開市場でどのように評価されるかを観測する価値が高い。") => {
            Some("观察 SpaceX 在宇宙运输、Starlink、卫星通信、政府合同，以及 AI / compute infrastructure 连接上如何被公开市场定价。")
        }
        ("SPY", "個別テーマではなく市場構造、流動性、指数トレンドの基準線として観測する。") => {
            Some("作为市场结构、流动性与指数趋势的基准线，而非个别主题进行观察。")
        }
        _ => None,
    }
}

/// 標準 watchlist の英語向け認知観測説明を返す。
pub(super) fn research_reason_en(symbol: &str, original: &str) -> Option<&'static str> {
    match (symbol, original) {
        ("NVDA", "AI インフラ需要、供給制約、粗利率、データセンター投資回収の変化率が高い。") => {
            Some("AI infrastructure demand, supply constraints, gross margin, and data-center investment returns continue to change rapidly.")
        }
        ("GOOG", "AI 収益化と検索防衛の論点は重要だが、長期ロジックは市場にかなり理解されている。") => {
            Some("AI monetization and search defense remain important, while the long-term thesis is already broadly understood by the market.")
        }
        ("MSFT", "クラウド、Copilot、データセンター投資の実行力は高いが、情報変化率は比較的安定している。") => {
            Some("Cloud, Copilot, and data-center investment execution are strong, while the rate of new information is comparatively stable.")
        }
        ("TSLA", "FSD、Robotaxi、Physical AI、製造自動化の不確実性が高く、認知増分も大きい。") => {
            Some("FSD, Robotaxi, Physical AI, and manufacturing automation remain highly uncertain and continue to offer substantial learning value.")
        }
        ("PLTR", "Ontology、企業 AI 組織化、AI ガバナンスの進化を観測する価値が高い。") => {
            Some("Ontology, enterprise AI organization, and AI governance continue evolving and warrant sustained observation.")
        }
        ("ISRG", "優良企業だが、価格と成長品質のバランスを観測するサンプルとして扱う。") => {
            Some("A high-quality company used as a sample for observing the balance between price and growth quality.")
        }
        ("U", "低位修復後の再評価、実需、収益構造の改善が続くかを観測する。") => {
            Some("Observe whether rerating after the low-base recovery is sustained by real demand and improving earnings structure.")
        }
        ("SPCX", "宇宙輸送、Starlink、衛星通信、政府契約、AI / compute infrastructure との接続が公開市場でどのように評価されるかを観測する価値が高い。") => {
            Some("Observe how public markets price SpaceX across launch, Starlink, satellite communications, government contracts, and its connection to AI / compute infrastructure.")
        }
        ("SPY", "個別テーマではなく市場構造、流動性、指数トレンドの基準線として観測する。") => {
            Some("Observe it as a baseline for market structure, liquidity, and index trends rather than as an individual theme.")
        }
        _ => None,
    }
}

/// 標準 watchlist の中国語向け観測命題を返す。
pub(super) fn asset_thesis_zh(symbol: &str, original: &str) -> Option<&'static str> {
    match (symbol, original) {
        ("NVDA", "AI インフラ需要が継続し、データセンター投資が収益へ転換し続けるかを観測する。") => {
            Some("观察 AI 基础设施需求能否延续，以及数据中心投资是否继续转化为收益。")
        }
        ("GOOG", "AI 商業化が検索、クラウド、広告の利益構造へ定着し、長期収益力を強化するかを観測する。") => {
            Some("观察 AI 商业化能否沉淀到搜索、云服务和广告的利润结构中，并强化长期盈利能力。")
        }
        ("MSFT", "Azure、Copilot、企業 AI 導入がデータセンター投資を正当化し続けるかを観測する。") => {
            Some("观察 Azure、Copilot 与企业 AI 导入能否持续证明数据中心投资的合理性。")
        }
        ("TSLA", "EV 企業から Physical AI / 自動運転 / ロボティクス企業へ転換できるかを観測する。") => {
            Some("观察其能否从电动车企业转变为 Physical AI、自动驾驶与机器人企业。")
        }
        ("PLTR", "企業 AI が単なるチャットではなく、Ontology と業務 OS として組織に定着するかを観測する。") => {
            Some("观察企业 AI 能否不止于聊天工具，而作为 Ontology 与业务操作系统沉淀到组织中。")
        }
        ("ISRG", "優良企業としての成長品質が、現在の価格と期待を正当化できるかを観測する。") => {
            Some("观察作为优质公司的成长质量，能否证明当前价格与预期合理。")
        }
        ("U", "低位修復後の価格再評価が、実需と収益構造改善で継続できるかを観測する。") => {
            Some("观察低位修复后的价格再评价，能否由真实需求与盈利结构改善延续。")
        }
        ("SPCX", "SpaceX が宇宙輸送、Starlink、衛星通信、政府契約を通じて、長期インフラ企業として公開市場で評価されるかを観測する。") => {
            Some("观察 SpaceX 是否能通过宇宙运输、Starlink、卫星通信和政府合同，被公开市场作为长期基础设施企业重新定价。")
        }
        ("SPY", "個別銘柄ではなく、市場全体の流動性、指数トレンド、リスクオン/オフの基準線として観測する。") => {
            Some("不作为个别标的，而作为全市场流动性、指数趋势与风险偏好的基准线观察。")
        }
        _ => None,
    }
}

/// 標準 watchlist の英語向け観測命題を返す。
pub(super) fn asset_thesis_en(symbol: &str, original: &str) -> Option<&'static str> {
    match (symbol, original) {
        ("NVDA", "AI インフラ需要が継続し、データセンター投資が収益へ転換し続けるかを観測する。") => {
            Some("Observe whether AI infrastructure demand persists and data-center investment continues converting into earnings.")
        }
        ("GOOG", "AI 商業化が検索、クラウド、広告の利益構造へ定着し、長期収益力を強化するかを観測する。") => {
            Some("Observe whether AI commercialization becomes embedded in search, cloud, and advertising profit structures and strengthens long-term earnings power.")
        }
        ("MSFT", "Azure、Copilot、企業 AI 導入がデータセンター投資を正当化し続けるかを観測する。") => {
            Some("Observe whether Azure, Copilot, and enterprise AI adoption continue to justify data-center investment.")
        }
        ("TSLA", "EV 企業から Physical AI / 自動運転 / ロボティクス企業へ転換できるかを観測する。") => {
            Some("Observe whether it can transition from an EV company into a Physical AI, autonomous driving, and robotics company.")
        }
        ("PLTR", "企業 AI が単なるチャットではなく、Ontology と業務 OS として組織に定着するかを観測する。") => {
            Some("Observe whether enterprise AI becomes embedded in organizations as an ontology and operating system rather than merely a chat tool.")
        }
        ("ISRG", "優良企業としての成長品質が、現在の価格と期待を正当化できるかを観測する。") => {
            Some("Observe whether the growth quality of a high-quality company can justify its current price and expectations.")
        }
        ("U", "低位修復後の価格再評価が、実需と収益構造改善で継続できるかを観測する。") => {
            Some("Observe whether rerating after the low-base recovery can continue through real demand and an improving earnings structure.")
        }
        ("SPCX", "SpaceX が宇宙輸送、Starlink、衛星通信、政府契約を通じて、長期インフラ企業として公開市場で評価されるかを観測する。") => {
            Some("Observe whether SpaceX can be valued by public markets as a long-term infrastructure company through launch, Starlink, satellite communications, and government contracts.")
        }
        ("SPY", "個別銘柄ではなく、市場全体の流動性、指数トレンド、リスクオン/オフの基準線として観測する。") => {
            Some("Observe it not as an individual security, but as a baseline for market-wide liquidity, index trends, and risk appetite.")
        }
        _ => None,
    }
}

/// 標準 watchlist の中国語向け観測焦点を返す。
pub(super) fn observation_focus_zh(symbol: &str, thesis: &str) -> Option<Vec<String>> {
    asset_thesis_zh(symbol, thesis)?;
    match symbol {
        "NVDA" => Some(strings(&[
            "数据中心订单与供应约束的持续性",
            "毛利率、库存与下一代 GPU 迁移质量",
            "云厂商资本开支与半导体需求的连接",
        ])),
        "GOOG" => Some(strings(&[
            "AI Overviews 与搜索广告收入能否并存",
            "Google Cloud 的成长与利润率",
            "Gemini / TPU / 自建基础设施的投资回报",
        ])),
        "MSFT" => Some(strings(&[
            "Azure 成长率与 AI 贡献",
            "Copilot 的实际使用与单价改善",
            "资本开支增加与营业利润率的平衡",
        ])),
        "TSLA" => Some(strings(&[
            "FSD 与 Robotaxi 的商业化进展",
            "电动车价格竞争与毛利率",
            "Optimus 与制造自动化的现实性",
        ])),
        "PLTR" => Some(strings(&[
            "AIP 导入持续性与商业收入成长",
            "客户扩展与存量客户使用深度",
            "AI 治理与业务流程整合",
        ])),
        "ISRG" => Some(strings(&[
            "da Vinci 导入台数与手术件数成长",
            "耗材收入与利润率稳定性",
            "价格是否过度领先于成长质量",
        ])),
        "U" => Some(strings(&[
            "游戏以外使用场景扩展",
            "盈利能力改善与成本结构",
            "能否从修复行情转向结构成长",
        ])),
        "SPCX" => Some(strings(&[
            "Starlink 的增长率与利润率",
            "发射业务的价格竞争力",
            "政府与防务合同的持续性",
            "IPO 后的供需、lockup 与流通股结构",
            "Tesla / xAI / Elon Musk 生态对市场心理的联动",
        ])),
        "SPY" => Some(strings(&[
            "指数趋势与市场广度的关系",
            "超大盘领导资产与全市场的偏离",
            "利率、流动性与风险承受度",
        ])),
        _ => None,
    }
}

/// 標準 watchlist の英語向け観測焦点を返す。
pub(super) fn observation_focus_en(symbol: &str, thesis: &str) -> Option<Vec<String>> {
    asset_thesis_en(symbol, thesis)?;
    match symbol {
        "NVDA" => Some(strings(&[
            "Persistence of data-center orders and supply constraints",
            "Gross margin, inventory, and quality of next-generation GPU migration",
            "Connection between hyperscaler capital spending and semiconductor demand",
        ])),
        "GOOG" => Some(strings(&[
            "Compatibility of AI Overviews with search advertising revenue",
            "Google Cloud growth and profitability",
            "Investment returns from Gemini, TPU, and internal infrastructure",
        ])),
        "MSFT" => Some(strings(&[
            "Azure growth and AI contribution",
            "Actual Copilot usage and pricing improvement",
            "Balance between capital spending growth and operating margin",
        ])),
        "TSLA" => Some(strings(&[
            "Commercialization progress of FSD and Robotaxi",
            "EV price competition and gross margin",
            "Feasibility of Optimus and manufacturing automation",
        ])),
        "PLTR" => Some(strings(&[
            "Durability of AIP adoption and commercial revenue growth",
            "Customer expansion and depth of existing customer usage",
            "Integration of AI governance and business processes",
        ])),
        "ISRG" => Some(strings(&[
            "Growth in da Vinci installations and procedure counts",
            "Stability of consumables revenue and margins",
            "Whether price is running too far ahead of growth quality",
        ])),
        "U" => Some(strings(&[
            "Expansion of use cases beyond games",
            "Profitability improvement and cost structure",
            "Ability to shift from recovery rally to structural growth",
        ])),
        "SPCX" => Some(strings(&[
            "Starlink growth rate and profitability",
            "Launch business pricing power and cost competitiveness",
            "Continuity of government and defense contracts",
            "Post-IPO supply-demand, lockup, and public float structure",
            "Market psychology linkage with Tesla / xAI / Elon Musk ecosystem",
        ])),
        "SPY" => Some(strings(&[
            "Relationship between index trend and market breadth",
            "Divergence between mega-cap leadership and the broader market",
            "Rates, liquidity, and risk tolerance",
        ])),
        _ => None,
    }
}

/// 標準 watchlist の中国語向け失効条件を返す。
pub(super) fn invalidation_zh(symbol: &str, thesis: &str) -> Option<Vec<String>> {
    asset_thesis_zh(symbol, thesis)?;
    match symbol {
        "NVDA" => Some(strings(&[
            "主要云厂商资本开支持续减速",
            "订单可见度或毛利率明确恶化",
            "AI 基础设施投资回收可能性减弱",
        ])),
        "GOOG" => Some(strings(&[
            "AI 投资持续压制利润率",
            "搜索广告防御能力下降",
            "云业务成长明确放缓",
        ])),
        "MSFT" => Some(strings(&[
            "AI 相关资本开支未能连接收入成长",
            "Copilot 导入低于预期",
            "云业务成长出现结构性放缓",
        ])),
        "TSLA" => Some(strings(&[
            "FSD / Robotaxi 商业化持续延迟",
            "电动车业务利润率下滑无法止住",
            "Physical AI 证据追不上价格预期",
        ])),
        "PLTR" => Some(strings(&[
            "收入成长相对预期放缓",
            "AIP 导入停留在实验阶段",
            "支撑高估值的证据不足",
        ])),
        "ISRG" => Some(strings(&[
            "手术件数或导入台数成长放缓",
            "竞争或监管导致利润率下降",
            "好公司但非好价格的状态长期化",
        ])),
        "U" => Some(strings(&[
            "盈利改善只是短期现象",
            "主要客户或开发者生态减弱",
            "只有价格修复而实体没有跟上",
        ])),
        "SPCX" => Some(strings(&[
            "Starlink 增长或利润率低于预期",
            "发射竞争或监管导致盈利能力下降",
            "公开市场预期明显跑在实际收入前面",
            "治理结构或关键人物依赖压制估值",
        ])),
        "SPY" => Some(strings(&[
            "指数明确跌破长期趋势",
            "市场广度恶化波及领导资产",
            "流动性环境结构性恶化",
        ])),
        _ => None,
    }
}

/// 標準 watchlist の英語向け失効条件を返す。
pub(super) fn invalidation_en(symbol: &str, thesis: &str) -> Option<Vec<String>> {
    asset_thesis_en(symbol, thesis)?;
    match symbol {
        "NVDA" => Some(strings(&[
            "Sustained slowdown in major hyperscaler capital spending",
            "Clear deterioration in order visibility or gross margin",
            "Weakening prospects for returns on AI infrastructure investment",
        ])),
        "GOOG" => Some(strings(&[
            "AI investment persistently compresses margin",
            "Search advertising defense weakens",
            "Cloud growth clearly slows",
        ])),
        "MSFT" => Some(strings(&[
            "AI-related capital spending fails to connect to revenue growth",
            "Copilot adoption falls short of expectations",
            "Cloud growth structurally slows",
        ])),
        "TSLA" => Some(strings(&[
            "Commercialization of FSD or Robotaxi remains delayed",
            "EV business margin decline fails to stabilize",
            "Physical AI evidence cannot catch up with price expectations",
        ])),
        "PLTR" => Some(strings(&[
            "Revenue growth slows relative to expectations",
            "AIP adoption remains at the experiment stage",
            "Evidence supporting high valuation becomes insufficient",
        ])),
        "ISRG" => Some(strings(&[
            "Procedure or installation growth slows",
            "Competition or regulation reduces profitability",
            "The condition of a good company at an unattractive price persists",
        ])),
        "U" => Some(strings(&[
            "Profitability improvement proves temporary",
            "Major customers or the developer ecosystem weaken",
            "Price recovery proceeds without fundamental follow-through",
        ])),
        "SPCX" => Some(strings(&[
            "Starlink growth or profitability falls below expectations",
            "Launch competition or regulation weakens profitability",
            "Public market expectations move too far ahead of actual revenue",
            "Governance or key-person dependency pressures valuation",
        ])),
        "SPY" => Some(strings(&[
            "The index clearly breaks its long-term trend",
            "Weak breadth spreads into leadership assets",
            "The liquidity environment structurally deteriorates",
        ])),
        _ => None,
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}
