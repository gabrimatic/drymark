#![no_main]

use libfuzzer_sys::fuzz_target;
use drymark_core::{Policy, sanitize};

fuzz_target!(|bytes: &[u8]| {
    let input = String::from_utf8_lossy(bytes);

    for policy in [Policy::PreserveAppearance, Policy::Thorough] {
        let once = sanitize(input.as_ref(), policy);
        let twice = sanitize(&once.text, policy);

        assert_eq!(twice.text, once.text);
        assert!(!twice.report.changed);
        assert_eq!(once.report.changed, once.text != input.as_ref());
        assert_eq!(once.report.input_bytes, input.len());
        assert_eq!(once.report.output_bytes, once.text.len());
        assert_eq!(once.report.input_scalars, input.chars().count());
        assert_eq!(once.report.output_scalars, once.text.chars().count());
        assert!(once.text.len() <= input.len().saturating_mul(4));
    }
});
