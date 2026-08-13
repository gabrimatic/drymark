//! Deterministic, local-first Unicode text sanitation.
//!
//! The default policy removes unambiguous hidden channels while protecting
//! recognized emoji, shaping-script joiners, variation sequences, and balanced
//! bidirectional isolates. The thorough policy deliberately removes every
//! format/default-ignorable channel and canonicalizes text for comparison.

use std::{collections::BTreeMap, fmt, num::NonZeroUsize};

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;
use unicode_script::UnicodeScript;
use zeroize::{Zeroize, Zeroizing};

const VARIATION_SEQUENCE_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/variation-sequences-v17.bin"));
const VARIATION_SEQUENCE_WIDTH: usize = 9;
const VARIATION_CONTEXT_ISOLATE: u8 = 1;
const VARIATION_CONTEXT_INITIAL: u8 = 2;
const VARIATION_CONTEXT_MEDIAL: u8 = 4;
const VARIATION_CONTEXT_FINAL: u8 = 8;
const GENERAL_CATEGORY_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/general-category-v17.bin"));
const JOINING_TYPE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/joining-type-v17.bin"));
const DEFAULT_IGNORABLE_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/default-ignorable-v17.bin"));
const INCB_LINKER_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/incb-linker-v17.bin"));
const BIDI_CLASS_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bidi-class-v17.bin"));
const EMOJI_AUTOMATON_NODES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/emoji-automaton-nodes-v17.bin"));
const EMOJI_AUTOMATON_EDGES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/emoji-automaton-edges-v17.bin"));
const EMOJI_NODE_WIDTH: usize = 13;
const EMOJI_EDGE_WIDTH: usize = 8;
const MAX_EMOJI_SEQUENCE_SCALARS: usize = 36;
const RANGE_RECORD_WIDTH: usize = 9;

/// The sanitation policy applied to input text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    /// Remove unambiguous channels while preserving contextual text behavior.
    PreserveAppearance,
    /// Remove all format channels and canonicalize text for comparison.
    Thorough,
}

/// A stable category used by privacy-preserving sanitation reports.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// C0/C1 controls other than supported line structure.
    Control,
    /// Zero-width separator, word joiner, or byte-order mark.
    ZeroWidth,
    /// Discretionary soft hyphen.
    SoftHyphen,
    /// Invisible mathematical separator/operator controls.
    InvisibleOperator,
    /// Combining grapheme joiner.
    CombiningGraphemeJoiner,
    /// Bidirectional marks, embeddings, overrides, or isolates.
    BidiControl,
    /// Zero-width joiner or non-joiner.
    JoinControl,
    /// Standard, emoji, Mongolian, or ideographic variation selector.
    VariationSelector,
    /// Unicode tag character or language tag.
    Tag,
    /// Unicode interlinear annotation control.
    AnnotationControl,
    /// Object replacement marker.
    ObjectReplacement,
    /// Reserved noncharacter.
    Noncharacter,
    /// Private-use scalar.
    PrivateUse,
    /// Invisible Hangul or Khmer filler.
    Filler,
    /// Scalar with the Unicode `Default_Ignorable_Code_Point` property not
    /// represented by a more specific category above.
    DefaultIgnorable,
    /// Another Unicode format control.
    OtherFormat,
    /// Whitespace or line-ending canonicalization.
    Whitespace,
}

/// Count for one report category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CategoryCount {
    /// Category represented by this entry.
    pub category: Category,
    /// Number of Unicode scalars represented by this entry.
    pub count: u32,
}

/// Metadata-only description of a sanitation pass.
///
/// The report intentionally cannot contain input excerpts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SanitizeReport {
    /// Whether output differs from input.
    pub changed: bool,
    /// UTF-8 bytes in the input.
    pub input_bytes: usize,
    /// UTF-8 bytes in the output.
    pub output_bytes: usize,
    /// Unicode scalar count in the input.
    pub input_scalars: usize,
    /// Unicode scalar count in the output.
    pub output_scalars: usize,
    /// Removed scalar counts, sorted by stable category.
    pub removed: Vec<CategoryCount>,
    /// Suspicious/contextual scalars observed but preserved.
    pub observed: Vec<CategoryCount>,
    /// Whether canonical Unicode normalization changed the intermediate text.
    pub normalized: bool,
    /// Number of whitespace/line-ending canonicalizations.
    pub canonicalized_whitespace: u32,
}

impl SanitizeReport {
    /// Return the number of removed scalars in a category.
    #[must_use]
    pub fn removed_count(&self, category: Category) -> u32 {
        count_for(&self.removed, category)
    }

    /// Return the number of observed-but-preserved scalars in a category.
    #[must_use]
    pub fn observed_count(&self, category: Category) -> u32 {
        count_for(&self.observed, category)
    }

    /// Return the total number of removed scalars.
    #[must_use]
    pub fn total_removed(&self) -> u32 {
        self.removed.iter().map(|entry| entry.count).sum()
    }

    /// Return the total number of suspicious/contextual scalars preserved.
    #[must_use]
    pub fn total_observed(&self) -> u32 {
        self.observed.iter().map(|entry| entry.count).sum()
    }
}

/// Sanitized text and its metadata-only report.
#[derive(Clone, Eq, PartialEq)]
pub struct SanitizedText {
    /// Sanitized UTF-8 text.
    pub text: String,
    /// Metadata-only mutation report.
    pub report: SanitizeReport,
}

impl fmt::Debug for SanitizedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SanitizedText")
            .field("text", &"[REDACTED]")
            .field("text_bytes", &self.text.len())
            .field("text_scalars", &self.report.output_scalars)
            .field("report", &self.report)
            .finish()
    }
}

/// Sanitize valid UTF-8 text under the selected policy.
///
/// This operation is deterministic, linear in the input length up to a small
/// bounded emoji-sequence look-ahead, and does not perform I/O.
#[must_use]
pub fn sanitize(input: &str, policy: Policy) -> SanitizedText {
    let chars = Zeroizing::new(input.chars().collect::<Vec<char>>());
    let preserve_context = policy == Policy::PreserveAppearance;
    let emoji_protected = preserve_context.then(|| emoji_protected_positions(&chars));
    let isolate_protected = preserve_context.then(|| balanced_isolate_positions(&chars));
    let mut output = String::with_capacity(input.len());
    let mut removed = BTreeMap::new();
    let mut observed = BTreeMap::new();

    for (index, character) in chars.iter().copied().enumerate() {
        match classify(
            &chars,
            index,
            character,
            policy,
            emoji_protected
                .as_ref()
                .is_some_and(|protected| protected[index]),
            isolate_protected
                .as_ref()
                .is_some_and(|protected| protected[index]),
        ) {
            Decision::Keep => output.push(character),
            Decision::Observe(category) => {
                increment(&mut observed, category);
                output.push(character);
            }
            Decision::Remove(category) => increment(&mut removed, category),
        }
    }

    let mut normalized = false;
    let mut canonicalized_whitespace = 0;
    if policy == Policy::Thorough {
        let (canonical, count) = canonicalize_whitespace(&output);
        output.zeroize();
        output = canonical;
        canonicalized_whitespace = count;

        let nfc: String = output.nfc().collect();
        normalized = nfc != output;
        output.zeroize();
        output = nfc;
    }

    let report = SanitizeReport {
        changed: output != input,
        input_bytes: input.len(),
        output_bytes: output.len(),
        input_scalars: chars.len(),
        output_scalars: output.chars().count(),
        removed: map_counts(removed),
        observed: map_counts(observed),
        normalized,
        canonicalized_whitespace,
    };

    SanitizedText {
        text: output,
        report,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    Keep,
    Observe(Category),
    Remove(Category),
}

#[allow(clippy::too_many_arguments)]
fn classify(
    chars: &[char],
    index: usize,
    character: char,
    policy: Policy,
    emoji_protected: bool,
    isolate_protected: bool,
) -> Decision {
    if is_noncharacter(character) {
        return Decision::Remove(Category::Noncharacter);
    }

    if general_kind(character) == GeneralKind::PrivateUse {
        return if policy == Policy::Thorough {
            Decision::Remove(Category::PrivateUse)
        } else {
            Decision::Observe(Category::PrivateUse)
        };
    }

    if is_control(character) {
        return if matches!(character, '\t' | '\n' | '\r') {
            Decision::Keep
        } else {
            Decision::Remove(Category::Control)
        };
    }

    match character {
        '\u{00ad}' => return Decision::Remove(Category::SoftHyphen),
        '\u{034f}' => return Decision::Remove(Category::CombiningGraphemeJoiner),
        '\u{200b}' | '\u{2060}' | '\u{feff}' => {
            return Decision::Remove(Category::ZeroWidth);
        }
        '\u{2061}'..='\u{2064}' => return Decision::Remove(Category::InvisibleOperator),
        '\u{fffc}' => return Decision::Remove(Category::ObjectReplacement),
        '\u{fff9}'..='\u{fffb}' => return Decision::Remove(Category::AnnotationControl),
        '\u{115f}' | '\u{1160}' | '\u{17b4}' | '\u{17b5}' | '\u{3164}' | '\u{ffa0}' => {
            return Decision::Remove(Category::Filler);
        }
        _ => {}
    }

    if is_tag(character) {
        return if policy == Policy::PreserveAppearance && emoji_protected {
            Decision::Observe(Category::Tag)
        } else {
            Decision::Remove(Category::Tag)
        };
    }

    if is_variation_selector(character) {
        return if policy == Policy::PreserveAppearance && valid_variation_selector(chars, index) {
            Decision::Observe(Category::VariationSelector)
        } else {
            Decision::Remove(Category::VariationSelector)
        };
    }

    if matches!(character, '\u{200c}' | '\u{200d}') {
        return if policy == Policy::PreserveAppearance
            && (emoji_protected || valid_join_control(chars, index))
        {
            Decision::Observe(Category::JoinControl)
        } else {
            Decision::Remove(Category::JoinControl)
        };
    }

    if is_bidi_control(character) {
        return if policy == Policy::PreserveAppearance && isolate_protected {
            Decision::Observe(Category::BidiControl)
        } else {
            Decision::Remove(Category::BidiControl)
        };
    }

    if is_default_ignorable(character) {
        return Decision::Remove(Category::DefaultIgnorable);
    }

    if general_kind(character) == GeneralKind::Format {
        return if policy == Policy::PreserveAppearance && is_visible_contextual_format(character) {
            Decision::Observe(Category::OtherFormat)
        } else {
            Decision::Remove(Category::OtherFormat)
        };
    }

    Decision::Keep
}

fn is_control(character: char) -> bool {
    general_kind(character) == GeneralKind::Control
}

fn is_noncharacter(character: char) -> bool {
    let value = u32::from(character);
    matches!(value, 0xFDD0..=0xFDEF) || value & 0xFFFE == 0xFFFE
}

fn is_tag(character: char) -> bool {
    matches!(character, '\u{e0001}' | '\u{e0020}'..='\u{e007f}')
}

fn is_variation_selector(character: char) -> bool {
    matches!(
        character,
        '\u{180b}'..='\u{180d}'
            | '\u{180f}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{e0100}'..='\u{e01ef}'
    )
}

fn is_default_ignorable(character: char) -> bool {
    lookup_range_value(DEFAULT_IGNORABLE_BYTES, character).is_some()
}

fn valid_variation_selector(chars: &[char], index: usize) -> bool {
    let Some(previous) = index
        .checked_sub(1)
        .and_then(|value| chars.get(value))
        .copied()
    else {
        return false;
    };
    if is_variation_selector(previous) {
        return false;
    }

    let Some(context_mask) = registered_variation_context(previous, chars[index]) else {
        return false;
    };
    context_mask == 0 || context_mask & shaping_context(chars, index - 1, index) != 0
}

fn registered_variation_context(base: char, selector: char) -> Option<u8> {
    let mut target = [0_u8; 8];
    target[..4].copy_from_slice(&u32::from(base).to_be_bytes());
    target[4..].copy_from_slice(&u32::from(selector).to_be_bytes());
    let mut low = 0;
    let mut high = VARIATION_SEQUENCE_BYTES.len() / VARIATION_SEQUENCE_WIDTH;

    // The fixed iteration budget makes malformed generated tables fail closed
    // instead of allowing an accidental non-progressing search.
    for _ in 0..usize::BITS {
        if low >= high {
            break;
        }
        let middle = low + (high - low) / 2;
        let candidate = variation_sequence_at(middle);
        if candidate < target {
            low = middle.checked_add(1)?;
        } else {
            high = middle;
        }
    }

    (low < VARIATION_SEQUENCE_BYTES.len() / VARIATION_SEQUENCE_WIDTH
        && variation_sequence_at(low) == target)
        .then(|| variation_context_at(low))
}

fn variation_sequence_at(index: usize) -> [u8; 8] {
    let offset = index * VARIATION_SEQUENCE_WIDTH;
    [
        VARIATION_SEQUENCE_BYTES[offset],
        VARIATION_SEQUENCE_BYTES[offset + 1],
        VARIATION_SEQUENCE_BYTES[offset + 2],
        VARIATION_SEQUENCE_BYTES[offset + 3],
        VARIATION_SEQUENCE_BYTES[offset + 4],
        VARIATION_SEQUENCE_BYTES[offset + 5],
        VARIATION_SEQUENCE_BYTES[offset + 6],
        VARIATION_SEQUENCE_BYTES[offset + 7],
    ]
}

fn variation_context_at(index: usize) -> u8 {
    VARIATION_SEQUENCE_BYTES[index * VARIATION_SEQUENCE_WIDTH + 8]
}

fn shaping_context(chars: &[char], base_index: usize, selector_index: usize) -> u8 {
    let base = chars[base_index];
    let base_kind = joining_kind(base);
    let previous = chars[..base_index]
        .iter()
        .rev()
        .copied()
        .find(|candidate| joining_kind(*candidate) != JoiningKind::Transparent);
    // The selector itself has transparent joining type, so including it avoids
    // index arithmetic without changing the first meaningful neighbor.
    let next = chars[selector_index..]
        .iter()
        .copied()
        .find(|candidate| joining_kind(*candidate) != JoiningKind::Transparent);

    let joins_previous = previous.is_some_and(|candidate| {
        scripts_can_shape_together(base, candidate)
            && joins_following(joining_kind(candidate))
            && joins_preceding(base_kind)
    });
    let joins_next = next.is_some_and(|candidate| {
        scripts_can_shape_together(base, candidate)
            && joins_following(base_kind)
            && joins_preceding(joining_kind(candidate))
    });

    match (joins_previous, joins_next) {
        (false, false) => VARIATION_CONTEXT_ISOLATE,
        (false, true) => VARIATION_CONTEXT_INITIAL,
        (true, true) => VARIATION_CONTEXT_MEDIAL,
        (true, false) => VARIATION_CONTEXT_FINAL,
    }
}

fn joins_following(kind: JoiningKind) -> bool {
    matches!(
        kind,
        JoiningKind::Dual | JoiningKind::Left | JoiningKind::JoinCausing
    )
}

fn joins_preceding(kind: JoiningKind) -> bool {
    matches!(
        kind,
        JoiningKind::Dual | JoiningKind::Right | JoiningKind::JoinCausing
    )
}

fn scripts_can_shape_together(base: char, neighbor: char) -> bool {
    joining_kind(neighbor) == JoiningKind::JoinCausing
        || !base
            .script_extension()
            .intersection(neighbor.script_extension())
            .is_empty()
}

fn valid_join_control(chars: &[char], index: usize) -> bool {
    let Some(immediate_previous) = index
        .checked_sub(1)
        .and_then(|previous| chars.get(previous))
        .copied()
    else {
        return false;
    };

    if is_incb_linker(immediate_previous) {
        return valid_virama_join_context(chars, index);
    }

    let left = chars[..index]
        .iter()
        .rev()
        .copied()
        .find(|candidate| joining_kind(*candidate) != JoiningKind::Transparent);
    let right = chars[index + 1..]
        .iter()
        .copied()
        .find(|candidate| joining_kind(*candidate) != JoiningKind::Transparent);

    if left.is_some_and(is_join_control) || right.is_some_and(is_join_control) {
        return false;
    }

    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    scripts_can_shape_together(left, right)
        && joins_following(joining_kind(left))
        && joins_preceding(joining_kind(right))
}

fn is_join_control(character: char) -> bool {
    matches!(character, '\u{200c}' | '\u{200d}')
}

fn is_incb_linker(character: char) -> bool {
    lookup_range_value(INCB_LINKER_BYTES, character).is_some()
}

fn valid_virama_join_context(chars: &[char], index: usize) -> bool {
    let Some(next) = chars
        .get(index + 1)
        .copied()
        .filter(|value| is_letter(*value))
    else {
        return false;
    };

    let mut base = None;
    for candidate in chars[..index.saturating_sub(1)].iter().rev().copied() {
        if is_letter(candidate) {
            base = Some(candidate);
            break;
        }
        if !is_mark(candidate) {
            return false;
        }
    }

    base.is_some_and(|value| {
        !value
            .script_extension()
            .intersection(next.script_extension())
            .is_empty()
    })
}

fn is_letter(character: char) -> bool {
    general_kind(character) == GeneralKind::Letter
}

fn is_mark(character: char) -> bool {
    general_kind(character) == GeneralKind::Mark
}

fn is_bidi_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{206a}'..='\u{206f}'
    )
}

fn balanced_isolate_positions(chars: &[char]) -> Vec<bool> {
    let mut protected = vec![false; chars.len()];
    let mut stack = Vec::new();
    for (index, character) in chars.iter().copied().enumerate() {
        match character {
            '\u{2066}'..='\u{2068}' => stack.push(index),
            '\u{2069}' => {
                if let Some(start) = stack.pop()
                    && chars[start..index]
                        .iter()
                        .copied()
                        .any(is_meaningful_isolate_content)
                {
                    protected[start] = true;
                    protected[index] = true;
                }
            }
            '\r' | '\n' | '\u{001c}'..='\u{001e}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => {
                stack.clear();
            }
            _ => {}
        }
    }
    protected
}

fn is_meaningful_isolate_content(character: char) -> bool {
    if character.is_whitespace()
        || is_noncharacter(character)
        || is_control(character)
        || is_default_ignorable(character)
    {
        return false;
    }

    if matches!(
        character,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{200b}'
            | '\u{2060}'..='\u{2064}'
            | '\u{115f}'
            | '\u{1160}'
            | '\u{17b4}'
            | '\u{17b5}'
            | '\u{3164}'
            | '\u{ffa0}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffc}'
    ) {
        return false;
    }

    bidi_strong(character).is_some() || general_kind(character) != GeneralKind::Format
}

fn is_visible_contextual_format(character: char) -> bool {
    matches!(
        character,
        '\u{0600}'..='\u{0605}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1345f}'
    )
}

fn emoji_protected_positions(chars: &[char]) -> Vec<bool> {
    emoji_protected_positions_with_probe_count(chars).0
}

fn emoji_protected_positions_with_probe_count(chars: &[char]) -> (Vec<bool>, usize) {
    let mut protected = vec![false; chars.len()];
    let mut state = 0;
    let mut probes = 0_usize;

    for (index, character) in chars.iter().copied().enumerate() {
        state = 'resolve: {
            let mut candidate_state = state;
            for _ in 0..=MAX_EMOJI_SEQUENCE_SCALARS {
                probes = probes.saturating_add(1);
                if let Some(next) = emoji_automaton_transition(candidate_state, character) {
                    break 'resolve next;
                }
                let Some(non_root) = NonZeroUsize::new(candidate_state) else {
                    break 'resolve 0;
                };
                candidate_state = emoji_automaton_failure(non_root.get());
            }
            0
        };

        let output_length = emoji_automaton_output_length(state);
        if output_length == 0 {
            continue;
        }
        let start = index + 1 - output_length;
        for position in start..=index {
            if matches!(chars[position], '\u{200d}') || is_tag(chars[position]) {
                protected[position] = true;
            }
        }
    }
    (protected, probes)
}

fn emoji_automaton_transition(node: usize, character: char) -> Option<usize> {
    let offset = node.checked_mul(EMOJI_NODE_WIDTH)?;
    let edge_start = read_u32(EMOJI_AUTOMATON_NODES, offset)? as usize;
    let edge_count = read_u32(EMOJI_AUTOMATON_NODES, offset + 4)? as usize;
    let target = u32::from(character);
    let mut low = 0;
    let mut high = edge_count;

    // Edge lists are sorted at build time. Bound the search so corrupt data or
    // an implementation regression cannot create a non-progressing loop.
    for _ in 0..usize::BITS {
        if low >= high {
            break;
        }
        let middle = low + (high - low) / 2;
        let edge_offset = edge_start
            .checked_add(middle)?
            .checked_mul(EMOJI_EDGE_WIDTH)?;
        let scalar = read_u32(EMOJI_AUTOMATON_EDGES, edge_offset)?;
        if scalar < target {
            low = middle.checked_add(1)?;
        } else {
            high = middle;
        }
    }

    if low >= edge_count {
        return None;
    }
    let edge_offset = edge_start.checked_add(low)?.checked_mul(EMOJI_EDGE_WIDTH)?;
    (read_u32(EMOJI_AUTOMATON_EDGES, edge_offset)? == target)
        .then(|| read_u32(EMOJI_AUTOMATON_EDGES, edge_offset + 4).map(|value| value as usize))
        .flatten()
}

fn emoji_automaton_failure(node: usize) -> usize {
    let Some(offset) = node.checked_mul(EMOJI_NODE_WIDTH) else {
        return 0;
    };
    read_u32(EMOJI_AUTOMATON_NODES, offset + 8).map_or(0, |value| value as usize)
}

fn emoji_automaton_output_length(node: usize) -> usize {
    let Some(offset) = node
        .checked_mul(EMOJI_NODE_WIDTH)
        .and_then(|value| value.checked_add(12))
    else {
        return 0;
    };
    EMOJI_AUTOMATON_NODES
        .get(offset)
        .copied()
        .map_or(0, usize::from)
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod emoji_automaton_tests {
    use super::{
        EMOJI_AUTOMATON_NODES, EMOJI_NODE_WIDTH, emoji_automaton_failure,
        emoji_protected_positions_with_probe_count,
    };

    #[test]
    fn emoji_automaton_has_a_linear_probe_budget() {
        let standalone = vec!['😀'; 250_000];
        let (protected, probes) = emoji_protected_positions_with_probe_count(&standalone);
        assert!(!protected.into_iter().any(|value| value));
        assert!(probes <= standalone.len() * 2);

        let sequence = "😶\u{200d}🌫\u{fe0f}".chars().collect::<Vec<_>>();
        let repeated = sequence.repeat(50_000);
        let (protected, probes) = emoji_protected_positions_with_probe_count(&repeated);
        assert!(
            repeated
                .iter()
                .zip(protected)
                .all(|(character, is_protected)| *character != '\u{200d}' || is_protected)
        );
        assert!(probes <= repeated.len() * 2);
    }

    #[test]
    fn every_compiled_failure_link_is_read_exactly() {
        let node_count = EMOJI_AUTOMATON_NODES.len() / EMOJI_NODE_WIDTH;
        let mut saw_non_root_failure = false;

        for node in 0..node_count {
            let offset = node * EMOJI_NODE_WIDTH + 8;
            let expected = u32::from_be_bytes([
                EMOJI_AUTOMATON_NODES[offset],
                EMOJI_AUTOMATON_NODES[offset + 1],
                EMOJI_AUTOMATON_NODES[offset + 2],
                EMOJI_AUTOMATON_NODES[offset + 3],
            ]) as usize;
            assert_eq!(emoji_automaton_failure(node), expected, "node {node}");
            saw_non_root_failure |= expected != 0;
        }

        assert!(saw_non_root_failure);
    }
}

#[cfg(test)]
mod contextual_helper_tests {
    use super::{
        VARIATION_CONTEXT_FINAL, VARIATION_CONTEXT_INITIAL, VARIATION_CONTEXT_ISOLATE,
        VARIATION_CONTEXT_MEDIAL, is_meaningful_isolate_content, is_visible_contextual_format,
        shaping_context, valid_join_control,
    };

    const SELECTOR: char = '\u{fe0f}';

    #[test]
    fn shaping_context_requires_script_and_both_joining_directions() {
        assert_eq!(
            shaping_context(&['\u{a840}', SELECTOR, '\u{a840}'], 0, 1),
            VARIATION_CONTEXT_INITIAL
        );
        assert_eq!(
            shaping_context(&['\u{a840}', '\u{a840}', SELECTOR], 1, 2),
            VARIATION_CONTEXT_FINAL
        );
        assert_eq!(
            shaping_context(&['\u{a840}', '\u{a840}', SELECTOR, '\u{a840}'], 1, 2),
            VARIATION_CONTEXT_MEDIAL
        );

        // Different scripts must not shape merely because both code points are
        // dual-joining.
        assert_eq!(
            shaping_context(&['\u{0628}', '\u{a840}', SELECTOR], 1, 2),
            VARIATION_CONTEXT_ISOLATE
        );
        // A right-joining previous character cannot join what follows.
        assert_eq!(
            shaping_context(&['\u{10ac5}', '\u{10ac0}', SELECTOR], 1, 2),
            VARIATION_CONTEXT_ISOLATE
        );
        // A left-joining base cannot join what precedes it.
        assert_eq!(
            shaping_context(&['\u{a840}', '\u{a872}', SELECTOR], 1, 2),
            VARIATION_CONTEXT_ISOLATE
        );
        // A right-joining base cannot join what follows it.
        assert_eq!(
            shaping_context(&['\u{10ac5}', SELECTOR, '\u{10ac0}'], 0, 1),
            VARIATION_CONTEXT_ISOLATE
        );
        // A left-joining next character cannot join what precedes it.
        assert_eq!(
            shaping_context(&['\u{a840}', SELECTOR, '\u{a872}'], 0, 1),
            VARIATION_CONTEXT_ISOLATE
        );
    }

    #[test]
    fn join_controls_reject_neighbors_and_validate_virama_marks() {
        let consecutive = ['\u{0628}', '\u{200c}', '\u{200c}', '\u{0628}'];
        assert!(!valid_join_control(&consecutive, 1));
        assert!(!valid_join_control(&consecutive, 2));

        let valid = "क़्\u{200d}ष".chars().collect::<Vec<_>>();
        assert!(valid_join_control(&valid, 3));

        let interrupted = "क.्\u{200d}ष".chars().collect::<Vec<_>>();
        assert!(!valid_join_control(&interrupted, 3));
    }

    #[test]
    fn isolate_content_classification_covers_each_distinct_reason() {
        for rejected in [' ', '\u{fdd0}', '\u{0001}', '\u{200b}'] {
            assert!(!is_meaningful_isolate_content(rejected), "{rejected:?}");
        }
        for meaningful in ['A', '\u{0301}', '\u{070f}'] {
            assert!(is_meaningful_isolate_content(meaningful), "{meaningful:?}");
        }
    }

    #[test]
    fn visible_contextual_formats_are_an_exact_allowlist() {
        for allowed in [
            '\u{0600}',
            '\u{070f}',
            '\u{110bd}',
            '\u{13430}',
            '\u{1345f}',
        ] {
            assert!(is_visible_contextual_format(allowed), "{allowed:?}");
        }
        for rejected in ['A', '\u{180e}', '\u{2060}'] {
            assert!(!is_visible_contextual_format(rejected), "{rejected:?}");
        }
    }
}

fn canonicalize_whitespace(input: &str) -> (String, u32) {
    let mut output = String::with_capacity(input.len());
    let mut changed = 0_u32;
    let mut chars = input.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                trim_trailing_horizontal_whitespace(&mut output, &mut changed);
                output.push('\n');
                changed = changed.saturating_add(1);
            }
            '\n' | '\u{2028}' | '\u{2029}' => {
                trim_trailing_horizontal_whitespace(&mut output, &mut changed);
                output.push('\n');
                if character != '\n' {
                    changed = changed.saturating_add(1);
                }
            }
            candidate if general_kind(candidate) == GeneralKind::SpaceSeparator => {
                output.push(' ');
                if candidate != ' ' {
                    changed = changed.saturating_add(1);
                }
            }
            _ => output.push(character),
        }
    }

    trim_trailing_horizontal_whitespace(&mut output, &mut changed);
    (output, changed)
}

fn trim_trailing_horizontal_whitespace(output: &mut String, changed: &mut u32) {
    while matches!(output.chars().next_back(), Some(' ' | '\t')) {
        output.pop();
        *changed = changed.saturating_add(1);
    }
}

fn increment(counts: &mut BTreeMap<Category, u32>, category: Category) {
    counts
        .entry(category)
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
}

fn map_counts(counts: BTreeMap<Category, u32>) -> Vec<CategoryCount> {
    counts
        .into_iter()
        .map(|(category, count)| CategoryCount { category, count })
        .collect()
}

fn count_for(counts: &[CategoryCount], category: Category) -> u32 {
    counts
        .iter()
        .find(|entry| entry.category == category)
        .map_or(0, |entry| entry.count)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralKind {
    Other,
    Control,
    Format,
    PrivateUse,
    SpaceSeparator,
    Letter,
    Mark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JoiningKind {
    NonJoining,
    Transparent,
    Dual,
    Left,
    Right,
    JoinCausing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BidiStrong {
    LeftToRight,
    RightToLeft,
}

fn general_kind(character: char) -> GeneralKind {
    match lookup_range_value(GENERAL_CATEGORY_BYTES, character) {
        Some(1) => GeneralKind::Control,
        Some(2) => GeneralKind::Format,
        Some(3) => GeneralKind::PrivateUse,
        Some(4) => GeneralKind::SpaceSeparator,
        Some(5) => GeneralKind::Letter,
        Some(6) => GeneralKind::Mark,
        _ => GeneralKind::Other,
    }
}

fn joining_kind(character: char) -> JoiningKind {
    match lookup_range_value(JOINING_TYPE_BYTES, character) {
        Some(1) => JoiningKind::Transparent,
        Some(2) => JoiningKind::Dual,
        Some(3) => JoiningKind::Left,
        Some(4) => JoiningKind::Right,
        Some(5) => JoiningKind::JoinCausing,
        _ => JoiningKind::NonJoining,
    }
}

fn bidi_strong(character: char) -> Option<BidiStrong> {
    match lookup_range_value(BIDI_CLASS_BYTES, character) {
        Some(1) => Some(BidiStrong::LeftToRight),
        Some(2 | 3) => Some(BidiStrong::RightToLeft),
        _ => None,
    }
}

fn lookup_range_value(table: &[u8], character: char) -> Option<u8> {
    debug_assert_eq!(table.len() % RANGE_RECORD_WIDTH, 0);
    let target = u32::from(character);
    let mut low = 0;
    let mut high = table.len() / RANGE_RECORD_WIDTH;

    // Range tables are generated and sorted, but the lookup still fails closed
    // after a machine-word-sized binary-search budget.
    for _ in 0..usize::BITS {
        if low >= high {
            break;
        }
        let middle = low + (high - low) / 2;
        let offset = middle * RANGE_RECORD_WIDTH;
        let start = u32::from_be_bytes([
            table[offset],
            table[offset + 1],
            table[offset + 2],
            table[offset + 3],
        ]);
        let end = u32::from_be_bytes([
            table[offset + 4],
            table[offset + 5],
            table[offset + 6],
            table[offset + 7],
        ]);

        if target < start {
            high = middle;
        } else if target > end {
            low = middle.checked_add(1)?;
        } else {
            return Some(table[offset + 8]);
        }
    }

    None
}

#[cfg(test)]
mod unicode_table_tests {
    use std::io;

    use super::{
        BIDI_CLASS_BYTES, BidiStrong, DEFAULT_IGNORABLE_BYTES, GENERAL_CATEGORY_BYTES, GeneralKind,
        INCB_LINKER_BYTES, JOINING_TYPE_BYTES, JoiningKind, bidi_strong, general_kind, is_mark,
        joining_kind, lookup_range_value,
    };

    #[test]
    fn compiled_enum_decoders_cover_every_runtime_variant() {
        for (character, expected) in [
            ('1', GeneralKind::Other),
            ('\u{0001}', GeneralKind::Control),
            ('\u{200b}', GeneralKind::Format),
            ('\u{e000}', GeneralKind::PrivateUse),
            ('\u{00a0}', GeneralKind::SpaceSeparator),
            ('A', GeneralKind::Letter),
            ('\u{0301}', GeneralKind::Mark),
        ] {
            assert_eq!(general_kind(character), expected, "{character:?}");
        }
        assert!(is_mark('\u{0301}'));
        assert!(!is_mark('A'));

        for (character, expected) in [
            ('A', JoiningKind::NonJoining),
            ('\u{0301}', JoiningKind::Transparent),
            ('\u{0628}', JoiningKind::Dual),
            ('\u{a872}', JoiningKind::Left),
            ('\u{0627}', JoiningKind::Right),
            ('\u{0640}', JoiningKind::JoinCausing),
        ] {
            assert_eq!(joining_kind(character), expected, "{character:?}");
        }

        assert_eq!(bidi_strong('A'), Some(BidiStrong::LeftToRight));
        assert_eq!(bidi_strong('\u{05d0}'), Some(BidiStrong::RightToLeft));
        assert_eq!(bidi_strong('\u{0628}'), Some(BidiStrong::RightToLeft));
        assert_eq!(bidi_strong('1'), None);
    }

    #[test]
    fn compiled_tables_match_every_selected_unicode_17_source_range()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_source_ranges(
            include_str!("../data/unicode-17/DerivedGeneralCategory.txt"),
            GENERAL_CATEGORY_BYTES,
            |property| match property {
                "Cc" => Some(1),
                "Cf" => Some(2),
                "Co" => Some(3),
                "Zs" => Some(4),
                value if value.starts_with('L') => Some(5),
                value if value.starts_with('M') => Some(6),
                _ => None,
            },
        )?;
        assert_source_ranges(
            include_str!("../data/unicode-17/DerivedJoiningType.txt"),
            JOINING_TYPE_BYTES,
            |property| match property {
                "T" => Some(1),
                "D" => Some(2),
                "L" => Some(3),
                "R" => Some(4),
                "C" => Some(5),
                _ => None,
            },
        )?;
        assert_source_ranges(
            include_str!("../data/unicode-17/DerivedCoreProperties.txt"),
            DEFAULT_IGNORABLE_BYTES,
            |property| (property == "Default_Ignorable_Code_Point").then_some(1),
        )?;
        assert_source_ranges(
            include_str!("../data/unicode-17/DerivedCoreProperties.txt"),
            INCB_LINKER_BYTES,
            |property| (property == "InCB; Linker").then_some(1),
        )?;
        assert_source_ranges(
            include_str!("../data/unicode-17/DerivedBidiClass.txt"),
            BIDI_CLASS_BYTES,
            |property| match property {
                "L" => Some(1),
                "R" => Some(2),
                "AL" => Some(3),
                _ => None,
            },
        )?;
        Ok(())
    }

    fn assert_source_ranges<F>(
        source: &str,
        table: &[u8],
        encode: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(&str) -> Option<u8>,
    {
        for line in source.lines() {
            let data = line.split_once('#').map_or(line, |(value, _)| value).trim();
            let Some((range, property)) = data.split_once(';') else {
                continue;
            };
            let Some(expected) = encode(property.trim()) else {
                continue;
            };
            let (start, end) = range
                .trim()
                .split_once("..")
                .map_or((range.trim(), range.trim()), |(start, end)| (start, end));
            let start = u32::from_str_radix(start, 16)?;
            let end = u32::from_str_radix(end, 16)?;

            for scalar in [start, start + (end - start) / 2, end] {
                let character = char::from_u32(scalar).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("source contains invalid Unicode scalar U+{scalar:04X}"),
                    )
                })?;
                assert_eq!(
                    lookup_range_value(table, character),
                    Some(expected),
                    "lookup mismatch at U+{scalar:04X}"
                );
            }
        }
        Ok(())
    }
}
