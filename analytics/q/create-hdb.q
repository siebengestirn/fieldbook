/ Build a date-partitioned HDB release from a certified Parquet export.
/ Arguments: .z.x 0 = export directory, .z.x 1 = release directory.
.pq:use`kx.pq
src:.z.x 0
dst:.z.x 1
system "mkdir -p ",dst
crun:.pq.pq `$src,"/datasets/capture_run.parquet";
defaultdate:first exec date from crun;
partmaps:`journal_observation`sbe_message`sbe_book_level`capture_instrument`capture_price_range`capture_tick_band`capture_run!`source`serialization_symbol`serialization_symbol`symbol`symbol`symbol`venue;
manifest:.j.k raze read0 `$src,"/export-manifest.json";
buildone:{[name]
    if[0=(manifest`datasets)[name][`rows]; :`skipped_empty];
    tbl:select from .pq.pq `$src,"/datasets/",(string name),".parquet";
    if[0=count tbl; :`skipped_empty];
    if[name=`journal_observation; tbl:update source:`$source,direction:`$direction,record_kind:`$record_kind from tbl];
    if[name=`sbe_message; tbl:update message_kind:`$message_kind,mapping_status:`$mapping_status,serialization_symbol:`$serialization_symbol,side:`$side from tbl];
    if[name=`sbe_book_level; tbl:update serialization_symbol:`$serialization_symbol,side:`$side from tbl];
    if[name=`capture_instrument; tbl:update exchange:`$exchange,symbol:`$symbol,price_unit_kind:`$price_unit_kind,quantity_unit_kind:`$quantity_unit_kind from tbl];
    if[name=`capture_price_range; tbl:update symbol:`$symbol from tbl];
    if[name=`capture_tick_band; tbl:update symbol:`$symbol from tbl];
    if[name=`capture_run; tbl:update venue:`$venue,environment:`$environment,journal_code_commit:`$journal_code_commit,journal_provenance_status:`$journal_provenance_status from tbl];
    pc:$[
        name=`journal_observation;`source;
        name in `sbe_message`sbe_book_level;`serialization_symbol;
        name in `capture_instrument`capture_price_range`capture_tick_band;`symbol;
        `venue
        ];
    if[not `date in cols tbl; tbl:update date:defaultdate from tbl];
    dates:exec distinct date from tbl;
    .an.tbl:tbl;
    .an.name:name;
    .an.pc:pc;
    {[d] part:select from .an.tbl where date=d; set[.an.name;part]; .Q.dpft[hsym `$dst;d;.an.pc;.an.name];} each dates;
    `built
    };
names:`journal_observation`sbe_message`sbe_book_level`capture_instrument`capture_price_range`capture_tick_band`capture_run;
statuses:buildone each names;
fh:`$dst,"/../build-result.json";
fh 0: enlist .j.j `datasets`status!(string each names;statuses);
exit 0
