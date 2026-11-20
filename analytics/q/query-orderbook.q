/ SBE projection queries: message kinds, snapshot levels and depth.
\l analytics/q/common.q
b:an.open[an.dir;"sbe_message"]
l:an.open[an.dir;"sbe_book_level"]
result:`messages`by_kind`by_mapping`levels`levels_by_side`depth_by_symbol`first_received_us`last_received_us!(first exec count i from b;select count i by message_kind from b;select count i by mapping_status from b;first exec count i from l;select count i by side from l;select count i by serialization_symbol,side from l;first exec min(received_at_us) from b;first exec max(received_at_us) from b)
an.emit[an.results,"/q-orderbook.json";result]
exit 0
