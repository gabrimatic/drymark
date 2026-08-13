#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

use drymark_core::{Category, Policy, sanitize};

const STANDARDIZED: &str = include_str!("../data/unicode-17/StandardizedVariants.txt");
const EMOJI: &str = include_str!("../data/unicode-17/emoji-variation-sequences.txt");
const IDEOGRAPHIC: &str = include_str!("../data/unicode-17/IVD_Sequences-2025-07-14.txt");
const CONTEXT_ISOLATE: u8 = 1;
const CONTEXT_INITIAL: u8 = 2;
const CONTEXT_MEDIAL: u8 = 4;
const CONTEXT_FINAL: u8 = 8;

#[test]
fn honors_every_unicode_registered_pair_and_its_standalone_context() {
    let registry = registry();
    assert_eq!(registry.len(), 31_730);

    for ((base, selector), context) in registry {
        let input = String::from_iter([base, selector]);

        let preserve = sanitize(&input, Policy::PreserveAppearance);
        if context == 0 || context & CONTEXT_ISOLATE != 0 {
            assert_eq!(
                preserve.text,
                input,
                "sequence: U+{:04X} U+{:04X}",
                u32::from(base),
                u32::from(selector)
            );
            assert_eq!(
                preserve.report.observed_count(Category::VariationSelector),
                1
            );
        } else {
            assert_eq!(preserve.text, base.to_string());
            assert_eq!(
                preserve.report.removed_count(Category::VariationSelector),
                1
            );
        }

        let thorough = sanitize(&input, Policy::Thorough);
        assert_eq!(thorough.text, base.to_string());
        assert_eq!(
            thorough.report.removed_count(Category::VariationSelector),
            1
        );
    }
}

#[test]
fn preserves_all_63_context_qualified_standardized_variants_only_in_allowed_shapes() {
    let records = STANDARDIZED
        .lines()
        .filter_map(|line| parse_record(line, true))
        .filter(|(_, context)| *context != 0)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 63);

    for ((base, selector), contexts) in records {
        let connector = if u32::from(base) >= 0x10AC0 {
            '\u{10ac0}'
        } else {
            '\u{1820}'
        };
        for (context, scalars) in [
            (CONTEXT_ISOLATE, vec![base, selector]),
            (CONTEXT_INITIAL, vec![base, selector, connector]),
            (CONTEXT_MEDIAL, vec![connector, base, selector, connector]),
            (CONTEXT_FINAL, vec![connector, base, selector]),
        ] {
            if contexts & context == 0 {
                continue;
            }
            let input = String::from_iter(scalars);
            let result = sanitize(&input, Policy::PreserveAppearance);
            assert_eq!(
                result.text,
                input,
                "context {context}; U+{:04X} U+{:04X}",
                u32::from(base),
                u32::from(selector)
            );
        }
    }
}

#[test]
fn rejects_every_unregistered_selector_for_every_registered_base() {
    let registry = registry();
    let bases = registry
        .iter()
        .map(|((base, _), _)| *base)
        .collect::<BTreeSet<_>>();
    assert_eq!(bases.len(), 16_053);
    let selectors = variation_selectors();
    let expected_invalid_pairs = bases.len() * selectors.len() - registry.len();
    let mut checked = 0_usize;

    for base in bases {
        for selector in selectors
            .iter()
            .copied()
            .filter(|selector| !registry.contains_key(&(base, *selector)))
        {
            let input = String::from_iter([base, selector]);
            let result = sanitize(&input, Policy::PreserveAppearance);
            assert_eq!(
                result.text,
                base.to_string(),
                "sequence: U+{:04X} U+{:04X}",
                u32::from(base),
                u32::from(selector)
            );
            assert_eq!(result.report.removed_count(Category::VariationSelector), 1);
            checked += 1;
        }
    }
    assert_eq!(checked, expected_invalid_pairs);
}

fn registry() -> BTreeMap<(char, char), u8> {
    let mut registry = BTreeMap::new();
    for (source, contextual) in [(STANDARDIZED, true), (EMOJI, false), (IDEOGRAPHIC, false)] {
        for (pair, context) in source
            .lines()
            .filter_map(|line| parse_record(line, contextual))
        {
            registry
                .entry(pair)
                .and_modify(|existing| {
                    *existing = if *existing == 0 || context == 0 {
                        0
                    } else {
                        *existing | context
                    };
                })
                .or_insert(context);
        }
    }
    registry
}

fn parse_record(line: &str, contextual: bool) -> Option<((char, char), u8)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let data = trimmed
        .split_once('#')
        .map_or(trimmed, |(value, _)| value)
        .trim();
    let fields = data.split(';').map(str::trim).collect::<Vec<_>>();
    let sequence = fields
        .first()
        .copied()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>();
    assert_eq!(sequence.len(), 2, "record: {line}");
    if sequence.len() != 2 {
        return None;
    }

    let base = u32::from_str_radix(sequence[0], 16)
        .ok()
        .and_then(char::from_u32);
    let selector = u32::from_str_radix(sequence[1], 16)
        .ok()
        .and_then(char::from_u32);
    assert!(base.is_some() && selector.is_some(), "record: {line}");
    let context = if contextual {
        fields
            .get(2)
            .copied()
            .unwrap_or_default()
            .split_whitespace()
            .fold(0, |mask, value| {
                let bit = match value {
                    "isolate" => CONTEXT_ISOLATE,
                    "initial" => CONTEXT_INITIAL,
                    "medial" => CONTEXT_MEDIAL,
                    "final" => CONTEXT_FINAL,
                    _ => 0,
                };
                assert_ne!(bit, 0, "invalid shaping context in record: {line}");
                mask | bit
            })
    } else {
        0
    };
    base.zip(selector).map(|pair| (pair, context))
}

fn variation_selectors() -> Vec<char> {
    (0x180B..=0x180D)
        .chain([0x180F])
        .chain(0xFE00..=0xFE0F)
        .chain(0xE0100..=0xE01EF)
        .filter_map(char::from_u32)
        .collect()
}
