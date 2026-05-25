#!/usr/bin/env python3
"""DDD / Clean Architecture の依存方向を検証する軽量 checker。"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

PROJECT_ROOT = Path(__file__).resolve().parents[1]
IMPORT_START_RE = re.compile(r"^\s*(?:use|pub\s+use)\s+(.+)")
INLINE_CRATE_PATH_RE = re.compile(
    r"\bcrate\s*::\s*features(?:\s*::\s*[A-Za-z_][A-Za-z0-9_]*)+"
)
CONCRETE_TYPE_DEFAULTS = (
    "FutuClient",
    "YahooProvider",
    "FinnhubFetcher",
    "SECEDGARFetcher",
    "WebFetcher",
    "FixtureFetcher",
)
REMOVED_CRATE_ROOT_PREFIXES = (
    "crate::application",
    "crate::core",
    "crate::domain",
    "crate::infrastructure",
    "crate::interface",
)


@dataclass(frozen=True)
class LayerRule:
    layer_path: str
    forbidden_import_prefixes: tuple[str, ...]


RULES: tuple[LayerRule, ...] = (
    LayerRule(
        "src/domain",
        (
            "crate::adapters",
            "crate::application",
            "crate::backtest",
            "crate::cli",
            "crate::config",
            "crate::core",
            "crate::data",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
            "super::application",
            "super::infrastructure",
            "super::interface",
        ),
    ),
    LayerRule(
        "src/application",
        (
            "crate::adapters",
            "crate::backtest",
            "crate::cli",
            "crate::config",
            "crate::core::notification",
            "crate::data",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/interface",
        (
            "crate::adapters",
            "crate::data",
            "crate::infrastructure",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/core",
        (
            "crate::adapters",
            "crate::application",
            "crate::backtest",
            "crate::cli",
            "crate::data",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/backtest.rs",
        (
            "crate::adapters",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/cli.rs",
        (
            "crate::adapters",
            "crate::infrastructure::evidence_ingestion",
            "crate::infrastructure::evidence_store",
            "crate::infrastructure::notify",
            "crate::infrastructure::persistence",
            "crate::infrastructure::transition_log",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/config.rs",
        (
            "crate::interface",
        ),
    ),
)


@dataclass(frozen=True)
class Violation:
    path: Path
    line: int
    import_path: str
    forbidden_prefix: str

    def format(self, root: Path) -> str:
        rel = self.path.relative_to(root)
        return (
            f"{rel}:{self.line}: forbidden import `{self.import_path}` "
            f"matches `{self.forbidden_prefix}`"
        )


@dataclass(frozen=True)
class FeatureAclManifest:
    feature_roots: dict[str, dict[str, tuple[str, ...]]]
    allowed_dependencies: dict[str, tuple[str, ...]]
    concrete_import_prefixes: tuple[str, ...]
    concrete_type_names: tuple[str, ...]


FEATURE_LAYER_FORBIDDEN_IMPORT_PREFIXES: dict[str, tuple[str, ...]] = {
    "domain": (
        "crate::adapters",
        "crate::application",
        "crate::config",
        "crate::core",
        "crate::infrastructure",
        "crate::interface",
    ),
    "application": (
        "crate::adapters",
        "crate::infrastructure",
        "crate::interface",
    ),
    "interface": (
        "crate::adapters",
    ),
    "infrastructure": (
        "crate::interface",
    ),
    "acl": (
        "crate::interface",
        "crate::infrastructure",
    ),
}

APPLICATION_IO_IMPORT_PREFIXES = (
    "std::fs",
    "std::net",
    "tokio::fs",
    "tokio::net",
    "reqwest",
)


def _strip_yaml_value(raw: str) -> str:
    return raw.strip().strip('"').strip("'")


def load_feature_acl_manifest(root: Path = PROJECT_ROOT) -> FeatureAclManifest:
    """feature ACL manifest を標準 library だけで読み取る。"""
    path = root / ".ai/architecture/feature_acl.yaml"
    if not path.exists():
        return FeatureAclManifest({}, {}, ("crate::adapters",), CONCRETE_TYPE_DEFAULTS)

    feature_roots: dict[str, dict[str, list[str]]] = {}
    allowed_dependencies: dict[str, list[str]] = {}
    concrete_import_prefixes: list[str] = []
    concrete_type_names: list[str] = []
    section_stack: list[tuple[int, str]] = []
    current_feature: str | None = None
    current_layer: str | None = None
    list_target: str | None = None

    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        indent = len(line) - len(line.lstrip(" "))
        stripped = line.strip()
        while section_stack and section_stack[-1][0] >= indent:
            popped = section_stack.pop()[1]
            if popped == "feature":
                current_feature = None
            if popped == "layer":
                current_layer = None
            if popped == "list":
                list_target = None

        if stripped.startswith("- "):
            value = _strip_yaml_value(stripped[2:])
            if current_feature and current_layer and list_target == "roots":
                feature_roots.setdefault(current_feature, {}).setdefault(current_layer, []).append(value)
            elif current_feature and list_target == "allowed_dependencies":
                allowed_dependencies.setdefault(current_feature, []).append(value)
            elif list_target == "concrete_import_prefixes":
                concrete_import_prefixes.append(value)
            elif list_target == "concrete_type_names":
                concrete_type_names.append(value)
            continue

        key = stripped.split(":", 1)[0]
        if indent == 2 and key not in {"domain", "adapters", "core"}:
            current_feature = key
            feature_roots.setdefault(current_feature, {})
            section_stack.append((indent, "feature"))
            continue
        if current_feature and key in {"domain", "application", "interface", "infrastructure", "acl"}:
            current_layer = key
            feature_roots.setdefault(current_feature, {}).setdefault(current_layer, [])
            section_stack.append((indent, "layer"))
            list_target = "roots"
            section_stack.append((indent + 1, "list"))
            continue
        if current_feature and key == "allowedDependencies":
            allowed_dependencies.setdefault(current_feature, [])
            list_target = "allowed_dependencies"
            section_stack.append((indent, "list"))
            continue
        if key == "externalConcreteImportPrefixes":
            list_target = "concrete_import_prefixes"
            section_stack.append((indent, "list"))
            continue
        if key == "concreteTypeNames":
            list_target = "concrete_type_names"
            section_stack.append((indent, "list"))
            continue

    roots = {
        feature: {layer: tuple(paths) for layer, paths in layers.items()}
        for feature, layers in feature_roots.items()
    }
    return FeatureAclManifest(
        roots,
        {feature: tuple(deps) for feature, deps in allowed_dependencies.items()},
        tuple(concrete_import_prefixes or ("crate::adapters",)),
        tuple(concrete_type_names or CONCRETE_TYPE_DEFAULTS),
    )


def rust_files(root: Path) -> Iterable[Path]:
    if root.is_file():
        yield root
        return

    for path in root.rglob("*.rs"):
        if "/target/" not in str(path):
            yield path


def normalize_import(raw: str) -> str:
    return raw.strip().replace(" ", "")


def relative_posix(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def path_matches_root(rel_path: str, root: str) -> bool:
    normalized = root.rstrip("/")
    return rel_path == normalized or rel_path.startswith(f"{normalized}/")


def feature_layer_for_path(rel_path: str, manifest: FeatureAclManifest) -> tuple[str, str] | None:
    for feature, layers in manifest.feature_roots.items():
        for layer, roots in layers.items():
            if any(path_matches_root(rel_path, root) for root in roots):
                return feature, layer
    return None


def feature_layer_for_import(import_path: str) -> tuple[str, str] | None:
    parts = import_path.split("::")
    if len(parts) < 4 or parts[0] != "crate" or parts[1] != "features":
        return None
    feature = parts[2]
    layer = parts[3]
    if layer not in {"domain", "application", "interface", "infrastructure", "acl"}:
        return None
    return feature, layer


def cli_feature_infrastructure_violations(path: Path) -> list[Violation]:
    violations: list[Violation] = []
    for line_no, import_path in imports_from(path):
        imported_feature_layer = feature_layer_for_import(import_path)
        if imported_feature_layer and imported_feature_layer[1] == "infrastructure":
            violations.append(Violation(path, line_no, import_path, "cli -> feature infrastructure"))
    return violations


def feature_acl_violations(path: Path, root: Path, manifest: FeatureAclManifest) -> list[Violation]:
    rel_path = relative_posix(path, root)
    feature_layer = feature_layer_for_path(rel_path, manifest)
    if not feature_layer:
        return []

    feature, layer = feature_layer
    violations: list[Violation] = []
    text = path.read_text(encoding="utf-8")
    is_acl = layer == "acl"
    is_infrastructure = layer == "infrastructure"
    is_application = layer == "application"
    allowed_dependencies = set(manifest.allowed_dependencies.get(feature, ()))

    for line_no, import_path in imports_from(path):
        for forbidden in FEATURE_LAYER_FORBIDDEN_IMPORT_PREFIXES.get(layer, ()):
            if import_path.startswith(forbidden):
                violations.append(Violation(path, line_no, import_path, f"feature {layer} forbidden import"))

        imported_feature_layer = feature_layer_for_import(import_path)
        if imported_feature_layer:
            imported_feature, imported_layer = imported_feature_layer
            if feature == "shared" and imported_feature != "shared":
                violations.append(Violation(path, line_no, import_path, "shared leaf dependency"))
            if imported_feature == "shared":
                if layer == "domain" and imported_layer != "domain":
                    violations.append(Violation(path, line_no, import_path, "shared non-domain dependency"))
                continue
            if imported_feature != feature and imported_feature not in allowed_dependencies:
                violations.append(Violation(path, line_no, import_path, "feature allowedDependencies"))
            if layer == "domain":
                if imported_feature != feature:
                    violations.append(Violation(path, line_no, import_path, "cross-feature domain dependency"))
                if imported_layer in {"application", "interface", "infrastructure", "acl"}:
                    violations.append(Violation(path, line_no, import_path, f"feature domain -> {imported_layer}"))
            if layer in {"domain", "application"} and imported_layer in {"interface", "infrastructure", "acl"}:
                violations.append(Violation(path, line_no, import_path, f"feature {layer} -> {imported_layer}"))
            if layer == "infrastructure" and imported_layer == "interface":
                violations.append(Violation(path, line_no, import_path, "feature infrastructure -> interface"))
            if layer == "acl" and imported_feature != feature and imported_layer == "infrastructure":
                violations.append(Violation(path, line_no, import_path, "acl concrete feature dependency"))
            if layer != "acl" and imported_feature != feature and imported_layer in {"infrastructure", "acl"}:
                violations.append(Violation(path, line_no, import_path, "cross-feature concrete dependency"))

        for forbidden in manifest.concrete_import_prefixes:
            if import_path.startswith(forbidden) and not (is_acl or is_infrastructure):
                violations.append(Violation(path, line_no, import_path, "non-ACL external concrete import"))
        if is_application:
            for io_prefix in APPLICATION_IO_IMPORT_PREFIXES:
                if import_path.startswith(io_prefix):
                    violations.append(Violation(path, line_no, import_path, "application IO import"))

    if not (is_acl or is_infrastructure):
        for concrete_type in manifest.concrete_type_names:
            pattern = re.compile(rf"\b{re.escape(concrete_type)}\b")
            for line_no, line in enumerate(text.splitlines(), start=1):
                if line.lstrip().startswith("//"):
                    continue
                if pattern.search(line):
                    violations.append(Violation(path, line_no, concrete_type, "non-ACL external concrete type"))
                    break

    if is_application:
        for line_no, line in enumerate(text.splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith("//"):
                continue
            for io_token in ("std::fs::", "tokio::fs::", "std::net::", "tokio::net::"):
                if io_token in stripped:
                    violations.append(Violation(path, line_no, io_token, "application IO usage"))
                    return violations

    return violations


def removed_crate_root_violations(path: Path) -> list[Violation]:
    violations: list[Violation] = []
    for line_no, import_path in imports_from(path):
        for forbidden in REMOVED_CRATE_ROOT_PREFIXES:
            if import_path.startswith(forbidden):
                violations.append(Violation(path, line_no, import_path, forbidden))
    return violations


def code_only_line(
    line: str,
    block_comment_depth: int = 0,
    raw_string_hashes: int | None = None,
) -> tuple[str, int, int | None]:
    result: list[str] = []
    in_string = False
    escape = False
    idx = 0
    while idx < len(line):
        ch = line[idx]
        next_ch = line[idx + 1] if idx + 1 < len(line) else ""
        if raw_string_hashes is not None:
            terminator = '"' + ("#" * raw_string_hashes)
            if line.startswith(terminator, idx):
                result.extend(" " * len(terminator))
                idx += len(terminator)
                raw_string_hashes = None
                continue
            result.append(" ")
            idx += 1
            continue
        if block_comment_depth > 0:
            if ch == "/" and next_ch == "*":
                block_comment_depth += 1
                result.extend("  ")
                idx += 2
                continue
            if ch == "*" and next_ch == "/":
                block_comment_depth -= 1
                result.extend("  ")
                idx += 2
                continue
            result.append(" ")
            idx += 1
            continue
        if not in_string and ch == "/" and next_ch == "/":
            break
        if not in_string and ch == "/" and next_ch == "*":
            block_comment_depth += 1
            result.extend("  ")
            idx += 2
            continue
        raw_match = re.match(r'r(#+)?"', line[idx:])
        if not in_string and raw_match:
            raw_string_hashes = len(raw_match.group(1) or "")
            result.extend(" " * len(raw_match.group(0)))
            idx += len(raw_match.group(0))
            continue
        if ch == '"' and not escape:
            in_string = not in_string
            result.append(" ")
        elif in_string:
            result.append(" ")
        else:
            result.append(ch)
        escape = in_string and ch == "\\" and not escape
        if ch != "\\":
            escape = False
        idx += 1
    return "".join(result), block_comment_depth, raw_string_hashes


def imports_from(path: Path) -> Iterable[tuple[int, str]]:
    pending_import: list[str] = []
    pending_start = 0
    skip_next_cfg_test_item = False
    cfg_test_brace_depth: int | None = None
    block_comment_depth = 0
    raw_string_hashes: int | None = None

    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        code_line, block_comment_depth, raw_string_hashes = code_only_line(
            line,
            block_comment_depth,
            raw_string_hashes,
        )
        stripped = code_line.strip()
        if stripped == "#[cfg(test)]":
            skip_next_cfg_test_item = True
            pending_import = []
            continue
        if cfg_test_brace_depth is not None:
            cfg_test_brace_depth += code_line.count("{") - code_line.count("}")
            if cfg_test_brace_depth <= 0:
                cfg_test_brace_depth = None
            continue
        if skip_next_cfg_test_item and stripped:
            if stripped.startswith("mod ") and "{" in stripped:
                cfg_test_brace_depth = stripped.count("{") - stripped.count("}")
                if cfg_test_brace_depth <= 0:
                    cfg_test_brace_depth = None
                skip_next_cfg_test_item = False
                continue
            skip_next_cfg_test_item = False
            continue
        if skip_next_cfg_test_item:
            continue
        if stripped.startswith("//") or stripped.startswith("///") or stripped.startswith("//"):
            continue

        if pending_import:
            pending_import.append(stripped)
            if ";" in stripped:
                yield pending_start, normalize_import(" ".join(pending_import).rstrip(";"))
                pending_import = []
                pending_start = 0
            continue

        match = IMPORT_START_RE.match(code_line)
        if match:
            import_body = match.group(1).strip()
            if ";" in import_body:
                yield line_no, normalize_import(import_body.rstrip(";"))
            else:
                pending_import = [import_body]
                pending_start = line_no
            continue

        for inline_path in INLINE_CRATE_PATH_RE.findall(code_line):
            yield line_no, re.sub(r"\s*::\s*", "::", inline_path)


def check_project(root: Path = PROJECT_ROOT) -> list[Violation]:
    violations: list[Violation] = []
    manifest = load_feature_acl_manifest(root)
    for path in rust_files(root / "src"):
        violations.extend(removed_crate_root_violations(path))
    for rule in RULES:
        layer_root = root / rule.layer_path
        if not layer_root.exists():
            continue
        for path in rust_files(layer_root):
            for line_no, import_path in imports_from(path):
                for forbidden in rule.forbidden_import_prefixes:
                    if import_path.startswith(forbidden):
                        violations.append(Violation(path, line_no, import_path, forbidden))
    cli_path = root / "src/cli.rs"
    if cli_path.exists():
        violations.extend(cli_feature_infrastructure_violations(cli_path))
    features_root = root / "src/features"
    if features_root.exists():
        for path in rust_files(features_root):
            violations.extend(feature_acl_violations(path, root, manifest))
    return violations


def main() -> int:
    root = PROJECT_ROOT
    violations = check_project(root)
    if violations:
        print("❌ architecture boundary violations:", file=sys.stderr)
        for violation in violations:
            print(f"  - {violation.format(root)}", file=sys.stderr)
        return 1
    print("✅ architecture boundary check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
