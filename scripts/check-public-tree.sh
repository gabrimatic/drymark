#!/usr/bin/env bash

set -euo pipefail

REPOSITORY="${1:-.}"

fail() {
  printf 'public-tree: %s\n' "$1" >&2
  exit 1
}

git -C "$REPOSITORY" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  || fail "not a Git working tree"

absolute_path_pattern='/Use''rs/[^/[:space:]]+|/ho''me/[^/[:space:]]+'

if git -C "$REPOSITORY" grep -IEn \
  -e "$absolute_path_pattern" \
  -- ':!Cargo.lock' ':!fuzz/Cargo.lock' ':!package-lock.json' >/dev/null 2>&1; then
  fail "tracked content contains a local home-directory path"
fi

printf '%s\n' "public tree is clean"
