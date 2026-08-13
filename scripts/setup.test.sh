#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/drymark-setup-test.XXXXXX")"

cleanup() {
  find "$TEST_ROOT" -type f -delete 2>/dev/null || true
  find "$TEST_ROOT" -depth -type d -exec rmdir {} \; 2>/dev/null || true
}
trap cleanup EXIT

fail() {
  echo "setup-test: $*" >&2
  exit 1
}

[[ -x "$REPO_ROOT/setup.sh" ]] || fail "setup.sh must exist and be executable"
[[ -f "$REPO_ROOT/setup.ps1" ]] || fail "setup.ps1 must exist"

help_output="$(cd "$TEST_ROOT" && "$REPO_ROOT/setup.sh" --help)"
[[ "$help_output" == *"Usage: ./setup.sh"* ]] || fail "help must be independent of the current directory"
[[ "$help_output" == *"--check"* ]] || fail "help must document prerequisite-only checking"
[[ "$help_output" == *"--build-only"* ]] || fail "help must document build-only mode"
[[ "$help_output" == *"--no-launch"* ]] || fail "help must document launch suppression"

set +e
invalid_output="$("$REPO_ROOT/setup.sh" --definitely-invalid 2>&1)"
invalid_status=$?
set -e
[[ $invalid_status -eq 2 ]] || fail "unknown options must exit 2"
[[ "$invalid_output" == *"Unknown option"* ]] || fail "unknown options need a clear diagnostic"

FAKE_BIN="$TEST_ROOT/bin"
mkdir -p "$FAKE_BIN" "$TEST_ROOT/toolchain/bin"

cat > "$FAKE_BIN/uname" <<'EOF'
#!/bin/sh
if [ "${1:-}" = "-s" ]; then printf '%s\n' Darwin; else printf '%s\n' Darwin; fi
EOF
cat > "$FAKE_BIN/node" <<'EOF'
#!/bin/sh
printf '%s\n' v22.0.0
EOF
cat > "$FAKE_BIN/npm" <<'EOF'
#!/bin/sh
printf '%s\n' "npm unexpectedly executed" >> "${DRYMARK_SETUP_TEST_LOG:?}"
exit 99
EOF
cat > "$FAKE_BIN/rustup" <<EOF
#!/bin/sh
if [ "\${1:-}" = "which" ] && [ "\${2:-}" = "cargo" ]; then
  printf '%s\\n' '$TEST_ROOT/toolchain/bin/cargo'
  exit 0
fi
exit 1
EOF
cat > "$TEST_ROOT/toolchain/bin/cargo" <<'EOF'
#!/bin/sh
if [ "$PWD" != "${DRYMARK_SETUP_EXPECTED_ROOT:?}" ]; then
  printf '%s\n' "cargo ran from the wrong directory: $PWD" >&2
  exit 98
fi
printf '%s\n' 'cargo 1.97.1 (test fixture)'
EOF
cat > "$FAKE_BIN/xcode-select" <<'EOF'
#!/bin/sh
if [ "${1:-}" = "-p" ]; then printf '%s\n' /Library/Developer/CommandLineTools; exit 0; fi
exit 1
EOF
chmod +x "$FAKE_BIN"/* "$TEST_ROOT/toolchain/bin/cargo"

TEST_LOG="$TEST_ROOT/commands.log"
: > "$TEST_LOG"
check_output="$(
  cd "$TEST_ROOT"
  DRYMARK_SETUP_TEST_LOG="$TEST_LOG" \
    DRYMARK_SETUP_EXPECTED_ROOT="$REPO_ROOT" \
    PATH="$FAKE_BIN:/usr/bin:/bin" \
    "$REPO_ROOT/setup.sh" --check
)"
[[ ! -s "$TEST_LOG" ]] || fail "--check must not install dependencies or build"
[[ "$check_output" == *"Rust 1.97.1"* ]] || fail "rustup cargo fallback was not used"
[[ "$check_output" == *"Prerequisites ready"* ]] || fail "--check did not report success"

if grep -Eni 'launchagent|launchdaemon|login item|scheduled task|autostart|cron' \
  "$REPO_ROOT/setup.sh" "$REPO_ROOT/setup.ps1" >/dev/null; then
  fail "setup scripts must not create persistent startup mechanisms"
fi

if grep -En 'find[[:space:]]+"\$backup".*-delete' "$REPO_ROOT/setup.sh" >/dev/null; then
  fail "macOS replacement must keep the previous app recoverable"
fi

grep -F 'Previous DryMark moved to Trash' "$REPO_ROOT/setup.sh" >/dev/null \
  || fail "macOS replacement must explain where the recoverable previous app went"

INSTALL_FIXTURE="$TEST_ROOT/install-fixture"
FIXTURE_REPO="$INSTALL_FIXTURE/repo"
INSTALL_HOME="$INSTALL_FIXTURE/home"
INSTALL_ROOT="$INSTALL_HOME/Applications"
INSTALL_BIN="$INSTALL_FIXTURE/bin"
SOURCE_APP="$FIXTURE_REPO/target/release/bundle/macos/DryMark.app"
DESTINATION_APP="$INSTALL_ROOT/DryMark.app"
TRANSACTION="$INSTALL_ROOT/.DryMark.setup-transaction"

mkdir -p "$FIXTURE_REPO" "$INSTALL_HOME" "$INSTALL_BIN" "$SOURCE_APP"
cp "$REPO_ROOT/setup.sh" "$FIXTURE_REPO/setup.sh"
chmod +x "$FIXTURE_REPO/setup.sh"
printf '%s\n' NEW > "$SOURCE_APP/marker"

cat > "$INSTALL_BIN/uname" <<'EOF'
#!/bin/sh
printf '%s\n' Darwin
EOF
cat > "$INSTALL_BIN/node" <<'EOF'
#!/bin/sh
printf '%s\n' v22.0.0
EOF
cat > "$INSTALL_BIN/npm" <<'EOF'
#!/bin/sh
case "${1:-}" in
  ci|run) exit 0 ;;
esac
exit 99
EOF
cat > "$INSTALL_BIN/cargo" <<'EOF'
#!/bin/sh
printf '%s\n' 'cargo 1.97.1 (test fixture)'
EOF
cat > "$INSTALL_BIN/xcode-select" <<'EOF'
#!/bin/sh
exit 0
EOF
cat > "$INSTALL_BIN/xattr" <<'EOF'
#!/bin/sh
exit 0
EOF
cat > "$INSTALL_BIN/pgrep" <<'EOF'
#!/bin/sh
exit 1
EOF
cat > "$INSTALL_BIN/ditto" <<'EOF'
#!/bin/sh
cp -R "$1" "$2"
EOF
cat > "$INSTALL_BIN/codesign" <<'EOF'
#!/bin/sh
target=''
for argument in "$@"; do target="$argument"; done
if [ -n "${DRYMARK_CODESIGN_FAIL_TARGET:-}" ] \
  && [ "$target" = "$DRYMARK_CODESIGN_FAIL_TARGET" ]; then
  exit 42
fi
exit 0
EOF
cat > "$INSTALL_BIN/open" <<'EOF'
#!/bin/sh
exit 97
EOF
cat > "$INSTALL_BIN/mv" <<'EOF'
#!/bin/sh
/bin/mv "$@"
status=$?
if [ $status -eq 0 ] \
  && [ "${DRYMARK_SIGNAL_AFTER_INSTALL_MOVE:-}" = 1 ] \
  && [ "${1##*/}" = Staging.app ] \
  && [ "${2##*/}" = DryMark.app ]; then
  kill -TERM "$PPID"
fi
exit $status
EOF
chmod +x "$INSTALL_BIN"/*

reset_installed_app() {
  mkdir -p "$DESTINATION_APP"
  printf '%s\n' OLD > "$DESTINATION_APP/marker"
}

run_failed_install() {
  local failed_target="$1"
  set +e
  install_output="$({
    cd "$TEST_ROOT"
    HOME="$INSTALL_HOME" \
      DRYMARK_INSTALL_DIR="$INSTALL_ROOT" \
      DRYMARK_CODESIGN_FAIL_TARGET="$failed_target" \
      PATH="$INSTALL_BIN:/usr/bin:/bin" \
      "$FIXTURE_REPO/setup.sh" --no-launch
  } 2>&1)"
  install_status=$?
  set -e
  [[ $install_status -ne 0 ]] || fail "injected signature failure unexpectedly succeeded"
}

reset_installed_app
run_failed_install "$DESTINATION_APP"
[[ "$(<"$DESTINATION_APP/marker")" == OLD ]] \
  || fail "failed final verification did not restore the previous app"
[[ ! -e "$TRANSACTION" ]] \
  || fail "failed final verification left an active transaction"
[[ "$install_output" == *"previous DryMark app was restored"* ]] \
  || fail "rollback did not report restoration"

run_failed_install "$TRANSACTION/Staging.app"
[[ "$(<"$DESTINATION_APP/marker")" == OLD ]] \
  || fail "failed staging verification changed the existing app"
[[ ! -e "$TRANSACTION" ]] \
  || fail "failed staging verification left an active transaction"

mkdir -p "$TRANSACTION"
run_failed_install "$TEST_ROOT/never-matched"
[[ "$(<"$DESTINATION_APP/marker")" == OLD ]] \
  || fail "unfinished-transaction detection changed the existing app"
[[ "$install_output" == *"unfinished DryMark setup"* ]] \
  || fail "unfinished transactions need a clear recovery diagnostic"
rmdir "$TRANSACTION"

reset_installed_app
set +e
signal_output="$({
  cd "$TEST_ROOT"
  HOME="$INSTALL_HOME" \
    DRYMARK_INSTALL_DIR="$INSTALL_ROOT" \
    DRYMARK_SIGNAL_AFTER_INSTALL_MOVE=1 \
    PATH="$INSTALL_BIN:/usr/bin:/bin" \
    "$FIXTURE_REPO/setup.sh" --no-launch
} 2>&1)"
signal_status=$?
set -e
[[ $signal_status -eq 143 ]] || fail "signal injection must preserve the signal-derived status"
[[ "$(<"$DESTINATION_APP/marker")" == OLD ]] \
  || fail "a signal after the install rename did not restore the previous app"
[[ ! -e "$TRANSACTION" ]] \
  || fail "signal rollback left an active transaction"
[[ "$signal_output" == *"previous DryMark app was restored"* ]] \
  || fail "signal rollback did not report restoration"

echo "setup scripts pass path, fallback, rollback injection, transaction recovery, and no-persistence checks"
