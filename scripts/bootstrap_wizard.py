"""Side-effect-free Bootstrap Wizard state machine.

The session is deliberately an in-memory value object. Callers may serialize it
outside the target repository, while later Work Items decide how detection,
drift checks, and writes are performed.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from enum import Enum
from typing import Any

BOOTSTRAP_LIFECYCLE = "bootstrap_adoption"
BOOTSTRAP_CREATION_MODE = "bootstrap_installer"
NOT_CLAIMED = "not_claimed"


class WizardError(ValueError):
    """Raised when a Bootstrap Wizard transition is not valid for the phase."""


class Phase(str, Enum):
    DETECT = "detect"
    PROPOSE = "propose"
    CONFIGURE = "configure"
    REVIEW = "review"
    CONFIRM = "confirm"
    CONFIRMED = "confirmed"
    WRITE = "write"
    CANCELLED = "cancelled"


@dataclass(frozen=True)
class Session:
    """External Bootstrap Wizard state; it never performs repository I/O."""

    session_id: str
    phase: Phase = Phase.DETECT
    upstream_revision: str | None = None
    detected: dict[str, Any] | None = None
    proposal: dict[str, Any] | None = None
    configuration: dict[str, Any] | None = None
    review: dict[str, Any] | None = None
    resume_phase: Phase | None = None
    lifecycle_type: str = BOOTSTRAP_LIFECYCLE
    creation_mode: str = BOOTSTRAP_CREATION_MODE
    readiness: str = NOT_CLAIMED

    def transition(self, event: str, **payload: Any) -> Session:
        """Apply one explicit event and return a new session value."""
        if event == "cancel":
            if self.phase in {Phase.CANCELLED, Phase.CONFIRMED, Phase.WRITE}:
                raise WizardError(f"cannot cancel from {self.phase.value}")
            return replace(self, phase=Phase.CANCELLED, resume_phase=self.phase)
        if event == "resume":
            if self.phase is not Phase.CANCELLED or self.resume_phase is None:
                raise WizardError("resume requires a cancelled session")
            return replace(self, phase=self.resume_phase, resume_phase=None)
        if event == "write" and self.phase is Phase.CONFIRMED:
            return replace(self, phase=Phase.WRITE)
        if event == "write":
            raise WizardError("write requires a confirmed session")
        if event == "back":
            previous = {
                Phase.PROPOSE: Phase.DETECT,
                Phase.CONFIGURE: Phase.PROPOSE,
                Phase.REVIEW: Phase.CONFIGURE,
                Phase.CONFIRM: Phase.REVIEW,
            }.get(self.phase)
            if previous is None:
                raise WizardError(f"cannot go back from {self.phase.value}")
            return self._at(previous)
        if self.phase in {Phase.CANCELLED, Phase.WRITE}:
            raise WizardError("resume the cancelled session before continuing")
        if event == "detect" and self.phase is Phase.DETECT:
            return replace(
                self,
                phase=Phase.PROPOSE,
                upstream_revision=self._required(payload, "upstream_revision"),
                detected=self._mapping(payload, "detected"),
            )
        if event == "propose" and self.phase is Phase.PROPOSE:
            return replace(self, phase=Phase.CONFIGURE, proposal=self._mapping(payload, "proposal"))
        if event == "configure" and self.phase is Phase.CONFIGURE:
            return replace(
                self, phase=Phase.REVIEW, configuration=self._mapping(payload, "configuration")
            )
        if event == "review" and self.phase is Phase.REVIEW:
            return replace(self, phase=Phase.CONFIRM, review=self._mapping(payload, "review"))
        if event == "confirm" and self.phase is Phase.CONFIRM:
            if not payload.get("approved", False):
                raise WizardError("confirmation requires explicit approval")
            if payload.get("upstream_revision", self.upstream_revision) != self.upstream_revision:
                return self.invalidate(payload["upstream_revision"])
            return replace(self, phase=Phase.CONFIRMED)
        raise WizardError(f"event {event!r} is invalid from {self.phase.value}")

    def invalidate(self, upstream_revision: str) -> Session:
        """Discard downstream decisions when an upstream fact changes."""
        if upstream_revision == self.upstream_revision:
            return self
        if self.phase in {Phase.DETECT, Phase.CANCELLED}:
            return replace(self, upstream_revision=upstream_revision)
        return replace(
            self,
            phase=Phase.PROPOSE,
            upstream_revision=upstream_revision,
            proposal=None,
            configuration=None,
            review=None,
            resume_phase=None,
        )

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-compatible snapshot suitable for external storage."""
        return {
            "sessionId": self.session_id,
            "phase": self.phase.value,
            "upstreamRevision": self.upstream_revision,
            "detected": self.detected,
            "proposal": self.proposal,
            "configuration": self.configuration,
            "review": self.review,
            "resumePhase": self.resume_phase.value if self.resume_phase else None,
            "lifecycleType": self.lifecycle_type,
            "creationMode": self.creation_mode,
            "readiness": self.readiness,
        }

    def _at(self, phase: Phase) -> Session:
        if phase is Phase.DETECT:
            return replace(self, phase=phase, proposal=None, configuration=None, review=None)
        if phase is Phase.PROPOSE:
            return replace(self, phase=phase, configuration=None, review=None)
        if phase is Phase.CONFIGURE:
            return replace(self, phase=phase, review=None)
        return replace(self, phase=phase)

    @staticmethod
    def _required(payload: dict[str, Any], key: str) -> str:
        value = payload.get(key)
        if not isinstance(value, str) or not value:
            raise WizardError(f"{key} must be a non-empty string")
        return value

    @staticmethod
    def _mapping(payload: dict[str, Any], key: str) -> dict[str, Any]:
        value = payload.get(key)
        if not isinstance(value, dict):
            raise WizardError(f"{key} must be a mapping")
        return dict(value)


def validate_bootstrap_record(record: Any) -> list[str]:
    """Validate the public Bootstrap Adoption record without claiming readiness."""
    if not isinstance(record, dict):
        return ["bootstrap record must be an object"]
    issues: list[str] = []
    if record.get("lifecycleType") != BOOTSTRAP_LIFECYCLE:
        issues.append("lifecycleType must be bootstrap_adoption")
    if record.get("creationMode") != BOOTSTRAP_CREATION_MODE:
        issues.append("creationMode must be bootstrap_installer")
    if record.get("readiness") != NOT_CLAIMED:
        issues.append("readiness must remain not_claimed")
    if record.get("phase") not in {phase.value for phase in Phase}:
        issues.append("phase is unsupported")
    if not isinstance(record.get("sessionId"), str) or not record["sessionId"]:
        issues.append("sessionId is required")
    return issues
