#!/usr/bin/env python3
"""criterion 벤치를 돌려 benches/baseline.json과 비교한다(plan/00-conventions.md: 10% 이상 느려지면 실패).

사용법:
  python3 -I scripts/bench-check.py            # 벤치를 새로 돌려 비교. 회귀가 있으면 종료 코드 1
  python3 -I scripts/bench-check.py --update   # 벤치를 새로 돌려 기준값을 다시 쓴다
  --results DIR    벤치를 돌리지 않고 DIR의 criterion 결과를 읽는다(테스트용)
  --baseline FILE  기준값 파일 경로(기본 benches/baseline.json)

벤치는 매번 새 임시 디렉터리(CRITERION_HOME)에 결과를 쓴다. target/criterion에 남은 낡은 결과나
이전 실행의 결과가 섞이면 회귀를 놓치기 때문이다(S0-T3 리뷰 1 지적 3·4).
기준값은 측정한 기기에서만 의미가 있다. 기기가 바뀌면(CI 러너 등) 그 기기에서 --update로 다시 잡는다.
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
THRESHOLD = 0.10


def run_benches(out_dir):
    env = dict(os.environ, CRITERION_HOME=str(out_dir))
    subprocess.run(["cargo", "bench", "--workspace"], cwd=ROOT, env=env, check=True)


def read_results(results_dir):
    """결과 디렉터리 아래 각 벤치의 평균(ns). 같은 full_id가 둘 이상이면 오류."""
    results = {}
    duplicates = set()
    for bench_json in sorted(results_dir.glob("**/new/benchmark.json")):
        name = json.loads(bench_json.read_text())["full_id"]
        mean = json.loads((bench_json.parent / "estimates.json").read_text())["mean"]["point_estimate"]
        if name in results:
            duplicates.add(name)
        results[name] = mean
    return results, duplicates


def update(current, baseline_path):
    # 소수 셋째 자리까지 남기되 0이 되지 않게 한다. 0이면 다음 비교에서 나눌 수 없다.
    data = {name: max(round(ns, 3), 0.001) for name, ns in sorted(current.items())}
    baseline_path.parent.mkdir(parents=True, exist_ok=True)
    baseline_path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"bench-check: 기준값 {len(data)}개를 {baseline_path}에 썼다")
    return 0


def compare(current, baseline_path):
    baseline = json.loads(baseline_path.read_text())
    failed = False
    for name, base in sorted(baseline.items()):
        if name not in current:
            # 기준값에 있는 벤치가 사라지면 회귀를 놓치므로 실패로 본다.
            print(f"FAIL {name}: 결과 없음", file=sys.stderr)
            failed = True
            continue
        if base <= 0:
            print(f"FAIL {name}: 기준값이 0 이하다({base}). --update로 다시 잡는다", file=sys.stderr)
            failed = True
            continue
        ratio = current[name] / base
        status = "FAIL" if ratio > 1 + THRESHOLD else "ok  "
        failed |= status == "FAIL"
        print(f"{status} {name}: {current[name]:.1f} ns (기준 {base:.1f}, {ratio - 1:+.1%})")
    for name in sorted(set(current) - set(baseline)):
        print(f"new  {name}: {current[name]:.1f} ns (기준값 없음, --update로 추가)")
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--results", type=pathlib.Path)
    parser.add_argument("--baseline", type=pathlib.Path, default=ROOT / "benches" / "baseline.json")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as tmp:
        results_dir = args.results
        if results_dir is None:
            results_dir = pathlib.Path(tmp)
            run_benches(results_dir)
        current, duplicates = read_results(results_dir)

    if not current:
        print("bench-check: 결과가 없다", file=sys.stderr)
        return 1
    if duplicates:
        # 어느 것이 최신인지 알 수 없다. 하나를 고르면 회귀를 숨길 수 있다.
        print(f"bench-check: 같은 벤치 이름의 결과가 중복이다: {sorted(duplicates)}", file=sys.stderr)
        return 1
    if args.update:
        return update(current, args.baseline)
    return compare(current, args.baseline)


if __name__ == "__main__":
    sys.exit(main())
