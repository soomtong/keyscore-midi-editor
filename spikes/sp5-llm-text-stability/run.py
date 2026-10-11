"""SP-5 러너. OPENROUTER_API_KEY는 환경변수에서만 읽고 어디에도 쓰지 않는다."""
import json, os, ssl, sys, time, re, threading, urllib.request
from concurrent.futures import ThreadPoolExecutor
from fractions import Fraction
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ksparse import parse

HERE = os.path.dirname(os.path.abspath(__file__))
CTX = ssl.create_default_context(cafile="/etc/ssl/cert.pem")
URL = "https://openrouter.ai/api/v1/chat/completions"
TAG = os.environ.get("SP5_TAG", "rdefault")
EXTRA = {"reasoning": {"enabled": False}} if TAG == "rnone" else None
MODELS = ["openai/gpt-5.6-luna", "~anthropic/claude-haiku-latest", "google/gemma-3-12b-it"]


def render(tpl, fx):
    tracks = "; ".join(
        f"@track {t['name']} ch{t['ch']}" + (f" prog{t['prog']}" if t['prog'] is not None else "") + f" ({t['role']})"
        for t in fx["tracks"])
    v = dict(bars=fx["bars"], tempo=fx["tempo"], time=fx["time"], bar_desc=fx["bar_desc"], key=fx["key"], tracks=tracks, request=fx["request"])
    for k, x in v.items():
        tpl = tpl.replace("{{" + k + "}}", str(x))
    return tpl


def call(model, messages, max_tokens, extra=None, timeout=90):
    body = {"model": model, "messages": messages, "max_tokens": max_tokens, "stream": True, "temperature": 0.7}
    if extra:
        body.update(extra)
    req = urllib.request.Request(URL, data=json.dumps(body).encode(), headers={
        "Authorization": "Bearer " + os.environ["OPENROUTER_API_KEY"], "Content-Type": "application/json"})
    t0 = time.perf_counter()
    ttft = None
    text, usage, err, finish = [], None, None, None
    try:
        with urllib.request.urlopen(req, timeout=timeout, context=CTX) as r:
            for line in r:
                line = line.decode().strip()
                if not line.startswith("data:"):
                    continue
                d = line[5:].strip()
                if d == "[DONE]":
                    break
                j = json.loads(d)
                if "error" in j:
                    err = j["error"]
                    break
                if j.get("usage"):
                    usage = j["usage"]
                for c in j.get("choices", []):
                    delta = c.get("delta", {})
                    piece = delta.get("content")
                    if piece:
                        if ttft is None:
                            ttft = time.perf_counter() - t0
                        text.append(piece)
                    if c.get("finish_reason"):
                        finish = c["finish_reason"]
    except urllib.error.HTTPError as e:
        err = f"HTTP {e.code}: {e.read().decode()[:300]}"
    except Exception as e:
        err = f"{type(e).__name__}: {e}"
    total = time.perf_counter() - t0
    return dict(text="".join(text), ttft=ttft, total=total, usage=usage, error=err, finish=finish)


def bar_len(ts):
    a, b = ts.split("/")
    return Fraction(int(a), int(b))


def one(args):
    fx, model, n, system, tpl, extra = args
    tag = TAG
    path = os.path.join(HERE, "runs", f"{fx['id']}__{model.replace('/', '_').replace('~', '')}__{tag}__{n}.json")
    if os.path.exists(path):
        return
    msgs = [{"role": "system", "content": system}, {"role": "user", "content": render(tpl, fx)}]
    r = call(model, msgs, 2500, extra)
    r.update(fixture=fx["id"], model=model, n=n, tag=tag, ts=time.strftime("%Y-%m-%dT%H:%M:%S%z"))
    p = parse(r["text"], bar_len(fx["time"])) if r["text"] else dict(ok=False, errors=["빈 응답" if not r["error"] else "API 오류"], warnings=[], tracks=[])
    # 기대 트랙·마디 수 검사
    exp = {t["name"].lower(): t for t in fx["tracks"]}
    got = {t["name"].lower(): t for t in p["tracks"]}
    probs = []
    for k, t in exp.items():
        g = got.get(k)
        if not g:
            probs.append(f"트랙 누락 {t['name']}")
        elif g["ch"] != t["ch"]:
            probs.append(f"채널 불일치 {t['name']}")
    for g in p["tracks"]:
        if g["name"].lower() not in exp:
            probs.append(f"예상 밖 트랙 {g['name']}")
    bars_ok = bool(p["tracks"]) and not probs
    for g in p["tracks"]:
        if g.get("bars_by_len") is None or Fraction(g["bars_by_len"]) != fx["bars"]:
            bars_ok = False
    if fx["id"] == "f4_vocal_lyrics" and p["tracks"]:
        g = p["tracks"][0]
        if not g.get("lyrics"):
            probs.append("가사 없음")
    r["parse"] = p
    r["parse_ok"] = p["ok"]
    r["bars_ok"] = bars_ok and p["ok"]
    r["structure_problems"] = probs
    r["bar_warnings"] = [w for w in p["warnings"] if w.startswith("마디")]
    with open(path, "w") as f:
        json.dump(r, f, ensure_ascii=False, indent=1)
    print(fx["id"], model, n, "ok" if p["ok"] else "FAIL", f"{r['total']:.1f}s", flush=True)


if __name__ == "__main__":
    reps = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    conc = int(sys.argv[2]) if len(sys.argv) > 2 else 3
    system = open(os.path.join(HERE, "prompts/system.md")).read()
    tpl = open(os.path.join(HERE, "prompts/task_new.md")).read()
    fxs = json.load(open(os.path.join(HERE, "fixtures/fixtures.json")))
    only = sys.argv[3:] 
    jobs = [(fx, m, n, system, tpl, EXTRA) for fx in fxs for m in MODELS if not only or m in only for n in range(reps)]
    with ThreadPoolExecutor(conc) as ex:
        list(ex.map(one, jobs))
