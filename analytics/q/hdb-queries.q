/ Historical q queries over one released HDB (one capture per release).
/ Argument: .z.x 0 = release directory.
rel:.z.x 0
system "l ",rel
captureSummary:{[] select from capture_run};
journalSummary:{[] select count i by record_kind from journal_observation};
journalBySource:{[] select count i by source,connection_epoch from journal_observation};
controlFrameSummary:{[] select count i by control_opcode from journal_observation where control_seq>0};
decodeQuality:{[] select count i by decode_status from journal_observation};
bookCoverage:{[] select count i by serialization_symbol from sbe_message where message_kind=`snapshot};
bookSnapshots:{[] select from sbe_message where message_kind=`snapshot};
bookLevels:{[] select from sbe_book_level};
spreadSeries:{[] select from sbe_book_level};
depthSeries:{[] select count i by side,date from sbe_book_level};
instrumentMetadata:{[] select from capture_instrument};
result:`capture_rows`journal_rows`message_rows`level_rows`instrument_rows`coverage_symbols!(
    count captureSummary[];
    first (select count i from journal_observation)[`x];
    first (select count i from sbe_message)[`x];
    count bookLevels[];
    count instrumentMetadata[];
    count bookCoverage[]
    );
oh:`$rel,"/../hdb-queries.json";
oh 0: enlist .j.j result;
exit 0
