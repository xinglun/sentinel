#!/usr/bin/env python3
"""Read-only project fact scanner for AI Cockpit boundary calibration."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path
from typing import Any

from ai_common import simple_yaml_lists, simple_yaml_scalars

LANGUAGE_SIGNALS = {
    "python": ("pyproject.toml", "setup.py", "requirements.txt", "**/*.py"),
    "dart": ("pubspec.yaml", "**/*.dart"),
    "java": ("pom.xml", "**/*.java"),
    "kotlin": ("**/*.kt", "**/*.kts"),
    "ruby": ("Gemfile", "**/*.rb"),
    "javascript/typescript": ("package.json", "**/*.ts", "**/*.tsx", "**/*.js"),
    "go": ("go.mod", "**/*.go"),
    "rust": ("Cargo.toml", "**/*.rs"),
    "csharp": ("**/*.csproj", "**/*.cs"),
    "swift": ("Package.swift", "**/*.swift"),
    "php": ("composer.json", "**/*.php"),
}
BUILD_SIGNALS = {
    "gradle": ("build.gradle", "build.gradle.kts", "gradlew"),
    "maven": ("pom.xml",),
    "flutter": ("pubspec.yaml",),
    "npm": ("package.json",),
    "python-packaging": ("pyproject.toml", "setup.py"),
    "cargo": ("Cargo.toml",),
    "go-modules": ("go.mod",),
    "bundler": ("Gemfile",),
    "composer": ("composer.json",),
    "dotnet": ("**/*.sln", "**/*.csproj"),
    "swift-package-manager": ("Package.swift",),
    "cocoapods": ("Podfile",),
    "xcode-project": ("*.xcodeproj",),
    "xcode-workspace": ("*.xcworkspace",),
}
INFRA_SIGNALS = {
    "github-actions": (".github/workflows/*.yml", ".github/workflows/*.yaml"),
    "gitlab-ci": (".gitlab-ci.yml",),
    "docker": ("Dockerfile", "docker-compose.yml", "compose.yaml"),
    "kubernetes": ("k8s/**", "kubernetes/**", "helm/**"),
    "terraform": ("**/*.tf",),
    "fastlane": ("fastlane/Fastfile",),
    "database-migrations": ("migrations/**", "db/migrate/**"),
    "code-generation": ("build_runner.yaml", "openapi.yaml", "**/*.g.dart", "**/*.generated.*"),
}
STATE_MANAGEMENT_TERMS = ("riverpod", "provider", "flutter_bloc", "bloc", "redux", "mobx")
NATIVE_ROOTS = ("android", "ios", "macos", "windows", "linux")
QUALITY_TARGET_TERMS = (
    "test",
    "check",
    "quality",
    "lint",
    "format",
    "coverage",
    "verify",
    "audit",
)
CRITICAL_DOMAIN_TERMS = {
    "payment": "payments",
    "payments": "payments",
    "billing": "billing",
    "checkout": "payments",
    "finance": "finance",
    "payroll": "payroll",
    "health": "health",
    "medical": "health",
    "identity": "identity",
    "auth": "identity",
    "security": "security",
}


def repository_entries(root: Path) -> list[Path]:
    """Return repository-owned paths, falling back to filesystem fixtures outside Git."""
    try:
        result = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            capture_output=True,
            check=False,
        )
    except OSError:
        result = None
    if result is not None and result.returncode == 0:
        entries: set[Path] = set()
        for raw in result.stdout.decode("utf-8", errors="replace").split("\0"):
            if not raw:
                continue
            relative = Path(raw)
            entries.add(relative)
            entries.update(relative.parents)
        return sorted(root / relative for relative in entries)
    return sorted(path for path in root.rglob("*") if path.is_file() or path.is_dir())


def first_evidence(root: Path, patterns: tuple[str, ...], entries: list[Path]) -> str | None:
    for pattern in patterns:
        matches = sorted(
            path
            for path in entries
            if path.is_file() or path.is_dir()
            if path.relative_to(root).match(pattern)
        )
        if matches:
            return matches[0].relative_to(root).as_posix()
    return None


def findings(
    root: Path, signals: dict[str, tuple[str, ...]], entries: list[Path]
) -> list[dict[str, str]]:
    result = []
    for value, patterns in signals.items():
        evidence = first_evidence(root, patterns, entries)
        if evidence:
            result.append({"value": value, "confidence": "high", "evidence": evidence})
    return result


def framework_findings(root: Path) -> list[dict[str, str]]:
    result: list[dict[str, str]] = []
    pubspec = root / "pubspec.yaml"
    if pubspec.is_file() and "flutter:" in pubspec.read_text(encoding="utf-8", errors="ignore"):
        result.append(
            {"value": "flutter", "confidence": "high", "evidence": "pubspec.yaml:flutter"}
        )
    for build in (root / "build.gradle", root / "build.gradle.kts", root / "pom.xml"):
        if (
            build.is_file()
            and "spring" in build.read_text(encoding="utf-8", errors="ignore").lower()
        ):
            result.append({"value": "spring-boot", "confidence": "high", "evidence": build.name})
            break
    gemfile = root / "Gemfile"
    if (
        gemfile.is_file()
        and "rails" in gemfile.read_text(encoding="utf-8", errors="ignore").lower()
    ):
        result.append({"value": "rails", "confidence": "high", "evidence": "Gemfile"})
    pyproject_text = ""
    for path in (root / "pyproject.toml", root / "requirements.txt"):
        if path.is_file():
            pyproject_text += path.read_text(encoding="utf-8", errors="ignore").lower()
    for name in ("django", "fastapi", "pytorch", "tensorflow"):
        if name in pyproject_text:
            result.append(
                {"value": name, "confidence": "medium", "evidence": "Python dependency manifest"}
            )
    return result


def directory_candidates(root: Path, names: tuple[str, ...], kind: str) -> list[dict[str, str]]:
    result = []
    for name in names:
        path = root / name
        if path.exists():
            result.append(
                {"path": f"{name}/**", "kind": kind, "confidence": "medium", "evidence": name}
            )
    return result


def suffix_directory_candidates(
    root: Path,
    suffix: str,
    kind: str,
    *,
    confidence: str = "medium",
) -> list[dict[str, str]]:
    """リポジトリ直下の *Tests 等、名前サフィックスでディレクトリ候補を収集する。"""
    result: list[dict[str, str]] = []
    try:
        for path in sorted(root.iterdir()):
            if not path.is_dir() or path.name.startswith("."):
                continue
            if path.name.endswith(suffix):
                result.append(
                    {
                        "path": f"{path.name}/**",
                        "kind": kind,
                        "confidence": confidence,
                        "evidence": path.name,
                    }
                )
    except OSError:
        return result
    return result


def xcode_production_candidates(root: Path, entries: list[Path]) -> list[dict[str, str]]:
    """*.xcodeproj 同階層の保守的なソースディレクトリ候補を提案する。"""
    result: list[dict[str, str]] = []
    seen: set[str] = set()
    projects = sorted(
        path for path in entries if path.is_dir() and path.relative_to(root).match("*.xcodeproj")
    )
    for project in projects:
        if not project.is_dir():
            continue
        stem = project.stem
        for candidate_name in (stem, "Sources", "Classes", "src"):
            if candidate_name in seen:
                continue
            path = root / candidate_name
            if not path.is_dir() or candidate_name.startswith("."):
                continue
            if candidate_name.endswith("Tests") or candidate_name in {"Tests", "test", "tests"}:
                continue
            seen.add(candidate_name)
            result.append(
                {
                    "path": f"{candidate_name}/**",
                    "kind": "production",
                    "confidence": "medium",
                    "evidence": f"{project.name}:{candidate_name}",
                }
            )
            break
    return result


def merge_boundary_candidates(*groups: list[dict[str, str]]) -> list[dict[str, str]]:
    merged: list[dict[str, str]] = []
    seen: set[str] = set()
    for group in groups:
        for item in group:
            path = item.get("path")
            if not isinstance(path, str) or path in seen:
                continue
            seen.add(path)
            merged.append(item)
    return merged


def project_signals(
    root: Path, infrastructure: list[dict[str, str]]
) -> dict[str, list[dict[str, str]]]:
    dependency_text = ""
    for name in ("pubspec.yaml", "package.json", "Gemfile", "pyproject.toml"):
        path = root / name
        if path.is_file():
            dependency_text += path.read_text(encoding="utf-8", errors="ignore").lower()
    state = [
        {"value": term, "confidence": "medium", "evidence": "dependency manifest"}
        for term in STATE_MANAGEMENT_TERMS
        if term in dependency_text
    ]
    by_value = {item["value"]: item for item in infrastructure}
    native = [
        {"value": name, "confidence": "medium", "evidence": name}
        for name in NATIVE_ROOTS
        if (root / name).is_dir()
    ]
    return {
        "stateManagement": state,
        "codeGeneration": [by_value["code-generation"]] if "code-generation" in by_value else [],
        "databaseMigrations": [by_value["database-migrations"]]
        if "database-migrations" in by_value
        else [],
        "ciReleaseDeployment": [
            item
            for item in infrastructure
            if item["value"]
            in {"github-actions", "gitlab-ci", "fastlane", "docker", "kubernetes", "terraform"}
        ],
        "native": native,
    }


def quality_command_candidates(root: Path) -> list[dict[str, str]]:
    """Collect command-shaped quality candidates without executing them."""
    result: list[dict[str, str]] = []
    makefile = root / "Makefile"
    if makefile.is_file():
        for line in makefile.read_text(encoding="utf-8", errors="ignore").splitlines():
            match = re.match(r"^([A-Za-z0-9_.-]+):", line)
            if not match:
                continue
            target = match.group(1)
            if target == ".PHONY" or not any(
                term in target.lower() for term in QUALITY_TARGET_TERMS
            ):
                continue
            result.append(
                {"value": f"make {target}", "confidence": "high", "evidence": f"Makefile:{target}"}
            )
    package = root / "package.json"
    if package.is_file():
        try:
            scripts = json.loads(package.read_text(encoding="utf-8")).get("scripts", {})
        except (OSError, json.JSONDecodeError, AttributeError):
            scripts = {}
        if isinstance(scripts, dict):
            for name in sorted(str(key) for key in scripts if isinstance(key, str)):
                if any(term in name.lower() for term in QUALITY_TARGET_TERMS):
                    result.append(
                        {
                            "value": f"npm {name}",
                            "confidence": "high",
                            "evidence": f"package.json:scripts.{name}",
                        }
                    )
    return result


def critical_domain_candidates(root: Path, entries: list[Path]) -> list[dict[str, str]]:
    """Surface domain-sensitive path signals for explicit human review."""
    found: dict[str, dict[str, str]] = {}
    for path in entries:
        relative = path.relative_to(root).as_posix().lower()
        for term, domain in CRITICAL_DOMAIN_TERMS.items():
            if re.search(rf"(^|[/_.-]){re.escape(term)}([/_.-]|$)", relative):
                found.setdefault(
                    domain,
                    {"value": domain, "confidence": "medium", "evidence": relative},
                )
    return [found[key] for key in sorted(found)]


def scan_project(root: Path) -> dict[str, Any]:
    entries = repository_entries(root)
    production = merge_boundary_candidates(
        directory_candidates(
            root, ("src", "lib", "app", "Sources", "cmd", "pkg", "internal"), "production"
        ),
        xcode_production_candidates(root, entries),
    )
    tests = merge_boundary_candidates(
        directory_candidates(root, ("tests", "test", "Tests", "spec"), "test"),
        suffix_directory_candidates(root, "Tests", "test"),
    )
    generated = directory_candidates(
        root, ("build", "dist", "generated", ".dart_tool"), "generated"
    )
    critical = directory_candidates(
        root,
        (
            ".github",
            "fastlane",
            "migrations",
            "db",
            "infra",
            "deploy",
            "release",
            "security",
            "android",
            "ios",
        ),
        "critical",
    )
    unknowns = []
    if not production:
        unknowns.append(
            "blocking: production roots could not be determined from common directory signals"
        )
    if not tests:
        unknowns.append(
            "blocking: test roots could not be determined from common directory signals"
        )
    coverage = simple_yaml_lists(root / ".ai" / "guards" / "coverage_policy.yaml")
    boundary = simple_yaml_scalars(root / ".ai" / "guards" / "file_boundary.yaml")
    review = simple_yaml_lists(root / ".ai" / "guards" / "ai_review_policy.yaml")
    guard_mismatches = []
    for item in production:
        if item["path"] not in coverage.get("production.include", []):
            guard_mismatches.append(
                {"kind": "production", "path": item["path"], "evidence": item["evidence"]}
            )
    for item in tests:
        if item["path"] not in coverage.get("tests.include", []):
            guard_mismatches.append(
                {"kind": "test", "path": item["path"], "evidence": item["evidence"]}
            )
    for item in generated:
        if f"{item['path']}.boundary" not in boundary:
            guard_mismatches.append(
                {"kind": "generated", "path": item["path"], "evidence": item["evidence"]}
            )
    review_patterns = review.get("requiredReviewChecklist.include", [])
    for item in critical:
        if item["path"] not in review_patterns:
            guard_mismatches.append(
                {"kind": "critical", "path": item["path"], "evidence": item["evidence"]}
            )
    infrastructure = findings(root, INFRA_SIGNALS, entries)
    signals = project_signals(root, infrastructure)
    signals["qualityCommands"] = quality_command_candidates(root)
    signals["criticalDomains"] = critical_domain_candidates(root, entries)
    return {
        "reportVersion": 1,
        "detectedFacts": {
            "languages": findings(root, LANGUAGE_SIGNALS, entries),
            "frameworks": framework_findings(root),
            "buildSystems": findings(root, BUILD_SIGNALS, entries),
            "infrastructure": infrastructure,
        },
        "projectSignals": signals,
        "suggestedBoundaries": {
            "productionRoots": production,
            "featureRoots": production,
            "testRoots": tests,
            "generatedPaths": generated,
            "criticalPaths": critical,
        },
        "guardMismatches": guard_mismatches,
        "unknowns": unknowns,
        "disclaimer": "Detected facts and suggestions are not approval decisions.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--output", default="target/ai_project_doctor_report.json")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    output = root / args.output
    report = scan_project(root)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"project doctor report: {output.relative_to(root)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
