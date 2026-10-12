#!/usr/bin/env python3
"""criterion 결과를 benches/baseline.json과 비교한다(plan/00-conventions.md: 10% 이상 느려지면 실패).

사용법:
  python3 -I scripts/bench-check.py            # 비교. 회귀가 있으면 종료 코드 1
  python3 -I scripts/bench-check.py --update   # 현재 결과로 기준값을 다시 쓴다

기준값은 측정한 기기에서만 의미가 있다. 기기가 바뀌면(CI 러너 등) 그 기기에서 --update로 다시 잡는다.
"""
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CRITERION = ROOT / "target" / "criterion"
BASELINE = ROOT / "benches" / "baseline.json"
THRESHOLD = 0.10


def current_results():
    """target/criterion 아래 각 벤치의 최신 평균(ns)."""
    results = {}
    for bench_json in CRITERION.glob("**/new/benchmark.json"):
        meta = json.loads(bench_json.read_text())
        estimates = json.loads((bench_json.parent / "estimates.json").read_text())
        results[meta["full_id"]] = estimates["mean"]["point_estimate"]
    return results


def main():
    current = current_results()
    if not current:
        print("bench-check: target/criterion에 결과가 없다. cargo bench를 먼저 돌린다.", file=sys.stderr)
        return 1
    if "--update" in sys.argv[1:]:
        BASELINE.parent.mkdir(exist_ok=True)
        data = {name: round(ns, 1) for name, ns in sorted(current.items())}
        BASELINE.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
        print(f"bench-check: 기준값 {len(data)}개를 {BASELINE.relative_to(ROOT)}에 썼다")
        return 0

    baseline = json.loads(BASELINE.read_text())
    failed = False
    for name, base in sorted(baseline.items()):
        if name not in current:
            # 기준값에 있는 벤치가 사라지면 회귀를 놓치므로 실패로 본다.
            print(f"FAIL {name}: 결과 없음", file=sys.stderr)
            failed = True
            continue
        ratio = current[name] / base
        status = "FAIL" if ratio > 1 + THRESHOLD else "ok  "
        failed |= status == "FAIL"
        print(f"{status} {name}: {current[name]:.1f} ns (기준 {base:.1f}, {ratio - 1:+.1%})")
    for name in sorted(set(current) - set(baseline)):
        print(f"new  {name}: {current[name]:.1f} ns (기준값 없음, --update로 추가)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
