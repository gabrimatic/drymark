#!/usr/bin/env bash

set -euo pipefail

export LC_ALL=C
export TZ=UTC
export SOURCE_DATE_EPOCH=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MEDIA_DIR="${DRYMARK_MEDIA_DIR:-$REPO_ROOT/docs/media}"

HERO_NAME="drymark-hero.png"
POSTER_NAME="drymark-demo-poster.png"
VIDEO_NAME="drymark-demo.mp4"
GIF_NAME="drymark-demo.gif"
MANIFEST_NAME="manifest.sha256"
MAX_BYTES=10485760

fail() {
  echo "export-media: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

probe_value() {
  local file="$1"
  local entries="$2"
  ffprobe -v error -select_streams v:0 -show_entries "$entries" \
    -of default=noprint_wrappers=1:nokey=1 "$file" | head -n 1
}

stream_count() {
  local file="$1"
  local type="$2"
  ffprobe -v error -select_streams "$type" -show_entries stream=index \
    -of csv=p=0 "$file" | awk 'NF { count += 1 } END { print count + 0 }'
}

file_size() {
  local file="$1"
  if stat -f '%z' "$file" >/dev/null 2>&1; then
    stat -f '%z' "$file"
  else
    stat -c '%s' "$file"
  fi
}

assert_artifact() {
  local name="$1"
  [[ -s "$MEDIA_DIR/$name" ]] || fail "missing artifact: $name"
}

validate_source() {
  local source="$1"
  [[ -f "$source" ]] || fail "capture master not found: $source"
  [[ "$(stream_count "$source" v)" -eq 1 ]] || fail "capture master must contain exactly one video stream"
  [[ "$(stream_count "$source" a)" -eq 0 ]] || fail "capture master must not contain an audio stream"

  local dimensions duration
  dimensions="$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=s=x:p=0 "$source")"
  [[ "$dimensions" == "3520x1980" ]] || fail "capture master must be 3520x1980, found $dimensions"

  duration="$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$source")"
  awk -v duration="$duration" 'BEGIN { exit !(duration >= 10.5) }' \
    || fail "capture master must be at least 10.5 seconds"
}

validate_png() {
  local file="$1"
  local label="$2"
  local format dimensions
  format="$(magick identify -quiet -format '%m' "$file")"
  dimensions="$(magick identify -quiet -format '%wx%h' "$file")"
  [[ "$format" == "PNG" ]] || fail "$label must be PNG"
  [[ "$dimensions" == "1920x1080" ]] || fail "$label must be 1920x1080, found $dimensions"
  if magick identify -quiet -verbose "$file" | grep -E '^  Profiles:|^  Comment:|^[[:space:]]+(exif|xmp):' >/dev/null; then
    fail "$label contains descriptive image metadata"
  fi
}

validate_video() {
  local file="$MEDIA_DIR/$VIDEO_NAME"
  local codec dimensions pixel_format frame_rate duration bytes

  [[ "$(stream_count "$file" v)" -eq 1 ]] || fail "$VIDEO_NAME must contain exactly one video stream"
  [[ "$(stream_count "$file" a)" -eq 0 ]] || fail "$VIDEO_NAME must not contain an audio stream"
  [[ "$(ffprobe -v error -show_entries stream=index -of csv=p=0 "$file" | awk 'NF { count += 1 } END { print count + 0 }')" -eq 1 ]] \
    || fail "$VIDEO_NAME must contain exactly one stream"

  codec="$(probe_value "$file" stream=codec_name)"
  dimensions="$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=s=x:p=0 "$file")"
  pixel_format="$(probe_value "$file" stream=pix_fmt)"
  frame_rate="$(probe_value "$file" stream=r_frame_rate)"
  duration="$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file")"
  bytes="$(file_size "$file")"

  [[ "$codec" == "h264" ]] || fail "$VIDEO_NAME must use H.264, found $codec"
  [[ "$dimensions" == "1920x1080" ]] || fail "$VIDEO_NAME must be 1920x1080, found $dimensions"
  [[ "$pixel_format" == "yuv420p" ]] || fail "$VIDEO_NAME must use yuv420p, found $pixel_format"
  [[ "$frame_rate" == "30/1" ]] || fail "$VIDEO_NAME must be 30 fps, found $frame_rate"
  awk -v duration="$duration" 'BEGIN { exit !(duration >= 9.9 && duration <= 10.1) }' \
    || fail "$VIDEO_NAME must be 10 seconds, found $duration"
  [[ "$bytes" -lt "$MAX_BYTES" ]] || fail "$VIDEO_NAME must be below 10 MiB"

  local moov_offset mdat_offset
  moov_offset="$(LC_ALL=C grep -aob 'moov' "$file" | head -n 1 | cut -d: -f1)"
  mdat_offset="$(LC_ALL=C grep -aob 'mdat' "$file" | head -n 1 | cut -d: -f1)"
  [[ -n "$moov_offset" && -n "$mdat_offset" && "$moov_offset" -lt "$mdat_offset" ]] \
    || fail "$VIDEO_NAME is not fast-start optimized"
}

validate_gif() {
  local file="$MEDIA_DIR/$GIF_NAME"
  local format dimensions frame_rate duration bytes
  format="$(magick identify -quiet -format '%m' "$file[0]")"
  dimensions="$(magick identify -quiet -format '%wx%h' "$file[0]")"
  frame_rate="$(probe_value "$file" stream=r_frame_rate)"
  duration="$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file")"
  bytes="$(file_size "$file")"

  [[ "$format" == "GIF" ]] || fail "$GIF_NAME must be GIF"
  [[ "$dimensions" == "960x540" ]] || fail "$GIF_NAME must be 960x540, found $dimensions"
  [[ "$frame_rate" == "12/1" ]] || fail "$GIF_NAME must be 12 fps, found $frame_rate"
  awk -v duration="$duration" 'BEGIN { exit !(duration >= 9.9 && duration <= 10.1) }' \
    || fail "$GIF_NAME must be 10 seconds, found $duration"
  [[ "$bytes" -lt "$MAX_BYTES" ]] || fail "$GIF_NAME must be below 10 MiB"
}

validate_no_private_metadata() {
  local artifact
  for artifact in "$HERO_NAME" "$POSTER_NAME" "$VIDEO_NAME" "$GIF_NAME"; do
    if strings "$MEDIA_DIR/$artifact" | grep -Ei '/Use''rs/|file://|[A-Za-z]:\\\\Users\\\\' >/dev/null; then
      fail "$artifact contains a local path marker"
    fi
  done

  if strings "$MEDIA_DIR/$VIDEO_NAME" | grep -Ei 'x264|Lavf|Lavc' >/dev/null; then
    fail "$VIDEO_NAME contains encoder-identifying metadata"
  fi

  local metadata unexpected_metadata
  metadata="$(ffprobe -v error -show_entries format_tags:stream_tags -of default "$MEDIA_DIR/$VIDEO_NAME")"
  unexpected_metadata="$(printf '%s\n' "$metadata" | grep -Ev '^\[(STREAM|/STREAM|FORMAT|/FORMAT)\]$|^TAG:language=und$|^TAG:handler_name=VideoHandler$|^TAG:major_brand=isom$|^TAG:minor_version=512$|^TAG:compatible_brands=isomiso2avc1mp41$' || true)"
  [[ -z "$unexpected_metadata" ]] || fail "$VIDEO_NAME contains descriptive metadata"
}

validate_no_material_purple() {
  local sample_dir sample_image purple_fraction
  sample_dir="$(mktemp -d "${TMPDIR:-/tmp}/drymark-purple-check.XXXXXX")"
  sample_image="$sample_dir/video-samples.png"
  trap 'find "$sample_dir" -type f -delete 2>/dev/null || true; rmdir "$sample_dir" 2>/dev/null || true' RETURN

  ffmpeg -hide_banner -loglevel error -nostdin -y \
    -i "$MEDIA_DIR/$VIDEO_NAME" \
    -vf 'fps=1,scale=480:270:flags=lanczos,tile=5x2' \
    -frames:v 1 -an -map_metadata -1 "$sample_image"

  purple_fraction="$(magick \
    "$MEDIA_DIR/$HERO_NAME" \
    "$MEDIA_DIR/$POSTER_NAME" \
    "$sample_image" \
    -append -colorspace HSL \
    -fx '(r > 0.70 && r < 0.91 && g > 0.35 && b > 0.15) ? 1 : 0' \
    -format '%[fx:mean]' info:)"

  awk -v fraction="$purple_fraction" 'BEGIN { exit !(fraction < 0.0001) }' \
    || fail "public media contains a material purple region"

  find "$sample_dir" -type f -delete
  rmdir "$sample_dir"
  trap - RETURN
}

validate_manifest() {
  local manifest="$MEDIA_DIR/$MANIFEST_NAME"
  local expected
  expected="$(printf '%s\n' "$GIF_NAME" "$VIDEO_NAME" "$POSTER_NAME" "$HERO_NAME" | sort)"

  awk \
    -v first="$POSTER_NAME" \
    -v second="$GIF_NAME" \
    -v third="$VIDEO_NAME" \
    -v fourth="$HERO_NAME" \
    '
      {
        expected = (NR == 1 ? first : NR == 2 ? second : NR == 3 ? third : NR == 4 ? fourth : "")
        if (expected == "" || length($1) != 64 || $1 ~ /[^0-9a-f]/ || substr($0, 65, 2) != "  " || substr($0, 67) != expected) {
          invalid = 1
        }
      }
      END { exit !(NR == 4 && !invalid) }
    ' "$manifest" || fail "$MANIFEST_NAME has an invalid format"

  [[ "$(awk '{ print $2 }' "$manifest")" == "$expected" ]] \
    || fail "$MANIFEST_NAME must list the four public artifacts in stable order"
  (cd "$MEDIA_DIR" && shasum -a 256 -c "$MANIFEST_NAME" >/dev/null) \
    || fail "$MANIFEST_NAME checksum verification failed"
}

validate_outputs() (
  if [[ $# -eq 1 ]]; then
    MEDIA_DIR="$1"
  elif [[ $# -ne 0 ]]; then
    fail "validate_outputs accepts at most one media directory"
  fi

  require_command ffmpeg
  require_command ffprobe
  require_command magick
  require_command shasum

  assert_artifact "$HERO_NAME"
  assert_artifact "$POSTER_NAME"
  assert_artifact "$VIDEO_NAME"
  assert_artifact "$GIF_NAME"
  assert_artifact "$MANIFEST_NAME"

  validate_png "$MEDIA_DIR/$HERO_NAME" "$HERO_NAME"
  validate_png "$MEDIA_DIR/$POSTER_NAME" "$POSTER_NAME"
  validate_video
  validate_gif
  validate_no_private_metadata
  validate_no_material_purple
  validate_manifest
)

write_manifest() {
  local destination="${1:-$MEDIA_DIR}"
  (
    cd "$destination"
    printf '%s\n' "$GIF_NAME" "$VIDEO_NAME" "$POSTER_NAME" "$HERO_NAME" \
      | sort \
      | while IFS= read -r artifact; do shasum -a 256 "$artifact"; done \
      > "$MANIFEST_NAME"
  )
}

export_media() {
  local source="$1"
  local temp_dir hero_raw poster_raw
  require_command ffmpeg
  require_command ffprobe
  require_command magick
  require_command shasum
  validate_source "$source"

  mkdir -p "$MEDIA_DIR"
  temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/drymark-media-export.XXXXXX")"
  hero_raw="$temp_dir/hero-raw.png"
  poster_raw="$temp_dir/poster-raw.png"
  trap 'find "$temp_dir" -type f -delete 2>/dev/null || true; rmdir "$temp_dir" 2>/dev/null || true' EXIT

  ffmpeg -hide_banner -loglevel error -nostdin -y \
    -ss 4.600 -i "$source" -frames:v 1 \
    -vf 'scale=1920:1080:flags=lanczos,setsar=1' \
    -an -map_metadata -1 "$hero_raw"
  magick "$hero_raw" -strip -colorspace sRGB \
    -define png:exclude-chunks=date,time -define png:compression-level=9 \
    "$temp_dir/$HERO_NAME"

  ffmpeg -hide_banner -loglevel error -nostdin -y \
    -ss 6.200 -i "$source" -frames:v 1 \
    -vf 'scale=1920:1080:flags=lanczos,setsar=1' \
    -an -map_metadata -1 "$poster_raw"
  magick "$poster_raw" -strip -colorspace sRGB \
    -define png:exclude-chunks=date,time -define png:compression-level=9 \
    "$temp_dir/$POSTER_NAME"

  ffmpeg -hide_banner -loglevel error -nostdin -y \
    -i "$source" -map 0:v:0 \
    -vf 'trim=start=0.500,setpts=PTS-STARTPTS,fps=30:round=near,trim=duration=10.000,setpts=PTS-STARTPTS,scale=1920:1080:flags=lanczos,setsar=1,format=yuv420p' \
    -an -c:v libx264 -preset slow -crf 20 -profile:v high -level:v 4.1 \
    -pix_fmt yuv420p -tag:v avc1 -threads 1 \
    -bsf:v 'filter_units=remove_types=6' \
    -movflags +faststart -fflags +bitexact -flags:v +bitexact \
    -map_metadata -1 -map_chapters -1 \
    -metadata title= -metadata comment= -metadata creation_time= -metadata encoder= \
    -metadata:s:v:0 encoder= "$temp_dir/$VIDEO_NAME"

  ffmpeg -hide_banner -loglevel error -nostdin -y \
    -i "$source" \
    -filter_complex '[0:v]trim=start=0.500,setpts=PTS-STARTPTS,fps=12:round=near,trim=duration=10.000,setpts=PTS-STARTPTS,scale=960:540:flags=lanczos,split[frames][palette_source];[palette_source]palettegen=max_colors=160:stats_mode=diff[palette];[frames][palette]paletteuse=dither=sierra2_4a:diff_mode=rectangle' \
    -loop 0 -an -map_metadata -1 "$temp_dir/$GIF_NAME"

  write_manifest "$temp_dir"
  validate_outputs "$temp_dir"

  mv "$temp_dir/$HERO_NAME" "$MEDIA_DIR/$HERO_NAME"
  mv "$temp_dir/$POSTER_NAME" "$MEDIA_DIR/$POSTER_NAME"
  mv "$temp_dir/$VIDEO_NAME" "$MEDIA_DIR/$VIDEO_NAME"
  mv "$temp_dir/$GIF_NAME" "$MEDIA_DIR/$GIF_NAME"
  mv "$temp_dir/$MANIFEST_NAME" "$MEDIA_DIR/$MANIFEST_NAME"

  find "$temp_dir" -type f -delete
  rmdir "$temp_dir"
  trap - EXIT
}

usage() {
  cat <<'EOF'
Usage:
  tools/readme-demo/export-media.sh /path/to/drymark-demo-master.mov
  tools/readme-demo/export-media.sh --check [/path/to/drymark-demo-master.mov]

Set DRYMARK_MEDIA_DIR to validate an alternate output directory.
EOF
}

case "${1:-}" in
  --check)
    shift
    if [[ $# -gt 1 ]]; then
      usage >&2
      exit 2
    fi
    if [[ $# -eq 1 ]]; then
      validate_source "$1"
    fi
    validate_outputs
    echo "export-media: public artifacts are valid"
    ;;
  -h|--help)
    usage
    ;;
  "")
    usage >&2
    exit 2
    ;;
  *)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    export_media "$1"
    echo "export-media: public artifacts exported and validated"
    ;;
esac
