#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_ONLY=false
BUILD_ONLY=false
NO_LAUNCH=false
MACOS_TRANSACTION_ACTIVE=false
MACOS_INSTALL_COMMITTED=false
MACOS_HAD_DESTINATION=false
MACOS_TRANSACTION=''
MACOS_DESTINATION=''
MACOS_STAGING=''
MACOS_PREVIOUS=''
MACOS_TRASH_SUFFIX=''

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
DIM='\033[2m'
BOLD='\033[1m'
RESET='\033[0m'

if [[ ! -t 1 || -n "${NO_COLOR:-}" ]]; then
  RED=''
  GREEN=''
  CYAN=''
  DIM=''
  BOLD=''
  RESET=''
fi

usage() {
  cat <<'EOF'
DryMark setup

Usage: ./setup.sh [option]

Options:
  --check       Check prerequisites without installing or building
  --build-only  Install source dependencies and build without installing
  --no-launch   Build and install DryMark without opening it
  -h, --help    Show this help

Run with no option to build, install, and open DryMark.
EOF
}

fail() {
  printf '\n%b✗%b %s\n\n' "$RED" "$RESET" "$1" >&2
  exit "${2:-1}"
}

step() {
  printf '\n%b▶%b %s\n' "$CYAN" "$RESET" "$1"
}

ok() {
  printf '  %b✓%b %s\n' "$GREEN" "$RESET" "$1"
}

info() {
  printf '  %b›%b %s\n' "$DIM" "$RESET" "$1"
}

warn() {
  printf '  %b!%b %s\n' "$RED" "$RESET" "$1" >&2
}

clear_local_finder_metadata() {
  local app="$1"
  xattr -d com.apple.FinderInfo "$app" >/dev/null 2>&1 || true
  xattr -d com.apple.ResourceFork "$app" >/dev/null 2>&1 || true
}

move_macos_item_to_trash() {
  local source="$1"
  local label="$2"
  local target="$HOME/.Trash/DryMark.$label.$MACOS_TRASH_SUFFIX.app"

  [[ -e "$source" ]] || return 0
  mkdir -p "$HOME/.Trash" || return 1
  [[ ! -e "$target" ]] || return 1
  mv "$source" "$target"
}

rollback_macos_transaction() {
  local rejected="$MACOS_TRANSACTION/Rejected.app"

  [[ "$MACOS_TRANSACTION_ACTIVE" == true ]] || return 0

  if [[ -e "$MACOS_PREVIOUS" && -e "$MACOS_DESTINATION" ]] \
    || [[ "$MACOS_HAD_DESTINATION" != true \
      && ! -e "$MACOS_STAGING" \
      && -e "$MACOS_DESTINATION" ]]; then
    if [[ ! -e "$rejected" ]] && mv "$MACOS_DESTINATION" "$rejected"; then
      :
    else
      warn "The unsuccessful replacement remains at $MACOS_DESTINATION."
    fi
  fi

  if [[ -e "$MACOS_PREVIOUS" && ! -e "$MACOS_DESTINATION" ]]; then
    if mv "$MACOS_PREVIOUS" "$MACOS_DESTINATION"; then
      warn "The previous DryMark app was restored."
    else
      warn "Restore the previous app manually from $MACOS_PREVIOUS."
    fi
  fi

  if ! move_macos_item_to_trash "$MACOS_STAGING" incomplete; then
    warn "Incomplete staging remains at $MACOS_STAGING."
  fi
  if ! move_macos_item_to_trash "$rejected" rejected; then
    warn "The rejected replacement remains at $rejected."
  fi

  rmdir "$MACOS_TRANSACTION" >/dev/null 2>&1 || true
}

finish_macos_transaction_on_exit() {
  local status=$?
  trap - EXIT HUP INT TERM
  if [[ "$MACOS_TRANSACTION_ACTIVE" == true && "$MACOS_INSTALL_COMMITTED" != true ]]; then
    rollback_macos_transaction
  fi
  exit "$status"
}

for argument in "$@"; do
  case "$argument" in
    --check) CHECK_ONLY=true ;;
    --build-only) BUILD_ONLY=true ;;
    --no-launch) NO_LAUNCH=true ;;
    -h|--help)
      usage
      exit 0
      ;;
    *) fail "Unknown option: $argument" 2 ;;
  esac
done

selected_modes=0
for mode in "$CHECK_ONLY" "$BUILD_ONLY" "$NO_LAUNCH"; do
  [[ "$mode" == true ]] && selected_modes=$((selected_modes + 1))
done
[[ $selected_modes -le 1 ]] || fail "Choose only one of --check, --build-only, or --no-launch." 2

cd "$SCRIPT_DIR"

resolve_cargo() {
  local cargo_path
  if command -v cargo >/dev/null 2>&1; then
    command -v cargo
    return 0
  fi

  if command -v rustup >/dev/null 2>&1; then
    cargo_path="$(rustup which cargo 2>/dev/null || true)"
    if [[ -n "$cargo_path" && -x "$cargo_path" ]]; then
      printf '%s\n' "$cargo_path"
      return 0
    fi
  fi

  return 1
}

node_major_version() {
  local version
  version="$(node --version 2>/dev/null || true)"
  version="${version#v}"
  printf '%s\n' "${version%%.*}"
}

check_linux_dependencies() {
  local missing=()
  local package

  command -v pkg-config >/dev/null 2>&1 || fail \
    "pkg-config is required. Install the Tauri prerequisites for your Linux distribution."

  for package in gtk+-3.0 webkit2gtk-4.1 ayatana-appindicator3-0.1; do
    pkg-config --exists "$package" || missing+=("$package")
  done

  if [[ ${#missing[@]} -gt 0 ]]; then
    fail "Missing Linux desktop libraries: ${missing[*]}. See docs/platforms.md for installation commands."
  fi
}

install_macos() {
  local source_app="$SCRIPT_DIR/target/release/bundle/macos/DryMark.app"
  local install_parent="${DRYMARK_INSTALL_DIR:-$HOME/Applications}"
  local destination="$install_parent/DryMark.app"
  local transaction="$install_parent/.DryMark.setup-transaction"
  local staging="$transaction/Staging.app"
  local backup="$transaction/Previous.app"
  local timestamp
  timestamp="$(date '+%Y%m%d-%H%M%S')"
  local trash_previous="$HOME/.Trash/DryMark.previous.$timestamp.$$.app"

  [[ -d "$source_app" ]] || fail "The macOS application bundle was not produced."
  clear_local_finder_metadata "$source_app"
  codesign --verify --deep --strict --verbose=2 "$source_app"
  mkdir -p "$install_parent"

  if pgrep -x drymark-desktop >/dev/null 2>&1; then
    fail "DryMark is open. Quit it, then run setup again."
  fi

  if ! mkdir "$transaction"; then
    fail "An unfinished DryMark setup is at $transaction. Restore Previous.app first if DryMark.app is missing."
  fi
  MACOS_TRANSACTION_ACTIVE=true
  MACOS_TRANSACTION="$transaction"
  MACOS_DESTINATION="$destination"
  MACOS_STAGING="$staging"
  MACOS_PREVIOUS="$backup"
  MACOS_TRASH_SUFFIX="$timestamp.$$"
  [[ -e "$destination" ]] && MACOS_HAD_DESTINATION=true
  trap finish_macos_transaction_on_exit EXIT
  trap 'exit 129' HUP
  trap 'exit 130' INT
  trap 'exit 143' TERM

  if ! ditto "$source_app" "$staging"; then
    fail "DryMark could not be staged in $install_parent."
  fi
  clear_local_finder_metadata "$staging"
  if ! codesign --verify --deep --strict --verbose=2 "$staging"; then
    fail "The staged DryMark app did not pass signature verification."
  fi

  if [[ -e "$destination" ]] && ! mv "$destination" "$backup"; then
    fail "The existing DryMark app could not be prepared for replacement."
  fi

  if ! mv "$staging" "$destination"; then
    fail "DryMark could not be installed in $install_parent."
  fi

  clear_local_finder_metadata "$destination"
  if ! codesign --verify --deep --strict --verbose=2 "$destination"; then
    fail "The installed DryMark app did not pass signature verification."
  fi
  MACOS_INSTALL_COMMITTED=true
  ok "Installed $destination"

  if [[ -e "$backup" ]]; then
    if mkdir -p "$HOME/.Trash" && mv "$backup" "$trash_previous"; then
      info "Previous DryMark moved to Trash: $trash_previous"
    else
      warn "Previous DryMark remains recoverable at $backup"
    fi
  fi

  rmdir "$transaction" >/dev/null 2>&1 || true
  MACOS_TRANSACTION_ACTIVE=false
  trap - EXIT HUP INT TERM

  if [[ "$NO_LAUNCH" == false ]]; then
    open "$destination"
    ok "DryMark is open"
  else
    info "Open $destination when you are ready."
  fi
}

install_linux() {
  local source_binary="$SCRIPT_DIR/target/release/drymark-desktop"
  local install_root="${XDG_DATA_HOME:-$HOME/.local/share}/drymark"
  local bin_root="$HOME/.local/bin"
  local applications_root="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  local desktop_file="$applications_root/info.gabrimatic.drymark.desktop"

  [[ -x "$source_binary" ]] || fail "The Linux executable was not produced."
  mkdir -p "$install_root" "$bin_root" "$applications_root"
  install -m 0755 "$source_binary" "$install_root/drymark-desktop"
  ln -sfn "$install_root/drymark-desktop" "$bin_root/drymark"

  {
    printf '%s\n' '[Desktop Entry]'
    printf '%s\n' 'Type=Application'
    printf '%s\n' 'Name=DryMark'
    printf 'Exec=%s\n' "$install_root/drymark-desktop"
    printf '%s\n' 'Terminal=false'
    printf '%s\n' 'Categories=Utility;'
    printf '%s\n' 'Comment=Remove inspectable hidden text channels locally'
  } > "$desktop_file"

  ok "Installed $install_root/drymark-desktop"
  if [[ ":$PATH:" != *":$bin_root:"* ]]; then
    info "Add $bin_root to PATH to run DryMark as: drymark"
  fi

  if [[ "$NO_LAUNCH" == false ]]; then
    "$install_root/drymark-desktop" >/dev/null 2>&1 &
    ok "DryMark is open"
  else
    info "Run $install_root/drymark-desktop when you are ready."
  fi
}

printf '\n%b╭────────────────────────────────────────╮%b\n' "$BOLD" "$RESET"
printf '%b│%b  %bDryMark%b · Setup                     %b│%b\n' "$BOLD" "$RESET" "$CYAN" "$RESET" "$BOLD" "$RESET"
printf '%b│%b  %bLocal clipboard watermark removal%b    %b│%b\n' "$BOLD" "$RESET" "$DIM" "$RESET" "$BOLD" "$RESET"
printf '%b╰────────────────────────────────────────╯%b\n' "$BOLD" "$RESET"

step "Checking prerequisites"
OS_NAME="$(uname -s)"
case "$OS_NAME" in
  Darwin)
    xcode-select -p >/dev/null 2>&1 || fail \
      "Xcode Command Line Tools are required. Run: xcode-select --install"
    ok "macOS build tools"
    ;;
  Linux)
    check_linux_dependencies
    ok "Linux desktop libraries"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    fail "Use setup.ps1 from PowerShell on Windows."
    ;;
  *) fail "Unsupported operating system: $OS_NAME" ;;
esac

command -v node >/dev/null 2>&1 || fail "Node.js 22 or newer is required: https://nodejs.org"
command -v npm >/dev/null 2>&1 || fail "npm is required and normally ships with Node.js."
NODE_MAJOR="$(node_major_version)"
[[ "$NODE_MAJOR" =~ ^[0-9]+$ && "$NODE_MAJOR" -ge 22 ]] || fail \
  "Node.js 22 or newer is required; found $(node --version 2>/dev/null || printf unknown)."
ok "Node.js $(node --version)"

CARGO_BIN="$(resolve_cargo)" || fail \
  "Rust 1.97.1 is required. Install rustup from https://rustup.rs, then rerun setup."
export PATH="$(dirname "$CARGO_BIN"):$PATH"
CARGO_VERSION="$($CARGO_BIN --version)"
[[ "$CARGO_VERSION" == "cargo 1.97.1 "* ]] || fail \
  "Rust 1.97.1 is required; found $CARGO_VERSION. Run: rustup toolchain install 1.97.1"
ok "Rust 1.97.1"

if [[ "$CHECK_ONLY" == true ]]; then
  printf '\n%b✓ Prerequisites ready.%b\n\n' "$GREEN" "$RESET"
  exit 0
fi

step "Installing source dependencies"
npm ci
ok "Source dependencies ready"

step "Building DryMark"
case "$OS_NAME" in
  Darwin)
    npm run tauri -- build --bundles app -- --locked
    clear_local_finder_metadata "$SCRIPT_DIR/target/release/bundle/macos/DryMark.app"
    codesign --verify --deep --strict --verbose=2 \
      "$SCRIPT_DIR/target/release/bundle/macos/DryMark.app"
    ;;
  Linux)
    npm run tauri -- build --no-bundle -- --locked
    ;;
esac
ok "Native build complete"

if [[ "$BUILD_ONLY" == true ]]; then
  printf '\n%b✓ Build complete.%b\n\n' "$GREEN" "$RESET"
  exit 0
fi

step "Installing DryMark"
case "$OS_NAME" in
  Darwin) install_macos ;;
  Linux) install_linux ;;
esac

printf '\n%b✓ Setup complete.%b\n\n' "$GREEN" "$RESET"
