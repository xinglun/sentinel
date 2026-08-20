#!/usr/bin/env python3
"""Validate documentation front matter and supported-stack lists."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path, PurePosixPath
from typing import cast

from ai_documentation_authority import validate_registry
from ai_documentation_journey import validate_journeys, validate_topics
from install_ai_cockpit import STACKS

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_FRONT_MATTER = ("author", "title", "description")
README_FILES = ("README.md", "README.ja.md", "README.zh-CN.md")
FORMAL_METADATA_FIELDS = ("audience", "status", "authority", "lastVerifiedBy")
FORMAL_AUDIENCES = {"adopter", "maintainer", "security_reviewer", "auditor", "contributor"}
FORMAL_STATUSES = {"current", "reference", "historical", "draft"}
FORMAL_AUTHORITIES = {"canonical", "derived", "explanatory", "archived_evidence"}
WI07_FORMAL_DOCUMENTS = {
    "docs/getting-started/installation.md": ("current", "canonical"),
    "docs/getting-started/first-calibration.md": ("current", "canonical"),
    "docs/getting-started/first-work-item.md": ("current", "canonical"),
    "docs/concepts/trust-layer.md": ("current", "canonical"),
    "docs/concepts/evidence-governance.md": ("current", "canonical"),
    "docs/concepts/decision-states.md": ("current", "canonical"),
    "docs/operations/quality-gates.md": ("current", "canonical"),
    "docs/operations/work-item-lifecycle.md": ("current", "canonical"),
    "docs/operations/recovery.md": ("current", "canonical"),
    "docs/security/threat-model.md": ("current", "canonical"),
    "docs/security/injection-boundary.md": ("current", "canonical"),
    "docs/security/supply-chain.md": ("current", "canonical"),
    "docs/reference/capability-truth-matrix.md": ("reference", "canonical"),
    "docs/reference/documentation-architecture.md": ("reference", "canonical"),
    "docs/reference/schemas.md": ("reference", "canonical"),
    "docs/reference/commands.md": ("reference", "canonical"),
    "docs/archive/plans/README.md": ("historical", "archived_evidence"),
    "docs/archive/reviews/README.md": ("historical", "archived_evidence"),
    "docs/archive/historical-designs/README.md": ("historical", "archived_evidence"),
}
README_SECTION_MARKERS = {
    "identity",
    "problem",
    "how-it-works",
    "decision-states",
    "quick-start",
    "boundary",
    "documentation",
}
README_CAPABILITY_MARKER = "<!-- release-capabilities: auditable-adoption,sha256-verification -->"
README_PREREQUISITE_MARKER = (
    "<!-- install-prerequisites: python3.11,git-initial-commit,curl,gnu-make,posix -->"
)
VERIFIED_STACKS: tuple[str, ...] = (
    "python",
    "go",
    "rust",
    "typescript",
    "java",
    "kotlin",
    "ruby",
    "php",
    "csharp",
    "flutter",
    "android",
    "swift",
)
WORKFLOW_IMPLEMENTED_STACKS: tuple[str, ...] = ()
TEMPLATE_ONLY_STACKS = ("generic",)
JAPANESE_STYLE_RULES = {
    "Gemini, Claude, Codex": "use Japanese punctuation between agent names",
    "実行時の安全性を確保": "do not overstate command registry guarantees",
    "Use this stack preset": "translate instructional prose into Japanese",
    "Suggested guard patterns": "translate instructional prose into Japanese",
    "阻断": "use Japanese terminology such as ブロッキング",
    "確信度": "use 信頼度 for confidence in Japanese documentation",
}
COMMAND_EVIDENCE_LABELS = {
    "syntax_tested",
    "fixture_executed",
    "hosted_executed",
    "adopter_required",
    "illustrative_only",
}
EXECUTABLE_FENCE_LANGUAGES = {"sh", "bash", "shell", "console", "make", "zsh"}
CAPABILITY_MATRIX_RELATIVE_LINK = str(
    PurePosixPath("..") / "reference" / "capability-truth-matrix.md"
)
DOCUMENTED_INSTALLER_OPTIONS = {
    "--create-adoption",
    "--dry-run",
    "--interactive",
    "--replace-glossary",
    "--stack",
    "--update-makefile",
    "--upgrade",
    "--upgrade-with-active",
    "--with-examples",
}
DOCUMENTED_INSTALLER_ENV = {
    "AI_COCKPIT_TEMPLATE_REF",
    "AI_COCKPIT_TEMPLATE_SHA256",
}
README_BOOTSTRAP_ENV = {
    "AI_COCKPIT_TEMPLATE_PUBLIC_REPOSITORY",
    "AI_COCKPIT_TEMPLATE_RAW_BASE",
    "AI_COCKPIT_TEMPLATE_REPO",
    "AI_COCKPIT_TEMPLATE_SOURCE",
}
CANONICAL_PUBLIC_SOURCE_DEFAULTS = {
    "https://github.com/spirex-ds-dev/ai-cockpit-template.git",
    "https://raw.githubusercontent.com/spirex-ds-dev/ai-cockpit-template",
}
LAYERED_DOCUMENTS = {
    "30-second-start": {
        "wizard-start",
        "does",
        "does-not",
        "after-installation",
    },
    "standard-adoption-guide": {
        "adoption",
        "calibration",
        "work-item",
        "ci",
        "human-approval",
        "target-project-adaptation",
    },
    "security-release-verification": {
        "release-metadata",
        "digest",
        "provenance",
        "sbom",
        "trust-root",
        "private-mirror",
        "local-source",
        "enterprise-boundary",
    },
}
LANGUAGE_SUFFIXES = {"en": "", "zh-CN": ".zh-CN", "ja": ".ja"}
BEGINNER_INSTALLATION_STAGES = (
    "before-you-start",
    "open-your-project",
    "copy-discovery-prompt",
    "review-read-only-report",
    "choose-wizard-options",
    "review-installation-plan",
    "approve-scaffold-write",
    "inspect-scaffold",
    "complete-calibration",
    "run-local-checks",
    "complete-first-work-item",
    "review-pr-and-hosted-ci",
    "merge-and-close",
    "recover-from-a-stop",
    "confirm-installation-success",
)
CALIBRATION_STAGES = (
    "repository-role",
    "language-and-stack",
    "source-boundaries",
    "test-boundaries",
    "generated-artifacts",
    "critical-paths",
    "quality-commands",
    "review-requirements",
    "risks-and-unknowns",
    "adoption-readiness",
)
PROMPT_SAFETY_BOUNDARIES = (
    "read-only-discovery",
    "explain-evidence-unknowns",
    "plan-before-write",
    "human-confirmation-before-write",
    "no-downstream-authority",
    "preserve-user-changes",
)
INSTALLATION_PLATFORMS = ("ios", "android", "java")
PLATFORM_INSTALLATION_STAGES = (
    "detect-project",
    "collect-toolchain-evidence",
    "choose-stack-and-boundaries",
    "discover-quality-commands",
    "calibrate-generated-and-critical-paths",
    "stop-and-recover",
    "verify-platform-adoption",
)
PLATFORM_EVIDENCE_BOUNDARY = "<!-- platform-boundary: no-toolchain-device-signing-hosted-claim -->"
PLATFORM_ENTRY_MARKER = "<!-- platform-entry: work-item-first -->"
PLATFORM_NEXT_MARKER = "<!-- platform-next: calibration-and-recovery -->"
LEGACY_PLATFORM_MARKERS = (
    "<!-- platform-step-table:",
    "<!-- platform-filled-example:",
    "<!-- platform-stage:",
)
COMMAND_GUIDE_MARKER = "<!-- command-guide: purpose,success,failure -->"
PLATFORM_PROMPT_MARKER = "<!-- platform-prompt: copy-ready -->"
SCAFFOLD_REVIEW_TABLE_MARKER = "<!-- scaffold-review-table: copy-request,expected,pass,stop -->"
CALIBRATION_REVIEW_TABLE_MARKER = (
    "<!-- calibration-review-table: copy-request,example,pass,stop -->"
)
CALIBRATION_COMPLETION_TABLE_MARKER = (
    "<!-- calibration-completion-checklist: "
    "state,evidence,answer,candidate,owner-reviewer,pass-stop -->"
)
CALIBRATION_ANSWER_TYPES_MARKER = (
    "<!-- calibration-answer-types: yes_no,alternative_input,unknown,not_applicable -->"
)
CALIBRATION_YES_NO_BOUNDARY = "<!-- calibration-yes-no: type=yes_no,values=Y-or-N -->"
CALIBRATION_RUNTIME_BOUNDARY = (
    "<!-- calibration-runtime-boundary: unknown-machine-blocked,confirmations-candidate-bound -->"
)
CALIBRATION_OBSOLETE_RUNTIME_BOUNDARY = (
    "<!-- calibration-runtime-boundary: "
    "unknown-not-machine-blocked,confirmations-not-candidate-bound -->"
)
INSTALLATION_PLAN_RELEASE_BINDING = (
    "<!-- installation-plan-release-binding: "
    "resolved-tag,metadata,asset,digest,installer,wizard -->"
)
RELEASE_METADATA_BOUNDARY = (
    "<!-- release-metadata-boundary: "
    "provider-discovers-latest-verifiable,tag-pinned-verifies-evidence -->"
)
RELEASE_FALLBACK_APPROVAL = (
    "<!-- release-fallback-approval: failed-newer-evidence,owner-review,reverify -->"
)
MAKE_ENTRYPOINT_BOUNDARY = "<!-- make-entrypoint-boundary: included-makefile-or-explicit-f -->"
MAKE_COMPOSITE_BOUNDARY = (
    "<!-- make-composite-boundary: selected-entrypoint-propagates-through-ai-finish -->"
)
CALIBRATION_CONFIRMATION_BOUNDARY = (
    "<!-- calibration-confirmation-boundary: phase-records,external-actor-identity -->"
)
CALIBRATION_CI_GAP_BOUNDARY = (
    "<!-- calibration-ci-gap-boundary: plan,approval,implementation,verification -->"
)
CALIBRATION_SESSION_PERSISTENCE_BOUNDARY = (
    "<!-- calibration-session-persistence-boundary: "
    "structured-checklist-evidence,candidate-bound -->"
)
CALIBRATION_SESSION_EVIDENCE_BOUNDARY = (
    "<!-- calibration-session-evidence-boundary: "
    "combined-stage-seven-column-record,labels-not-actor-proof -->"
)
CALIBRATION_SESSION_COMPLETE_CLAIM = {
    "en": (
        "The Session persists all schema-supported data needed for the complete "
        "seven-column review row in the combined stage record."
    ),
    "zh-CN": ("Session 会在合并后的阶段记录中持久化完整七列审核行所需的全部 schema 支持数据。"),
    "ja": ("Session は、7 列の確認表に必要な全項目を、段階ごとの記録として保存します。"),
}
CALIBRATION_WORK_ITEM_EVIDENCE_BOUNDARY = {
    "en": (
        "The Work Item keeps governance rationale, acceptance, owner decisions, "
        "and links to external review evidence; it does not replace the Session facts."
    ),
    "zh-CN": (
        "Work Item 保存治理理由、验收、Owner 决定及外部审核证据链接；"
        "它不会替代 Session 中的事实记录。"
    ),
    "ja": (
        "Work Item には、ガバナンス上の理由、受入条件、Owner の判断、"
        "外部レビューの根拠へのリンクを記録します。"
        "Session の事実記録を置き換えるものではありません。"
    ),
}
CALIBRATION_REVIEWER_LABEL_LIMITATION = {
    "en": (
        "Recorded `reviewer` and `owner` labels do not prove who performed the review "
        "or that duties were independently separated."
    ),
    "zh-CN": (
        "记录下来的 `reviewer` 和 `owner` 标签不能证明实际由谁完成审核，"
        "也不能证明职责已经独立分离。"
    ),
    "ja": (
        "保存された `reviewer` / `owner` ラベルは文字列にすぎず、"
        "レビュー実施者の本人確認も、"
        "独立した役割分離が成立したことも証明しません。"
    ),
}
CALIBRATION_COMMAND_STORAGE_BOUNDARY = {
    "en": (
        "After I decide, use `answer` to persist the answer fields and resulting stage "
        "completion state. Use `record-evidence` to persist observed evidence, the "
        "Candidate change, Owner/Reviewer labels, and PASS/STOP decision details in "
        "`checklistEvidence`."
    ),
    "zh-CN": (
        "我决定后，用 `answer` 保存回答字段及由此产生的阶段完成状态；用 "
        "`record-evidence` 将观察证据、Candidate change、Owner/Reviewer 标签和 "
        "PASS/STOP 判定详情保存到 `checklistEvidence`。"
    ),
    "ja": (
        "判断後は、`answer` で回答項目と、それに伴う段階の完了状態を保存します。"
        "`record-evidence` では、確認した根拠、Candidate の変更案、"
        "Owner/Reviewer ラベル、PASS/STOP と判断内容を `checklistEvidence` "
        "に保存します。"
    ),
}
CALIBRATION_NARROW_SESSION_PATTERNS = {
    "en": (
        re.compile(r"\bSession\s+(?:stores|persists)\s+only\b", re.IGNORECASE),
        re.compile(r"\bSession\b[^.]{0,300}\bauthoritative\s+only\b", re.IGNORECASE),
        re.compile(
            r"\bonly\b[^.]{0,200}\b(?:persisted|stored)\s+in\s+the\s+Session\b",
            re.IGNORECASE,
        ),
        re.compile(
            r"\bSession\b[^.]{0,160}\b(?:answer|answers|stage\s+state)\b"
            r"[^.;]{0,80}\bonly\b",
            re.IGNORECASE,
        ),
        re.compile(
            r"\b(?:it|Session)\s+does\s+not\s+store\s+(?:the\s+)?other\s+"
            r"checklist\s+columns\b",
            re.IGNORECASE,
        ),
    ),
    "zh-CN": (
        re.compile(r"Session\s*(?:中)?(?:只|仅)(?:保存|持久化|对)"),
        re.compile(r"(?:它|Session)[^。；\n]{0,120}不保存(?:清单中的)?其他列"),
    ),
    "ja": (
        re.compile(
            r"Session[^。\n]*(?:保存するのは|が保存するのは)"
            r"[^。\n]*(?:だけ|のみ)(?:です|。)"
        ),
        re.compile(r"Session\s*には[^。\n]*(?:しか保存され|しか記録され)"),
        re.compile(r"Session\s*は[^。\n]{0,120}のみを(?:保存|記録)"),
        re.compile(r"表の他の列は\s*Session\s*に保存しません"),
    ),
}
CALIBRATION_COMPLETION_HEADING = {
    "en": "### Calibration completion checklist",
    "zh-CN": "### 校准完成记录清单",
    "ja": "### Calibration 完了記録チェックリスト",
}
CALIBRATION_ACTIVATION_ATOMICITY_BOUNDARY = (
    "<!-- calibration-activation-atomicity: "
    "active-session-rollback-transaction,candidate-digest-bound -->"
)
CALIBRATION_TRANSACTION_RUNTIME_TERMS = (
    "`record-evidence`",
    "`prepare-candidate`",
    "consistency unproved",
)
TAG_PINNED_RELEASE_METADATA_TEMPLATE = (
    "https://raw.githubusercontent.com/spirex-ds-dev/"
    "ai-cockpit-template/<resolved-tag>/release.json"
)
MOVING_MAIN_RELEASE_METADATA = (
    "https://raw.githubusercontent.com/spirex-ds-dev/ai-cockpit-template/main/release.json"
)
CALIBRATION_ACTIVATION_STAGES = ("plan-before-approval", "bounded-approval")
PLATFORM_STEP_TABLE_MARKER = "<!-- platform-step-table: copy-request,example,pass,stop -->"
PLATFORM_FILLED_EXAMPLE_MARKER = "<!-- platform-filled-example: seven-stages -->"
PLATFORM_STAGE5_PROPOSAL_MARKER = "<!-- platform-stage5: proposal-only -->"
INSTALLATION_PROOFREADING_CHECKLIST_MARKER = (
    "<!-- installation-proofreading-checklist: "
    "version-neutral,prompt-first,steps,calibration,platforms,tables,links,lifecycle -->"
)
INSTALLATION_VERSION_NEUTRAL_TEXT = {
    "en": "This guide is version-neutral:",
    "zh-CN": "本手顺与版本无关：",
    "ja": "この手順は version-neutral です。",
}
INSTALLATION_PROOFREADING_HEADING = {
    "en": "## Installation-document proofreading checklist",
    "zh-CN": "## 安装文档校对清单",
    "ja": "## インストール文書の校正チェックリスト",
}
LIFECYCLE_ORDER_MARKER = "<!-- lifecycle-order: adoption-close-before-configuration -->"
LIFECYCLE_APPROVAL_STAGES = (
    "adoption-closure-plan",
    "adoption-closure-execute",
    "configuration-closure-plan",
    "configuration-closure-execute",
)
PROMPT_AUTHORITY_TEXT = {
    "en": "Do not create, edit, delete, commit, push, open or merge a PR, or publish.",
    "zh-CN": "不要创建、编辑、删除、commit、push、创建或合并 PR，也不要发布。",
    "ja": "作成、編集、削除、commit、push、PR 作成・マージ、公開は禁止です。",
}
SEMANTIC_DOMAINS = {
    "north-star",
    "product-boundary",
    "installation-flow",
    "human-confirmation",
    "security-limits",
    "prompt-injection-limits",
    "enterprise-compliance-boundary",
    "supported-scope",
    "release-version",
    "task-outcome-fields",
}
HISTORICAL_MARKER = (
    "> **Historical Record**\n"
    "> **Not Current Product Documentation**\n"
    "> **Do Not Use As Runtime Instruction**"
)
STALE_UI_LOCALIZATION_CLAIMS = {
    "Japanese is the default UI locale",
    "既定の Wizard 言語は日本語です",
    "Wizard 默认语言是日语",
}
STALE_PUBLISHED_TAG_CLAIMS = {
    "highest published semantic-version tag",
    "公开的语义化版本标签中选择最高版本",
    "公開済みのセマンティックバージョンタグから最新",
}


def documentation_files(root: Path) -> list[Path]:
    files = [root / name for name in README_FILES]
    files.append(root / ".ai" / "README.md")
    files.append(root / ".ai" / "glossary.md")
    files.extend(sorted((root / "docs").rglob("*.md")))
    files.extend(sorted((root / "examples").glob("*/README.md")))
    return files


def front_matter_errors(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        return [f"{path}: missing YAML front matter"]
    closing = text.find("\n---\n", 4)
    if closing < 0:
        return [f"{path}: unterminated YAML front matter"]
    block = text[4:closing]
    keys = {
        match.group(1)
        for line in block.splitlines()
        if (match := re.match(r"^([A-Za-z][A-Za-z0-9_-]*):", line))
    }
    return [
        f"{path}: front matter missing {key}" for key in REQUIRED_FRONT_MATTER if key not in keys
    ]


def _front_matter_values(path: Path) -> tuple[dict[str, str], set[str]]:
    text = path.read_text(encoding="utf-8")
    closing = text.find("\n---\n", 4)
    if not text.startswith("---\n") or closing < 0:
        return {}, set()
    values: dict[str, str] = {}
    audiences: set[str] = set()
    current_key = ""
    for line in text[4:closing].splitlines():
        match = re.match(r"^([A-Za-z][A-Za-z0-9_-]*):\s*(.*?)\s*$", line)
        if match:
            current_key = match.group(1)
            values[current_key] = match.group(2).strip("\"'")
            continue
        if current_key == "audience" and (item := re.match(r"^\s+-\s+([a-z_]+)\s*$", line)):
            audiences.add(item.group(1))
    if values.get("audience"):
        audiences.add(values["audience"])
    return values, audiences


def formal_document_metadata_errors(root: Path) -> list[str]:
    errors: list[str] = []
    for relative, (expected_status, expected_authority) in WI07_FORMAL_DOCUMENTS.items():
        path = root / relative
        if not path.is_file():
            continue
        values, audiences = _front_matter_values(path)
        for field in FORMAL_METADATA_FIELDS:
            if field not in values:
                errors.append(f"{relative}: front matter missing {field}")
        invalid_audiences = audiences - FORMAL_AUDIENCES
        if not audiences:
            errors.append(f"{relative}: audience must contain at least one allowed value")
        for audience in sorted(invalid_audiences):
            errors.append(f"{relative}: invalid audience: {audience}")
        status = values.get("status")
        authority = values.get("authority")
        if status and status not in FORMAL_STATUSES:
            errors.append(f"{relative}: invalid status: {status}")
        elif status and status != expected_status:
            errors.append(f"{relative}: expected status {expected_status}, found {status}")
        if authority and authority not in FORMAL_AUTHORITIES:
            errors.append(f"{relative}: invalid authority: {authority}")
        elif authority and authority != expected_authority:
            errors.append(f"{relative}: expected authority {expected_authority}, found {authority}")
    return errors


def documentation_architecture_errors(root: Path) -> list[str]:
    errors: list[str] = []
    for relative in WI07_FORMAL_DOCUMENTS:
        if not (root / relative).is_file():
            errors.append(f"{relative}: required WI07 canonical document is missing")
    marker_pattern = re.compile(r"<!--\s*readme-section:\s*([a-z0-9-]+)\s*-->")
    for name in README_FILES:
        path = root / name
        text = path.read_text(encoding="utf-8")
        markers = marker_pattern.findall(text)
        for marker in sorted(set(markers) - README_SECTION_MARKERS):
            errors.append(f"{name}: unsupported README section marker: {marker}")
        for marker in sorted(README_SECTION_MARKERS - set(markers)):
            errors.append(f"{name}: missing README section marker: {marker}")
        if len(markers) != len(README_SECTION_MARKERS):
            errors.append(f"{name}: README sections must map one-to-one to the WI07 entry model")
        if len(text.splitlines()) > 140:
            errors.append(f"{name}: README entry page exceeds 140 lines")
    return errors


def tier_marker() -> str:
    return (
        "<!-- stack-tiers: verified="
        + ",".join(VERIFIED_STACKS)
        + "; workflow-implemented="
        + ",".join(WORKFLOW_IMPLEMENTED_STACKS)
        + "; preset-only="
        + ",".join(TEMPLATE_ONLY_STACKS)
        + " -->"
    )


def stack_errors(root: Path) -> list[str]:
    ordered_stacks = [
        "generic",
        "rust",
        "flutter",
        "typescript",
        "python",
        "go",
        "java",
        "android",
        "kotlin",
        "swift",
        "ruby",
        "php",
        "csharp",
    ]
    if set(ordered_stacks) != STACKS:
        return [
            "scripts/check_docs_metadata.py: canonical stack order does not match installer STACKS"
        ]

    marker = tier_marker()
    errors = []
    configuration = (root / "docs" / "configuration.md").read_text(encoding="utf-8")
    configuration_list = "\n".join(ordered_stacks)
    if configuration_list not in configuration:
        errors.append("docs/configuration.md: supported-stack list does not match installer STACKS")
    if marker not in configuration:
        errors.append(
            "docs/configuration.md: stack compatibility tiers do not match executable CI evidence"
        )
    return errors


def installation_command_errors(root: Path) -> list[str]:
    release = json.loads((root / "release.json").read_text(encoding="utf-8"))
    release_tag = release["releaseTag"]
    candidate_path = root / "next-release.json"
    documented_release_tags = {release_tag}
    installer_tag = release_tag
    if candidate_path.is_file():
        candidate = json.loads(candidate_path.read_text(encoding="utf-8"))
        candidate_tag = candidate.get("releaseTag")
        documented_release_tags.add(candidate_tag)
        if candidate.get("releaseState") == "candidate" and candidate.get("published") is False:
            installer_tag = candidate_tag
    archive_capability = release["capabilities"]["sha256ArchiveVerification"]
    if isinstance(archive_capability, dict):
        sha256_published = (
            archive_capability.get("supported") is True
            and archive_capability.get("verified") is True
        )
    else:
        sha256_published = archive_capability is True
    errors = []
    for path in documentation_files(root):
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        if relative in README_FILES and re.search(r"\bv\d+\.\d+\.\d+\b", text):
            errors.append(
                f"{relative}: primary README must not hardcode a concrete release version"
            )
        for number, line in enumerate(text.splitlines(), start=1):
            if (
                "raw.githubusercontent.com/spirex-ds-dev/ai-cockpit-template/main/install.sh"
                in line
            ):
                errors.append(
                    f"{relative}:{number}: remote installer must use a fixed tag or commit"
                )
            if (
                "--stack" in line
                and "install" in line
                and "--upgrade" not in line
                and "--update-makefile" not in line
            ):
                errors.append(
                    f"{relative}:{number}: install command with --stack requires --update-makefile"
                )
            if (
                relative.startswith("examples/")
                and "--stack" in line
                and "install" in line
                and "--create-adoption" not in line
            ):
                errors.append(
                    f"{relative}:{number}: example install command must create auditable adoption evidence"
                )
            for tag in re.findall(r"v\d+\.\d+\.\d+", line):
                if relative.startswith(
                    (
                        "docs/releases/",
                        "docs/audits/",
                        "docs/superpowers/plans/",
                        "docs/superpowers/specs/",
                    )
                ):
                    continue
                if tag not in documented_release_tags:
                    errors.append(
                        f"{relative}:{number}: documented release {tag} does not match release.json {release_tag}"
                    )
            if (
                not sha256_published
                and "AI_COCKPIT_TEMPLATE_SHA256" in line
                and "does **not** implement" not in line
                and "additional assertion" not in line
                and "追加のアサーション" not in line
                and "附加断言" not in line
            ):
                errors.append(
                    f"{relative}:{number}: SHA256 verification is not published for {release_tag}"
                )
    install_script = (root / "install.sh").read_text(encoding="utf-8")
    advanced_installation_reference = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (
            root / "docs" / "getting-started" / "30-second-start.md",
            root / "docs" / "reference" / "distribution.md",
            root / "docs" / "reference" / "upgrade.md",
        )
    )
    for option in sorted(DOCUMENTED_INSTALLER_OPTIONS):
        if option not in install_script:
            errors.append(f"install.sh: documented installer option is not implemented: {option}")
        if option not in advanced_installation_reference:
            errors.append(
                "advanced installation reference: "
                f"implemented installer option is undocumented: {option}"
            )
    for variable in sorted(DOCUMENTED_INSTALLER_ENV):
        if variable not in install_script:
            errors.append(
                f"install.sh: documented installer environment variable is not implemented: {variable}"
            )
        if variable not in advanced_installation_reference:
            errors.append(
                "advanced installation reference: "
                f"installer environment variable is undocumented: {variable}"
            )
    quick_starts = {
        "en": root / "docs" / "getting-started" / "30-second-start.md",
        "ja": root / "docs" / "getting-started" / "30-second-start.ja.md",
        "zh-CN": root / "docs" / "getting-started" / "30-second-start.zh-CN.md",
    }
    for language, path in quick_starts.items():
        text = path.read_text(encoding="utf-8")
        if "main/release.json" not in text or "$RELEASE_TAG/install.sh" not in text:
            errors.append(
                f"{path.relative_to(root).as_posix()}: quick start must resolve the tagged installer from release.json"
            )
        for source in sorted(CANONICAL_PUBLIC_SOURCE_DEFAULTS):
            if source not in text:
                errors.append(
                    f"{path.relative_to(root).as_posix()}: canonical public source default is missing: {source}"
                )
        if language == "en" and "--interactive" not in text:
            errors.append("docs/getting-started/30-second-start.md: wizard entry is missing")
    if not isinstance(installer_tag, str) or (
        f'REF="${{AI_COCKPIT_TEMPLATE_REF:-{installer_tag}}}"' not in install_script
    ):
        errors.append("install.sh: default ref does not match canonical release candidate")
    return errors


def japanese_style_errors(root: Path) -> list[str]:
    errors = []
    paths = [
        root / "README.ja.md",
        *sorted((root / "docs").rglob("*.md")),
        *sorted((root / "examples").glob("*/README.md")),
    ]
    for path in paths:
        relative = path.relative_to(root).as_posix()
        if not (path.name == "README.ja.md" or path.name.endswith(".ja.md")):
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            for phrase, reason in JAPANESE_STYLE_RULES.items():
                if phrase in line:
                    errors.append(f"{relative}:{number}: Japanese style: {reason}: {phrase}")
            if re.search(r"\d+つ", line):
                errors.append(
                    f"{relative}:{number}: Japanese style: add a space between a number and つ"
                )
    return errors


def capability_claim_errors(root: Path) -> list[str]:
    """Reject known stale capability claims using the authoritative matrix."""
    matrix_path = root / "docs" / "reference" / "capability-truth-matrix.json"
    matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
    statuses = {item["id"]: item["status"] for item in matrix["capabilities"]}
    stale_claims: list[str] = []
    if statuses.get("ten_stage_calibration_session") == "implemented":
        stale_claims.extend(
            (
                "ten-stage session and Candidate activation remain planned capabilities",
                "10 Stage セッションと Candidate 有効化は計画中の能力です",
                "十 Stage 会话与 Candidate 激活仍属于计划能力",
                "10 Stage セッションと Candidate 有効化は専用 Work Item の完了まで計画中です",
            )
        )
    if statuses.get("candidate_activation_and_active_preservation") == "implemented":
        stale_claims.append(
            "Candidate activation and preservation of the old Active Configuration are planned capabilities until the corresponding Work Item evidence exists"
        )
    errors: list[str] = []
    paths = (
        root / "README.md",
        root / "README.ja.md",
        root / "README.zh-CN.md",
        root / "docs" / "getting-started" / "installation.md",
        root / "docs" / "getting-started" / "installation.zh-CN.md",
        root / "docs" / "getting-started" / "installation.ja.md",
        root / "docs" / "reference" / "upgrade.md",
    )
    for path in paths:
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        for claim in stale_claims:
            if claim in text:
                errors.append(
                    f"{relative}: unsupported current-capability claim contradicts matrix: {claim}"
                )
    return errors


def documentation_fact_errors(root: Path) -> list[str]:
    """Bind prominent WI-10 prose facts to executable repository behavior."""
    errors: list[str] = []
    makefile = (root / "Makefile").read_text(encoding="utf-8")
    floor_match = re.search(r"--cov-fail-under=([0-9]+(?:\.[0-9]+)?)", makefile)
    if floor_match is None:
        errors.append("Makefile: project-test coverage floor is missing")
    else:
        floor = f"{floor_match.group(1)}%"
        configuration = (root / "docs" / "configuration.md").read_text(encoding="utf-8")
        if floor not in configuration:
            errors.append(
                f"docs/configuration.md: documented coverage floor differs from Makefile: {floor}"
            )

    for name in README_FILES:
        text = (root / name).read_text(encoding="utf-8")
        for claim in sorted(STALE_UI_LOCALIZATION_CLAIMS | STALE_PUBLISHED_TAG_CLAIMS):
            if claim in text:
                errors.append(f"{name}: unsupported documentation claim: {claim}")

    authoritative = [
        root / "docs" / "getting-started" / "installation.md",
        root / "docs" / "getting-started" / "installation.ja.md",
    ]
    for suffix in LANGUAGE_SUFFIXES.values():
        authoritative.extend(_layer_path(root, stem, suffix) for stem in LAYERED_DOCUMENTS)
    for path in authoritative:
        if path.is_file() and "git merge-base HEAD origin/main" in path.read_text(encoding="utf-8"):
            relative = path.relative_to(root).as_posix()
            errors.append(f"{relative}: adopter guidance must not assume origin/main")

    for suffix in LANGUAGE_SUFFIXES.values():
        path = _layer_path(root, "standard-adoption-guide", suffix)
        text = path.read_text(encoding="utf-8")
        lifecycle = (
            "make ai-finish TASK=adopt_ai_cockpit",
            'git commit -m "adopt AI Cockpit governance"',
            "make check-ai-pr",
            "make ai-close-work-item TASK=adopt_ai_cockpit",
        )
        positions = [text.find(item) for item in lifecycle]
        if any(position < 0 for position in positions) or positions != sorted(positions):
            relative = path.relative_to(root).as_posix()
            errors.append(
                f"{relative}: adoption lifecycle must commit archive evidence before PR check and closure"
            )
    return errors


def _layer_path(root: Path, stem: str, suffix: str) -> Path:
    return root / "docs" / "getting-started" / f"{stem}{suffix}.md"


def multilingual_layer_errors(root: Path) -> list[str]:
    """Require complete same-language WI-10 layers and semantic-domain parity."""
    errors: list[str] = []
    readmes = {
        "en": root / "README.md",
        "zh-CN": root / "README.zh-CN.md",
        "ja": root / "README.ja.md",
    }
    for language, suffix in LANGUAGE_SUFFIXES.items():
        language_text: list[str] = [readmes[language].read_text(encoding="utf-8")]
        for stem, required_domains in LAYERED_DOCUMENTS.items():
            path = _layer_path(root, stem, suffix)
            relative = path.relative_to(root).as_posix()
            if not path.is_file():
                errors.append(f"{relative}: required WI-10 language document is missing")
                continue
            text = path.read_text(encoding="utf-8")
            language_text.append(text)
            found = set(re.findall(r"<!--\s*doc-domain:\s*([a-z0-9-]+)\s*-->", text))
            for domain in sorted(required_domains - found):
                errors.append(f"{relative}: missing required documentation domain: {domain}")

        combined = "\n".join(language_text)
        found_semantics = set(re.findall(r"<!--\s*semantic-domain:\s*([a-z0-9-]+)\s*-->", combined))
        for domain in sorted(SEMANTIC_DOMAINS - found_semantics):
            errors.append(f"{readmes[language].name}: missing semantic domain: {domain}")
        if CAPABILITY_MATRIX_RELATIVE_LINK not in combined:
            errors.append(
                f"{readmes[language].name}: layered guidance must link Capability Truth Matrix"
            )
    return errors


def command_evidence_errors(root: Path) -> list[str]:
    """Require explicit conservative evidence labels for WI-10 executable fences."""
    errors: list[str] = []
    paths: list[Path] = []
    for suffix in LANGUAGE_SUFFIXES.values():
        paths.extend(_layer_path(root, stem, suffix) for stem in LAYERED_DOCUMENTS)
    paths.extend(
        (
            root / "docs" / "getting-started" / "installation.md",
            root / "docs" / "getting-started" / "installation.zh-CN.md",
            root / "docs" / "getting-started" / "installation.ja.md",
        )
    )
    marker_pattern = re.compile(r"^<!--\s*command-evidence:\s*([a-z_]+)\s*-->$")
    for path in paths:
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        lines = path.read_text(encoding="utf-8").splitlines()
        for number, line in enumerate(lines, start=1):
            marker = marker_pattern.match(line.strip())
            if marker and marker.group(1) not in COMMAND_EVIDENCE_LABELS:
                errors.append(
                    f"{relative}:{number}: unknown command evidence label: {marker.group(1)}"
                )
            if marker:
                following = lines[number].strip() if number < len(lines) else ""
                following_fence = re.match(r"^```([A-Za-z0-9_-]*)\s*$", following)
                if (
                    following_fence is None
                    or following_fence.group(1).lower() not in EXECUTABLE_FENCE_LANGUAGES
                ):
                    errors.append(
                        f"{relative}:{number}: command-evidence is not attached to an executable fence"
                    )
            fence = re.match(r"^```([A-Za-z0-9_-]*)\s*$", line.strip())
            if fence is None or fence.group(1).lower() not in EXECUTABLE_FENCE_LANGUAGES:
                continue
            preceding = lines[number - 2].strip() if number >= 2 else ""
            match = marker_pattern.match(preceding)
            if match is None:
                errors.append(
                    f"{relative}:{number}: executable command fence is missing command-evidence"
                )
    return errors


def _marker_values(text: str, marker: str) -> set[str]:  # pragma: no cover
    return set(re.findall(rf"<!--\s*{re.escape(marker)}:\s*([a-z0-9-]+)\s*-->", text))


def _step_table_errors(  # pragma: no cover
    text: str,
    *,
    marker: str,
    expected_rows: int,
    relative: str,
    label: str,
    require_pass_stop: bool = True,
    require_copy_request: bool = True,
) -> list[str]:
    """Require one consecutive Markdown table with ordered beginner decision rows."""
    marker_position = text.find(marker)
    if marker_position < 0:
        return [f"{relative}: missing {label} decision table"]
    section_end = text.find("\n## ", marker_position)
    section = text[marker_position : section_end if section_end >= 0 else None]
    section_lines = section.splitlines()
    table_start = next(
        (index for index, line in enumerate(section_lines) if line.startswith("|")),
        None,
    )
    if table_start is None:
        return [f"{relative}: {label} decision table must contain {expected_rows} rows"]
    table_lines: list[str] = []
    for line in section_lines[table_start:]:
        if not line.startswith("|"):
            break
        table_lines.append(line)
    all_table_lines = [line for line in section_lines if line.startswith("|")]
    if len(table_lines) != expected_rows + 2:
        if len(all_table_lines) >= expected_rows + 2:
            return [f"{relative}: {label} table must be one uninterrupted Markdown table"]
        return [f"{relative}: {label} decision table must contain {expected_rows} rows"]
    header = table_lines[0]
    if require_pass_stop and ("PASS" not in header or "STOP" not in header):
        return [f"{relative}: {label} decision table must expose PASS and STOP columns"]
    numbered_rows: list[tuple[int, list[str]]] = []
    for line in table_lines[2:]:
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        match = re.match(r"^(\d+)(?:[.\s]|$)", cells[0]) if cells else None
        if match:
            numbered_rows.append((int(match.group(1)), cells))
    if [number for number, _ in numbered_rows] != list(range(1, expected_rows + 1)):
        return [f"{relative}: {label} decision rows must be ordered 1-{expected_rows}"]
    errors: list[str] = []
    for number, cells in numbered_rows:
        if len(cells) != 5:
            errors.append(f"{relative}: {label} row {number} must contain 5 columns")
            continue
        if any(not cell for cell in cells):
            errors.append(f"{relative}: {label} row {number} has an empty decision field")
            continue
        if require_copy_request:
            has_curly_request = "“" in cells[1] and "”" in cells[1]
            has_japanese_request = "「" in cells[1] and "」" in cells[1]
            if not (has_curly_request or has_japanese_request):
                errors.append(f"{relative}: {label} row {number} lacks a copy-ready request")
    return errors


def _calibration_completion_checklist_errors(  # pragma: no cover
    text: str, *, relative: str
) -> list[str]:
    """Require a fillable ten-stage checklist distinct from calibration explanation."""
    if text.count(CALIBRATION_COMPLETION_TABLE_MARKER) != 1:
        return [f"{relative}: missing complete calibration checklist"]
    marker_position = text.find(CALIBRATION_COMPLETION_TABLE_MARKER)
    section_end = text.find("\n## ", marker_position)
    section_lines = text[marker_position : section_end if section_end >= 0 else None].splitlines()
    table_start = next(
        (index for index, line in enumerate(section_lines) if line.startswith("|")),
        None,
    )
    if table_start is None:
        return [f"{relative}: complete calibration checklist must contain 10 rows"]
    table_lines: list[str] = []
    for line in section_lines[table_start:]:
        if not line.startswith("|"):
            break
        table_lines.append(line)
    if len(table_lines) != len(CALIBRATION_STAGES) + 2:
        return [f"{relative}: complete calibration checklist must contain 10 rows"]

    rows: list[list[str]] = [
        [cell.strip() for cell in line.strip().strip("|").split("|")] for line in table_lines[2:]
    ]
    found_stage_ids: list[str] = []
    errors: list[str] = []
    for number, cells in enumerate(rows, start=1):
        match = re.match(r"^(\d+)\.\s+([a-z-]+)\b", cells[0]) if cells else None
        if match:
            found_stage_ids.append(match.group(2))
        if len(cells) != 7 or any(not cell for cell in cells):
            errors.append(
                f"{relative}: calibration checklist row {number} must contain all 7 fields"
            )
            continue
        if "[ ]" not in cells[1] and "[x]" not in cells[1].lower():
            errors.append(f"{relative}: calibration checklist row {number} lacks completion state")
        if "Candidate" not in cells[4]:
            errors.append(f"{relative}: calibration checklist row {number} lacks Candidate change")
        if "Owner" not in cells[5] or "Reviewer" not in cells[5]:
            errors.append(f"{relative}: calibration checklist row {number} lacks Owner/Reviewer")
        if "PASS" not in cells[6] or "STOP" not in cells[6]:
            errors.append(
                f"{relative}: calibration checklist row {number} lacks PASS/STOP decision"
            )
    if found_stage_ids != list(CALIBRATION_STAGES):
        errors.append(
            f"{relative}: calibration checklist stage IDs must match the ten-stage "
            "calibration order"
        )
    return errors


def _executable_fence_bodies(text: str) -> list[str]:  # pragma: no cover
    return re.findall(
        r"^```(?:sh|bash|shell|console|make|zsh)\s*$\n(.*?)^```\s*$",
        text,
        flags=re.MULTILINE | re.DOTALL,
    )


def _installation_visible_copy_errors(  # pragma: no cover
    text: str, *, language: str, relative: str
) -> list[str]:
    """Require reader-visible version-neutral and proofreading guidance."""
    errors: list[str] = []
    if INSTALLATION_VERSION_NEUTRAL_TEXT[language] not in text:
        errors.append(f"{relative}: missing reader-visible version-neutral rule")
    if INSTALLATION_PROOFREADING_HEADING[language] not in text:
        errors.append(f"{relative}: missing reader-visible installation proofreading heading")
    return errors


def _calibration_session_evidence_errors(  # pragma: no cover
    text: str, *, language: str, relative: str
) -> list[str]:
    """Require the complete trilingual Session and Work Item evidence boundary."""
    errors: list[str] = []
    heading = CALIBRATION_COMPLETION_HEADING[language]
    start = text.find(heading)
    section_end = text.find("\n### ", start + len(heading)) if start >= 0 else -1
    section = text[start : section_end if section_end >= 0 else None] if start >= 0 else ""
    section_without_comments = re.sub(r"<!--.*?-->", "", section, flags=re.DOTALL)
    visible_section = re.sub(r"```.*?```", "", section_without_comments, flags=re.DOTALL)
    boundary_position = section.find(CALIBRATION_SESSION_EVIDENCE_BOUNDARY)
    prompt_start = section.find("```text\n", boundary_position) if boundary_position >= 0 else -1
    prompt_end = section.find("\n```", prompt_start + len("```text\n")) if prompt_start >= 0 else -1
    prompt = (
        section[prompt_start + len("```text\n") : prompt_end]
        if prompt_start >= 0 and prompt_end >= 0
        else ""
    )

    if CALIBRATION_SESSION_EVIDENCE_BOUNDARY not in section:
        errors.append(f"{relative}: missing complete Session evidence boundary marker")
    if CALIBRATION_SESSION_COMPLETE_CLAIM[language] not in visible_section:
        errors.append(f"{relative}: Session persistence claim omits complete checklistEvidence")
    if any(
        pattern.search(visible_section) for pattern in CALIBRATION_NARROW_SESSION_PATTERNS[language]
    ):
        errors.append(f"{relative}: Session persistence claim omits complete checklistEvidence")
    if CALIBRATION_WORK_ITEM_EVIDENCE_BOUNDARY[language] not in visible_section:
        errors.append(f"{relative}: missing Work Item governance and external-evidence boundary")
    if CALIBRATION_REVIEWER_LABEL_LIMITATION[language] not in visible_section:
        errors.append(f"{relative}: missing reviewer/owner label limitation")
    if CALIBRATION_COMMAND_STORAGE_BOUNDARY[language] not in prompt:
        errors.append(f"{relative}: missing answer/checklistEvidence command storage boundary")
    return errors


def _beginner_installation_route_errors(root: Path) -> list[str]:
    """Validate the thin beginner route and its separated advanced routes."""
    errors: list[str] = []
    beginner_routes = {
        "en": {
            "handoff": "After installation, start a separate project-calibration Work Item.",
            "internal": ("Candidate", "phase record", "Session schema"),
        },
        "zh-CN": {
            "handoff": "安装完成后，开始独立的工程校准 Work Item。",
            "internal": ("Candidate", "phase record", "Session schema"),
        },
        "ja": {
            "handoff": "インストール後は、独立したプロジェクト校正 Work Item を開始します。",
            "internal": ("Candidate", "phase record", "Session schema"),
        },
    }
    route_files = (
        "docs/getting-started/installation-security{suffix}.md",
        "docs/getting-started/calibration{suffix}.md",
        "docs/troubleshooting/installation{suffix}.md",
        "docs/reference/calibration-session-model{suffix}.md",
    )
    readmes = {
        "en": root / "README.md",
        "zh-CN": root / "README.zh-CN.md",
        "ja": root / "README.ja.md",
    }
    for language, suffix in LANGUAGE_SUFFIXES.items():
        installation_relative = f"docs/getting-started/installation{suffix}.md"
        installation = root / installation_relative
        if not installation.is_file():
            errors.append(
                f"{installation_relative}: required beginner installation guide is missing"
            )
        else:
            text = installation.read_text(encoding="utf-8")
            route = beginner_routes[language]
            handoff = cast(str, route["handoff"])
            internal_terms = cast(tuple[str, ...], route["internal"])
            if handoff not in text:
                errors.append(f"{installation_relative}: missing post-install Work Item handoff")
            if any(term in text for term in internal_terms):
                errors.append(
                    f"{installation_relative}: internal calibration mechanics belong in the reference route"
                )
            if len(text.splitlines()) > 260:
                errors.append(f"{installation_relative}: beginner page exceeds 260 lines")
            for route_template in route_files:
                route_relative = route_template.format(suffix=suffix)
                if not (root / route_relative).is_file():
                    errors.append(
                        f"{route_relative}: required separated installation route is missing"
                    )
                elif Path(route_relative).name not in text:
                    errors.append(f"{installation_relative}: missing route link: {route_relative}")
            if "Unknown" not in text:
                errors.append(f"{installation_relative}: missing Unknown stop boundary")
            if "commit" not in text or "push" not in text or "pull request" not in text.lower():
                errors.append(f"{installation_relative}: missing separated authority boundary")
            if (
                "examples/ios" not in text
                or "examples/android" not in text
                or "examples/java" not in text
            ):
                errors.append(f"{installation_relative}: missing platform example route")

        for platform in INSTALLATION_PLATFORMS:
            relative = f"docs/getting-started/examples/{platform}{suffix}.md"
            platform_page = root / relative
            if not platform_page.is_file():
                errors.append(f"{relative}: required platform installation example is missing")
                continue
            platform_text = platform_page.read_text(encoding="utf-8")
            if PLATFORM_ENTRY_MARKER not in platform_text:
                errors.append(f"{relative}: missing Work Item-first platform entry")
            if PLATFORM_NEXT_MARKER not in platform_text:
                errors.append(f"{relative}: missing calibration and recovery route")
            if PLATFORM_EVIDENCE_BOUNDARY not in platform_text:
                errors.append(f"{relative}: missing platform evidence boundary")
            if any(marker in platform_text for marker in LEGACY_PLATFORM_MARKERS):
                errors.append(f"{relative}: contains legacy seven-stage platform flow")
            calibration_link = (Path("..") / f"calibration{suffix}.md").as_posix()
            troubleshooting_link = (
                Path("..") / Path("..") / "troubleshooting" / f"installation{suffix}.md"
            ).as_posix()
            if calibration_link not in platform_text or troubleshooting_link not in platform_text:
                errors.append(f"{relative}: missing same-language calibration or recovery link")

        readme_text = readmes[language].read_text(encoding="utf-8")
        if installation_relative not in readme_text:
            errors.append(
                f"{readmes[language].name}: missing same-language beginner installation entry: {installation_relative}"
            )
    return errors


def beginner_installation_errors(root: Path) -> list[str]:
    """Require complete novice-safe trilingual installation and platform routes."""
    return _beginner_installation_route_errors(root)

    r"""Historical checker retained below for archive-compatible source context.
            novice_stages = _marker_values(text, "novice-stage")
            for stage in BEGINNER_INSTALLATION_STAGES:
                if stage not in novice_stages:
                    errors.append(
                        f"{installation_relative}: missing novice installation stage: {stage}"
                    )
            calibration_stages = _marker_values(text, "calibration-stage")
            for stage in CALIBRATION_STAGES:
                if stage not in calibration_stages:
                    errors.append(f"{installation_relative}: missing calibration stage: {stage}")
            prompt_boundaries = _marker_values(text, "prompt-safety")
            for boundary in PROMPT_SAFETY_BOUNDARIES:
                if boundary not in prompt_boundaries:
                    errors.append(
                        f"{installation_relative}: missing prompt safety boundary: {boundary}"
                    )
            novice_positions = [
                text.find(f"<!-- novice-stage: {stage} -->")
                for stage in BEGINNER_INSTALLATION_STAGES
            ]
            if all(position >= 0 for position in novice_positions) and novice_positions != sorted(
                novice_positions
            ):
                errors.append(
                    f"{installation_relative}: novice installation stages are out of order"
                )
            calibration_positions = [
                text.find(f"<!-- calibration-stage: {stage} -->") for stage in CALIBRATION_STAGES
            ]
            if all(
                position >= 0 for position in calibration_positions
            ) and calibration_positions != sorted(calibration_positions):
                errors.append(f"{installation_relative}: calibration stages are out of order")
            if PROMPT_AUTHORITY_TEXT[language] not in text:
                errors.append(
                    f"{installation_relative}: copy-ready discovery prompt lost its "
                    "no-write/no-downstream-authority sentence"
                )
            if RELEASE_METADATA_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing dynamic tag-pinned release metadata boundary"
                )
            errors.extend(
                _installation_visible_copy_errors(
                    text,
                    language=language,
                    relative=installation_relative,
                )
            )
            if RELEASE_FALLBACK_APPROVAL not in text:
                errors.append(
                    f"{installation_relative}: missing bounded older-release fallback approval"
                )
            if MAKE_ENTRYPOINT_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing installed Make entrypoint boundary"
                )
            if MAKE_COMPOSITE_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing composite Make entrypoint propagation boundary"
                )
            if TAG_PINNED_RELEASE_METADATA_TEMPLATE not in text:
                errors.append(
                    f"{installation_relative}: missing resolved-tag release metadata template"
                )
            if MOVING_MAIN_RELEASE_METADATA in text:
                errors.append(
                    f"{installation_relative}: moving main release metadata must not "
                    "verify a tagged asset"
                )
            release_boundary_position = text.find(RELEASE_METADATA_BOUNDARY)
            release_section_end = text.find("\n## ", release_boundary_position)
            release_section = text[
                release_boundary_position : (
                    release_section_end if release_section_end >= 0 else None
                )
            ]
            if re.search(r"\bv\d+\.\d+\.\d+\b", release_section):
                errors.append(
                    f"{installation_relative}: installation discovery must not "
                    "hardcode a release version"
                )
            if CALIBRATION_ANSWER_TYPES_MARKER not in text:
                errors.append(
                    f"{installation_relative}: missing exact Calibration Session "
                    "answer-type mapping"
                )
            if CALIBRATION_YES_NO_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing yes_no type and Y/N value boundary"
                )
            if CALIBRATION_RUNTIME_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing current Calibration Session runtime boundary"
                )
            if CALIBRATION_OBSOLETE_RUNTIME_BOUNDARY in text:
                errors.append(
                    f"{installation_relative}: obsolete non-enforced Calibration runtime "
                    "boundary must be removed"
                )
            if INSTALLATION_PLAN_RELEASE_BINDING not in text:
                errors.append(
                    f"{installation_relative}: installation plan must bind verified "
                    "release evidence to the installer entrypoint"
                )
            if CALIBRATION_CONFIRMATION_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing confirmation phase and actor-identity boundary"
                )
            if CALIBRATION_CI_GAP_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing CI-gap plan, approval, implementation, "
                    "and verification path"
                )
            if CALIBRATION_SESSION_PERSISTENCE_BOUNDARY not in text:
                errors.append(
                    f"{installation_relative}: missing Session checklist-persistence boundary"
                )
            errors.extend(
                _calibration_session_evidence_errors(
                    text,
                    language=language,
                    relative=installation_relative,
                )
            )
            if CALIBRATION_ACTIVATION_ATOMICITY_BOUNDARY not in text:
                errors.append(f"{installation_relative}: missing Active/Session atomicity boundary")
            for runtime_term in CALIBRATION_TRANSACTION_RUNTIME_TERMS:
                if runtime_term not in text:
                    errors.append(
                        f"{installation_relative}: missing Calibration transaction "
                        f"runtime term: {runtime_term}"
                    )
            activation_stages = _marker_values(text, "calibration-activation")
            for activation_stage in CALIBRATION_ACTIVATION_STAGES:
                if activation_stage not in activation_stages:
                    errors.append(
                        f"{installation_relative}: missing separate calibration "
                        f"activation stage: {activation_stage}"
                    )
            if LIFECYCLE_ORDER_MARKER not in text:
                errors.append(
                    f"{installation_relative}: adoption must close before configuration starts"
                )
            lifecycle_approvals = _marker_values(text, "lifecycle-approval")
            for approval in LIFECYCLE_APPROVAL_STAGES:
                if approval not in lifecycle_approvals:
                    errors.append(
                        f"{installation_relative}: missing separate lifecycle approval: {approval}"
                    )
            errors.extend(
                _step_table_errors(
                    text,
                    marker=SCAFFOLD_REVIEW_TABLE_MARKER,
                    expected_rows=7,
                    relative=installation_relative,
                    label="scaffold review",
                )
            )
            errors.extend(
                _calibration_completion_checklist_errors(
                    text,
                    relative=installation_relative,
                )
            )
            errors.extend(
                _step_table_errors(
                    text,
                    marker=INSTALLATION_PROOFREADING_CHECKLIST_MARKER,
                    expected_rows=8,
                    relative=installation_relative,
                    label="installation proofreading",
                    require_copy_request=False,
                )
            )
            errors.extend(
                _step_table_errors(
                    text,
                    marker=CALIBRATION_REVIEW_TABLE_MARKER,
                    expected_rows=10,
                    relative=installation_relative,
                    label="calibration review",
                )
            )
            executable_fences = len(
                re.findall(
                    r"^```(?:sh|bash|shell|console|make|zsh)\s*$",
                    text,
                    flags=re.MULTILINE,
                )
            )
            if executable_fences and text.count(COMMAND_GUIDE_MARKER) != executable_fences:
                errors.append(
                    f"{installation_relative}: retained commands require purpose, success, "
                    "and failure guidance"
                )

        readme_text = readmes[language].read_text(encoding="utf-8")
        if any(
            "make ai-finish" in body and "git add ." in body
            for body in _executable_fence_bodies(readme_text)
        ):
            errors.append(
                f"{readmes[language].name}: finish and commit must use separate command blocks"
            )
        if installation_relative not in readme_text:
            errors.append(
                f"{readmes[language].name}: missing same-language beginner installation entry: "
                f"{installation_relative}"
            )
        for layer in linked_layers[language]:
            layer_relative = layer.relative_to(root).as_posix()
            local_installation_link = f"installation{suffix}.md"
            if local_installation_link not in layer.read_text(encoding="utf-8"):
                errors.append(
                    f"{layer_relative}: missing same-language beginner installation entry: "
                    f"{installation_relative}"
                )

        for platform in INSTALLATION_PLATFORMS:
            relative = f"docs/getting-started/examples/{platform}{suffix}.md"
            path = root / relative
            if not path.is_file():
                errors.append(f"{relative}: required platform installation example is missing")
                continue
            text = path.read_text(encoding="utf-8")
            found_stages = _marker_values(text, "platform-stage")
            for stage in PLATFORM_INSTALLATION_STAGES:
                if stage not in found_stages:
                    errors.append(f"{relative}: missing platform installation stage: {stage}")
            if PLATFORM_EVIDENCE_BOUNDARY not in text:
                errors.append(f"{relative}: missing platform evidence boundary")
            if PLATFORM_PROMPT_MARKER not in text:
                errors.append(f"{relative}: missing copy-ready platform prompt")
            if text.count(PLATFORM_STAGE5_PROPOSAL_MARKER) != 1:
                errors.append(f"{relative}: platform Stage 5 must remain proposal-only")
            elif text.find(PLATFORM_STAGE5_PROPOSAL_MARKER) > text.find(PLATFORM_STEP_TABLE_MARKER):
                errors.append(
                    f"{relative}: platform Stage 5 proposal-only marker must remain "
                    "outside the table"
                )
            errors.extend(
                _step_table_errors(
                    text,
                    marker=PLATFORM_STEP_TABLE_MARKER,
                    expected_rows=7,
                    relative=relative,
                    label="platform step",
                )
            )
            errors.extend(
                _step_table_errors(
                    text,
                    marker=PLATFORM_FILLED_EXAMPLE_MARKER,
                    expected_rows=7,
                    relative=relative,
                    label="platform filled example",
                    require_pass_stop=False,
                    require_copy_request=False,
                )
            )
            installation_link = f"examples/{platform}{suffix}.md"
            if installation.is_file() and installation_link not in installation_text:
                errors.append(
                    f"{installation_relative}: missing same-language platform entry: {relative}"
                )
    return errors


    """


def historical_context_errors(root: Path) -> list[str]:
    """Validate current/historical context without mutating immutable archives."""
    registry_path = root / "docs" / "reference" / "documentation-context-registry.json"
    if not registry_path.is_file():
        return ["docs/reference/documentation-context-registry.json: missing context registry"]
    try:
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return ["docs/reference/documentation-context-registry.json: invalid JSON"]
    errors: list[str] = []
    if registry.get("schemaVersion") != 1:
        errors.append("docs/reference/documentation-context-registry.json: schemaVersion must be 1")
    entries = registry.get("entries")
    if not isinstance(entries, list):
        return [
            *errors,
            "docs/reference/documentation-context-registry.json: entries must be a list",
        ]
    by_path: dict[str, dict[str, object]] = {}
    archive_pattern_found = False
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"documentation context entry {index} must be an object")
            continue
        path = entry.get("path")
        context = entry.get("context")
        mutable = entry.get("mutable")
        if not isinstance(path, str) or not path:
            errors.append(f"documentation context entry {index} requires path")
            continue
        if path in by_path:
            errors.append(f"documentation context path is duplicated: {path}")
        by_path[path] = entry
        if context not in {"current_instruction", "historical_record", "implementation_record"}:
            errors.append(f"documentation context path has invalid context: {path}")
        if not isinstance(mutable, bool):
            errors.append(f"documentation context path requires boolean mutable: {path}")
        if path == ".ai/work-items/archive/**":
            archive_pattern_found = context == "historical_record" and mutable is False
            continue
        candidate = root / path
        if not candidate.is_file():
            errors.append(f"documentation context path does not exist: {path}")
            continue
        if (
            context != "current_instruction"
            and mutable is True
            and HISTORICAL_MARKER not in candidate.read_text(encoding="utf-8")
        ):
            errors.append(f"{path}: missing historical context marker")

    governed = [
        *sorted((root / "docs" / "superpowers" / "plans").glob("*.md")),
        *sorted((root / "docs" / "superpowers" / "specs").glob("*.md")),
    ]
    for path in governed:
        relative = path.relative_to(root).as_posix()
        if relative not in by_path:
            errors.append(f"{relative}: missing from documentation context registry")
    if not archive_pattern_found:
        errors.append(
            ".ai/work-items/archive/**: immutable historical archive classification is missing"
        )
    return errors


def documentation_authority_errors(root: Path) -> list[str]:
    """Bind the new agent routing registry to real document front matter."""
    relative = "docs/reference/documentation-authority-registry.json"
    registry_path = root / relative
    if not registry_path.is_file():
        return [f"{relative}: missing authority registry"]
    try:
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return [f"{relative}: invalid JSON"]
    errors = [f"{relative}: {error}" for error in validate_registry(registry)]
    errors.extend(
        f"{relative}: {error}"
        for error in [*validate_topics(registry, root), *validate_journeys(registry, root)]
    )
    documents = registry.get("documents") if isinstance(registry, dict) else None
    if not isinstance(documents, list):
        return errors
    for item in documents:
        if not isinstance(item, dict) or not isinstance(item.get("path"), str):
            continue
        path = root / item["path"]
        if not path.is_file():
            errors.append(f"{item['path']}: authority-registry document is missing")
            continue
        values, _ = _front_matter_values(path)
        for field in ("authority", "instructional", "status", "supersededBy"):
            expected = item.get(field)
            actual = values.get(field)
            normalized_expected = (
                ""
                if expected is None
                else str(expected).lower()
                if isinstance(expected, bool)
                else str(expected)
            )
            if actual != normalized_expected:
                errors.append(
                    f"{item['path']}: front matter {field} does not match authority registry"
                )
    return errors


def japanese_uninstall_errors(root: Path) -> list[str]:
    """Keep recovery details out of the beginner path while preserving a Japanese route."""
    relative = "docs/troubleshooting/installation.ja.md"
    if not (root / relative).is_file():
        return [f"{relative}: Japanese installation recovery route is missing"]
    return []


def check_repository(root: Path) -> list[str]:
    errors = []
    for path in documentation_files(root):
        errors.extend(front_matter_errors(path))
    errors.extend(formal_document_metadata_errors(root))
    errors.extend(documentation_architecture_errors(root))
    errors.extend(stack_errors(root))
    errors.extend(installation_command_errors(root))
    errors.extend(japanese_style_errors(root))
    errors.extend(capability_claim_errors(root))
    errors.extend(documentation_fact_errors(root))
    errors.extend(multilingual_layer_errors(root))
    errors.extend(command_evidence_errors(root))
    errors.extend(beginner_installation_errors(root))
    errors.extend(japanese_uninstall_errors(root))
    errors.extend(historical_context_errors(root))
    errors.extend(documentation_authority_errors(root))
    return errors


def main() -> int:
    errors = check_repository(ROOT)
    if errors:
        print("documentation metadata check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print("documentation metadata check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
