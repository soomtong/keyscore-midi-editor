import json, glob, os, statistics, collections
HERE = os.path.dirname(os.path.abspath(__file__))
rs = [json.load(open(p)) for p in sorted(glob.glob(HERE + "/runs/*.json"))]
def pct(v, q):
    v = sorted(v); k = (len(v) - 1) * q; f = int(k); c = min(f + 1, len(v) - 1)
    return v[f] + (v[c] - v[f]) * (k - f)
by = collections.defaultdict(list)
for r in rs: by[r["model"]].append(r)
print("model | n | ttft p50/p95/max | total p50/p95/max | >2s | parse_ok | 2bars_exact | in tok | out tok | cost/req | errors")
for m, L in by.items():
    tt = [r["ttft"] for r in L if r["ttft"]]; t = [r["total"] for r in L]
    two = sum(1 for r in L if r["parse_ok"] and r["bars"] == "2" and not any(w.startswith("마디") for w in r["warnings"]))
    cost = sum(r["usage"]["cost"] for r in L if r["usage"]) / len(L)
    print(f"{m} | {len(L)} | {pct(tt,.5):.2f}/{pct(tt,.95):.2f}/{max(tt):.2f} | {pct(t,.5):.2f}/{pct(t,.95):.2f}/{max(t):.2f} | {sum(x>2 for x in t)} | {sum(r['parse_ok'] for r in L)}/{len(L)} | {two}/{len(L)} | {statistics.mean(r['usage']['prompt_tokens'] for r in L):.0f} | {statistics.mean(r['usage']['completion_tokens'] for r in L):.0f} | ${cost:.5f} | {sum(1 for r in L if r['error'])}")
    print("   cached_tokens mean", statistics.mean((r["usage"].get("prompt_tokens_details") or {}).get("cached_tokens", 0) for r in L))
    c = collections.Counter(e[:60] for r in L for e in r["errors"]); print("   ", c.most_common(5))
    print("   first-run ts", L[0]["ts"], "last", L[-1]["ts"])
