# Architecture

## Data Flow

```text
Global shortcut / tray / Settings
              |
              v
      native clipboard adapter
              |
              v
     transaction coordinator
       |              |
       v              v
  pure sanitizer   second read
       |              |
       +------ compare+
              |
              v
      fresh plain-text write
              |
              v
       post-write read
              |
              v
     metadata-only UI state
```

## `drymark-core`

The core accepts `&str` plus a `Policy` and returns owned sanitized text with a
metadata-only report. It has no I/O, async runtime, platform code, clipboard
access, logging, or network dependency. Scratch scalar buffers, emoji candidates,
and replaced intermediate strings are zeroed before release; callers still own
the returned output and control its lifetime.

Classification is ordered so dangerous unconditional categories are handled
before contextual protection. A build-generated finite automaton recognizes
every supported qualification form in the vendored Unicode 17 emoji registry
with linear runtime and a small constant probe budget. Counts use saturating
arithmetic and stable sorted categories.

## `drymark-transaction`

The coordinator owns the safety sequence and depends on a two-method
`ClipboardPort`. `ClipboardSnapshot` stores text in a `Zeroizing<String>`,
redacts `Debug`, and carries optional revision plus format metadata.

The coordinator handles non-text, empty, oversized, changed, read-failed,
recheck-failed, write-failed, and post-write-unverified states without claiming
success. Platform adapters may provide less metadata than the coordinator can
compare; text is always compared. A successful write is followed by an exact
text read-back before `Cleaned` is returned.

## `drymark-cli`

The CLI reads at most 16 MiB from standard input. Its input, transformed output,
and invalid UTF-8 buffers are zeroed before release. `clean` emits only
transformed text, `scan` emits only metadata, and `--check` emits nothing.
Errors are fixed phrases and never contain source bytes.

## Desktop Application

Tauri owns the tray, windows, global shortcut, clipboard adapter, and local
preference file. React renders three bounded surfaces:

- A 380 by 540 tray popover.
- A resizable Settings window.
- A 360 by 84 silent visual toast.

The native command surface is deliberately small. Clipboard work runs on a
blocking worker rather than the UI thread. Frontend events contain status,
policy, shortcut state, version, timestamp, counts, and outcome kind only.

The app does not register autostart or a service. Preferences use same-directory
atomic replacement through `atomic-write-file` and contain no clipboard data.
Preference reads are capped at 16 KiB. Startup repairs malformed files and
invalid shortcuts with validated defaults. A failed
preference write is represented in frontend state and shown as session-only;
it is never silently presented as durable.

The pinned clipboard plugin uses `arboard` for native reads and writes. Its
plain-text write replaces the current native clipboard offer rather than adding
text to existing rich representations. The desktop adapter does not expose
format enumeration or revisions, so every text value is conservatively
rewritten. The 16 MiB desktop guard is evaluated after the native API returns
the string; the CLI alone enforces the bound during input acquisition.

## Failure Model

Platform errors are mapped into `busy`, `permission_denied`, `unavailable`,
`invalid_text`, or `platform` before they reach application state. Raw platform
messages are not forwarded because they can contain environment details.

Frontend feedback distinguishes watermark channels removed, no removable
channels, clipboard changed, empty, non-text, too large, read failed, recheck
failed, write failed, and write unverified. Pre-write failures identify that no
write occurred. Write and verification failures identify clipboard state as
unknown rather than assuming the prior value survived.
