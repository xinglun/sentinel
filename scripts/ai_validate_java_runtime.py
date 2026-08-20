"""Fail closed when a declared Java lane differs from its selected runtime."""

from __future__ import annotations

import argparse
import re
import subprocess  # nosec B404 - this gate observes the explicitly selected Java executable
import sys
from pathlib import Path

VERSION_PATTERN = re.compile(r'"(?:1\.)?(\d+)(?:[._+ -]|")')


def blocked(message: str) -> int:
    print(f"BLOCKED: {message}", file=sys.stderr)
    return 2


def selected_java(*, java_command: str, java_home: str) -> str:
    """Return the executable that Maven/Gradle will receive from configuration."""
    if java_home:
        return str(Path(java_home) / "bin" / "java")
    return java_command


def parse_java_major(version_output: str) -> int | None:
    """Parse legacy ``1.8`` and modern Java major forms from ``java -version``."""
    match = VERSION_PATTERN.search(version_output)
    if match is None:
        return None
    return int(match.group(1))


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify a declared Java lane against the selected java runtime."
    )
    parser.add_argument(
        "--lane", required=True, help="Adopter-defined lane identifier for diagnostics."
    )
    parser.add_argument(
        "--required-major",
        default="",
        help="Required Java major for the declared lane (for example 8, 17, or 21).",
    )
    parser.add_argument(
        "--java-command",
        default="java",
        help="Java executable selected when JAVA_HOME is not configured.",
    )
    parser.add_argument(
        "--java-home",
        default="",
        help="Optional JAVA_HOME whose bin/java is the selected runtime.",
    )
    return parser.parse_args(argv)


def required_major(value: str, lane: str) -> int | None:
    if not value.strip():
        blocked(
            f"required Java major is missing for lane '{lane}'. "
            "Recovery: set AI_COCKPIT_JAVA_REQUIRED_MAJOR to the project-approved major, then retry."
        )
        return None
    try:
        major = int(value)
    except ValueError:
        blocked(
            f"required Java major {value!r} for lane '{lane}' is not an integer. "
            "Recovery: set AI_COCKPIT_JAVA_REQUIRED_MAJOR to one positive Java major, then retry."
        )
        return None
    if major <= 0:
        blocked(
            f"required Java major {value!r} for lane '{lane}' is not positive. "
            "Recovery: set AI_COCKPIT_JAVA_REQUIRED_MAJOR to one positive Java major, then retry."
        )
        return None
    return major


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    expected = required_major(args.required_major, args.lane)
    if expected is None:
        return 2

    executable = selected_java(java_command=args.java_command, java_home=args.java_home)
    try:
        result = subprocess.run(  # nosec B603 - list-form, shell-free invocation of the selected runtime
            [executable, "-version"], text=True, capture_output=True, check=False, timeout=15
        )
    except FileNotFoundError:
        return blocked(
            f"lane '{args.lane}' selected Java executable {executable!r}, but it is unavailable. "
            "Recovery: configure the project-approved JAVA_HOME or AI_COCKPIT_JAVA_COMMAND, then retry."
        )
    except subprocess.TimeoutExpired:
        return blocked(
            f"lane '{args.lane}' selected Java executable {executable!r}, but version discovery timed out. "
            "Recovery: repair the project-approved runtime selection and retry."
        )

    output = f"{result.stdout}\n{result.stderr}"
    if result.returncode != 0:
        return blocked(
            f"lane '{args.lane}' could not query selected Java executable {executable!r}. "
            "Recovery: verify the project-approved runtime can run 'java -version', then retry."
        )
    actual = parse_java_major(output)
    if actual is None:
        return blocked(
            f"lane '{args.lane}' selected Java executable {executable!r}, but its version is unreadable. "
            "Recovery: configure a project-approved Java runtime with a readable 'java -version', then retry."
        )
    if actual != expected:
        return blocked(
            f"lane '{args.lane}' requires Java major {expected}, but selected runtime {executable!r} "
            f"reports actual major {actual}. Recovery: select the project-approved runtime or correct "
            "AI_COCKPIT_JAVA_REQUIRED_MAJOR, then retry."
        )

    print(
        f"Java runtime verified for lane '{args.lane}': required major {expected}; "
        f"actual major {actual}; executable {executable}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
