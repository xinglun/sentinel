"""Pure residual-risk signal policy."""
# mypy: disable-error-code=arg-type

from __future__ import annotations

from typing import Any

LEVELS = {"low": 1, "medium": 2, "high": 3}


def residual_risk_signal(summary: dict[str, Any] | None) -> dict[str, Any]:
    sources = ["summary.risk", "summary.residualRisks"]
    if summary is None:
        return {"value": "unknown", "evidence": ["summary is missing"], "sources": sources}
    levels: list[str] = []
    risk = summary.get("risk")
    if isinstance(risk, dict) and risk.get("level") in LEVELS:
        levels.append(risk["level"])
    for item in summary.get("residualRisks", []):
        if isinstance(item, dict) and item.get("level") in LEVELS:
            levels.append(item["level"])
    if not levels:
        return {
            "value": "unknown",
            "evidence": ["no residual risk evidence recorded"],
            "sources": sources,
        }
    level = max(levels, key=LEVELS.get)
    return {"value": level, "evidence": [f"highest residual risk: {level}"], "sources": sources}
