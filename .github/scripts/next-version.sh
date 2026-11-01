#!/usr/bin/env bash
# Prints the next release version (without the "v" prefix), or nothing when
# HEAD is already released.
#
# The first release is 1.0.0. After that, a minor bump when a commit since the
# last v* tag is a feature (subject "feat:", "feat(scope):" or "feat!:"), a
# patch bump otherwise.
set -euo pipefail

last=$(git describe --tags --abbrev=0 --match 'v[0-9]*.[0-9]*.[0-9]*' 2>/dev/null || true)
if [ -z "$last" ]; then
  echo "1.0.0"
  exit 0
fi

range="$last..HEAD"
if [ "$(git rev-list --count "$range")" -eq 0 ]; then
  exit 0
fi

IFS=. read -r major minor patch <<< "${last#v}"
if git log --format=%s "$range" | grep -qE '^feat(\([^)]*\))?!?:'; then
  echo "$major.$((minor + 1)).0"
else
  echo "$major.$minor.$((patch + 1))"
fi
