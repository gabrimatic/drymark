#!/usr/bin/env bash

set -euo pipefail

project_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
app_path="$project_dir/target/release/bundle/macos/DryMark.app"
process_name="drymark-desktop"

clear_local_finder_metadata() {
  /usr/bin/xattr -d com.apple.FinderInfo "$app_path" >/dev/null 2>&1 || true
  /usr/bin/xattr -d com.apple.ResourceFork "$app_path" >/dev/null 2>&1 || true
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf '%s\n' "DryMark packaged runtime smoke requires macOS." >&2
  exit 2
fi

if pgrep -x "$process_name" >/dev/null; then
  printf '%s\n' "DryMark is already running; nothing was changed." >&2
  exit 2
fi

cd "$project_dir"
if ! command -v cargo >/dev/null; then
  if ! command -v rustup >/dev/null; then
    printf '%s\n' "Rust 1.97.1 is required to build DryMark." >&2
    exit 2
  fi
  cargo_path="$(rustup which --toolchain 1.97.1 cargo)"
  PATH="$(dirname -- "$cargo_path"):$PATH"
  export PATH
fi
npm run tauri -- build --bundles app -- --locked
clear_local_finder_metadata
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app_path"

cleanup() {
  if pgrep -x "$process_name" >/dev/null; then
    /usr/bin/osascript -e 'tell application id "info.gabrimatic.drymark" to quit' >/dev/null 2>&1 || true
    for _ in {1..40}; do
      if ! pgrep -x "$process_name" >/dev/null; then
        return
      fi
      sleep 0.1
    done
    /usr/bin/pkill -TERM -x "$process_name" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

/usr/bin/open -na "$app_path"
for _ in {1..100}; do
  if pgrep -x "$process_name" >/dev/null; then
    break
  fi
  sleep 0.1
done

if ! pgrep -x "$process_name" >/dev/null; then
  printf '%s\n' "DryMark package did not start." >&2
  exit 1
fi

/usr/bin/xcrun swift scripts/macos-runtime-smoke.swift
cleanup
trap - EXIT INT TERM

clear_local_finder_metadata
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app_path"

if pgrep -x "$process_name" >/dev/null; then
  printf '%s\n' "DryMark runtime smoke left a process running." >&2
  exit 1
fi
