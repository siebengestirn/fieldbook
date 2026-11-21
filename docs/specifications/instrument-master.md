# Fieldbook V2 Specification 02: Universal Instrument Master

Status: specification. This document applies to `src/reference_data.rs` and
the opaque identity types in `src/identity.rs`. It defines an identity graph;
it does not assert that Fieldbook currently holds any listed instrument.

## Purpose

Define permanent identity for entities, instruments, listings, venues, contract
terms and their history. External identifiers are mapped as attributed,
effective-dated assertions. No external identifier is the internal identity.

## Scope

In scope:

- Opaque identity derivation and external identifier mapping.
- Venue master records with MIC roles and venue categories.
- Contract specifications and graph edges with effective dating.
- Resolution outcomes, evidence and GLEIF and OpenFIGI source handling.

Out of scope:

- The classification vocabulary in Specification 01.
- Transaction identifiers, order routing, execution and portfolio management.
- Any claim of data rights or redistribution permission for source files.

## Normative rules

Identity:

1. `InstrumentId`, `VenueId`, `MarketId` and `EntityId` shall be derived from
   explicit immutable inputs: namespace, assigning authority and stable key.
   Display labels, tickers and slugs shall not be derivation inputs.
2. `OpaqueId::derive` uses a unit separator between inputs and takes the first
   12 bytes of a SHA-256 digest. `EntityId` uses prefix `ent`, `MarketId` uses
   `mkt`, `InstrumentId` uses prefix `inst` and `VenueId` uses prefix `venue`.
3. A venue identifier mapping shall bind venue, listing, identifier assertion
   and provenance. The same identifier text at a different venue is a
   different mapping.
4. Resolution shall return one value of `Resolved`, `Ambiguous`, `Unresolved`
   or `Conflict`. Multiple candidates shall yield `Ambiguous`; no silent
   coercion to a first candidate is permitted.

External identifiers:

5. ISIN (ISO 6166:2021, edition 8, published 2021-02, under revision) shall be
   stored as an attributed external identifier with effective dates. An ISIN
   does not encode a trading venue.
6. MIC (ISO 10383:2012, edition 3, confirmed 2023) has three distinct roles:
   listing place, trading place and trade reporting facility. Each use of a
   MIC shall carry its role. The roles shall not be merged into one meaning.
7. CFI (ISO 10962:2021, edition 5, confirmed 2026) shall be stored as a
   classification attribute with provenance and effective dates. A CFI value
   can change on particular corporate events while instrument identity
   persists. CFI is not a key.
8. FISN (ISO 18774:2024, edition 2) is a human-readable label. It shall not be
   used as a join key.
9. LEI (ISO 17442) is 20 characters and identifies a legal entity. An LEI
   shall map to `EntityId` and shall not replace internal identity. The GLEIF
   Global LEI Index holds current and historical entity records and publishes
   free ISIN-to-LEI, MIC-to-LEI and BIC-to-LEI relationship files.
10. FIGI is unique to an instrument and does not change after assignment.
    Composite and share-class FIGIs link venue listings. Open Symbology is MIT
    licensed and the mapping API is free. Unlisted equities are excluded by
    default; a request shall set `includeUnlistedEquities=true` when unlisted
    or delisted equities are in scope, and shall record that setting.
11. UPI (ISO 4914) is the product identifier for OTC derivatives referenced in
    EU Delegated Regulation (EU) 2024/791, recital 24.
12. DTI (ISO 24165) is an eight-character identifier with a checksum assigned
    to a token and is open. DTI is a classification or identity input, not a
    universal key.
13. Currency codes shall follow ISO 4217:2015 alphabetic and numeric codes,
    which cover funds and metals. ISO 4217 is under review; record the list
    version applied to each mapping.

Reference graph:

14. A `Venue` record shall carry a `VenueCategory` and a jurisdiction.
    `ReferenceGraph::insert_venue` rejects a conflicting record for one
    `VenueId`; an identical repeat is idempotent.
15. `InstrumentSpecification` and `GraphEdge` shall carry `valid_from_us` and
    `valid_to_us`. `insert_specification` rejects overlapping validity windows
    for one instrument. `is_valid_at` defines applicability at an observation
    time.
16. `resolve_specification` yields `Unresolved` for zero matches, `Resolved`
    for one and `Ambiguous` for more than one. `resolve_venue_mapping` applies
    the same counts after filtering on `identifier.is_valid_at`. It does not
    infer a cross-listing identity.
17. `resolve_venue` accepts an observation time that the current implementation
    does not apply. Until venue records carry effective dates, callers shall
    not treat venue resolution as time-aware.

## Data model

```text
Entity (issuer)
  -> Instrument (InstrumentId)
       -> Listing (ListingId) -> Venue (VenueId)
       -> External identifiers (ISIN, FIGI, CFI, FISN, LEI, UPI, DTI)
       -> Specification (effective dated)
       -> Graph edges (issued_by, listed_on, successor_of, ...)
```

```rust
enum ReferenceResolution<T> {
    Resolved(T),
    Ambiguous,
    Unresolved,
    Conflict,
}
```

| Object | Declared fields |
|---|---|
| Venue | id, name, mic, category, jurisdiction, settlement_currency |
| InstrumentSpecification | instrument, legal_name, issuer, currency |
| InstrumentSpecification | multiplier, expiry_us, strike_mantissa |
| InstrumentSpecification | valid_from_us, valid_to_us |
| GraphEdge | from, to, relation, valid_from_us, valid_to_us |
| InstrumentIdentifier | scheme, assigning_authority, identifier_value |
| InstrumentIdentifier | scope, valid_from, valid_to, observed_at |
| InstrumentIdentifier | source_provenance |
| VenueInstrumentMapping | instrument, venue, listing, segment, identifier |

`VenueCategory` includes `exchange`, `multilateral_trading_facility`,
`alternative_trading_system`, `dark_pool`, `broker_internalizer`,
`dealer_quotation_system`, `trade_reporting_facility` and
`contributed_quotation_system`. `EdgeRelation` includes `issued_by`,
`listed_on`, `underlying_of`, `option_of`, `deliverable_of`, `successor_of`,
`constituent_of` and `venue_alias_of`.

## Evidence and verification requirements

- Record source, retrieval time, edition, coverage flags and SHA-256 hash for
  each GLEIF or OpenFIGI artifact. Keep downloaded files immutable.
- Verify an LEI has 20 characters before mapping it to an entity. Reject a
  malformed value and report the rejection.
- Verify composite and share-class FIGI links from the returned `compositeFIGI`
  and `shareClassFIGI` fields rather than from ticker text.
- Verify a MIC role from the ISO 10383 maintenance list. Do not infer the role
  from the code value.
- Preserve the current and historical GLEIF records separately. Do not replace
  a historical record with its current counterpart.
- Keep the tests `venue_conflicts_are_rejected`,
  `effective_dated_specifications_resolve_without_overwrite` and
  `venue_mapping_ambiguity_is_not_collapsed` passing on changed logic.
- Report counts of `Ambiguous`, `Unresolved` and `Conflict` outcomes. Account
  for every mapping attempt; do not drop candidates without a recorded result.
- Run `cargo xtask style` and `scripts/check_style.py` on changed documents.
  Run the Rust test suite on changed identity logic.

## Source references

- ISO 6166:2021, ISIN, edition 8, under revision:
  https://www.iso.org/standard/78502.html
- ISIN Organization, structure and assignment:
  https://www.isin.org/
- ISO 10383:2012, MIC, edition 3, confirmed 2023:
  https://www.iso.org/obp/ui/en/#iso:std:iso:10383:ed-3:v1:en
- ISO 10962:2021, CFI, edition 5, confirmed 2026:
  https://www.iso.org/standard/81140.html
- ISO 18774:2024, FISN, edition 2:
  https://www.iso.org/obp/ui/en/#iso:std:iso:18774:ed-2:v1:en
- ISO 17442-1:2020, LEI assignment, confirmed 2026:
  https://www.iso.org/standard/78829.html
- GLEIF Global LEI Index, current and historical records:
  https://www.gleif.org/en/lei-data/global-lei-index
- GLEIF LEI mapping index, ISIN, MIC and BIC relationship files:
  https://www.gleif.org/en/lei-data/lei-mapping
- OpenFIGI, Open Symbology under the MIT license:
  https://www.openfigi.com/
- OpenFIGI API overview, free mapping API:
  https://www.openfigi.com/api
- OpenFIGI documentation, unlisted equities, FIGI properties:
  https://www.openfigi.com/api/documentation
- ISO 4914, UPI, and EU Delegated Regulation (EU) 2024/791, recital 24:
  https://www.iso.org/obp/ui/en/#iso:std:iso:4914:ed-1:v1:en
  https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R0791
- ISO 24165, DTI:
  https://www.iso.org/obp/ui/en/#iso:std:iso:24165:ed-1:v1:en
- ISO 4217:2015, currency codes, under review:
  https://www.iso.org/iso-4217-currency-codes.html
- Internal alignment: `src/reference_data.rs`, `src/identity.rs` and
  `src/taxonomy.rs`.

## Explicit non-claims

- This document does not claim that Fieldbook currently acquires instruments,
  entities, venues, prices or transactions.
- It does not claim that ISIN, MIC, CFI, FISN, FIGI, LEI, UPI or DTI is the
  internal identity. Each is an external assertion in the graph.
- It does not claim that the three MIC roles are interchangeable.
- It does not claim that CFI or FISN text is stable across corporate events.
- It does not claim that OpenFIGI returns unlisted equities by default.
- It does not claim that GLEIF or OpenFIGI files may be redistributed. Source
  terms and usage restrictions apply separately.
- It does not define transaction identifiers such as UTI, order routing or
  trade execution.
- It does not claim that the current time argument in `resolve_venue` is
  applied by the implementation.
