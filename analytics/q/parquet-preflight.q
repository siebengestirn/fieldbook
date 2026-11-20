/ KDB-X Parquet preflight against a published export.
\l analytics/q/common.q
j:an.open[an.dir;"journal_observation"]
result:`q_version`q_date`journal_rows`by_record_kind!(
    .z.K;
    string .z.k;
    first exec count i from j;
    select count i by record_kind from j
    )
an.emit[an.results,"/q-preflight.json";result]
exit 0
