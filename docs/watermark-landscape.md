# Text Watermark Landscape

“Watermark” describes several unrelated mechanisms. The important question is
not whether a design uses hashing, encryption, or a secret key. It is where the
resulting signal is carried.

DryMark is a loss-aware clipboard sanitizer. It removes supported hidden
Unicode carriers and replaces rich clipboard representations with fresh plain
text. It does not guess at authorship or rewrite visible language.

## Capability Matrix

| Signal class | Where the signal lives | DryMark behavior |
| --- | --- | --- |
| Hidden Unicode steganography | Default-ignorables, direction controls, tags, variation selectors, noncharacters, private-use scalars, or other invisible code points | Removes supported carriers according to Preserve or Thorough policy. An encrypted or hashed payload needs no special handling because the carrier is removed without decoding it. |
| Rich clipboard marking | HTML, RTF, custom MIME types, comments, attributes, or application-specific clipboard objects | The desktop transaction replaces the clipboard with one fresh plain-text representation after its race checks. |
| Secret-key or hash-guided token watermark | Visible token choices selected with a pseudorandom function, hash, key, or keyed detector | Outside the lossless scope. The signal is the visible token sequence; removal requires changing words or tokens. |
| Statistical language watermark | Bias across word choice, punctuation, syntax, sentence order, or token distribution | Outside the lossless scope. A reliable attempt would require rewriting and could change meaning or style. |
| Homoglyph and visible-format encoding | Look-alike letters, unusual spacing, punctuation, or normalization differences | Thorough handles its documented Unicode and whitespace normalizations. Arbitrary look-alike replacement or punctuation rewriting is not lossless and is not claimed. |
| Signed file provenance | A signed manifest, asset hash, certificate chain, embedded file metadata, or remotely referenced manifest | Outside a text-only clipboard tool. Copying plain text does not make DryMark a general file-metadata sanitizer. |
| Sidecar or server record | A database, account event, request identifier, or separate provenance record | Not present in the copied text and therefore cannot be removed locally from it. |

## Hashing and Encryption

A cryptographic primitive can protect or select a watermark, but it does not
make the watermark a separate invisible layer by itself.

- A hidden character stream may encode ciphertext, a digest, an identifier, or
  error-correcting bits. DryMark removes the supported hidden characters; it
  does not need the key and never attempts to recover the payload.
- A keyed token watermark uses a secret to influence which visible next token
  is selected. The detector later checks the visible sequence with the same
  rule. Keeping every visible token unchanged keeps that signal unchanged.
- A content hash or digital signature may bind a generated file to signed
  provenance. That evidence belongs to the file or a referenced manifest, not
  necessarily to text copied from the file.

This boundary is fundamental: no sanitizer can promise both zero changes to a
visible token sequence and guaranteed removal of every watermark encoded by
that same sequence.

## Primary References

- Kirchenbauer et al., [A Watermark for Large Language Models](https://proceedings.mlr.press/v202/kirchenbauer23a.html), describes pseudorandom “green” token sets and statistical detection.
- Christ, Gunn, and Zamir, [Undetectable Watermarks for Language Models](https://arxiv.org/abs/2306.09194), constructs secret-key cryptographic watermarks from pseudorandom functions and explains their rewriting boundary.
- Dathathri et al., [Scalable watermarking for identifying large language model outputs](https://www.nature.com/articles/s41586-024-08025-4), describes context- and key-derived sampling used by SynthID-Text.
- The [C2PA Content Credentials specification](https://spec.c2pa.org/specifications/specifications/2.2/specs/ContentCredentials.html) defines signed, tamper-evident provenance manifests and their asset bindings.

The practical guarantee remains channel-specific: DryMark can prove which
supported copied-text carriers it removed. It cannot prove that arbitrary text
is free of all possible statistical, semantic, file-level, or external marks.
