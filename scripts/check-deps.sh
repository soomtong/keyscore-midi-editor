#!/usr/bin/env bash
# crate 사이 의존 방향(REQ 9.6, plan/00-conventions.md)과
# core·text의 금지 의존(GPUI·CoreMIDI·네트워크)을 검사한다.
# Cargo.toml 리뷰만으로는 전이 의존을 놓치므로 cargo tree로 실제 그래프를 본다.
# 회귀 테스트: scripts/check-deps.test.sh
set -euo pipefail

cd "$(dirname "$0")/.."

fail=0
errlog=$(mktemp)
trap 'rm -f "$errlog"' EXIT

# 모든 타깃·모든 의존 종류·모든 feature를 본다. 호스트 타깃만 보면 cfg(windows) 같은
# 플랫폼별 의존이, normal만 보면 dev·build 의존이(review-1 A~F), 기본 feature만 보면
# 다른 crate가 켜는 optional 의존이(review-2 #1) 검사를 빠져나간다.
# cargo의 경고(stderr)는 그래프 출력에 섞이지 않게 따로 받는다.
tree_of() {
  local crate=$1
  shift
  local out
  if ! out=$(cargo tree -p "$crate" --target all --all-features -e normal,build,dev --prefix none --format '{p}' "$@" 2>"$errlog"); then
    echo "check-deps: $crate: cargo tree 실패" >&2
    cat "$errlog" >&2
    return 1
  fi
  # 출력 형식이 바뀌어 빈 결과가 나오면 모든 검사가 조용히 통과하므로, 첫 줄이 crate 자신인지 확인한다.
  if [ "$(echo "$out" | head -n 1 | awk '{print $1}')" != "$crate" ]; then
    echo "check-deps: $crate: cargo tree 출력의 첫 줄이 crate 자신이 아니다" >&2
    echo "$out" | head -n 3 >&2
    return 1
  fi
  echo "$out"
}

# 직접 의존 이름 목록(자신 제외, 중복 제거, 공백 구분).
direct_deps() {
  echo "$1" | tail -n +2 | awk '{print $1}' | sort -u | tr '\n' ' ' | sed 's/ $//'
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
    # 픽스처·변환 도구는 앱 없이 돌아야 하므로 core와 text만 쓴다(plan/00-conventions.md).
    ksq-cli) echo "keyscore-core keyscore-text" ;;
    # 재생 도구는 모델을 읽어 MIDI로 보낸다(S1-T7). text·ai·app에 기대지 않는다.
    ksq-play) echo "keyscore-core keyscore-midi" ;;
  esac
}

# core·text·ksq-cli가 직접 쓸 수 있는 외부 crate(normal·build·dev 모두). 이 목록에 없는 이름은 실패다.
# 금지 목록은 닫혀 있어 새 네트워크·UI crate를 놓치므로(review-1 C), 직접 의존은 허용 목록으로 본다.
# REQ 9.6 표의 의존 열(serde, serde_json)과 plan/00-conventions.md의 도구(thiserror, proptest,
# insta, criterion)가 출발점이다. 새 crate가 필요한 Task는 여기에 추가하고 PR에서 리뷰받는다.
allowed_external() {
  case "$1" in
    # flate2: .ksqz gzip 저장(REQ IO-1, S0-T6).
    keyscore-core) echo "serde serde_json thiserror flate2 proptest insta criterion" ;;
    keyscore-text) echo "thiserror proptest insta criterion" ;;
    # serde_json·flate2: 파일의 format_version과 모르는 키를 직접 본다(core가 이미 쓰는 crate).
    ksq-cli) echo "thiserror serde_json flate2 proptest" ;;
  esac
}

# 허용 목록을 거친 전이 의존에서도 UI·MIDI·네트워크가 들어오지 않아야 한다(core·text·ksq-cli).
forbidden='^(gpui|gpui[-_].*|zed[-_].*|coremidi.*|core[-_]midi.*|objc|objc2.*|cocoa.*|core[-_]foundation.*|core[-_]graphics.*|metal|block2?|dispatch2?|reqwest|ureq|hyper|hyper[-_].*|h2|h3|http|http[-_].*|tokio|tokio[-_].*|async[-_]std|smol|mio|socket2|rustls.*|native[-_]tls|openssl.*|security[-_]framework.*|curl|curl[-_]sys|isahc|surf|tungstenite|tokio[-_]tungstenite)$'

crates="keyscore-core keyscore-text keyscore-midi keyscore-ai keyscore-app ksq-cli ksq-play"

for crate in $crates; do
  if ! tree=$(tree_of "$crate" --depth 1); then
    fail=1
    continue
  fi
  direct=$(direct_deps "$tree")

  actual_internal=$(echo "$direct" | tr ' ' '\n' | grep -E '^keyscore-' | tr '\n' ' ' | sed 's/ $//' || true)
  expected=$(expected_internal "$crate")
  if [ "$actual_internal" != "$expected" ]; then
    echo "check-deps: $crate 내부 의존이 다르다: 실제 [$actual_internal], 기대 [$expected]" >&2
    fail=1
  fi

  case "$crate" in
    keyscore-core | keyscore-text | ksq-cli)
      allowed=" $(allowed_external "$crate") "
      while read -r dep _version source; do
        case "$dep" in keyscore-*) continue ;; esac
        if [[ "$allowed" != *" $dep "* ]]; then
          echo "check-deps: $crate의 직접 의존 $dep가 허용 목록에 없다" >&2
          fail=1
        # 레지스트리 crate는 {p}에 소스가 붙지 않는다. path·git 소스는 「(...)」가 붙는다.
        # 허용된 이름을 다른 소스로 바꿔치기하는 것을 막는다(review-2 #2).
        elif [ -n "$source" ]; then
          echo "check-deps: $crate의 직접 의존 $dep가 레지스트리 crate가 아니다: $source" >&2
          fail=1
        fi
      # 「(*)」(반복 노드)와 「(proc-macro)」는 소스가 아니라 cargo tree의 표시라서 지운다.
      done < <(echo "$tree" | tail -n +2 | sed -e 's/ (\*)$//' -e 's/ (proc-macro)$//' | sort -u)
      ;;
  esac
done

for crate in keyscore-core keyscore-text ksq-cli; do
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
