# Security Policy

## Privacy by Design

- Clipboard text is processed locally and is never sent over a network.
- Text is not stored in preferences, history, logs, analytics, crash reports,
  or watermark-removal reports.
- Owned clipboard snapshots, removal-engine scratch buffers, and CLI input/output
  buffers are zeroed when dropped where ownership permits.
- Reports expose counts, sizes, categories, timestamps, and outcome states only.
- Watermark removal replaces rich clipboard representations with one plain-text value.
- DryMark does not install autostart, a login item, or a background service.

## Trust Boundaries

| Boundary | Treatment |
| --- | --- |
| Clipboard text | Sensitive; held only for the active transaction |
| Rich clipboard formats | Untrusted; discarded when a clean is committed |
| Clipboard revision and rewrite flags | Non-sensitive comparison metadata when an adapter exposes them |
| Preferences | Policy, shortcut, and visual-feedback flag only |
| Frontend IPC | Fixed commands and metadata-only state |
| Network | Not used by the application |

The OS clipboard itself remains a shared system resource. Other applications
with clipboard access can read it before or after DryMark runs. DryMark does
not claim to isolate the system clipboard from other processes. Operating-system
clipboard history, cloud clipboard, and device-continuity services may retain
or sync a value when the user has enabled them; DryMark neither controls nor
claims to disable those services.

## Race and Failure Safety

The coordinator performs a second read before writing. A changed text value
aborts the operation; adapter-provided revision and rewrite metadata are also
compared when available. After a write returns success, an immediate third read
must match before the operation is reported as successful.

OS clipboards do not provide an atomic compare-and-swap operation. A change in
the narrow interval between the final read and write can therefore be
overwritten. Post-write verification detects a resulting text mismatch but
cannot restore the prior value. Write errors and verification failures are
reported as an unknown clipboard state, never as success or as proof that
nothing changed.

The CLI enforces its 16 MiB limit while reading. The desktop guard runs after
the operating-system API has materialized clipboard text in the process.

## Vulnerability Reporting

Report vulnerabilities privately:

1. Do not open a public issue.
2. Use [GitHub private vulnerability reporting](https://github.com/gabrimatic/drymark/security/advisories/new).
3. Include a synthetic reproduction, demonstrated impact, affected platform,
   and suggested fix if available.

Expect acknowledgment within 48 hours.

## Supported Versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |

## Out of Scope

- Signals carried only by visible wording, semantics, punctuation, or token
  selection.
- Detection systems whose signal is absent from the copied text.
- Clipboard reads performed by another local process with OS-granted access.
- Documented rendering or machine-interpretation trade-offs in either removal
  policy; no policy can both erase every invisible channel and remain lossless.
