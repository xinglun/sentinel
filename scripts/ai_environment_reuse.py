"""Environment-bound evidence reuse policy built on WI-07 bindings.

The policy shapes an explicit, allowlisted environment snapshot and delegates
the final identity/freshness decision to ``ai_evidence_binding``. It never
reads the process environment wholesale, calls a provider, executes checks, or
authorizes a gate bypass.
"""

from __future__ import annotations

import json
import platform
import re
from collections.abc import Mapping
from datetime import datetime
from typing import Any

from ai_evidence_binding import build_binding, canonical_digest, decide_reuse

_DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")
_SECRET_KEY = re.compile(
    r"(password|passwd|secret|token|api[_-]?key|private[_-]?key|credential|authorization)",
    re.IGNORECASE,
)
_UNKNOWN = {"", "unknown", "n/a", "not_configured"}
_SNAPSHOT_FIELDS = {"runtime", "toolchain", "environment", "fingerprint"}


class EnvironmentReuseError(ValueError):
    """Raised when an environment snapshot cannot be trusted as evidence."""


def _require_safe_key(key: object, field: str) -> str:
    if not isinstance(key, str) or not key.strip():
        raise EnvironmentReuseError(f"{field} key must be a non-empty string")
    if _SECRET_KEY.search(key):
        raise EnvironmentReuseError(f"secret-like metadata key is forbidden: {field}.{key}")
    return key.strip()


def _normalize_values(values: Mapping[str, str], field: str) -> dict[str, str]:
    if not isinstance(values, Mapping) or not values:
        raise EnvironmentReuseError(f"{field} must be a non-empty object")
    normalized: dict[str, str] = {}
    for raw_key, raw_value in values.items():
        key = _require_safe_key(raw_key, field)
        if not isinstance(raw_value, str) or raw_value.strip().lower() in _UNKNOWN:
            raise EnvironmentReuseError(f"{field}.{key} is Unknown or malformed")
        normalized[key] = raw_value.strip()
    return dict(sorted(normalized.items()))


def environment_fingerprint(
    *, runtime: str, toolchain: Mapping[str, str], environment: Mapping[str, str]
) -> str:
    """Return a canonical digest of the complete allowlisted environment identity."""

    if not isinstance(runtime, str) or runtime.strip().lower() in _UNKNOWN:
        raise EnvironmentReuseError("runtime is Unknown or malformed")
    normalized_runtime = runtime.strip()
    normalized_toolchain = _normalize_values(toolchain, "toolchain")
    normalized_environment = _normalize_values(environment, "environment")
    return canonical_digest(
        {
            "runtime": normalized_runtime,
            "toolchain": normalized_toolchain,
            "environment": normalized_environment,
        }
    )


def build_environment_snapshot(
    *, runtime: str, toolchain: Mapping[str, str], environment: Mapping[str, str]
) -> dict[str, Any]:
    """Normalize explicit metadata into a digest-bound environment snapshot."""

    if not isinstance(runtime, str) or runtime.strip().lower() in _UNKNOWN:
        raise EnvironmentReuseError("runtime is Unknown or malformed")
    normalized_runtime = runtime.strip()
    normalized_toolchain = _normalize_values(toolchain, "toolchain")
    normalized_environment = _normalize_values(environment, "environment")
    return {
        "runtime": normalized_runtime,
        "toolchain": json.dumps(
            normalized_toolchain, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ),
        "environment": normalized_environment,
        "fingerprint": environment_fingerprint(
            runtime=normalized_runtime,
            toolchain=normalized_toolchain,
            environment=normalized_environment,
        ),
    }


def current_environment(*, toolchain: Mapping[str, str] | None = None) -> dict[str, Any]:
    """Build a portable allowlisted snapshot of the current local runtime."""

    explicit_toolchain = dict(toolchain or {})
    default_toolchain = {
        "python": platform.python_version(),
        "implementation": platform.python_implementation(),
    }
    default_toolchain.update(explicit_toolchain)
    return build_environment_snapshot(
        runtime=f"python-{platform.python_version()}",
        toolchain=default_toolchain,
        environment={
            "os": platform.system().lower(),
            "architecture": platform.machine().lower(),
        },
    )


def _validate_snapshot(snapshot: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(snapshot, Mapping) or set(snapshot) != _SNAPSHOT_FIELDS:
        raise EnvironmentReuseError("environment snapshot is malformed")
    runtime = snapshot["runtime"]
    if not isinstance(runtime, str) or runtime.strip().lower() in _UNKNOWN:
        raise EnvironmentReuseError("runtime is Unknown or malformed")
    toolchain_text = snapshot["toolchain"]
    environment = snapshot["environment"]
    fingerprint = snapshot["fingerprint"]
    if not isinstance(toolchain_text, str) or not toolchain_text:
        raise EnvironmentReuseError("toolchain snapshot is Unknown or malformed")
    try:
        toolchain = json.loads(toolchain_text)
    except json.JSONDecodeError as exc:
        raise EnvironmentReuseError("toolchain snapshot is malformed") from exc
    if not isinstance(toolchain, dict):
        raise EnvironmentReuseError("toolchain snapshot is malformed")
    normalized_toolchain = _normalize_values(toolchain, "toolchain")
    normalized_environment = _normalize_values(environment, "environment")
    if not isinstance(fingerprint, str) or not _DIGEST.fullmatch(fingerprint):
        raise EnvironmentReuseError("environment fingerprint is malformed")
    expected = environment_fingerprint(
        runtime=runtime,
        toolchain=normalized_toolchain,
        environment=normalized_environment,
    )
    if expected != fingerprint:
        raise EnvironmentReuseError("environment fingerprint does not match metadata")
    return {
        "runtime": runtime,
        "toolchain": json.dumps(
            normalized_toolchain, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ),
        "environment": normalized_environment,
        "fingerprint": fingerprint,
    }


def environment_dependency(snapshot: Mapping[str, Any]) -> dict[str, Any]:
    """Project a validated snapshot into the WI-07 environment dependency shape."""

    normalized = _validate_snapshot(snapshot)
    return {
        "environment": {
            "digest": normalized["fingerprint"],
            "runtime": normalized["runtime"],
            "toolchain": normalized["toolchain"],
        }
    }


def build_environment_binding(
    *,
    subject: Mapping[str, str],
    environment: Mapping[str, Any],
    scope_digest: str,
    governance_digest: str,
    producer: Mapping[str, str],
    created_at: datetime,
    expires_at: datetime,
) -> dict[str, Any]:
    """Create a WI-07 environment-bound binding from a validated snapshot."""

    return build_binding(
        subject=subject,
        classification="environment-bound",
        dependencies=environment_dependency(environment),
        scope_digest=scope_digest,
        governance_digest=governance_digest,
        producer=producer,
        created_at=created_at,
        expires_at=expires_at,
    )


def decide_environment_reuse(
    binding: Mapping[str, Any],
    current: Mapping[str, Any] | None,
    *,
    scope_digest: str | None,
    governance_digest: str | None,
    now: datetime,
) -> dict[str, Any]:
    """Return WI-07's exact environment reuse/rerun decision."""

    if not isinstance(binding, Mapping) or binding.get("classification") != "environment-bound":
        return {
            "state": "unknown",
            "action": "rerun",
            "reasons": ["binding_classification_mismatch"],
        }
    if current is None:
        return {
            "state": "unknown",
            "action": "rerun",
            "reasons": ["environment_snapshot_unknown"],
        }
    try:
        normalized = _validate_snapshot(current)
        dependency = environment_dependency(normalized)
    except EnvironmentReuseError:
        return {
            "state": "unknown",
            "action": "rerun",
            "reasons": ["environment_snapshot_invalid"],
        }
    return decide_reuse(
        binding,
        {
            "dependencies": dependency,
            "scopeDigest": scope_digest,
            "governanceDigest": governance_digest,
        },
        now=now,
    )
