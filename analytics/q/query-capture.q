/ Capture summary query over the Parquet virtual table.
\l analytics/q/common.q
c:an.open[an.dir;"capture_run"]
result:`rows`venue`environment`journal_run_id`provenance`expected`anchored`sbe_messages`journal_records!(first exec count i from c;first exec venue from c;first exec environment from c;first exec journal_run_id from c;first exec journal_provenance_status from c;first exec expected_symbols from c;first exec anchored_symbols from c;first exec sbe_messages from c;first exec journal_records from c)
an.emit[an.results,"/q-capture.json";result]
exit 0
