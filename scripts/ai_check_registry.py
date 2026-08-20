"""Deduplicated checker registration and structured stage results."""

from __future__ import annotations

from collections.abc import Callable, Iterable
from dataclasses import dataclass

STATUSES = {"passed", "failed", "warning", "skipped", "needs_human_confirmation"}
GATES = {"hard", "soft", "informational"}


@dataclass(frozen=True)
class CheckResult:
    checker_id: str
    status: str
    gate: str = "informational"
    reason_code: str = ""
    detail: str = ""

    def __post_init__(self) -> None:
        if self.status not in STATUSES:
            raise ValueError(f"unsupported check status: {self.status}")
        if self.gate not in GATES:
            raise ValueError(f"unsupported gate type: {self.gate}")
        if self.status == "skipped" and not self.reason_code:
            raise ValueError("skipped checks require a reason code")

    @classmethod
    def passed(cls, checker_id: str, *, gate: str = "hard", detail: str = "") -> CheckResult:
        return cls(checker_id, "passed", gate, "", detail)

    @classmethod
    def failed(cls, checker_id: str, *, gate: str = "hard", detail: str = "") -> CheckResult:
        return cls(checker_id, "failed", gate, "", detail)

    @classmethod
    def skipped(cls, checker_id: str, reason_code: str, *, detail: str = "") -> CheckResult:
        return cls(checker_id, "skipped", "informational", reason_code, detail)

    @classmethod
    def warning(
        cls,
        checker_id: str,
        *,
        gate: str = "soft",
        reason_code: str = "",
        detail: str = "",
    ) -> CheckResult:
        return cls(checker_id, "warning", gate, reason_code, detail)


class CheckerRegistry:
    def __init__(self) -> None:
        self._checkers: dict[str, Callable[[], CheckResult]] = {}

    @property
    def checker_ids(self) -> tuple[str, ...]:
        return tuple(self._checkers)

    def register(self, checker_id: str, checker: Callable[[], CheckResult]) -> None:
        if checker_id not in self._checkers:
            self._checkers[checker_id] = checker

    def run(
        self, requested: Iterable[str], *, available: set[str] | None = None
    ) -> list[CheckResult]:
        available_ids = set(self._checkers) if available is None else set(available)
        results: list[CheckResult] = []
        seen: set[str] = set()
        for checker_id in requested:
            if checker_id in seen:
                continue
            seen.add(checker_id)
            if checker_id not in available_ids or checker_id not in self._checkers:
                results.append(CheckResult.skipped(checker_id, "stage_not_applicable"))
                continue
            results.append(self._checkers[checker_id]())
        return results
