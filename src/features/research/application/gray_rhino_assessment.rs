use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessment, GrayRhinoAssessmentSnapshot,
    GrayRhinoEscalationInput, GrayRhinoObservationSource,
};
use chrono::NaiveDate;

/// 灰色のサイ観測入力を、日次監査可能な snapshot へ変換する。
pub(crate) fn build_gray_rhino_assessment(
    input: GrayRhinoEscalationInput,
    as_of_date: NaiveDate,
    previous: Option<GrayRhinoAssessmentSnapshot>,
) -> GrayRhinoAssessment {
    GrayRhinoAssessment {
        current: GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date,
            source: GrayRhinoObservationSource::ManualConfiguration,
            escalation: evaluate_gray_rhino_escalation(input),
        },
        previous,
    }
}
