# Unicode 17 Data Snapshot

DryMark vendors the Unicode data needed to classify hidden scalars and to
distinguish appearance-bearing sequences from arbitrary payloads. Runtime and
builds remain fully offline.

| File | Purpose | SHA-256 | Validated records |
| --- | --- | --- | ---: |
| `StandardizedVariants.txt` | Unicode 17 standardized variation sequences | `f55100b2fb11d3d75a37b8c1ab752192dbd1c4b12328c5ec6b38e3807c0ca597` | 1,353 |
| `emoji-variation-sequences.txt` | Unicode Emoji 17 presentation sequences | `bb3d09ef03f206012c7532dd52dc0a21c9efddba0135ea4cf0d9201b8b9bba7e` | 742 |
| `IVD_Sequences-2025-07-14.txt` | Ideographic Variation Database registrations | `0052165369b6c8783d19b041f0a70537a69d718d577b9df180453be9d8c10a87` | 39,501 |
| `DerivedGeneralCategory.txt` | Controls, formats, private use, spaces, letters, and marks | `d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e` | 2,541 selected ranges |
| `DerivedJoiningType.txt` | Contextual shaping join controls | `f39ebe974825d6736aee15582250307aa532b2cfab3caf3f86bd23fddc9c5c4d` | 542 selected ranges |
| `DerivedCoreProperties.txt` | `Default_Ignorable_Code_Point` and `Indic_Conjunct_Break=Linker` coverage | `24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08` | 27 + 20 selected ranges |
| `DerivedBidiClass.txt` | Strong bidirectional classes | `4867b4b7f0731ed1bfcd34cc6251211ff1542541fce0734b6fbda139ee80b3a4` | 1,295 selected ranges |
| `emoji-test.txt` | Complete fully qualified Emoji 17 conformance corpus | `1d8a944f88d7952f7ef7c5167fef3c67995bcae24543949710231b03a201acda` | 3,944 sequences |

Sources:

- <https://www.unicode.org/Public/17.0.0/ucd/StandardizedVariants.txt>
- <https://www.unicode.org/Public/17.0.0/ucd/emoji/emoji-variation-sequences.txt>
- <https://www.unicode.org/ivd/data/2025-07-14/IVD_Sequences.txt>
- <https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt>
- <https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedJoiningType.txt>
- <https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt>
- <https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedBidiClass.txt>
- <https://www.unicode.org/Public/emoji/17.0/emoji-test.txt>

The build script verifies every file hash and record count before emitting
compact sorted lookup tables. It deduplicates collection-level IVD registrations
into 31,730 unique base-selector pairs while retaining 63 normative shaping
context qualifiers. Tests compare every selected Unicode source range with the
compiled tables and exercise every fully qualified emoji.

The files are distributed under the Unicode License v3 in [LICENSE](LICENSE).
