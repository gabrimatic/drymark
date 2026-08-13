#![allow(missing_docs)]

use drymark_core::{Policy, sanitize};

#[test]
fn report_serialization_never_contains_input_excerpts() -> Result<(), serde_json::Error> {
    let secret = "private-token-ZXQ-9182\u{200b}";
    let result = sanitize(secret, Policy::PreserveAppearance);
    let json = serde_json::to_string(&result.report)?;
    assert!(!json.contains("private-token"));
    assert!(!json.contains("ZXQ-9182"));
    assert!(!json.contains("secret"));
    Ok(())
}

#[test]
fn sanitized_text_debug_output_is_redacted() {
    let secret = "private-token-ZXQ-9182\u{200b}";
    let debug = format!("{:?}", sanitize(secret, Policy::PreserveAppearance));

    assert!(!debug.contains("private-token"));
    assert!(!debug.contains("ZXQ-9182"));
    assert!(debug.contains("text_bytes"));
}

#[test]
fn report_is_deterministic_and_accounts_for_scalar_changes() {
    let input = "a\u{200b}\u{2060}b\u{fe0f}";
    let first = sanitize(input, Policy::PreserveAppearance).report;
    let second = sanitize(input, Policy::PreserveAppearance).report;
    assert_eq!(first, second);
    assert_eq!(
        first.input_scalars - first.output_scalars,
        first.total_removed() as usize
    );
}

#[test]
fn report_totals_include_every_observed_category() {
    let report = sanitize("👩‍💻 می\u{200c}روم", Policy::PreserveAppearance).report;
    assert!(report.total_observed() >= 2);
    assert_eq!(
        report.total_observed(),
        report.observed.iter().map(|entry| entry.count).sum::<u32>()
    );
}

#[test]
fn versioned_unicode_engines_are_locked_to_unicode_17() {
    assert_eq!(unicode_normalization::UNICODE_VERSION, (17, 0, 0));
    assert_eq!(unicode_properties::UNICODE_VERSION, (17, 0, 0));
    assert_eq!(unicode_script::UNICODE_VERSION, (17, 0, 0));
    assert_eq!(emojis::UNICODE_VERSION.major(), 17);
    assert_eq!(emojis::UNICODE_VERSION.minor(), 0);
}
