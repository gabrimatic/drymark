# Threat Model

## Objective

DryMark removes inspectable hidden watermark channels from LLM text according
to an explicit policy, then produces a fresh plain-text clipboard value. The
engine is vendor-neutral: it handles invisible controls, tags, selectors,
misleading directionality, and rich clipboard representations by channel rather
than by provider signature.

The primary invariant is narrow and testable:

> Given valid UTF-8 and a selected policy, the same input always produces the
> same output and a metadata-only report. A committed clipboard write contains
> only that output as plain text.

## Assets

- The exact visible text the user intends to paste.
- Legitimate emoji and language-shaping behavior.
- Clipboard confidentiality during the active operation.
- Clipboard freshness when another app or user changes it concurrently.
- Accurate feedback about whether a clean was committed.

## Adversary Capabilities

An input producer may place any valid Unicode scalar sequence in copied text and
may supply additional clipboard representations such as HTML or RTF. It may use
controls in unusual order, malformed emoji-like sequences, unbalanced bidi
isolates, noncharacters, private-use scalars, or very large text.

Another local process may change the clipboard while DryMark is removing
watermark channels.
DryMark assumes the operating system and its clipboard APIs are not
compromised.

## In Scope

| Channel | Defense |
| --- | --- |
| C0/C1 controls | Remove except supported tab and line structure |
| Format and default-ignorable scalars | Remove or contextually preserve under the selected policy |
| Bidi embeddings and overrides | Remove |
| Unicode tags | Preserve only inside a recognized emoji sequence in Preserve mode |
| Variation selectors and joiners | Preserve only in exact registered variation sequences or validated shaping/emoji contexts in Preserve mode |
| Noncharacters and invisible fillers | Remove |
| Private-use scalars | Report in Preserve; remove in Thorough |
| Rich clipboard formats | Replace with a fresh plain-text value on commit |
| Concurrent text changes | Re-read immediately before write, abort on mismatch, and verify text after writing |
| Oversized or invalid input | Reject without echoing content |
| Reports, errors, and frontend state | Counts and fixed categories only |

## Out of Scope

DryMark cannot identify or remove an LLM watermark signal that is not encoded
in inspectable text or clipboard representations. This includes:

- Visible word choice, punctuation, spelling, capitalization, or sentence order.
- Semantic, statistical, or token-distribution signals.
- Signals retained only by a remote service, account, document ID, or server
  log.
- Images, PDFs, office documents, or other non-text clipboard values.
- Visible homoglyphs and confusable characters; changing them is not lossless.
- Clipboard reads by another local process before or after watermark removal.
- A format-only clipboard change that the platform adapter cannot observe.
- Retention or synchronization by operating-system clipboard history, cloud
  clipboard, or device-continuity services enabled by the user.
- The unavoidable interval between a final clipboard read and write on
  platforms without compare-and-swap clipboard primitives.
- Restoration of a clipboard value overwritten during that interval; the
  post-write check can detect a mismatch but cannot safely roll it back.

No tool can promise lossless removal of those channels while also promising
identical visible text. DryMark keeps that limitation visible in Settings and
documentation.

## Policy Trade-offs

Preserve is the default. It retains recognized emoji sequences, contextual
shaping controls, and balanced bidi isolates, while removing direction marks,
annotation delimiters, invisible mathematical operators, and other ambiguous
channels. Preserved suspicious scalars are counted as `observed`. The policy
minimizes presentation changes but cannot guarantee identical rendering or
machine interpretation for specialist text.

Thorough removes all format channels handled by the engine, private-use scalars,
joiners, selectors, tags, and bidi controls, then canonicalizes line endings,
separator spaces, trailing horizontal whitespace, and NFC. It is more suitable
for comparison or indexing and may change presentation.

## Privacy Properties

- The core is a pure function and performs no I/O.
- The desktop application has no network feature or analytics dependency.
- Clipboard snapshots, core scratch buffers, and CLI input/output buffers use
  zeroizing storage where ownership permits.
- Errors are reduced to stable classifications before crossing the frontend
  boundary.
- Preferences contain only policy, shortcut, and visual-feedback state.
- There is no clipboard history, raw-text report, or content logging path.

The current desktop adapter exposes neither format enumeration nor a clipboard
revision. It therefore requests a conservative plain-text rewrite for every
text clipboard. This removes formats at commit time but cannot detect a
format-only race before that commit.

Zeroization reduces residual heap exposure but does not guarantee erasure of OS
clipboard buffers, immutable copies inside platform frameworks, swap, or crash
dumps outside the process's control.

## Future Unicode Versions

Unknown scalars classified by Unicode as `Format` are removed by default unless
they are explicitly recognized as legitimate visible-context controls. New
emoji and script behavior still requires a dependency update, review, and
regression corpus. Scheduled dependency checks and fuzzing help surface that
work; they do not replace policy review.
