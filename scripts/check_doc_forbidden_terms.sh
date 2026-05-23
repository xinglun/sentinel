#!/usr/bin/env bash
set -euo pipefail

if [ ! -d docs ]; then
  echo "[doc-forbidden-terms-check] docs directory not found; skipped"
  exit 0
fi

failed=0
while IFS= read -r file; do
  if grep -nE '加仓|减仓|加減仓' "$file"; then
    failed=1
  fi
done < <(find docs -type f -name '*.md' | sort)

if [ "$failed" -ne 0 ]; then
  echo "[doc-forbidden-terms-check] forbidden Chinese trading terms found in Japanese docs" >&2
  exit 1
fi

echo "[doc-forbidden-terms-check] ok"
