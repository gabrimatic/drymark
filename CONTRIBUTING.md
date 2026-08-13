# Contributing

DryMark accepts focused fixes to Unicode policy, platform adapters, the CLI,
desktop UX, tests, and documentation.

## Development Setup

Install Node.js 22 or newer, Rust 1.97.1, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
git clone https://github.com/gabrimatic/drymark.git
cd drymark
./setup.sh --check
npm ci
cargo test --locked --workspace --all-targets
npm run test:ui
```

On Windows, use `.\setup.ps1 -Check` for the prerequisite check. Start the
desktop frontend with `npm run tauri -- dev`. Build a package with
`./setup.sh --build-only` on macOS or Linux, or `.\setup.ps1 -BuildOnly` on
Windows.

## Architecture Rules

- Keep `drymark-core` deterministic and free of I/O, OS, clipboard, and UI
  dependencies.
- Never log, serialize, persist, or include clipboard excerpts in errors.
- Re-read the clipboard before every write, abort on any mismatch, and verify
  the text after a reported-success write.
- Preserve mode must not remove a contextual sequence without a documented,
  tested reason.
- Every policy category that may alter rendering or machine interpretation must
  be called out in the UI and policy documentation.
- Do not add telemetry, analytics, accounts, network calls, autostart, or a
  background service.

## Required Checks

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets
npm run lint:ui
npm run test:ui
npm run build
npm audit --audit-level=high
cargo audit
cargo deny check
```

Changes to classification logic need a golden regression, a contextual-language
test where applicable, and an idempotence check. Changes to clipboard behavior
need success, race, read-failure, and write-failure coverage.

## Pull Request Checklist

- Keep one feature or fix per pull request.
- Explain the channel or failure mode being addressed.
- Add tests before changing watermark-removal behavior.
- Update README or policy documentation when visible behavior changes.
- Test the affected desktop flow on a real OS when platform code changes.
- Leave unrelated files alone and never include clipboard samples from real
  users.

## Reporting Bugs

Use the bug report template. Include the DryMark version, OS, active policy,
expected result, and a synthetic reproduction. Replace any private text with a
minimal placeholder before posting.

Security issues must follow [SECURITY.md](SECURITY.md) and must not be opened as
public issues.
