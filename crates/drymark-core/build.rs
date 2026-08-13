//! Compile pinned Unicode 17 data into compact, offline lookup tables.

use std::{
    collections::{BTreeMap, VecDeque, btree_map::Entry},
    env,
    error::Error,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const VARIATION_SOURCES: [(&str, usize, bool); 3] = [
    ("data/unicode-17/StandardizedVariants.txt", 1_353, true),
    ("data/unicode-17/emoji-variation-sequences.txt", 742, false),
    (
        "data/unicode-17/IVD_Sequences-2025-07-14.txt",
        39_501,
        false,
    ),
];

const DATA_SOURCES: [(&str, &str); 8] = [
    (
        "data/unicode-17/StandardizedVariants.txt",
        "f55100b2fb11d3d75a37b8c1ab752192dbd1c4b12328c5ec6b38e3807c0ca597",
    ),
    (
        "data/unicode-17/emoji-variation-sequences.txt",
        "bb3d09ef03f206012c7532dd52dc0a21c9efddba0135ea4cf0d9201b8b9bba7e",
    ),
    (
        "data/unicode-17/IVD_Sequences-2025-07-14.txt",
        "0052165369b6c8783d19b041f0a70537a69d718d577b9df180453be9d8c10a87",
    ),
    (
        "data/unicode-17/DerivedGeneralCategory.txt",
        "d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e",
    ),
    (
        "data/unicode-17/DerivedJoiningType.txt",
        "f39ebe974825d6736aee15582250307aa532b2cfab3caf3f86bd23fddc9c5c4d",
    ),
    (
        "data/unicode-17/DerivedCoreProperties.txt",
        "24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08",
    ),
    (
        "data/unicode-17/DerivedBidiClass.txt",
        "4867b4b7f0731ed1bfcd34cc6251211ff1542541fce0734b6fbda139ee80b3a4",
    ),
    (
        "data/unicode-17/emoji-test.txt",
        "1d8a944f88d7952f7ef7c5167fef3c67995bcae24543949710231b03a201acda",
    ),
];

const EXPECTED_UNIQUE_SEQUENCES: usize = 31_730;
const EXPECTED_CONTEXTUAL_VARIATION_RECORDS: usize = 63;
const CONTEXT_ISOLATE: u8 = 1;
const CONTEXT_INITIAL: u8 = 2;
const CONTEXT_MEDIAL: u8 = 4;
const CONTEXT_FINAL: u8 = 8;
const GENERAL_CATEGORY_RECORDS: usize = 2_541;
const JOINING_TYPE_RECORDS: usize = 542;
const DEFAULT_IGNORABLE_RECORDS: usize = 27;
const INCB_LINKER_RECORDS: usize = 20;
const BIDI_CLASS_RECORDS: usize = 1_295;
const EXPECTED_SENSITIVE_EMOJI_SEQUENCES: usize = 2_670;
const MAX_EMOJI_SEQUENCE_SCALARS: usize = 36;

#[derive(Clone, Copy, Debug)]
struct RangeRecord {
    start: u32,
    end: u32,
    value: u8,
}

#[derive(Default)]
struct EmojiAutomatonNode {
    edges: BTreeMap<u32, usize>,
    failure: usize,
    longest_output: u8,
}

fn main() -> Result<(), Box<dyn Error>> {
    for (source, expected_sha256) in DATA_SOURCES {
        println!("cargo:rerun-if-changed={source}");
        verify_source_hash(Path::new(source), expected_sha256)?;
    }

    let output_directory =
        PathBuf::from(env::var_os("OUT_DIR").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Cargo did not provide OUT_DIR")
        })?);

    compile_variation_sequences(&output_directory)?;
    compile_emoji_automaton(&output_directory)?;
    compile_range_table(
        Path::new("data/unicode-17/DerivedGeneralCategory.txt"),
        GENERAL_CATEGORY_RECORDS,
        general_category_code,
        &output_directory.join("general-category-v17.bin"),
    )?;
    compile_range_table(
        Path::new("data/unicode-17/DerivedJoiningType.txt"),
        JOINING_TYPE_RECORDS,
        joining_type_code,
        &output_directory.join("joining-type-v17.bin"),
    )?;
    compile_range_table(
        Path::new("data/unicode-17/DerivedCoreProperties.txt"),
        DEFAULT_IGNORABLE_RECORDS,
        default_ignorable_code,
        &output_directory.join("default-ignorable-v17.bin"),
    )?;
    compile_range_table(
        Path::new("data/unicode-17/DerivedCoreProperties.txt"),
        INCB_LINKER_RECORDS,
        incb_linker_code,
        &output_directory.join("incb-linker-v17.bin"),
    )?;
    compile_range_table(
        Path::new("data/unicode-17/DerivedBidiClass.txt"),
        BIDI_CLASS_RECORDS,
        bidi_class_code,
        &output_directory.join("bidi-class-v17.bin"),
    )?;

    Ok(())
}

fn compile_emoji_automaton(output_directory: &Path) -> Result<(), Box<dyn Error>> {
    let path = Path::new("data/unicode-17/emoji-test.txt");
    let source = fs::read_to_string(path)?;
    let mut nodes = vec![EmojiAutomatonNode::default()];
    let mut sequence_count = 0;

    for (line_index, line) in source.lines().enumerate() {
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

        let sequence = scalars
            .split_whitespace()
            .map(|value| parse_scalar(path, line_index, value))
            .collect::<Result<Vec<_>, _>>()?;
        if !sequence
            .iter()
            .copied()
            .any(|scalar| scalar == 0x200D || matches!(scalar, 0xE0001 | 0xE0020..=0xE007F))
        {
            continue;
        }
        if sequence.is_empty() || sequence.len() > MAX_EMOJI_SEQUENCE_SCALARS {
            return Err(
                invalid_record(path, line_index, "emoji sequence exceeds matcher bound").into(),
            );
        }

        insert_emoji_sequence(&mut nodes, &sequence)?;
        sequence_count += 1;
    }

    if sequence_count != EXPECTED_SENSITIVE_EMOJI_SEQUENCES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "emoji registry contains {sequence_count} sensitive sequences; expected {EXPECTED_SENSITIVE_EMOJI_SEQUENCES}"
            ),
        )
        .into());
    }

    compile_failure_links(&mut nodes);
    write_emoji_automaton(output_directory, &nodes)?;
    Ok(())
}

fn insert_emoji_sequence(
    nodes: &mut Vec<EmojiAutomatonNode>,
    sequence: &[u32],
) -> Result<(), Box<dyn Error>> {
    let mut node = 0;
    for scalar in sequence {
        let next = if let Some(next) = nodes[node].edges.get(scalar).copied() {
            next
        } else {
            let next = nodes.len();
            nodes.push(EmojiAutomatonNode::default());
            nodes[node].edges.insert(*scalar, next);
            next
        };
        node = next;
    }
    let length = u8::try_from(sequence.len())?;
    nodes[node].longest_output = nodes[node].longest_output.max(length);
    Ok(())
}

fn compile_failure_links(nodes: &mut [EmojiAutomatonNode]) {
    let mut queue = VecDeque::new();
    for child in nodes[0].edges.values().copied() {
        queue.push_back(child);
    }

    while let Some(node) = queue.pop_front() {
        let transitions = nodes[node]
            .edges
            .iter()
            .map(|(scalar, child)| (*scalar, *child))
            .collect::<Vec<_>>();
        for (scalar, child) in transitions {
            let mut failure = nodes[node].failure;
            while failure != 0 && !nodes[failure].edges.contains_key(&scalar) {
                failure = nodes[failure].failure;
            }
            if let Some(next) = nodes[failure].edges.get(&scalar).copied() {
                failure = next;
            }
            nodes[child].failure = failure;
            nodes[child].longest_output = nodes[child]
                .longest_output
                .max(nodes[failure].longest_output);
            queue.push_back(child);
        }
    }
}

fn write_emoji_automaton(
    output_directory: &Path,
    nodes: &[EmojiAutomatonNode],
) -> Result<(), Box<dyn Error>> {
    let mut node_output = File::create(output_directory.join("emoji-automaton-nodes-v17.bin"))?;
    let mut edge_output = File::create(output_directory.join("emoji-automaton-edges-v17.bin"))?;
    let mut edge_start = 0_u32;

    for node in nodes {
        let edge_count = u32::try_from(node.edges.len())?;
        node_output.write_all(&edge_start.to_be_bytes())?;
        node_output.write_all(&edge_count.to_be_bytes())?;
        node_output.write_all(&u32::try_from(node.failure)?.to_be_bytes())?;
        node_output.write_all(&[node.longest_output])?;

        for (scalar, child) in &node.edges {
            edge_output.write_all(&scalar.to_be_bytes())?;
            edge_output.write_all(&u32::try_from(*child)?.to_be_bytes())?;
        }
        edge_start = edge_start.checked_add(edge_count).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "emoji automaton edge overflow")
        })?;
    }
    Ok(())
}

fn compile_variation_sequences(output_directory: &Path) -> Result<(), Box<dyn Error>> {
    let mut sequences = BTreeMap::new();
    let mut contextual_records = 0;

    for (source, expected_rows, has_context_field) in VARIATION_SOURCES {
        let (rows, contexts) =
            parse_variation_source(Path::new(source), &mut sequences, has_context_field)?;
        contextual_records += contexts;
        if rows != expected_rows {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{source} contains {rows} records; expected {expected_rows}"),
            )
            .into());
        }
    }

    if contextual_records != EXPECTED_CONTEXTUAL_VARIATION_RECORDS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "variation registry contains {contextual_records} contextual records; expected {EXPECTED_CONTEXTUAL_VARIATION_RECORDS}"
            ),
        )
        .into());
    }

    if sequences.len() != EXPECTED_UNIQUE_SEQUENCES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "variation registry contains {} unique sequences; expected {EXPECTED_UNIQUE_SEQUENCES}",
                sequences.len()
            ),
        )
        .into());
    }

    let mut output = File::create(output_directory.join("variation-sequences-v17.bin"))?;
    for (sequence, context_mask) in sequences {
        output.write_all(&sequence.to_be_bytes())?;
        output.write_all(&[context_mask])?;
    }
    Ok(())
}

fn compile_range_table<F>(
    path: &Path,
    expected_records: usize,
    encode: F,
    output_path: &Path,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(&str) -> Option<u8>,
{
    let source = fs::read_to_string(path)?;
    let mut records = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let data = line.split_once('#').map_or(line, |(value, _)| value).trim();
        if data.is_empty() {
            continue;
        }
        let Some((range, property)) = data.split_once(';') else {
            return Err(invalid_record(path, line_index, "missing property separator").into());
        };
        let Some(value) = encode(property.trim()) else {
            continue;
        };
        let (start, end) = parse_range(path, line_index, range.trim())?;
        records.push(RangeRecord { start, end, value });
    }

    if records.len() != expected_records {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} contains {} selected records; expected {expected_records}",
                path.display(),
                records.len()
            ),
        )
        .into());
    }

    records.sort_unstable_by_key(|record| record.start);
    for pair in records.windows(2) {
        if pair[0].end >= pair[1].start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} contains overlapping selected ranges", path.display()),
            )
            .into());
        }
    }

    let mut output = File::create(output_path)?;
    for record in records {
        output.write_all(&record.start.to_be_bytes())?;
        output.write_all(&record.end.to_be_bytes())?;
        output.write_all(&[record.value])?;
    }
    Ok(())
}

fn general_category_code(property: &str) -> Option<u8> {
    match property {
        "Cc" => Some(1),
        "Cf" => Some(2),
        "Co" => Some(3),
        "Zs" => Some(4),
        value if value.starts_with('L') => Some(5),
        value if value.starts_with('M') => Some(6),
        _ => None,
    }
}

fn joining_type_code(property: &str) -> Option<u8> {
    match property {
        "T" => Some(1),
        "D" => Some(2),
        "L" => Some(3),
        "R" => Some(4),
        "C" => Some(5),
        _ => None,
    }
}

fn default_ignorable_code(property: &str) -> Option<u8> {
    (property == "Default_Ignorable_Code_Point").then_some(1)
}

fn incb_linker_code(property: &str) -> Option<u8> {
    (property == "InCB; Linker").then_some(1)
}

fn bidi_class_code(property: &str) -> Option<u8> {
    match property {
        "L" => Some(1),
        "R" => Some(2),
        "AL" => Some(3),
        _ => None,
    }
}

fn verify_source_hash(path: &Path, expected: &str) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} has SHA-256 {actual}; expected {expected}",
                path.display()
            ),
        )
        .into());
    }
    Ok(())
}

fn parse_variation_source(
    path: &Path,
    sequences: &mut BTreeMap<u64, u8>,
    has_context_field: bool,
) -> Result<(usize, usize), Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let mut records = 0;
    let mut contextual_records = 0;

    for (line_index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
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
        if sequence.len() != 2 {
            return Err(invalid_record(path, line_index, "expected two code points").into());
        }

        let base = parse_scalar(path, line_index, sequence[0])?;
        let selector = parse_scalar(path, line_index, sequence[1])?;
        if !matches!(
            selector,
            0x180B..=0x180D | 0x180F | 0xFE00..=0xFE0F | 0xE0100..=0xE01EF
        ) {
            return Err(invalid_record(path, line_index, "invalid variation selector").into());
        }

        let context_mask = if has_context_field {
            parse_variation_context(path, line_index, fields.get(2).copied().unwrap_or_default())?
        } else {
            0
        };
        contextual_records += usize::from(context_mask != 0);

        let sequence = (u64::from(base) << 32) | u64::from(selector);
        match sequences.entry(sequence) {
            Entry::Vacant(entry) => {
                entry.insert(context_mask);
            }
            Entry::Occupied(mut entry) => {
                let existing = *entry.get();
                *entry.get_mut() = if existing == 0 || context_mask == 0 {
                    0
                } else {
                    existing | context_mask
                };
            }
        }
        records += 1;
    }

    Ok((records, contextual_records))
}

fn parse_variation_context(
    path: &Path,
    line_index: usize,
    context: &str,
) -> Result<u8, Box<dyn Error>> {
    let mut mask = 0;
    for value in context.split_whitespace() {
        mask |= match value {
            "isolate" => CONTEXT_ISOLATE,
            "initial" => CONTEXT_INITIAL,
            "medial" => CONTEXT_MEDIAL,
            "final" => CONTEXT_FINAL,
            _ => {
                return Err(invalid_record(path, line_index, "invalid shaping context").into());
            }
        };
    }
    Ok(mask)
}

fn parse_range(path: &Path, line_index: usize, value: &str) -> Result<(u32, u32), Box<dyn Error>> {
    let (start, end) = value
        .split_once("..")
        .map_or((value, value), |(start, end)| (start, end));
    let start = parse_scalar(path, line_index, start)?;
    let end = parse_scalar(path, line_index, end)?;
    if start > end {
        return Err(invalid_record(path, line_index, "range start exceeds end").into());
    }
    Ok((start, end))
}

fn parse_scalar(path: &Path, line_index: usize, value: &str) -> Result<u32, Box<dyn Error>> {
    let scalar = u32::from_str_radix(value.trim(), 16)
        .map_err(|_| invalid_record(path, line_index, "invalid hexadecimal scalar"))?;
    if char::from_u32(scalar).is_none() {
        return Err(invalid_record(path, line_index, "invalid Unicode scalar").into());
    }
    Ok(scalar)
}

fn invalid_record(path: &Path, line_index: usize, message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}:{}: {message}", path.display(), line_index + 1),
    )
}
