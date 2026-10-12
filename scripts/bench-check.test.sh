#!/usr/bin/env bash
# scripts/bench-check.py의 회귀 테스트. 가짜 criterion 결과 디렉터리로 시험한다.
# 사례는 docs/reports/s0-t3/review-1.md 지적 3–5에서 왔다.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# fake_result <디렉터리> <full_id> <평균 ns>
fake_result() {
  local dir="$1/$(echo "$2" | tr '/' '_')_$RANDOM/new"
  mkdir -p "$dir"
  printf '{"full_id": "%s"}\n' "$2" > "$dir/benchmark.json"
  printf '{"mean": {"point_estimate": %s}}\n' "$3" > "$dir/estimates.json"
}

passed=0
failed=0

# expect <ok|fail> <이름> <출력에 있어야 할 문자열> -- <bench-check 인자...>
expect() {
  local want=$1 name=$2 reason=$3
  shift 4
  local got
  if python3 -I "$root/scripts/bench-check.py" "$@" >"$work/out.txt" 2>&1; then got=ok; else got=fail; fi
  if [ "$got" = "$want" ] && grep -qF -- "$reason" "$work/out.txt"; then
    echo "ok   - $name"
    passed=$((passed + 1))
  else
    echo "FAIL - $name: 기대 $want('$reason'), 실제 $got" >&2
    sed 's/^/       /' "$work/out.txt" >&2
    failed=$((failed + 1))
  fi
}

base="$work/baseline.json"
printf '{"g/a": 100.0, "g/b": 200.0}\n' > "$base"

r="$work/r1"; fake_result "$r" g/a 105; fake_result "$r" g/b 190
expect ok "10% 안은 통과" "ok   g/a" -- --results "$r" --baseline "$base"

r="$work/r2"; fake_result "$r" g/a 111; fake_result "$r" g/b 200
expect fail "10% 넘게 느려지면 실패" "FAIL g/a" -- --results "$r" --baseline "$base"

r="$work/r3"; fake_result "$r" g/a 100
expect fail "기준값에 있는 벤치가 결과에 없으면 실패" "FAIL g/b: 결과 없음" -- --results "$r" --baseline "$base"

r="$work/r4"; fake_result "$r" g/a 500; fake_result "$r" g/a 100; fake_result "$r" g/b 200
expect fail "같은 full_id의 결과가 둘이면 실패(리뷰 지적 3)" "중복" -- --results "$r" --baseline "$base"

printf '{"g/a": 0.0, "g/b": 200.0}\n' > "$work/zero.json"
r="$work/r5"; fake_result "$r" g/a 0.01; fake_result "$r" g/b 200
expect fail "기준값 0은 0 나눗셈 없이 실패(리뷰 지적 5)" "기준값이 0 이하" -- --results "$r" --baseline "$work/zero.json"

r="$work/r6"; fake_result "$r" g/a 0.01; fake_result "$r" g/b 200
expect ok "--update는 아주 작은 값을 0으로 쓰지 않는다" "기준값 2개" -- --results "$r" --baseline "$work/upd.json" --update
# 파일이 없으면 검사가 아무것도 하지 않으므로 존재를 먼저 요구한다.
if [ ! -f "$work/upd.json" ] || grep -qE '"g/a": 0(\.0)?[,}]' "$work/upd.json"; then
  echo "FAIL - --update가 파일을 안 썼거나 0을 썼다" >&2; failed=$((failed + 1))
else
  echo "ok   - --update 결과에 0 없음"; passed=$((passed + 1))
fi

r="$work/empty"; mkdir -p "$r"
expect fail "결과가 비면 실패" "결과가 없다" -- --results "$r" --baseline "$base"

echo "bench-check.test: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
