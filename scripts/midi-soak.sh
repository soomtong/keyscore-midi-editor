#!/usr/bin/env bash
# keyscore-midi 통합 테스트를 여러 번 돌려 드문 실패를 잡는다(S1-T1 리뷰 3·4 이월, S1-T2).
# 실패한 회차의 전체 출력을 target/midi-soak/<시각>-<pid>/run-<회차>.log에 남긴다.
#
# 사용: scripts/midi-soak.sh [회차 수, 기본 20] [회차 사이 쉬는 초, 기본 0]
# 쉬는 시간을 주면 MIDI 서버가 유휴로 내려갔다 다시 뜨는 경로도 시험한다(약 3초 뒤 내려간다).
set -u
runs=${1:-20}
pause=${2:-0}
# 실행마다 새 디렉터리를 써서 이전 실행의 실패 로그를 지우지 않는다(S1-T2 리뷰 1 제안 10).
out_dir="target/midi-soak/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$out_dir"
cargo test -q -p keyscore-midi --no-run >/dev/null 2>&1 || { echo "midi-soak: 빌드 실패" >&2; exit 2; }
failed=0
for i in $(seq 1 "$runs"); do
  log="$out_dir/run-$i.log"
  if cargo test -q -p keyscore-midi --no-fail-fast >"$log" 2>&1; then
    rm -f "$log"
  else
    failed=$((failed + 1))
    echo "midi-soak: 회차 $i 실패, 출력: $log" >&2
  fi
  [ "$pause" -gt 0 ] && sleep "$pause"
done
echo "midi-soak: $runs회 가운데 $failed회 실패"
[ "$failed" -eq 0 ]
