/ Journal observations: counts, ordering domains and decode quality.
\l analytics/q/common.q
j:an.open[an.dir;"journal_observation"]
result:`rows`by_schema`by_source`by_kind`by_direction`by_decode`latest_received_us!(first exec count i from j;select count i by journal_schema_version from j;select count i by source,connection_epoch from j;select count i by record_kind from j;select count i by direction from j;select count i by decode_status from j;first exec max(received_at_us) from j)
an.emit[an.results,"/q-journal.json";result]
exit 0
