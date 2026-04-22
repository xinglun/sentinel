#!/usr/bin/env bash
set -euo pipefail

require_contains() {
  local file="$1"
  local pattern="$2"
  if ! grep -Fq -- "$pattern" "$file"; then
    echo "[audit-doc-check] missing pattern in ${file}: ${pattern}" >&2
    exit 1
  fi
}

README_FILE="README.md"
RUNBOOK_FILE="docs/specs/WEEKLY_STATE_REVIEW_RUNBOOK.md"

# Language coverage
require_contains "$README_FILE" "output.language"
require_contains "$README_FILE" "zh-cn"
require_contains "$README_FILE" "en-us"
require_contains "$README_FILE" "ja-jp"
require_contains "$RUNBOOK_FILE" "output.language"
require_contains "$RUNBOOK_FILE" "zh-cn"
require_contains "$RUNBOOK_FILE" "en-us"
require_contains "$RUNBOOK_FILE" "ja-jp"

# Parameter behavior and methodology contract
require_contains "$README_FILE" "--date"
require_contains "$README_FILE" "--days"
require_contains "$README_FILE" "エラー終了"
require_contains "$README_FILE" "週末は自動的に連結"

require_contains "$RUNBOOK_FILE" "--date"
require_contains "$RUNBOOK_FILE" "--days"
require_contains "$RUNBOOK_FILE" "エラー終了"
require_contains "$RUNBOOK_FILE" "週末は自動連結"

echo "[audit-doc-check] ok"
