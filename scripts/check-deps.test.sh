#!/usr/bin/env bash
# scripts/check-deps.sh의 회귀 테스트.
# 위반을 하나씩 주입한 워크스페이스 사본에서 check-deps.sh가 실패하는지 본다.
# 검사 스크립트가 「아무것도 잡지 못하는」 상태로 퇴화하면 이 테스트가 잡는다.
# 사례 A~F는 docs/reports/s0-t1/review-1.md의 깨뜨리기 시도에서 왔다.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# 가짜 외부 crate. 이름만 맞으면 검사 대상이 되므로 내용은 비어 있어도 된다.
for name in reqwest ureq; do
  mkdir -p "$work/fake/$name/src"
  : > "$work/fake/$name/src/lib.rs"
  printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2024"\n\n[workspace]\n' "$name" \
    > "$work/fake/$name/Cargo.toml"
done

# 가짜 crate만 쓰므로 레지스트리 접근이 필요 없다. CI·오프라인에서도 같은 결과가 나와야 한다.
export CARGO_NET_OFFLINE=true

fresh_copy() {
  rm -rf "$work/ws"
  mkdir -p "$work/ws"
  cp -R "$root/Cargo.toml" "$root/Cargo.lock" "$root/rust-toolchain.toml" "$root/crates" "$root/scripts" "$work/ws/"
}

passed=0
failed=0

# expect <ok|fail> <이름> <crate 디렉터리> <Cargo.toml 끝에 붙일 내용>
expect() {
  local want=$1 name=$2 crate=$3 snippet=$4
  fresh_copy
  if [ -n "$crate" ]; then
    printf '\n%s\n' "$snippet" >> "$work/ws/crates/$crate/Cargo.toml"
  fi
  local got
  if "$work/ws/scripts/check-deps.sh" >"$work/out.txt" 2>&1; then got=ok; else got=fail; fi
  if [ "$got" = "$want" ]; then
    echo "ok   - $name"
    passed=$((passed + 1))
  else
    echo "FAIL - $name: 기대 $want, 실제 $got" >&2
    sed 's/^/       /' "$work/out.txt" >&2
    failed=$((failed + 1))
  fi
}

fake="$work/fake"

expect ok   "깨끗한 워크스페이스는 통과" "" ""
expect fail "core 직접 의존 reqwest" keyscore-core \
  "reqwest = { path = \"$fake/reqwest\" }"
expect fail "A: core의 target-specific reqwest" keyscore-core \
  "[target.'cfg(windows)'.dependencies]
reqwest = { path = \"$fake/reqwest\" }"
expect fail "C: core의 이름 바꾼 ureq" keyscore-core \
  "foo = { package = \"ureq\", path = \"$fake/ureq\" }"
expect fail "D: core의 dev 의존 keyscore-text" keyscore-core \
  "[dev-dependencies]
keyscore-text.workspace = true"
expect fail "E: core의 dev 의존 reqwest" keyscore-core \
  "[dev-dependencies]
reqwest = { path = \"$fake/reqwest\" }"
expect fail "F: text의 target-specific keyscore-midi" keyscore-text \
  "[target.'cfg(windows)'.dependencies]
keyscore-midi.workspace = true"
expect fail "text의 build 의존 ureq" keyscore-text \
  "[build-dependencies]
ureq = { path = \"$fake/ureq\" }"
expect fail "text → midi 건너뛴 방향" keyscore-text \
  "keyscore-midi.workspace = true"

echo "check-deps.test: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
