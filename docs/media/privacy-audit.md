# README media privacy audit

This audit covers the hero image, final-state poster, animated preview, and
silent MP4. The media contains only the synthetic fixture and DryMark's own
interface.

## Automated checks

| Check | Scope | Result |
| --- | --- | --- |
| OCR | Both full-resolution PNGs and 20 MP4 frames sampled at 2 fps | No identity, account, email, phone, IP address, filesystem path, or URL match |
| Binary strings | All four public artifacts | No identity token, email, phone-like number, IP address, local path, URL, or encoder signature match |
| Video streams | Final MP4 | Exactly one H.264 video stream; no audio stream |
| Video format | Final MP4 | 1920x1080, 30 fps, `yuv420p`, 300 frames, 10.000 seconds, fast-start |
| GIF format | Animated preview | 960x540, 12 fps, 120 frames, 10.000 seconds |
| Image metadata | Hero and poster PNGs | No embedded profiles or descriptive metadata |
| Container metadata | Final MP4 | Only allowlisted structural MP4 tags; no title, comment, artist, creation time, source path, or encoder tag |
| Size | MP4 and GIF | Each below 10 MiB |
| Checksums | All four public artifacts | Every entry in `manifest.sha256` verifies |
| Color | Both PNGs and MP4 samples | Material purple-pixel fraction below the `0.01%` rejection threshold |

The identity scan used the current local account and known owner tokens without
printing or recording those values. Pattern scans covered Unix and Windows
home paths, `file:` URLs, email addresses, IPv4 addresses, and phone-like
numeric sequences.

## False-positive disposition

An intermediate MP4 produced one broad phone-pattern match inside the H.264
encoder copyright year range. It was not captured UI, clipboard content, an
account identifier, or a phone number. The exporter now removes the
encoder-identifying SEI; the final binary-string scan returns zero matches.

The remaining numeric UI content consists of synthetic fixture counts and step
labels. Manual review confirmed that those values are product state, not
personal data.

## Visual review

The following were inspected at full size:

- the 4.600-second hero frame;
- the 6.200-second final-state poster;
- a ten-frame contact sheet sampled across the complete MP4.

The review found no menu bar, Dock, Desktop, clock, account, personal
notification, path, unrelated application, or private clipboard content. It
also confirmed the real result toast, the complete copy-clean-paste sequence,
the final zero-channel result, and the graphite, arctic-blue, mint, and amber
palette with no material purple region.

OCR is not treated as sufficient on its own; the full-size and contact-sheet
reviews cover text or interface details OCR may miss.

Run the repeatable artifact gates:

```bash
bash tools/readme-demo/export-media.sh --check
```
