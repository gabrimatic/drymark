#![allow(missing_docs)]

use drymark_core::{Category, Policy, sanitize};

#[test]
fn preserves_recognized_emoji_joiner_sequences_only() {
    for emoji in ["👩‍💻", "👨‍👩‍👧‍👦", "🏳️‍🌈", "🏴‍☠️"] {
        assert_eq!(sanitize(emoji, Policy::PreserveAppearance).text, emoji);
    }

    let orphan = sanitize("a\u{200d}b", Policy::PreserveAppearance);
    assert_eq!(orphan.text, "ab");
    assert_eq!(orphan.report.removed_count(Category::JoinControl), 1);
}

#[test]
fn preserves_contextual_joiners_in_shaping_scripts() {
    let persian = "می\u{200c}روم";
    let devanagari = "क्\u{200d}ष";
    assert_eq!(sanitize(persian, Policy::PreserveAppearance).text, persian);
    assert_eq!(
        sanitize(devanagari, Policy::PreserveAppearance).text,
        devanagari
    );

    for input in ["A\u{200c}B", "1\u{200d}2", "\u{200c}orphan", "tail\u{200d}"] {
        assert!(
            !sanitize(input, Policy::PreserveAppearance)
                .text
                .contains(['\u{200c}', '\u{200d}'])
        );
    }

    for input in [
        "क्\u{200d}",
        "क्\u{200d}A",
        "क्\u{200d}ب",
        "ب\u{200c}A",
        "A\u{200c}ب",
        "ب\u{200c}ܒ",
        "กฺ\u{200d}ข",
    ] {
        let result = sanitize(input, Policy::PreserveAppearance);
        assert!(
            !result.text.contains(['\u{200c}', '\u{200d}']),
            "input: {input:?}"
        );
        assert_eq!(result.report.removed_count(Category::JoinControl), 1);
    }
}

#[test]
fn removed_neighbors_cannot_leave_a_joiner_that_changes_on_a_second_pass() {
    let input = "क्\u{200d}\u{200b}";
    let once = sanitize(input, Policy::PreserveAppearance);
    let twice = sanitize(&once.text, Policy::PreserveAppearance);

    assert_eq!(once.text, twice.text);
    assert!(!twice.report.changed);
}

#[test]
fn preserves_valid_variation_sequences_and_drops_orphans_and_duplicates() {
    for sequence in ["❤️", "❤︎", "☕️", "*️⃣", "1️⃣"] {
        assert_eq!(
            sanitize(sequence, Policy::PreserveAppearance).text,
            sequence
        );
    }

    assert_eq!(
        sanitize("A\u{fe0f}B", Policy::PreserveAppearance).text,
        "AB"
    );
    assert_eq!(
        sanitize("❤\u{fe0f}\u{fe0f}", Policy::PreserveAppearance).text,
        "❤️"
    );
    assert_eq!(sanitize("\u{fe0f}", Policy::PreserveAppearance).text, "");
}

#[test]
fn preserves_only_registered_mongolian_and_ideographic_variation_sequences() {
    for sequence in ["\u{1820}\u{180b}", "一\u{e0100}", "葛\u{e0109}"] {
        let result = sanitize(sequence, Policy::PreserveAppearance);
        assert_eq!(result.text, sequence);
        assert_eq!(result.report.observed_count(Category::VariationSelector), 1);
    }

    let medial_mongolian = "\u{1820}\u{1820}\u{180c}\u{1820}";
    let result = sanitize(medial_mongolian, Policy::PreserveAppearance);
    assert_eq!(result.text, medial_mongolian);
    assert_eq!(result.report.observed_count(Category::VariationSelector), 1);

    for (input, expected) in [
        ("\u{1820}\u{180c}", "\u{1820}"),
        ("\u{1820}\u{180d}", "\u{1820}"),
        ("\u{1820}\u{180f}", "\u{1820}"),
        ("一\u{e0101}", "一"),
        ("葛\u{e0110}", "葛"),
        ("A\u{180b}B", "AB"),
        ("A\u{e0100}B", "AB"),
    ] {
        let result = sanitize(input, Policy::PreserveAppearance);
        assert_eq!(result.text, expected, "input: {input:?}");
        assert_eq!(result.report.removed_count(Category::VariationSelector), 1);
    }
}

#[test]
fn preserves_only_registered_standardized_and_emoji_variation_sequences() {
    for sequence in ["∩\u{fe00}", "#\u{fe0e}", "#\u{fe0f}", "\u{13012}\u{fe03}"] {
        let result = sanitize(sequence, Policy::PreserveAppearance);
        assert_eq!(result.text, sequence);
        assert_eq!(result.report.observed_count(Category::VariationSelector), 1);
    }

    for (input, expected) in [
        ("葛\u{fe00}", "葛"),
        ("∩\u{fe01}", "∩"),
        ("A\u{fe00}B", "AB"),
        ("A\u{fe0f}B", "AB"),
    ] {
        let result = sanitize(input, Policy::PreserveAppearance);
        assert_eq!(result.text, expected, "input: {input:?}");
        assert_eq!(result.report.removed_count(Category::VariationSelector), 1);
    }
}

#[test]
fn preserves_only_allowlisted_visible_format_characters() {
    let contextual = "A\u{0600}B";
    let preserve = sanitize(contextual, Policy::PreserveAppearance);
    assert_eq!(preserve.text, contextual);
    assert_eq!(preserve.report.observed_count(Category::OtherFormat), 1);

    let thorough = sanitize(contextual, Policy::Thorough);
    assert_eq!(thorough.text, "AB");
    assert_eq!(thorough.report.removed_count(Category::OtherFormat), 1);

    let unlisted = sanitize("A\u{180e}B", Policy::PreserveAppearance);
    assert_eq!(unlisted.text, "AB");
    assert_eq!(unlisted.report.removed_count(Category::DefaultIgnorable), 1);
}

#[test]
fn preserves_only_well_formed_subdivision_flag_tags() {
    let england = "🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}";
    let scotland = "🏴\u{e0067}\u{e0062}\u{e0073}\u{e0063}\u{e0074}\u{e007f}";
    let wales = "🏴\u{e0067}\u{e0062}\u{e0077}\u{e006c}\u{e0073}\u{e007f}";
    for flag in [england, scotland, wales] {
        assert_eq!(sanitize(flag, Policy::PreserveAppearance).text, flag);
    }

    let hidden_payload = "A\u{e0068}\u{e0069}\u{e007f}B";
    let invalid_flag = "🏴\u{e0067}\u{e0062}\u{e0062}\u{e0061}\u{e0064}\u{e007f}";
    assert_eq!(
        sanitize(hidden_payload, Policy::PreserveAppearance).text,
        "AB"
    );
    assert_eq!(
        sanitize(invalid_flag, Policy::PreserveAppearance).text,
        "🏴"
    );
}

#[test]
fn balanced_isolates_are_preserved_but_overrides_are_not() {
    let legitimate = "Name: \u{2067}مريم\u{2069} (admin)";
    assert_eq!(
        sanitize(legitimate, Policy::PreserveAppearance).text,
        legitimate
    );

    let unbalanced = "Name: \u{2067}مريم (admin)";
    assert_eq!(
        sanitize(unbalanced, Policy::PreserveAppearance).text,
        "Name: مريم (admin)"
    );

    let override_pair = "safe\u{202e}txt.exe\u{202c}";
    assert_eq!(
        sanitize(override_pair, Policy::PreserveAppearance).text,
        "safetxt.exe"
    );
}

#[test]
fn empty_or_cross_paragraph_isolates_are_removed() {
    for (input, expected) in [
        ("A\u{2067}\u{2069}B", "AB"),
        ("A\u{2067}\u{200b}\u{2069}B", "AB"),
        ("A\u{2067}\u{061c}\u{2069}B", "AB"),
        ("A\u{2067}\u{200e}\u{2069}B", "AB"),
        ("A\u{2067}\u{200f}\u{2069}B", "AB"),
        ("A\u{2067} \u{2069}B", "A B"),
        ("A\u{2067}RTL\ntext\u{2069}B", "ARTL\ntextB"),
        ("A\u{2067}RTL\u{001c}text\u{2069}B", "ARTLtextB"),
        ("A\u{2067}RTL\u{001d}text\u{2069}B", "ARTLtextB"),
        ("A\u{2067}RTL\u{001e}text\u{2069}B", "ARTLtextB"),
        ("A\u{2067}RTL\u{0085}text\u{2069}B", "ARTLtextB"),
        ("A\u{2067}\u{2066}\u{2069}\u{2069}B", "AB"),
    ] {
        assert_eq!(
            sanitize(input, Policy::PreserveAppearance).text,
            expected,
            "input: {input:?}"
        );
    }
}

#[test]
fn thorough_removes_all_contextual_format_controls() {
    let input = "👩‍💻 ❤️ می\u{200c}روم \u{2067}עברית\u{2069}";
    let result = sanitize(input, Policy::Thorough);
    for forbidden in [
        '\u{200c}', '\u{200d}', '\u{fe0e}', '\u{fe0f}', '\u{2066}', '\u{2067}', '\u{2068}',
        '\u{2069}',
    ] {
        assert!(!result.text.contains(forbidden), "left {forbidden:?}");
    }
}
