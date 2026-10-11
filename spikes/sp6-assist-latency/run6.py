"""SP-6 러너: 어시스트 요청 지연. 순차 실행(지연 측정이 목적). OPENROUTER_API_KEY는 환경변수에서만."""
import json, os, ssl, sys, time, urllib.request
from fractions import Fraction
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from ksparse import parse
CTX = ssl.create_default_context(cafile="/etc/ssl/cert.pem")
URL = "https://openrouter.ai/api/v1/chat/completions"
MODELS = ["openai/gpt-5.6-luna", "~anthropic/claude-haiku-latest"]


def call(model, messages, max_tokens, timeout=60):
    body = {"model": model, "messages": messages, "max_tokens": max_tokens, "stream": True, "temperature": 0.7,
            "reasoning": {"enabled": False}}
    req = urllib.request.Request(URL, data=json.dumps(body).encode(), headers={
        "Authorization": "Bearer " + os.environ["OPENROUTER_API_KEY"], "Content-Type": "application/json"})
    t0 = time.perf_counter()
    ttft, text, usage, err, finish = None, [], None, None, None
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
                    err = j["error"]; break
                if j.get("usage"):
                    usage = j["usage"]
                for c in j.get("choices", []):
                    p = c.get("delta", {}).get("content")
                    if p:
                        if ttft is None:
                            ttft = time.perf_counter() - t0
                        text.append(p)
                    if c.get("finish_reason"):
                        finish = c["finish_reason"]
    except Exception as e:
        err = f"{type(e).__name__}: {e}"
    return dict(text="".join(text), ttft=ttft, total=time.perf_counter() - t0, usage=usage, error=err, finish=finish)


if __name__ == "__main__":
    reps = int(sys.argv[1]) if len(sys.argv) > 1 else 30
    system = open(os.path.join(HERE, "prompts/system.md")).read()
    tpl = open(os.path.join(HERE, "prompts/task_assist.md")).read()
    ctxs = [open(os.path.join(HERE, f"contexts/ctx{i}.txt")).read().strip() for i in range(3)]
    for n in range(reps):
        for m in MODELS:  # 모델을 번갈아 돌려 시간대 편향을 줄인다
            path = os.path.join(HERE, "runs", f"{m.replace('/', '_').replace('~', '')}__{n}.json")
            if os.path.exists(path):
                continue
            ctx = ctxs[n % 3]
            v = dict(cursor_track="Lead", tempo=140, time="4/4", key="E minor", ctx_bars=8, bars=2, next_from=9, next_to=10, context=ctx)
            user = tpl
            for k, x in v.items():
                user = user.replace("{{" + k + "}}", str(x))
            r = call(m, [{"role": "system", "content": system}, {"role": "user", "content": user}], 300)
            p = parse(r["text"], Fraction(1)) if r["text"] else dict(ok=False, errors=["빈 응답"], warnings=[], tracks=[])
            t = p["tracks"][0] if p["tracks"] else None
            r.update(model=m, n=n, ctx=n % 3, parse_ok=p["ok"], errors=p["errors"], warnings=p["warnings"],
                     bars=(t or {}).get("bars_by_len"), only_lead=len(p["tracks"]) == 1 and p["tracks"][0]["name"].lower() == "lead",
                     ts=time.strftime("%Y-%m-%dT%H:%M:%S%z"), user_chars=len(user))
            json.dump(r, open(path, "w"), ensure_ascii=False, indent=1)
            print(m, n, f"ttft={r['ttft']}", f"total={r['total']:.2f}", p["ok"], r["bars"], flush=True)
