#![allow(missing_docs)]

use drymark_core::{Policy, sanitize};

const EMOJI_TEST: &str = include_str!("../data/unicode-17/emoji-test.txt");

#[test]
fn preserves_every_fully_qualified_unicode_17_emoji() -> Result<(), Box<dyn std::error::Error>> {
    let sequences = fully_qualified_sequences()?;
    assert_eq!(sequences.len(), 3_944);

    for sequence in sequences {
        assert!(
            emojis::get(&sequence).is_some(),
            "emoji registry does not recognize {sequence:?}"
        );

        let once = sanitize(&sequence, Policy::PreserveAppearance);
        assert_eq!(once.text, sequence, "changed registered emoji {sequence:?}");
        assert!(!once.report.changed, "reported a change for {sequence:?}");

        let twice = sanitize(&once.text, Policy::PreserveAppearance);
        assert_eq!(twice.text, sequence, "not idempotent for {sequence:?}");
        assert!(!twice.report.changed, "second pass changed {sequence:?}");
    }
    Ok(())
}

#[test]
fn thorough_removes_every_hidden_scalar_from_registered_emoji()
-> Result<(), Box<dyn std::error::Error>> {
    for sequence in fully_qualified_sequences()? {
        let result = sanitize(&sequence, Policy::Thorough);
        assert!(
            !result.text.chars().any(is_hidden_emoji_scalar),
            "left a hidden scalar in {sequence:?}: {:?}",
            result.text
        );
    }
    Ok(())
}

#[test]
fn emoji_matcher_bound_covers_the_complete_unicode_17_registry()
-> Result<(), Box<dyn std::error::Error>> {
    let maximum = fully_qualified_sequences()?
        .into_iter()
        .map(|sequence| sequence.chars().count())
        .max();

    assert!(maximum.is_some_and(|count| count <= 36));
    Ok(())
}

#[test]
fn preserves_every_sensitive_emoji_across_all_registered_qualification_levels()
-> Result<(), Box<dyn std::error::Error>> {
    let sequences = sensitive_sequences()?;
    assert_eq!(sequences.len(), 2_670);

    for sequence in sequences {
        let result = sanitize(&sequence, Policy::PreserveAppearance);
        assert_eq!(
            result.text, sequence,
            "changed registered emoji {sequence:?}"
        );
        assert!(!result.report.changed, "reported a change for {sequence:?}");
    }
    Ok(())
}

fn fully_qualified_sequences() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut sequences = Vec::new();
    for line in EMOJI_TEST.lines() {
        let data = line.split_once('#').map_or(line, |(value, _)| value).trim();
        let Some((scalars, status)) = data.split_once(';') else {
            continue;
        };
        if status.trim() != "fully-qualified" {
            continue;
        }

        let mut sequence = String::new();
        for value in scalars.split_whitespace() {
            let scalar = u32::from_str_radix(value, 16)?;
            let character = char::from_u32(scalar).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid Unicode scalar U+{scalar:04X}"),
                )
            })?;
            sequence.push(character);
        }
        sequences.push(sequence);
    }
    Ok(sequences)
}

fn sensitive_sequences() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut sequences = Vec::new();
    for line in EMOJI_TEST.lines() {
        let data = line.split_once('#').map_or(line, |(value, _)| value).trim();
        let Some((scalars, status)) = data.split_once(';') else {
            continue;
        };
        if !matches!(
            status.trim(),
            "fully-qualified" | "minimally-qualified" | "unqualified"
        ) {
            continue;
        }

        let mut sequence = String::new();
        let mut sensitive = false;
        for value in scalars.split_whitespace() {
            let scalar = u32::from_str_radix(value, 16)?;
            sensitive |= scalar == 0x200D || matches!(scalar, 0xE0001 | 0xE0020..=0xE007F);
            let character = char::from_u32(scalar).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid Unicode scalar U+{scalar:04X}"),
                )
            })?;
            sequence.push(character);
        }
        if sensitive {
            sequences.push(sequence);
        }
    }
    Ok(sequences)
}

fn is_hidden_emoji_scalar(character: char) -> bool {
    matches!(
        character,
        '\u{200d}'
            | '\u{180b}'..='\u{180d}'
            | '\u{180f}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
            | '\u{e0100}'..='\u{e01ef}'
    )
}
