#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/drymark-export-test.XXXXXX")"
trap 'find "$TEST_DIR" -type f -delete 2>/dev/null || true; find "$TEST_DIR" -depth -type d -exec rmdir {} \; 2>/dev/null || true' EXIT

set +e
output="$(DRYMARK_MEDIA_DIR="$TEST_DIR" bash "$SCRIPT_DIR/export-media.sh" --check 2>&1)"
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "expected validation to reject a missing export set" >&2
  exit 1
fi

if [[ "$output" != *"missing artifact: drymark-hero.png"* ]]; then
  echo "expected a precise missing-artifact diagnostic" >&2
  exit 1
fi

echo "export validation rejects a missing export set"

ffmpeg -hide_banner -loglevel error -nostdin -y \
  -f lavfi -i 'color=c=black:s=16x16:r=1:d=11' \
  -f lavfi -i 'color=c=white:s=16x16:r=1:d=11' \
  -map 0:v:0 -map 1:v:0 -c:v ffv1 "$TEST_DIR/two-video.mkv"

set +e
output="$(bash "$SCRIPT_DIR/export-media.sh" --check "$TEST_DIR/two-video.mkv" 2>&1)"
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "expected validation to reject a second source video stream" >&2
  exit 1
fi

if [[ "$output" != *"capture master must contain exactly one video stream"* ]]; then
  echo "expected an exact source-stream diagnostic" >&2
  exit 1
fi

echo "export validation rejects multiple source video streams"

ffmpeg -hide_banner -loglevel error -nostdin -y \
  -f lavfi -i 'color=c=black:s=640x360:r=1:d=11' \
  -c:v ffv1 "$TEST_DIR/wrong-geometry.mkv"

set +e
output="$(bash "$SCRIPT_DIR/export-media.sh" --check "$TEST_DIR/wrong-geometry.mkv" 2>&1)"
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "expected validation to reject the wrong capture geometry" >&2
  exit 1
fi

if [[ "$output" != *"capture master must be 3520x1980, found 640x360"* ]]; then
  echo "expected an exact source-geometry diagnostic" >&2
  exit 1
fi

echo "export validation rejects the wrong capture geometry"

REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TAMPERED_DIR="$TEST_DIR/tampered"
mkdir -p "$TAMPERED_DIR"
cp "$REPO_ROOT/docs/media/drymark-hero.png" "$TAMPERED_DIR/"
cp "$REPO_ROOT/docs/media/drymark-demo-poster.png" "$TAMPERED_DIR/"
cp "$REPO_ROOT/docs/media/drymark-demo.mp4" "$TAMPERED_DIR/"
cp "$REPO_ROOT/docs/media/drymark-demo.gif" "$TAMPERED_DIR/"
awk 'NR == 1 { printf "%s  %s\n", "0000000000000000000000000000000000000000000000000000000000000000", $2; next } { print }' \
  "$REPO_ROOT/docs/media/manifest.sha256" > "$TAMPERED_DIR/manifest.sha256"

set +e
output="$(DRYMARK_MEDIA_DIR="$TAMPERED_DIR" bash "$SCRIPT_DIR/export-media.sh" --check 2>&1)"
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "expected validation to reject a tampered checksum manifest" >&2
  exit 1
fi

if [[ "$output" != *"manifest.sha256 checksum verification failed"* ]]; then
  echo "expected an exact checksum-tamper diagnostic" >&2
  exit 1
fi

echo "export validation rejects a tampered checksum manifest"

awk '{ print $1 " " $2 }' "$REPO_ROOT/docs/media/manifest.sha256" \
  > "$TAMPERED_DIR/manifest.sha256"

set +e
output="$(DRYMARK_MEDIA_DIR="$TAMPERED_DIR" bash "$SCRIPT_DIR/export-media.sh" --check 2>&1)"
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "expected validation to reject malformed checksum lines" >&2
  exit 1
fi

if [[ "$output" != *"manifest.sha256 has an invalid format"* ]]; then
  echo "expected an exact checksum-format diagnostic" >&2
  exit 1
fi

echo "export validation rejects malformed checksum lines"
