/ shared helpers for analytics q scripts
/ arguments: .z.x 0 = export directory, .z.x 1 = result directory
.pq:use`kx.pq
an.open:{[dir;name] .pq.pq `$dir,"/datasets/",name,".parquet"}
an.emit:{[path;result] handle:`$path; handle 0: enlist .j.j result}
an.dir:.z.x 0
an.results:.z.x 1
