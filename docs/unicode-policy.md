# Unicode Policy

DryMark classifies Unicode scalars that can carry hidden LLM watermarks into
stable report categories before deciding whether to keep, observe, remove, or
canonicalize them.

## Always Removed

| Category | Representative values |
| --- | --- |
| Control | C0/C1 controls except tab, LF, and CR |
| Zero width | `U+200B`, `U+2060`, `U+FEFF` |
| Soft hyphen | `U+00AD` |
| Invisible operator | `U+2061` through `U+2064` |
| Combining grapheme joiner | `U+034F` |
| Annotation control | `U+FFF9` through `U+FFFB` |
| Object replacement | `U+FFFC` |
| Noncharacter | `U+FDD0` through `U+FDEF` and every scalar ending in `FFFE` or `FFFF` |
| Filler | Hangul and Khmer invisible filler values handled by the engine |
| Default-ignorable | Every Unicode 17 DICP scalar not represented by a more specific category |
| Unsafe bidi | Marks, embeddings, overrides, deprecated controls, and malformed isolates |

## Contextual Preserve Rules

### Emoji

The engine recognizes complete RGI emoji sequences through the `emojis`
database. A zero-width joiner or tag is preserved only when it participates in
a recognized sequence. Emoji variation selectors are checked against the exact
registered pair allowlist described below. Orphaned, duplicated, or
emoji-shaped payload sequences are removed.

Unicode subdivision flags are preserved only when their full tag sequence is a
recognized emoji. General tag payloads are removed.

### Script Shaping

`U+200C` and `U+200D` are preserved when Unicode joining types on both sides
support a same-script shaping connection or when the joiner follows a Unicode
17 `Indic_Conjunct_Break=Linker` between letters with intersecting
`Script_Extensions`. Adjacent join controls are rejected. Recognized emoji
sequences are handled by the emoji rule first.

This protects common Persian, Arabic-family, and Indic shaping while removing
joiners placed between unrelated Latin letters, digits, or text boundaries.

### Variation Selectors

Preserve mode accepts a base-selector pair only when it appears in one of the
three Unicode-sanctioned registries: Unicode 17 standardized variants, Unicode
Emoji 17 presentation sequences, or the 2025-07-14 Ideographic Variation
Database. This exact allowlist protects real Mongolian, mathematical, emoji,
Egyptian, and ideographic glyph choices without accepting arbitrary
default-ignorable selector payloads. The 63 standardized variants whose
registrations specify an isolate, initial, medial, or final shaping environment
are accepted only in those environments. Consecutive, orphaned,
context-invalid, and unregistered selectors are removed.

The variation registries and the official Unicode 17 general-category, joining,
default-ignorable, bidirectional, and emoji conformance data are vendored under
`crates/drymark-core/data/unicode-17/`. Builds and runtime checks are offline.

### Bidirectional Isolates

Balanced `LRI`, `RLI`, or `FSI` plus `PDI` pairs are preserved in Preserve mode
only within one paragraph and only when they enclose meaningful non-whitespace
content. Empty, control-only, cross-paragraph, stray, and unclosed isolates are
removed, as are embeddings, overrides, marks, and deprecated controls. Thorough
mode removes every bidi control.

### Visible-Context Format Controls

Some Arabic number signs, Syriac abbreviation marks, Egyptian hieroglyph
formatting controls, and related Unicode values have visible layout semantics.
Preserve mode keeps and reports the explicitly reviewed ranges. Thorough mode
removes them.

## Private Use

Private-use scalars can represent real glyphs in a private font or carry an
opaque payload. Preserve mode keeps and reports them because removal is not
appearance-safe. Thorough mode removes them.

## Thorough Canonicalization

After scalar classification, Thorough mode:

1. Converts CR, CRLF, line separator, and paragraph separator to LF.
2. Converts Unicode space separators to ASCII space.
3. Removes trailing spaces and tabs before line breaks and at end of input.
4. Applies Unicode NFC normalization.

Canonical composition can change UTF-8 byte length and, for composition
exclusions, can expand one scalar into multiple scalars. Tests therefore enforce
a conservative bounded-output invariant rather than assuming normalization can
never expand.

Preserve is appearance-conscious, not an absolute rendering or semantic
guarantee. It removes ALM/LRM/RLM direction marks, interlinear annotation
delimiters, soft hyphens, word joiners, invisible mathematical operators, and
specialist notation controls. Those values can carry legitimate layout or
machine-readable meaning as well as hidden payloads. Keeping every such control
would leave an invisible channel; removing them is inherently lossy.

## Report Semantics

`removed` counts scalars discarded during classification. `observed` counts
contextual or opaque scalars retained by Preserve mode. Whitespace changes and
normalization have dedicated metadata because they are transformations rather
than simple scalar removals. Reports never contain input excerpts.

## Adding a Rule

A policy change requires:

- A normative Unicode or language-behavior rationale.
- Golden tests for valid, orphaned, duplicated, and boundary placement.
- Preserve and Thorough expectations.
- Idempotence and metadata accounting coverage.
- A review of visible rendering or shaping impact.
