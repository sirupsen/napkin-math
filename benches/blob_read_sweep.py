#!/usr/bin/env python3
"""Blob storage range GETs, one at a time on one warm connection, against size.

The same shape as the disk rows' `storage --sweep`, so the numbers compare: random offsets, one
request in flight, p50 and p99 to the first byte and to the last. No dependencies.

    blob_read_sweep.py objects.txt [--secs 30] [--sizes 4096,8192,...] [--same-offset]

--same-offset rereads one range: what's left of the latency once the disks are out of the way.

objects.txt has one `url size` per line: S3 presigned URLs, or plain GCS URLs with a bearer token in
$BLOB_TOKEN. Use old objects spread over a large range, or you measure a cache.
"""
import http.client, os, random, sys, time, urllib.parse

args = sys.argv[1:]
secs = int(args[args.index("--secs") + 1]) if "--secs" in args else 30
sizes = [4096, 8192, 65536, 131072, 262144, 1 << 20, 4 << 20, 16 << 20]
if "--sizes" in args:
    sizes = [int(s) for s in args[args.index("--sizes") + 1].split(",")]
objects = [(u, int(n)) for u, n in (line.split() for line in open(args[0]) if line.strip())]
host = urllib.parse.urlsplit(objects[0][0]).netloc
token = os.environ.get("BLOB_TOKEN")
same = "--same-offset" in args
conn = http.client.HTTPSConnection(host)
buf = bytearray(16 << 20)


def get(url, start, size):
    parts = urllib.parse.urlsplit(url)
    path = parts.path + ("?" + parts.query if parts.query else "")
    headers = {"Range": f"bytes={start}-{start + size - 1}"}
    if token:
        headers["Authorization"] = "Bearer " + token
    t = time.perf_counter()
    conn.request("GET", path, headers=headers)
    r = conn.getresponse()
    assert r.status == 206, (r.status, r.read(300))
    first = time.perf_counter() - t
    while r.readinto(buf):
        pass
    return first * 1e3, (time.perf_counter() - t) * 1e3


print(f"{len(objects)} objects on {host.split('.')[-3:]}, one request at a time, {secs} s per size")
print("| size | requests | first byte p50 / p99 ms | whole p50 / p99 ms | MiB/s |")
print("| ---- | -------- | ----------------------- | ------------------ | ----- |")
for size in sizes:
    url, n = random.choice(objects)
    get(url, 0, min(size, 4096))
    first, whole = [], []
    start = time.time()
    while time.time() - start < secs:
        url, n = objects[0] if same else random.choice(objects)
        f, w = get(url, 0 if same else random.randrange(0, n - size) // size * size, size)
        first.append(f)
        whole.append(w)
    pct = lambda v, p: sorted(v)[int((len(v) - 1) * p)]
    print(
        f"| {size} | {len(whole)} | {pct(first, .5):.1f} / {pct(first, .99):.1f} | "
        f"{pct(whole, .5):.1f} / {pct(whole, .99):.1f} | {size * len(whole) / sum(whole) * 1e3 / (1 << 20):.0f} |",
        flush=True,
    )
