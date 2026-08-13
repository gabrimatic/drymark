#![allow(missing_docs)]

use drymark_core::{Policy, sanitize};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn sanitation_is_idempotent(input in any::<String>(), thorough in any::<bool>()) {
        let policy = if thorough { Policy::Thorough } else { Policy::PreserveAppearance };
        let once = sanitize(&input, policy);
        let twice = sanitize(&once.text, policy);
        prop_assert_eq!(&twice.text, &once.text);
        prop_assert!(!twice.report.changed);
    }

    #[test]
    fn output_expansion_is_bounded_and_reports_match(input in any::<String>(), thorough in any::<bool>()) {
        let policy = if thorough { Policy::Thorough } else { Policy::PreserveAppearance };
        let result = sanitize(&input, policy);
        prop_assert!(result.text.len() <= input.len().saturating_mul(4));
        prop_assert_eq!(result.report.input_bytes, input.len());
        prop_assert_eq!(result.report.output_bytes, result.text.len());
        prop_assert_eq!(result.report.changed, result.text != input);
    }

    #[test]
    fn printable_ascii_is_identity(input in "[ -~\\t\\n\\r]{0,4096}") {
        let result = sanitize(&input, Policy::PreserveAppearance);
        prop_assert_eq!(&result.text, &input);
        prop_assert!(!result.report.changed);
    }
}

#[test]
fn very_large_adversarial_input_remains_linear_in_size() {
    let input = "A\u{200b}👩‍💻\u{202e}B\u{202c}\n".repeat(250_000);
    let result = sanitize(&input, Policy::PreserveAppearance);
    assert!(result.text.len() <= input.len());
    assert_eq!(result.report.input_bytes, input.len());
}

#[test]
fn contextual_control_neighborhoods_are_exhaustively_idempotent() {
    let fragments = [
        "",
        "A",
        "ب",
        "क",
        "\u{094d}",
        "👩",
        "💻",
        "\u{061c}",
        "\u{200b}",
        "\u{200c}",
        "\u{200d}",
        "\u{2067}",
        "\u{2069}",
        "\u{fe0f}",
        "\u{e0067}",
        "\u{e007f}",
    ];

    for first in fragments {
        for second in fragments {
            for third in fragments {
                for fourth in fragments {
                    let input = format!("{first}{second}{third}{fourth}");
                    for policy in [Policy::PreserveAppearance, Policy::Thorough] {
                        let once = sanitize(&input, policy);
                        let twice = sanitize(&once.text, policy);
                        assert_eq!(
                            once.text, twice.text,
                            "input: {input:?}; policy: {policy:?}"
                        );
                        assert!(
                            !twice.report.changed,
                            "input: {input:?}; policy: {policy:?}"
                        );
                    }
                }
            }
        }
    }
}
