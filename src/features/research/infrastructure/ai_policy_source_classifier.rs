#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AiPolicyClassification {
    pub(crate) policy_type: &'static str,
    pub(crate) policy_stage: &'static str,
}

/// AI Policy の候補を token 境界と政策主体・作用の組み合わせで分類する。
pub(crate) fn classify_ai_policy_text(text: &str) -> Option<AiPolicyClassification> {
    let text = normalize_text(text)?;
    let has_ai_subject = contains_any(
        &text,
        &[
            "ai",
            "artificial intelligence",
            "advanced ai",
            "frontier ai",
            "frontier model",
            "foundation model",
            "language model",
        ],
    );
    let has_policy_target = contains_any(
        &text,
        &[
            "development",
            "training",
            "compute",
            "deployment",
            "release",
            "model",
            "safety",
            "regulation",
            "restriction",
            "rulemaking",
            "standards",
            "governance",
            "bill",
            "legislation",
        ],
    );
    let has_policy_action = contains_any(
        &text,
        &[
            "slow",
            "slowing",
            "slowdown",
            "pause",
            "paused",
            "pauses",
            "restrict",
            "restriction",
            "limit",
            "limits",
            "ban",
            "bans",
            "regulate",
            "regulation",
            "introduce",
            "introduced",
            "propose",
            "proposed",
            "proposal",
            "call for",
            "calls for",
            "urge",
            "urges",
            "enact",
            "enacts",
            "enacted",
            "effective",
            "rulemaking",
            "coordinate",
            "coordinates",
            "coordination",
            "discuss",
            "discussion",
            "debate",
            "debates",
            "consider",
            "considers",
        ],
    );
    if !has_ai_subject || !has_policy_target || !has_policy_action {
        return None;
    }

    let policy_stage = resolve_policy_stage(&text)?;
    let policy_type = resolve_policy_type(&text, policy_stage)?;
    Some(AiPolicyClassification {
        policy_type,
        policy_stage,
    })
}

fn resolve_policy_stage(text: &str) -> Option<&'static str> {
    if contains_any(
        text,
        &["enact", "enacts", "enacted", "signed into law", "law takes effect"],
    ) {
        return Some("ENACTED");
    }
    if contains_any(text, &["effective", "enters into force", "in force"]) {
        return Some("EFFECTIVE");
    }
    if contains_any(
        text,
        &["formal rulemaking", "proposed rule", "agency rule", "regulatory rule"],
    ) {
        return Some("FORMAL_RULEMAKING");
    }
    if contains_any(
        text,
        &[
            "lawmakers",
            "legislators",
            "legislative",
            "bill",
            "legislation",
            "introduce bill",
            "introduced bill",
        ],
    ) {
        return Some("LEGISLATIVE_PROPOSAL");
    }
    if contains_any(
        text,
        &[
            "industry leaders",
            "tech leaders",
            "company leaders",
            "companies",
            "developers",
            "industry",
        ],
    ) && contains_any(
        text,
        &["call for", "calls for", "urge", "urges", "propose", "proposed"],
    ) {
        return Some("INDUSTRY_PROPOSAL");
    }
    if contains_any(
        text,
        &[
            "discuss",
            "discussion",
            "debate",
            "debates",
            "talks",
            "consider",
            "considers",
            "concern",
            "concerns",
            "coordinate",
            "coordinates",
            "coordination",
        ],
    ) {
        return Some("DISCUSSION");
    }
    None
}

fn resolve_policy_type(text: &str, policy_stage: &str) -> Option<&'static str> {
    if policy_stage == "LEGISLATIVE_PROPOSAL" {
        return Some("LEGISLATIVE_PROPOSAL");
    }
    if contains_any(text, &["deployment", "deploy"]) && contains_any(text, &[
        "restrict", "restriction", "limit", "limits", "ban", "bans", "pause", "paused",
        "pauses",
    ]) {
        return Some("DEPLOYMENT_RESTRICTION");
    }
    if contains_any(text, &["release", "model release"]) && contains_any(text, &[
        "restrict", "restriction", "limit", "limits", "ban", "bans", "pause", "paused",
        "pauses",
    ]) {
        return Some("MODEL_RELEASE_RESTRICTION");
    }
    if contains_any(text, &["training", "compute"]) && contains_any(text, &[
        "restrict", "restriction", "limit", "limits", "ban", "bans", "pause", "paused",
        "pauses",
    ]) {
        return Some("TRAINING_COMPUTE_RESTRICTION");
    }
    if contains_any(text, &["safety", "standards", "governance"])
        && contains_any(text, &["coordinate", "coordinates", "coordination", "agreement", "accord"])
    {
        return Some("AI_SAFETY_COORDINATION");
    }
    if contains_any(text, &["frontier", "advanced", "development"])
        && contains_any(text, &["slow", "slowing", "slowdown", "pause", "paused", "pauses"])
    {
        return Some("FRONTIER_PACING");
    }
    None
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    let text_tokens = lexical_tokens(text);
    keywords.iter().any(|keyword| {
        let keyword_tokens = lexical_tokens(keyword);
        !keyword_tokens.is_empty()
            && text_tokens
                .windows(keyword_tokens.len())
                .any(|window| window == keyword_tokens.as_slice())
    })
}

fn normalize_text(text: &str) -> Option<String> {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty()
        || normalized.contains('\u{fffd}')
        || normalized
            .chars()
            .any(|character| character.is_control() && !character.is_whitespace())
        || !normalized
            .chars()
            .any(|character| character.is_alphanumeric())
    {
        return None;
    }
    Some(normalized)
}

fn lexical_tokens(text: &str) -> Vec<&str> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::classify_ai_policy_text;

    #[test]
    fn classifies_industry_frontier_pacing_proposal() {
        let classification = classify_ai_policy_text(
            "industry leaders call for slowing frontier AI development",
        )
        .expect("industry proposal should be classified");

        assert_eq!(classification.policy_type, "FRONTIER_PACING");
        assert_eq!(classification.policy_stage, "INDUSTRY_PROPOSAL");
    }

    #[test]
    fn classifies_legislative_advanced_ai_proposal() {
        let classification = classify_ai_policy_text(
            "lawmakers introduce bill to pause advanced AI development",
        )
        .expect("legislative proposal should be classified");

        assert_eq!(classification.policy_type, "LEGISLATIVE_PROPOSAL");
        assert_eq!(classification.policy_stage, "LEGISLATIVE_PROPOSAL");
    }

    #[test]
    fn classifies_enacted_deployment_restriction() {
        let classification = classify_ai_policy_text(
            "government enacts an advanced AI deployment restriction",
        )
        .expect("enacted restriction should be classified");

        assert_eq!(classification.policy_type, "DEPLOYMENT_RESTRICTION");
        assert_eq!(classification.policy_stage, "ENACTED");
    }

    #[test]
    fn unrelated_ai_business_news_is_not_an_ai_policy_observation() {
        assert!(classify_ai_policy_text("AI startup pauses hiring").is_none());
    }

    #[test]
    fn malformed_text_fails_closed() {
        for text in ["", "   ", "\u{fffd}", "\u{0} AI policy"] {
            assert!(classify_ai_policy_text(text).is_none(), "{text:?}");
        }
    }
}
