#![allow(missing_docs)]

use drymark_core::{Category, Policy, sanitize};
use unicode_normalization::UnicodeNormalization;

#[test]
fn preserve_removes_unambiguous_invisible_channels() {
    let cases = [
        ("zero\u{200b}width", "zerowidth", Category::ZeroWidth),
        ("word\u{2060}joiner", "wordjoiner", Category::ZeroWidth),
        ("byte\u{feff}order", "byteorder", Category::ZeroWidth),
        ("soft\u{00ad}hyphen", "softhyphen", Category::SoftHyphen),
        (
            "a\u{2062}b\u{2063}c\u{2064}d",
            "abcd",
            Category::InvisibleOperator,
        ),
        ("a\u{034f}b", "ab", Category::CombiningGraphemeJoiner),
        ("a\u{fffc}b", "ab", Category::ObjectReplacement),
        (
            "a\u{fff9}x\u{fffa}y\u{fffb}b",
            "axyb",
            Category::AnnotationControl,
        ),
    ];

    for (input, expected, category) in cases {
        let result = sanitize(input, Policy::PreserveAppearance);
        assert_eq!(result.text, expected, "input: {input:?}");
        assert!(result.report.removed_count(category) > 0);
    }
}

#[test]
fn removes_opaque_256_bit_payloads_when_the_hidden_carrier_is_supported() {
    let digest = [
        0x9f_u8, 0x86, 0xd0, 0x81, 0x88, 0x4c, 0x7d, 0x65, 0x9a, 0x2f, 0xea, 0xa0, 0xc5, 0x5a,
        0xd0, 0x15, 0xa3, 0xbf, 0x4f, 0x1b, 0x2b, 0x0b, 0x82, 0x2c, 0xd1, 0x5d, 0x6c, 0x15, 0xb0,
        0xf0, 0x0a, 0x08,
    ];
    let payload = digest
        .iter()
        .flat_map(|byte| {
            (0..8).rev().map(move |shift| {
                if byte & (1 << shift) == 0 {
                    '\u{200b}'
                } else {
                    '\u{2060}'
                }
            })
        })
        .collect::<String>();
    let input = format!("visible{payload} text");

    let result = sanitize(&input, Policy::PreserveAppearance);
    assert_eq!(result.text, "visible text");
    assert_eq!(result.report.total_removed(), 256);

    let second = sanitize(&result.text, Policy::PreserveAppearance);
    assert_eq!(second.text, result.text);
    assert!(!second.report.changed);
}

#[test]
fn removes_every_known_invisible_filler() {
    for filler in [
        '\u{115f}', '\u{1160}', '\u{17b4}', '\u{17b5}', '\u{3164}', '\u{ffa0}',
    ] {
        let input = format!("A{filler}B");
        for policy in [Policy::PreserveAppearance, Policy::Thorough] {
            let result = sanitize(&input, policy);
            assert_eq!(result.text, "AB", "left filler {filler:?}");
            assert_eq!(result.report.removed_count(Category::Filler), 1);
        }
    }
}

#[test]
fn every_unicode_17_default_ignorable_is_removed_when_standalone() {
    let ranges = [
        (0x00ad, 0x00ad),
        (0x034f, 0x034f),
        (0x061c, 0x061c),
        (0x115f, 0x1160),
        (0x17b4, 0x17b5),
        (0x180b, 0x180e),
        (0x180f, 0x180f),
        (0x200b, 0x200f),
        (0x202a, 0x202e),
        (0x2060, 0x206f),
        (0x3164, 0x3164),
        (0xfe00, 0xfe0f),
        (0xfeff, 0xfeff),
        (0xffa0, 0xffa0),
        (0xfff0, 0xfff8),
        (0x1bca0, 0x1bca3),
        (0x1d173, 0x1d17a),
        (0xe0000, 0xe0fff),
    ];

    for (start, end) in ranges {
        for value in start..=end {
            let Some(character) = char::from_u32(value) else {
                continue;
            };
            let input = character.to_string();
            for policy in [Policy::PreserveAppearance, Policy::Thorough] {
                assert_eq!(
                    sanitize(&input, policy).text,
                    "",
                    "left U+{value:04X} under {policy:?}"
                );
            }
        }
    }
}

#[test]
fn default_ignorables_have_a_distinct_report_category() {
    let input = "A\u{2065}\u{fff0}\u{e0000}\u{e0080}\u{e01f0}B";
    let result = sanitize(input, Policy::PreserveAppearance);
    assert_eq!(result.text, "AB");
    assert_eq!(result.report.removed_count(Category::DefaultIgnorable), 5);
}

#[test]
fn removes_controls_but_keeps_plain_text_line_structure() {
    let input = "a\0b\u{0007}c\td\ne\rf\u{0085}g";
    let result = sanitize(input, Policy::PreserveAppearance);
    assert_eq!(result.text, "abc\td\ne\rfg");
    assert_eq!(result.report.removed_count(Category::Control), 3);
}

#[test]
fn removes_noncharacters_in_both_policies() {
    let input = "a\u{fdd0}b\u{fffe}c\u{1ffff}d\u{10ffff}e";
    for policy in [Policy::PreserveAppearance, Policy::Thorough] {
        let result = sanitize(input, policy);
        assert_eq!(result.text, "abcde");
        assert_eq!(result.report.removed_count(Category::Noncharacter), 4);
    }
}

#[test]
fn preserve_reports_private_use_but_thorough_removes_it() {
    let input = "a\u{e000}b\u{f0000}c\u{100000}d";
    let preserve = sanitize(input, Policy::PreserveAppearance);
    assert_eq!(preserve.text, input);
    assert_eq!(preserve.report.observed_count(Category::PrivateUse), 3);

    let thorough = sanitize(input, Policy::Thorough);
    assert_eq!(thorough.text, "abcd");
    assert_eq!(thorough.report.removed_count(Category::PrivateUse), 3);
}

#[test]
fn removes_deprecated_and_override_bidi_controls() {
    let input = "left\u{202e}abc\u{202c}right\u{206a}end";
    let result = sanitize(input, Policy::PreserveAppearance);
    assert_eq!(result.text, "leftabcrightend");
    assert_eq!(result.report.removed_count(Category::BidiControl), 3);
}

#[test]
fn thorough_canonicalizes_unicode_whitespace_and_line_endings() {
    let input = "Cafe\u{301}\r\nA\u{00a0}B\u{2007}C\u{202f}D\rE  \n";
    let result = sanitize(input, Policy::Thorough);
    assert_eq!(result.text, "Café\nA B C D\nE\n");
    assert!(result.report.normalized);
    assert!(result.report.canonicalized_whitespace > 0);
}

#[test]
fn thorough_whitespace_report_counts_only_real_changes() {
    let unchanged = sanitize("A\nB C", Policy::Thorough);
    assert_eq!(unchanged.text, "A\nB C");
    assert_eq!(unchanged.report.canonicalized_whitespace, 0);

    let changed = sanitize("A\u{2028}B\u{00a0}C", Policy::Thorough);
    assert_eq!(changed.text, "A\nB C");
    assert_eq!(changed.report.canonicalized_whitespace, 2);
}

#[test]
fn unchanged_plain_text_has_an_empty_report() {
    let input = "Plain text — Ελληνικά — 日本語\nSecond line.";
    let result = sanitize(input, Policy::PreserveAppearance);
    assert_eq!(result.text, input);
    assert!(!result.report.changed);
    assert_eq!(result.report.total_removed(), 0);
    assert_eq!(result.report.input_bytes, result.report.output_bytes);
}

#[test]
fn thorough_normalization_handles_composition_exclusions() {
    let input = "\u{0a59}";
    let result = sanitize(input, Policy::Thorough);
    assert_eq!(result.text.nfc().collect::<String>(), result.text);
    assert!(result.text.len() <= input.len() * 4);
}
