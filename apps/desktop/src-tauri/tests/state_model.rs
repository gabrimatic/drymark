#![allow(missing_docs)]

use drymark_core::{Policy, sanitize};
use drymark_desktop_lib::{FrontendResult, frontend_result, shortcut_display};
use drymark_transaction::CleanOutcome;

#[test]
fn cleaned_outcome_maps_to_metadata_only_frontend_result() -> Result<(), serde_json::Error> {
    let report = sanitize("PRIVATE-ZXQ\u{200b}", Policy::PreserveAppearance).report;
    let mapped = frontend_result(CleanOutcome::Cleaned { report }, "15:42".to_owned());
    assert_eq!(
        mapped,
        FrontendResult {
            kind: "cleaned",
            removed: 1,
            observed: 0,
            canonicalized: false,
            formatting_cleared: true,
            at: "15:42".to_owned(),
        }
    );
    let json = serde_json::to_string(&mapped)?;
    assert!(!json.contains("PRIVATE-ZXQ"));
    Ok(())
}

#[test]
fn race_outcome_never_claims_success() {
    let mapped = frontend_result(CleanOutcome::ClipboardChanged, "15:43".to_owned());
    assert_eq!(mapped.kind, "clipboard_changed");
    assert_eq!(mapped.removed, 0);
    assert_eq!(mapped.observed, 0);
    assert!(!mapped.canonicalized);
    assert!(!mapped.formatting_cleared);
}

#[test]
fn canonicalization_and_observation_metadata_reach_the_frontend() {
    let canonical = frontend_result(
        CleanOutcome::Cleaned {
            report: sanitize("Cafe\u{301}\r\n", Policy::Thorough).report,
        },
        "15:44".to_owned(),
    );
    assert!(canonical.canonicalized);

    let observed = frontend_result(
        CleanOutcome::AlreadyClean {
            report: sanitize("👩‍💻", Policy::PreserveAppearance).report,
        },
        "15:45".to_owned(),
    );
    assert!(observed.observed > 0);
}

#[test]
fn shortcut_display_uses_native_mac_symbols_without_changing_storage() {
    assert_eq!(shortcut_display("Alt+Shift+V", true), "⌥ ⇧ V");
    assert_eq!(shortcut_display("Control+Shift+K", true), "⌃ ⇧ K");
    assert_eq!(shortcut_display("Alt+Shift+V", false), "Alt Shift V");
}
