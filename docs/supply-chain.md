# Supply-chain Policy

DryMark commits its Rust and npm lockfiles. Continuous integration audits
known Rust advisories, npm vulnerabilities, dependency licenses, duplicate
versions, and registry sources. Direct dependencies marked unmaintained fail
the Rust policy check; transitive maintenance notices remain visible for
upstream tracking.

All GitHub Actions are pinned to full commit SHAs. Rust resolution is validated
against committed lockfiles before builds, fuzzing, mutation testing, and
packaging. CI-installed Rust tools use explicit versions with their own locked
installation graphs. The required CI dependency graph includes frontend and
Rust tests, three-platform desktop compilation, bounded fuzz runs, dependency
policy checks, and mutation testing; scheduled jobs extend fuzzing duration and
repeat advisory checks.

## Reviewed Advisory Exception

`RUSTSEC-2024-0429` affects `glib::VariantStrIter` in `glib` versions before
0.20. Tauri's supported Linux WebKit/GTK3 stack currently resolves `glib` 0.18,
and no compatible patched GTK3 release exists. DryMark does not import or call
`VariantStrIter`; the affected crate is present only through Tauri's Linux UI
stack. The advisory is therefore explicitly acknowledged in `deny.toml`, not
silenced globally.

This exception must be removed when Tauri's Linux runtime adopts `glib` 0.20 or
newer, or immediately if DryMark gains code that calls the affected API. All
vulnerability advisories continue to fail the build.

## License Policy

`deny.toml` contains the complete allowlist. It includes permissive licenses
used by the platform clipboard and build stacks, including BSL-1.0 and
Apache-2.0 with LLVM exception. Unknown registries, unknown Git sources, and
licenses outside that list fail the policy check.
