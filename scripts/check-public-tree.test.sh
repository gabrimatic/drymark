#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKER="$SCRIPT_DIR/check-public-tree.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/drymark-public-tree-test.XXXXXX")"

cleanup() {
  find "$TEST_ROOT" -type f -delete 2>/dev/null || true
  find "$TEST_ROOT" -depth -type d -exec rmdir {} \; 2>/dev/null || true
}
trap cleanup EXIT

fail() {
  echo "public-tree-test: $*" >&2
  exit 1
}

new_repo() {
  local name="$1"
  local path="$TEST_ROOT/$name"
  mkdir -p "$path"
  git -C "$path" init -q
  git -C "$path" config user.name Test
  git -C "$path" config user.email test@example.invalid
  printf '%s\n' '# Public project' > "$path/README.md"
  git -C "$path" add README.md
  git -C "$path" commit -qm initial
  printf '%s\n' "$path"
}

expect_rejected() {
  local repo="$1"
  set +e
  output="$("$CHECKER" "$repo" 2>&1)"
  status=$?
  set -e
  [[ $status -ne 0 ]] || fail "expected tracked private material to be rejected"
  [[ "$output" != *"secret fixture contents"* ]] || fail "checker printed file contents"
}

[[ -x "$CHECKER" ]] || fail "public tree checker must exist and be executable"

clean_repo="$(new_repo clean)"
"$CHECKER" "$clean_repo" >/dev/null || fail "clean repository was rejected"

path_repo="$(new_repo path)"
printf '/%s/%s/%s\n' Users private-person project > "$path_repo/README.md"
git -C "$path_repo" add README.md
expect_rejected "$path_repo"

untracked_repo="$(new_repo untracked)"
printf '/%s/%s/%s\n' Users private-person project > "$untracked_repo/local.txt"
"$CHECKER" "$untracked_repo" >/dev/null || fail "untracked files were inspected"

echo "public tree checker rejects tracked home-directory paths without inspecting untracked files"
