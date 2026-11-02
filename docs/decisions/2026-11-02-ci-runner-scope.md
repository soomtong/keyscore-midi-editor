# 결정: 원격 CI(GitHub macOS 러너)에서 돌릴 시험의 범위

- 날짜: 2026-11-02
- 상태: 결정 대기
- 관련: PR https://github.com/soomtong/keyscore-midi-editor/pull/1, `docs/reports/ci/first-run.md`, `.github/workflows/ci.yml`

## 상황

원격 CI는 `cargo test --workspace`가 첫 실패에서 멈춰 실패 하나만 보였다. PR 1에서 `--no-fail-fast`로 바꾸자(실행 41442250458, 커밋 d19b27b) 시험 대상 14개가 실패했다. 이 실패들은 PR이 만든 것이 아니라 `main`에서도 가려져 있던 것이다. fmt·clippy는 통과하고, `midi-keepalive`는 시험 끝까지 살아 있었다. 사람의 Mac에서는 이 시험들이 모두 통과해 왔다(각 Task 보고서).

실패는 셋으로 나뉜다.

1. 실시간·벽시계 타이밍(8개 대상). 러너가 공유 VM이라 스레드 깨움이 늦다.
   - `keyscore-midi` `scheduler::playback_jitter_p99_within_half_ms_short`: p99 4.48 ms(기준 0.5 ms, REQ 9.2). VM에서는 맞출 수 없는 수치다.
   - `keyscore-midi` `review_s1_t4_r2::many_swaps_do_not_accumulate_rounding_drift`, `review_s1_t5::click_longer_than_beat_stays_on_grid`(클릭 하나 더), `review_s3_t8_r3::pause_just_before_loop_end_can_return_loop_end`.
   - `keyscore-app` `review_s2_t5a_r2`(이 PR이 고침, 1d771b7), `review_s2_t5a_r3::many_metronome_toggles_while_playing_stay_in_sync`, `review_s3_t8_r2::pause_with_full_queue_keeps_playing_and_a_later_pause_works`(첫 박 `[40, 41]`), `transport_bar::tap_tempo_sets_tempo_from_intervals`(84.4 BPM).
2. CoreMIDI 환경(2개 대상).
   - `keyscore-midi` `review_s1_t1_r3::create_virtual_output_long_names_are_enumerated_verbatim`: `MIDISourceCreateWithProtocol` `-307`.
   - `keyscore-app` `review_s2_t6c` 둘: 「감시가 시작되지 않았다」(핫플러그 감시).
3. 파일 시스템 오류 문구(`ksq-cli` 4개 대상). `/dev/<없는 이름>`에 `-o`로 쓸 때 러너는 `EACCES`를 돌려주는데 시험은 「특수 자리」 문구를 기대한다. `/var/folders/...` 안의 링크 사슬에서 종료 코드가 0으로 나온다. macOS 판(러너 15, 사람 Mac은 다를 수 있음)이나 권한 차이로 보이며, 제품 문구 분기의 이식성 결함일 수 있다.

REQ에 CI 범위는 없다. 시험을 끄거나 빼서 녹색을 만드는 것은 에이전트가 정할 일이 아니다(`CLAUDE.md`: 기존 시험을 지우거나 `#[ignore]`하지 않는다).

## 선택지

- A(추천). 둘로 나눈다. 3(ksq-cli)은 결함일 수 있으니 별도 Task로 원인을 찾아 고친다. 1·2는 원격 CI에서 이름으로 가려 돌리지 않고(예: `cargo test` 뒤 `--skip`, 또는 `nextest` 프로필), 그 목록을 워크플로에 적는다. 그 시험들은 지금처럼 사람 Mac의 자동 게이트(`cargo test --workspace`)가 맡는다. CI 녹색이 「VM에서 결정적으로 돌 수 있는 시험 전부」를 뜻하게 된다.
- B. 원격 CI는 지금처럼 전부 돌리고 빨간 상태를 받아들인다. PR 병합 조건은 사람 Mac의 게이트로 둔다. CI는 fmt·clippy·deny와 실패 목록 참고용이다.
- C. 1·2를 하나씩 경과 시간 기준 등으로 VM에서도 견디게 고친다(PR 1의 두 시험처럼). 지터 p99 같은 실시간 기준은 VM에서 뜻이 없어 C만으로는 녹색이 되지 않는다. A와 섞어 쓸 수 있다.
- D. 사람 Mac을 self-hosted 러너로 붙인다. 실시간 시험이 뜻을 갖지만, 러너 관리와 보안(공개 저장소의 PR이 사람 Mac에서 돈다) 부담이 있다.

## 그동안

PR 1은 그 범위(두 시험의 벽시계 여유, MIDI 서버 keepalive, `--no-fail-fast`)만 담는다. CI가 녹색이 되지 않으므로 병합하지 않고 결정을 기다린다.

## 결정

(사람)
