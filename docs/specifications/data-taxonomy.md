# Fieldbook V2 Specification 01: Universal Data Taxonomy

Status: specification. This document applies to the vocabulary declared in
`src/taxonomy.rs`. It defines classification terms; it does not assert that
Fieldbook acquires any represented asset or information.

## Purpose

Provide a stable vocabulary for classifying financial and economic information
that is independent of any source or venue. The vocabulary separates asset
domain, instrument kind, information category, depth model, delivery frequency
and temporal status. Each term is a declared contract string, not a display
label.

## Scope

In scope:

- The six classification dimensions and the country and currency code types.
- Mapping of external classification schemes onto those dimensions.
- Validation and evidence rules for changes to the vocabulary.

Out of scope:

- Instrument identity, entity identity, venues and listings. These belong to
  Specification 02.
- Event schemas, connectors, licensing and geographic coverage.
- Any claim that Fieldbook holds, ingests or distributes represented data.

External standards bear on classification as inputs:

- CFI (ISO 10962:2021, edition 5, confirmed 2026) is a classification input.
- DTI (ISO 24165) is a classification and token-identity input.
- FISN (ISO 18774:2024, edition 2) is a human-readable label, not a key.
- ISO 4217:2015 supplies alphabetic and numeric currency codes.

## Normative rules

1. A classification value shall use the exact snake_case contract string
   declared in `src/taxonomy.rs`.
2. Display names, ticker symbols and locale text shall not serve as
   classification values.
3. CFI and DTI values shall be stored as attributed attributes with effective
   dates. They shall not replace internal instrument identity. A CFI value can
   change on particular corporate events while instrument identity persists.
4. FISN text shall be stored as a label. It shall not be used as a join key.
5. CFI and DTI are classification or identity inputs, not universal keys.
6. A currency code shall be three uppercase ASCII letters at the shape level.
   Membership in the ISO 4217:2015 code list, including fund and metal codes,
   shall be checked against the maintained list. ISO 4217 is under review;
   record the list version used for each validation.
7. A country code shall be two uppercase ASCII letters from ISO 3166-1
   alpha-2. Shape validation alone does not assert code-list membership.
8. Terms shall be additive. A new term requires a declared contract string and
   an update to the uniqueness test. An existing string shall not be
   repurposed for a different meaning.
9. Dimensions shall not be collapsed into one another. DepthModel and
   DataCategory describe distinct properties of an observation.

## Data model

The six dimensions are declared as enums with stable `as_str` projections:

```text
AssetClass       11 terms: equity, fixed_income, digital_asset, ...
InstrumentKind   40 terms: common_share, government_bond, ...
DataCategory     19 terms: level1_quotes, corporate_actions, ...
DepthModel        6 terms: level1, consolidated, participant_entitled
Frequency         5 terms: real_time, end_of_day, on_demand
TemporalStatus    7 terms: historical, corrected, revised
```

Each observation carries one value per applicable dimension. The code types
are declared separately:

| Type | Shape rule | Naming authority |
|---|---|---|
| CountryCode | two uppercase ASCII letters | ISO 3166-1 alpha-2 |
| CurrencyCode | three uppercase ASCII letters | ISO 4217:2015 |

```rust
CountryCode::new("GB");   // valid shape
CurrencyCode::new("USD"); // valid shape
```

The constructors validate shape only. The `InvalidCode` error reports a
`Country` or `Currency` failure. Authority-list membership is a separate
check and is not performed by these constructors.

## Evidence and verification requirements

- Keep the contract strings stable. The test `asset_strings_are_stable` fixes
  the serialization of `AssetClass::FixedIncome` as `fixed_income`.
- Keep declared strings distinct. The test `data_categories_are_distinct`
  sorts and deduplicates every `DataCategory` string. Add an equivalent check
  for any other enum that gains contract strings.
- Record the exact edition, confirmation or review status, retrieval time and
  SHA-256 hash of each downloaded standards artifact or code list.
- Treat a code-list membership result as evidence tied to a named list
  version. Do not infer it from a three-letter shape.
- Run the repository gates `cargo xtask style` and `scripts/check_style.py`
  on changed documentation. Run the Rust test suite on changed vocabulary.

## Source references

- ISO 10962:2021, CFI, edition 5, confirmed 2026:
  https://www.iso.org/standard/81140.html
- ISO 24165, DTI:
  https://www.iso.org/obp/ui/en/#iso:std:iso:24165:ed-1:v1:en
- ISO 18774:2024, FISN, edition 2:
  https://www.iso.org/obp/ui/en/#iso:std:iso:18774:ed-2:v1:en
- ISO 4217:2015, currency codes, under review:
  https://www.iso.org/iso-4217-currency-codes.html
- Internal alignment: `src/taxonomy.rs`, `docs/style-guide.md` and
  `scripts/assurance/english_audit.py`.

## Explicit non-claims

- This document does not claim that Fieldbook acquires, preserves or sells any
  asset class, instrument kind or information category.
- It does not claim that the vocabulary is a complete map of world markets.
- It does not claim that CFI, DTI, FISN or ISO 4217 codes are internal keys.
- It does not claim that shape validation proves code-list membership.
- It does not claim that ISO 4217 is static; the standard is under review.
- It does not define instrument identity, entity identity, venues, listings,
  contracts or their history. Those are covered by Specification 02.
