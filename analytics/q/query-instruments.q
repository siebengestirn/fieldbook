/ Capture-scoped instrument metadata and numeric specifications.
\l analytics/q/common.q
ins:an.open[an.dir;"capture_instrument"]
m:.j.k raze read0 `$an.dir,"/export-manifest.json"
ds:m`datasets
result:`instruments`by_exchange`by_price_scale`ranges`bands`by_unit`by_quantity_unit!(first exec count i from ins;select count i by exchange from ins;select count i by price_scale from ins;(ds`capture_price_range)`rows;(ds`capture_tick_band)`rows;select count i by price_unit_kind from ins;select count i by quantity_unit_kind from ins)
an.emit[an.results,"/q-instruments.json";result]
exit 0
