/ Verify one HDB release in a fresh q process against its Parquet manifest.
/ Arguments: .z.x 0 = release directory, .z.x 1 = export directory.
rel:.z.x 0
src:.z.x 1
system "l ",rel
m:.j.k raze read0 `$src,"/export-manifest.json"
b:.j.k raze read0 `$rel,"/../build-result.json"
built:`$ b`datasets
statuses:`$ b`status
skipped:built where statuses=`skipped_empty
names:`journal_observation`sbe_message`sbe_book_level`capture_instrument`capture_price_range`capture_tick_band`capture_run
countone:{[nm] if[nm in skipped; :0]; :count value nm;};
counts:countone each names;
result:`datasets`rows`manifest_rows!(string each names;counts;(m`row_counts)names);
oh:`$rel,"/../hdb-verify.json";
oh 0: enlist .j.j result;
exit 0
