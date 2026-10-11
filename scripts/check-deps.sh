#!/usr/bin/env bash
# crate 사이 의존 방향(REQ 9.6, plan/00-conventions.md)과
# core·text의 금지 의존(GPUI·CoreMIDI·네트워크)을 검사한다.
# Cargo.toml 리뷰만으로는 전이 의존을 놓치므로 cargo tree로 실제 그래프를 본다.
set -euo pipefail

cd "$(dirname "$0")/.."

fail=0
errlog=$(mktemp)
trap 'rm -f "$errlog"' EXIT

# cargo의 경고(stderr)가 그래프 출력에 섞이지 않게 stdout만 쓴다.
tree_of() {
  if ! cargo tree -p "$1" -e normal,build --prefix none --format '{p}' "${@:2}" 2>"$errlog"; then
    echo "check-deps: $1: cargo tree 실패" >&2
    cat "$errlog" >&2
    return 1
  fi
}

# crate별로 허용된 워크스페이스 내부 직접 의존. 정확히 이 집합이어야 한다.
# 빠진 의존도 실패로 보는 이유: 방향을 선언으로 고정해 두어야 이후 Task가
# 「없으니까 아무 방향이나」 추가하는 일을 리뷰 전에 막을 수 있다.
expected_internal() {
  case "$1" in
    keyscore-core) echo "" ;;
    keyscore-text) echo "keyscore-core" ;;
    keyscore-midi) echo "keyscore-core" ;;
    keyscore-ai) echo "keyscore-core keyscore-text" ;;
    keyscore-app) echo "keyscore-ai keyscore-core keyscore-midi keyscore-text" ;;
  esac
}

crates="keyscore-core keyscore-text keyscore-midi keyscore-ai keyscore-app"

for crate in $crates; do
  # depth 1 = 직접 의존. normal·build 의존만 본다(dev 의존은 제품 그래프가 아니다).
  if ! tree=$(tree_of "$crate" --depth 1); then
    fail=1
    continue
  fi
  # 첫 줄은 crate 자신이다. 위치가 아니라 이름으로 걸러야 출력 형식 변화에 덜 민감하다.
  actual=$(echo "$tree" | awk '{print $1}' | grep -E '^keyscore-' | grep -vx "$crate" | sort -u | tr '\n' ' ' | sed 's/ $//' || true)
  expected=$(expected_internal "$crate")
  if [ "$actual" != "$expected" ]; then
    echo "check-deps: $crate 내부 의존이 다르다: 실제 [$actual], 기대 [$expected]" >&2
    fail=1
  fi
done

# core·text는 UI·MIDI·네트워크 없이 CLI와 속성 테스트로 돌아야 한다(REQ 9.6).
forbidden='^(gpui|gpui-.*|gpui_.*|coremidi|coremidi-sys|core-midi-sys|reqwest|hyper|tokio)$'
for crate in keyscore-core keyscore-text; do
  if ! tree=$(tree_of "$crate"); then
    fail=1
    continue
  fi
  bad=$(echo "$tree" | awk '{print $1}' | grep -E "$forbidden" | sort -u | tr '\n' ' ' || true)
  if [ -n "$bad" ]; then
    echo "check-deps: $crate가 금지 crate에 의존한다: $bad" >&2
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "check-deps: ok"
