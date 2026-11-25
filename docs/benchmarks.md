# Speed test: Gale vs aria2c vs curl

Measured 2026-11-26 on a Linux cloud VM, 2 CPUs, 7 GB RAM; client and server on the same machine (localhost). Median of three runs; every downloaded file was checked against a reference SHA-256.

Run it yourself:

```
cargo build --release -p gale-core --example bench_server --example bench_get
scripts/bench/run.sh
```

- **Gale**: `bench_get` (the app's download engine), 16 connections.
- **aria2c** 1.37.0: `-x16 -s16 -k1M --max-tries=0 --retry-wait=1`.
- **curl** 8.5.0: one connection, `--retry 30 --retry-all-errors -C -`.
- Server: `bench_server` (in `crates/gale-core/examples`), serving generated data from memory.

| Test | Gale | aria2c | curl |
|---|---|---|---|
| A. 256 MiB, server slows each connection to 4 MiB/s | 57.0 MiB/s (4.5 s) | 59.5 MiB/s (4.3 s) | 4.0 MiB/s (64.0 s) |
| B. 1 GiB, no limits, local server | 370.1 MiB/s (2.8 s) | 388.1 MiB/s (2.6 s) | 1155.8 MiB/s (0.9 s) |
| C. 128 MiB, server allows 4 connections at 2 MiB/s | 7.9 MiB/s (16.3 s) | 7.7 MiB/s (16.7 s) | 2.0 MiB/s (64.0 s) |
| D. 256 MiB, unreliable server | 117.6 MiB/s (2.2 s) | 352.4 MiB/s (0.7 s) | gave up (connection dropped, error 18) |

Single runs (MiB/s, or the error):

- A: gale [56.8, 57.0, 57.0]; aria2c [59.8, 56.8, 59.5]; curl [4.0, 4.0, 4.0]
- B: gale [357.8, 412.2, 370.1]; aria2c [443.0, 388.1, 381.5]; curl [1288.8, 1120.6, 1155.8]
- C: gale [7.9, 7.9, 7.9]; aria2c [7.7, 7.7, 7.7]; curl [2.0, 2.0, 2.0]
- D: gale [117.6, 69.2, 121.8]; aria2c [353.2, 339.7, 352.4]; curl ['fail(exit 18: curl: (52) Empty reply from server curl: (18) transfer closed with 259063808 bytes remaining to read curl: (18) transfer)', 'fail(exit 52: curl: (18) transfer closed with 257884160 bytes remaining to read curl: (18) transfer closed with 267059200 bytes remain)', 'fail(exit 18: curl: (18) transfer closed with 267976704 bytes remaining to read curl: (52) Empty reply from server curl: (18) transfer)']

## What it shows

- A, C: with servers that slow down or limit each connection, several connections are what make the difference; Gale and aria2c are level.
- B: without limits, one connection over loopback (curl) is fastest on this 2-CPU machine. Splitting costs CPU that a real internet line never notices; on a normal home connection the line is the limit for all three.
- D: aria2c fetches small 1 MiB pieces, so a dropped connection costs little. Gale reconnects right after a drop that delivered data (0.7.0); before that fix this test took Gale 33.5 s.
- Not measured: IDM and other Windows-only tools; real internet servers (results there depend on the server and the line).
