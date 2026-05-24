#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RhinoEscalationState {
    Background,
    Visible,
    Expanding,
    Normalized,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Moderate,
    Elevated,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayRhinoEscalationInput {
    pub risk_expansion_rate: RiskLevel,
    pub constraint_growth_rate: RiskLevel,
    pub dependency_centralization: RiskLevel,
    pub awareness_decay: RiskLevel,
    pub narrative_overconfidence: RiskLevel,
    pub single_point_fragility: RiskLevel,
    pub fallback_survivability_risk: RiskLevel,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayRhinoEscalation {
    pub escalation_state: RhinoEscalationState,
    pub risk_expansion_rate: RiskLevel,
    pub constraint_growth_rate: RiskLevel,
    pub dependency_centralization: RiskLevel,
    pub awareness_decay: RiskLevel,
    pub narrative_overconfidence: RiskLevel,
    pub single_point_fragility: RiskLevel,
    pub fallback_survivability_risk: RiskLevel,
    pub notes: Vec<String>,
    pub suppressed_note_count: usize,
}

impl RiskLevel {
    pub fn score(self) -> i32 {
        match self {
            Self::Low => 0,
            Self::Moderate => 1,
            Self::Elevated => 2,
            Self::High => 3,
        }
    }

    pub fn is_elevated_or_higher(self) -> bool {
        matches!(self, Self::Elevated | Self::High)
    }
}

impl GrayRhinoEscalation {
    pub fn escalation_score(&self) -> i32 {
        self.risk_expansion_rate.score()
            + self.dependency_centralization.score()
            + self.awareness_decay.score()
            + self.narrative_overconfidence.score()
            + self.single_point_fragility.score()
            + self.fallback_survivability_risk.score()
            - self.constraint_growth_rate.score()
    }

    pub fn is_awareness_decay_detected(&self) -> bool {
        self.awareness_decay.is_elevated_or_higher()
    }

    pub fn is_dependency_concentrated(&self) -> bool {
        self.dependency_centralization.is_elevated_or_higher()
    }
}

pub fn evaluate_gray_rhino_escalation(input: GrayRhinoEscalationInput) -> GrayRhinoEscalation {
    let safe_notes: Vec<String> = input
        .notes
        .iter()
        .filter(|note| is_structural_observation_note_allowed(note))
        .cloned()
        .collect();
    let suppressed_note_count = input.notes.len().saturating_sub(safe_notes.len());
    let score = input.risk_expansion_rate.score()
        + input.dependency_centralization.score()
        + input.awareness_decay.score()
        + input.narrative_overconfidence.score()
        + input.single_point_fragility.score()
        + input.fallback_survivability_risk.score()
        - input.constraint_growth_rate.score();

    let critical_single_point = input.risk_expansion_rate == RiskLevel::High
        && input.dependency_centralization == RiskLevel::High
        && input.single_point_fragility == RiskLevel::High
        && input.fallback_survivability_risk.is_elevated_or_higher()
        && matches!(
            input.constraint_growth_rate,
            RiskLevel::Low | RiskLevel::Moderate
        );
    let normalized_blindness = input.risk_expansion_rate.is_elevated_or_higher()
        && input.awareness_decay.is_elevated_or_higher()
        && input.narrative_overconfidence.is_elevated_or_higher();

    let escalation_state = if critical_single_point {
        RhinoEscalationState::Critical
    } else if normalized_blindness && score >= 5 {
        RhinoEscalationState::Normalized
    } else if score >= 4 {
        RhinoEscalationState::Expanding
    } else if score >= 2 || input.risk_expansion_rate.is_elevated_or_higher() {
        RhinoEscalationState::Visible
    } else {
        RhinoEscalationState::Background
    };

    GrayRhinoEscalation {
        escalation_state,
        risk_expansion_rate: input.risk_expansion_rate,
        constraint_growth_rate: input.constraint_growth_rate,
        dependency_centralization: input.dependency_centralization,
        awareness_decay: input.awareness_decay,
        narrative_overconfidence: input.narrative_overconfidence,
        single_point_fragility: input.single_point_fragility,
        fallback_survivability_risk: input.fallback_survivability_risk,
        notes: safe_notes,
        suppressed_note_count,
    }
}

fn is_structural_observation_note_allowed(note: &str) -> bool {
    let lower = note.to_lowercase();
    !forbidden_note_terms()
        .iter()
        .any(|term| lower.contains(term))
}

fn forbidden_note_terms() -> &'static [&'static str] {
    &[
        "buy",
        "sell",
        "gate",
        "execution",
        "trend_cohesion",
        "bearish",
        "conspiracy",
        "musk",
        "买入",
        "卖出",
        "買入",
        "売却",
        "马上卖出",
        "崩塌",
        "泡沫",
        "政治",
        "人格",
        "陰謀",
        "崩壊",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(
        risk_expansion_rate: RiskLevel,
        constraint_growth_rate: RiskLevel,
        dependency_centralization: RiskLevel,
        awareness_decay: RiskLevel,
        narrative_overconfidence: RiskLevel,
        single_point_fragility: RiskLevel,
        fallback_survivability_risk: RiskLevel,
    ) -> GrayRhinoEscalationInput {
        GrayRhinoEscalationInput {
            risk_expansion_rate,
            constraint_growth_rate,
            dependency_centralization,
            awareness_decay,
            narrative_overconfidence,
            single_point_fragility,
            fallback_survivability_risk,
            notes: Vec::new(),
        }
    }

    #[test]
    fn escalation_state_transitions_from_background_to_expanding() {
        let background = evaluate_gray_rhino_escalation(input(
            RiskLevel::Low,
            RiskLevel::Moderate,
            RiskLevel::Low,
            RiskLevel::Low,
            RiskLevel::Low,
            RiskLevel::Low,
            RiskLevel::Low,
        ));
        let expanding = evaluate_gray_rhino_escalation(input(
            RiskLevel::Elevated,
            RiskLevel::Moderate,
            RiskLevel::Elevated,
            RiskLevel::Moderate,
            RiskLevel::Moderate,
            RiskLevel::Moderate,
            RiskLevel::Moderate,
        ));

        assert_eq!(
            background.escalation_state,
            RhinoEscalationState::Background
        );
        assert_eq!(expanding.escalation_state, RhinoEscalationState::Expanding);
    }

    #[test]
    fn normalized_state_detects_success_amplified_blindness() {
        let escalation = evaluate_gray_rhino_escalation(input(
            RiskLevel::Elevated,
            RiskLevel::Low,
            RiskLevel::Elevated,
            RiskLevel::High,
            RiskLevel::Elevated,
            RiskLevel::Moderate,
            RiskLevel::Moderate,
        ));

        assert_eq!(
            escalation.escalation_state,
            RhinoEscalationState::Normalized
        );
        assert!(escalation.is_awareness_decay_detected());
    }

    #[test]
    fn critical_state_requires_single_point_and_fallback_survivability_risk() {
        let normalized = evaluate_gray_rhino_escalation(input(
            RiskLevel::High,
            RiskLevel::Low,
            RiskLevel::High,
            RiskLevel::High,
            RiskLevel::High,
            RiskLevel::Moderate,
            RiskLevel::Moderate,
        ));
        let critical = evaluate_gray_rhino_escalation(input(
            RiskLevel::High,
            RiskLevel::Low,
            RiskLevel::High,
            RiskLevel::High,
            RiskLevel::High,
            RiskLevel::High,
            RiskLevel::High,
        ));

        assert_eq!(
            normalized.escalation_state,
            RhinoEscalationState::Normalized
        );
        assert_eq!(critical.escalation_state, RhinoEscalationState::Critical);
    }

    #[test]
    fn dependency_concentration_contributes_to_score() {
        let dispersed = evaluate_gray_rhino_escalation(input(
            RiskLevel::Moderate,
            RiskLevel::Moderate,
            RiskLevel::Low,
            RiskLevel::Low,
            RiskLevel::Moderate,
            RiskLevel::Low,
            RiskLevel::Low,
        ));
        let concentrated = evaluate_gray_rhino_escalation(input(
            RiskLevel::Moderate,
            RiskLevel::Moderate,
            RiskLevel::High,
            RiskLevel::Low,
            RiskLevel::Moderate,
            RiskLevel::Low,
            RiskLevel::Low,
        ));

        assert!(concentrated.escalation_score() > dispersed.escalation_score());
        assert!(concentrated.is_dependency_concentrated());
    }

    #[test]
    fn awareness_decay_detection_is_separate_from_market_direction() {
        let escalation = evaluate_gray_rhino_escalation(input(
            RiskLevel::Moderate,
            RiskLevel::Moderate,
            RiskLevel::Moderate,
            RiskLevel::Elevated,
            RiskLevel::Moderate,
            RiskLevel::Low,
            RiskLevel::Low,
        ));

        assert!(escalation.is_awareness_decay_detected());
        assert!(!format!("{:?}", escalation).contains("Buy"));
        assert!(!format!("{:?}", escalation).contains("Sell"));
    }

    #[test]
    fn unsafe_notes_are_suppressed_before_output() {
        let escalation = evaluate_gray_rhino_escalation(GrayRhinoEscalationInput {
            risk_expansion_rate: RiskLevel::Elevated,
            constraint_growth_rate: RiskLevel::Low,
            dependency_centralization: RiskLevel::High,
            awareness_decay: RiskLevel::High,
            narrative_overconfidence: RiskLevel::Elevated,
            single_point_fragility: RiskLevel::Moderate,
            fallback_survivability_risk: RiskLevel::Moderate,
            notes: vec![
                "Infrastructure concentration continues expanding.".to_string(),
                "马上卖出".to_string(),
                "Musk 非常危险".to_string(),
            ],
        });

        assert_eq!(escalation.notes.len(), 1);
        assert_eq!(escalation.suppressed_note_count, 2);
        assert_eq!(
            escalation.notes[0],
            "Infrastructure concentration continues expanding."
        );
    }
}
