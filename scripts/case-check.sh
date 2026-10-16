#!/usr/bin/env bash
# macOS disks are case-insensitive: fails when two files, or two modules' import paths
# (`./Drill` for Drill.tsx and drill.ts), differ only by case.
set -euo pipefail
cd "$(dirname "$0")/.."

clashes() { sort -u | awk '{ k = tolower($0); if (k in seen) print seen[k] " and " $0; else seen[k] = $0 }'; }

found=$(
  git ls-files | clashes
  git ls-files | grep -E '\.(tsx?|jsx?|mjs)$' | sed -E 's/\.(tsx?|jsx?|mjs)$//' | clashes
)
if [ -n "$found" ]; then
  echo "Names that differ only by case clash on macOS:" >&2
  echo "$found" >&2
  exit 1
fi
