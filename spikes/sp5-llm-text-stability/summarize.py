import json, glob, os, statistics, collections, re
HERE = os.path.dirname(os.path.abspath(__file__))
rs = [json.load(open(p)) for p in sorted(glob.glob(HERE + "/runs/*.json"))]
def pct(v, q):
    v = sorted(v); k = (len(v) - 1) * q; f = int(k); c = min(f + 1, len(v) - 1)
    return v[f] + (v[c] - v[f]) * (k - f)
by = collections.defaultdict(list)
for r in rs: by[r["model"] + " [" + r["tag"] + "]"].append(r)
print("model | n | parse_ok | bars_ok | p50 | p95 | ttft p50 | out tok mean | cost")
for m, L in by.items():
    t = [r["total"] for r in L]; tt = [r["ttft"] for r in L if r["ttft"]]
    ot = [r["usage"]["completion_tokens"] for r in L if r["usage"]]
    cost = sum(r["usage"].get("cost", 0) for r in L if r["usage"])
    print(f"{m} | {len(L)} | {sum(r['parse_ok'] for r in L)}/{len(L)} | {sum(r['bars_ok'] for r in L)}/{len(L)} | {pct(t,.5):.1f} | {pct(t,.95):.1f} | {pct(tt,.5):.1f} | {statistics.mean(ot):.0f} | ${cost:.4f}")
print()
print("per fixture parse_ok/bars_ok")
for m, L in by.items():
    row = []
    for f in sorted({r["fixture"] for r in L}):
        x = [r for r in L if r["fixture"] == f]
        row.append(f"{f}: {sum(r['parse_ok'] for r in x)}/{sum(r['bars_ok'] for r in x)}")
    print(m, " | ".join(row))
print()
print("errors (normalized)")
for m, L in by.items():
    c = collections.Counter()
    for r in L:
        for e in r["parse"]["errors"]:
            c[re.sub(r"L\d+: ", "", re.sub(r"track \S+: ", "", e))[:70]] += 1
        for p in r["structure_problems"]: c["STRUCT " + p] += 1
    print(m); [print("   ", n, k) for k, n in c.most_common(12)]
amb = collections.Counter()
for r in rs:
    for w in r["parse"]["warnings"]:
        if w.startswith("AMBIG"): amb[re.sub(r"L\d+ ", "", w)[:60]] += 1
print("AMBIG warnings", amb.most_common(8))
bw = collections.Counter(); 
for m, L in by.items(): bw[m] = sum(1 for r in L if r["bar_warnings"])
print("runs with bar-length warnings", dict(bw))

print()
print("extra metrics: pipe_ok (parse ok & every track has 8 bar-line segments & structure ok), exact (parse ok & no bar warnings & bars_ok), truncated(finish=length), non-truncated parse_ok")
for m, L in by.items():
    pipe = sum(1 for r in L if r["parse_ok"] and not r["structure_problems"] and r["parse"]["tracks"] and all(t["bars_by_pipe"] == 8 for t in r["parse"]["tracks"]))
    exact = sum(1 for r in L if r["bars_ok"] and not r["bar_warnings"])
    tr = [r for r in L if r["finish"] == "length"]
    nt = [r for r in L if r["finish"] != "length"]
    print(f"{m} | pipe_ok {pipe}/{len(L)} | exact {exact}/{len(L)} | truncated {len(tr)} | parse_ok excl. truncated {sum(r['parse_ok'] for r in nt)}/{len(nt)} | structure_problems {sum(1 for r in L if r['structure_problems'])}")
