# Testing

DryMark treats transformations as security-sensitive data integrity code.
The test strategy checks examples, general invariants, failure ordering, real
clipboard behavior, and whether the tests notice deliberately broken logic.

## Deterministic Suites

| Suite | Coverage |
| --- | --- |
| Golden Unicode tests | Hidden watermark channels, opaque 256-bit payload carriers, controls, noncharacters, private use, normalization, and report counts |
| Contextual tests | Emoji ZWJ sequences, tags, Mongolian and Han selectors, Persian and Indic shaping, bidi isolates |
| Property tests | Idempotence, bounded output, report consistency, and printable ASCII identity across random strings |
| Large-input test | Repeated adversarial controls and emoji at multi-million-scalar scale |
| Transaction tests | Success, clean no-op, conservative rewrite, text/revision/rewrite races, size limit, and pre/post-write failures |
| CLI tests | Exact stdout, empty stderr, exit codes, invalid UTF-8, JSON privacy, and version output |
| Frontend tests | Accessible tray workflow, policy selection, shortcut recording, silent toast, race copy, unknown-write states, and preference-write warnings |
| Packaged macOS runtime | Strict bundle-signature verification, real application window, default shortcut, Preserve fixture, rich-format replacement including empty text, non-text input, and silent visual feedback |
| Cross-platform CI | Rust tests, Clippy, native compilation on all three platforms, a verified macOS app, a Debian package, and a Windows NSIS installer |
| Source setup | Isolated option parsing, caller-directory independence, prerequisite-only mode, Rust toolchain fallback, no-persistence checks, native source builds, and recoverable macOS app replacement |
| Public surface | Tracked home-path guard, README media structure, deterministic export validation, checksums, and GitHub-rendered GIF verification |

Run the standard gates:

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

## Fuzzing

The `sanitize` target feeds every arbitrary byte string through lossy UTF-8
decoding and both policies, then checks idempotence, report accounting,
deterministic change state, and the output bound.

The `transaction` target varies pre-write and post-write clipboard reads,
revision equality, rewrite state, policy, and text. It asserts that a write
occurs only for an eligible matching transaction and that `Cleaned` is returned
only after the read-back exactly matches the sanitizer result.

```bash
cargo install cargo-fuzz --version 0.13.2 --locked
cd fuzz
cargo +nightly-2026-08-10 metadata --locked --no-deps --format-version 1
cargo +nightly-2026-08-10 fuzz run sanitize -- -max_total_time=60
cargo +nightly-2026-08-10 fuzz run transaction -- -max_total_time=60
```

CI runs both targets under AddressSanitizer. Longer local runs and retained
private corpora are encouraged before policy releases.

## Mutation Testing

Mutation testing changes branches, constants, match arms, and return values one
at a time. A surviving mutation indicates behavior the deterministic suite does
not distinguish.

```bash
cargo install cargo-mutants --version 27.1.0 --locked
cargo mutants -p drymark-core --jobs 4 --timeout 60 --cargo-arg=--locked
```

Release review requires inspecting every missed or timed-out mutant. Equivalent
or unbuildable mutations must be documented; meaningful survivors need a new
test or implementation correction.

The 0.1.0 baseline with cargo-mutants 27.1.0 generated 281 core mutations: 274
were caught, 7 were rejected by the compiler, and none were missed or timed
out. The fixed-search budgets and direct contextual-table tests are part of
that baseline.

## Real Runtime Checklist

On macOS, `make runtime-smoke-macos` builds a fresh ad-hoc-signed app bundle,
refuses to disturb an already-running DryMark process, launches that package,
exercises the default shortcut against synthetic clipboard fixtures, restores
the prior clipboard, and terminates the process it launched. The current
branded bundle must pass this gate before claiming packaged runtime evidence;
a static build does not satisfy it.

1. Build the native package, not only the web frontend, and verify its bundle signature.
2. Open Settings and confirm the shortcut reports Ready or a specific conflict.
3. Clean a fixture containing zero-width, word-joiner, emoji ZWJ, shaping ZWNJ,
   and bidi override values.
4. Verify Preserve removes unsafe values but keeps the valid emoji and shaping
   sequence byte-for-byte.
5. Verify Thorough removes all format controls and canonicalizes CRLF, spaces,
   and NFC.
6. Change the clipboard during a clean and confirm no success is claimed; also
   force a post-write mismatch and confirm the state is reported as unknown.
7. Verify empty text with extra formats is rewritten to plain empty text, while
   empty plain text on an adapter that can prove no rewrite is needed and
   non-text clipboards are left untouched.
8. Confirm feedback is visual only and no audio element or OS sound path exists.
9. Inspect preferences and application logs for the fixture text.
10. Quit the app and confirm no helper, service, or shortcut process remains.
