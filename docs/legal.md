# Legal and Responsible Use

_Status: 13 August 2026. This is general EU-level information, with Germany as
an implementation example. It is not legal advice._

## Short Answer

No reviewed EU rule makes publishing or using an ordinary copied-text
sanitizer inherently unlawful. There is also no general legal right to remove
watermarks. The answer for a particular use depends on who acts, what the mark
does, which content is involved, why it is removed, and the applicable national
law and contracts.

DryMark is intended for authorized clipboard hygiene, privacy,
interoperability, Unicode safety, and testing with synthetic or user-controlled
text. It is not intended to defeat a disclosure duty, copyright protection,
rights-management information, signed provenance, or access controls.

## EU Artificial Intelligence Act

[Article 50 of Regulation (EU) 2024/1689](https://eur-lex.europa.eu/eli/reg/2024/1689/oj)
has applied since 2 August 2026 and places distinct duties on providers and
deployers:

- Providers of AI systems, including general-purpose AI systems, generating
  synthetic audio, image, video, or text content must ensure that their outputs
  are marked in a machine-readable format and detectable as artificially
  generated or manipulated, subject to the limits and exceptions in
  Article 50(2).
- A deployer publishing generated or manipulated text to inform the public on
  matters of public interest must disclose its artificial origin unless the
  text has undergone human review or editorial control and a natural or legal
  person holds editorial responsibility, or another Article 50(4) exception
  applies.
- Article 2(10) excludes the obligations of deployers who are natural persons
  using AI systems in a purely personal, non-professional activity.
  Article 3(4)'s deployer definition likewise excludes personal,
  non-professional use. Other law and contractual duties can still apply.

Under [Regulation (EU) 2026/1744](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32026R1744),
providers of covered systems placed on the market before 2 August 2026 have
until 2 December 2026 to take the necessary steps to comply with
Article 50(2).

Article 50 does not state a general offence for a recipient who removes a
machine-readable mark, nor does it authorize removal. Removing a carrier does
not cancel an independently applicable disclosure duty. A provider cannot use
sanitization in its output pipeline to avoid its own marking duty.

The European Commission's
[Article 50 guidelines](https://digital-strategy.ec.europa.eu/en/library/guidelines-transparency-obligations-providers-and-deployers-ai-systems)
are non-binding guidance. They encourage preservation of marks by downstream
actors, especially in professional distribution chains, but only the Court of
Justice of the European Union can give an authoritative interpretation of the
Act.

## Copyright Protection and Rights Information

“Watermark” is not one legal category. Hashing, encryption, or a secret key
does not determine the legal result; the mark's purpose and operation do.

[Article 6 of Directive 2001/29/EC](https://eur-lex.europa.eu/eli/dir/2001/29/oj)
protects effective technological measures designed to prevent or restrict
copyright-relevant acts not authorized by a rightholder. It also requires
protection against products or services promoted, primarily designed, or of
only limited commercially significant use for that circumvention. A simple
origin-transparency mark normally serves a different function from access or
copy control, but each implementation must be assessed on its facts.

[Article 7 of Directive 2001/29/EC](https://eur-lex.europa.eu/eli/dir/2001/29/oj)
separately protects electronic rights-management information identifying a
work or other protected subject matter, its author or another rightholder,
terms and conditions of use, or numbers and codes representing that
information. Its removal rule covers knowing, unauthorized removal where the
person knows or has reasonable grounds to know that doing so induces, enables,
facilitates, or conceals copyright, related-right, or database-right
infringement. An origin indicator is not
automatically rights-management information; an identifier or payload that
encodes or resolves to authorship, ownership, or licensing data may be.

The Court of Justice held in
[Nintendo v PC Box, C-355/12](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:62012CJ0355)
that anti-circumvention protection concerns measures directed at unauthorized
copyright acts, must be proportionate, and must account for commercially
significant legitimate uses.

Germany implements these rules through
[§95a UrhG](https://www.gesetze-im-internet.de/urhg/__95a.html),
[§95c UrhG](https://www.gesetze-im-internet.de/urhg/__95c.html), and
[§108b UrhG](https://www.gesetze-im-internet.de/urhg/__108b.html). The last
provision creates criminal consequences in specified circumstances. A free or
open-source licence is not an automatic exemption. Other EU Member States have
their own civil, procedural, and criminal implementations.

## Signed Provenance, Deception, and Contracts

[C2PA Content Credentials](https://spec.c2pa.org/specifications/)
are a technical standard, not legislation. A signed manifest can still contain
rights-management information or carry contractual, evidentiary,
consumer-protection, or fraud significance. DryMark processes copied text; it
does not alter file manifests, signatures, certificate chains, sidecars, or
server records.

Removing a mark does not make deception lawful. For business-to-consumer
commercial practices,
[Articles 6 and 7 of Directive 2005/29/EC](https://eur-lex.europa.eu/eli/dir/2005/29/oj)
prohibit misleading actions and material omissions capable of causing a
consumer to take a transactional decision they otherwise would not have taken.
National fraud, unfair-competition, academic, employment, professional, and
evidentiary rules may also apply. Service terms, publishing agreements,
workplace policies, and customer contracts can impose additional duties even
where no statute directly prohibits the technical act.

## Responsible-use Boundary

Use DryMark only on text you own, are authorized to modify, or may lawfully
process. Do not use it to:

- conceal artificial generation where disclosure is legally or contractually
  required;
- falsely claim human authorship or provenance;
- remove copyright rights-management information;
- bypass DRM, access controls, or copy controls;
- alter signed, official, evidentiary, or provenance-bearing files; or
- facilitate fraud, impersonation, plagiarism, or consumer deception.

These limits must remain consistent with the project's actual behavior and
marketing. A disclaimer cannot cure functionality promoted or primarily
designed for unlawful circumvention. Obtain advice from a qualified lawyer for
a real deployment, disputed content, professional publishing, or a planned
feature involving rights data, decryption, signatures, or access control.
