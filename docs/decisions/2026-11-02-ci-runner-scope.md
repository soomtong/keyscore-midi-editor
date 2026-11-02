# 결정: 원격 CI(GitHub macOS 러너)에서 돌릴 시험의 범위

- 날짜: 2026-11-02
- 상태: 결정됨(A, YoungTaek, 2026-11-02)
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

A(YoungTaek, 2026-11-02). `ksq-cli`은 새 브랜치·PR의 별도 Task로 고친다(사람이 새 브랜치 푸시를 허락함).

## 반영

- 원격 CI는 아래 시험을 이름으로 건너뛴다(`.github/workflows/ci.yml`의 `--skip` 목록). 사람 Mac의 자동 게이트(`cargo test --workspace`)에서는 그대로 돈다.
  - 실시간 기준 측정(REQ 9.2, NF): `playback_jitter_p99_within_half_ms_short`, `playback_drift_within_1ms_short`, `send_now_latency_p99_within_3ms`. 지터만 실패했지만 셋 모두 VM에서 뜻이 없는 하드웨어 기준이라 함께 뺀다.
  - 벽시계 타이밍(이 PR 실행에서 실패한 것만): `many_swaps_do_not_accumulate_rounding_drift`, `click_longer_than_beat_stays_on_grid`, `pause_just_before_loop_end_can_return_loop_end`, `many_metronome_toggles_while_playing_stay_in_sync`, `pause_with_full_queue_keeps_playing_and_a_later_pause_works`, `tap_tempo_sets_tempo_from_intervals`.
  - CoreMIDI 환경: `create_virtual_output_long_names_are_enumerated_verbatim`, `brk_metronome_port_vanish_and_return_while_playing`, `brk_port_list_refreshes_on_hotplug`.
- 실행 7·8(PR 1)에서 더 드러나 더한 것(분류 1, 스케줄러가 보낸 호스트 시각이나 시간 창 안 개수를 재는 시험이라 VM에서 고칠 길이 없다): `play_sends_events_at_scheduled_host_times`(오차 2.42 ms), `clock_sends_24_ppqn_with_start_spp_continue_stop`(Clock 29개), `swap_after_pass_capped_mid_tick_with_clock_keeps_clock_grid`(Clock 0이 12 ms 늦음), `swap_to_new_meter_keeps_click_grid_and_follows_new_accents`(클릭 8개 대 7개). 러너의 실패는 실행마다 달라, 한 번만 실패한 시험도 이 분류면 더했다. 실행 11(PR 2)에서 `swap_after_pass_capped_right_before_clock_sends_events_and_clocks_once`가 같은 `assert_clock_grid`(Clock 한 칸 오차 11 ms)로 실패해, 그 도우미를 쓰는 나머지 둘(`swap_after_pass_capped_at_later_tick_keeps_clock_on_that_tick`, `many_swaps_with_clock_on_keep_clock_grid`)까지 함께 더했다.
- 고쳐서 CI에서 돌리는 것: `pause_position_follows_tempo_change_in_song`, `pause_after_several_tempo_edits_matches_elapsed_time`, `preview_during_playback_is_released_at_song_end`(곡 끝을 0.25 s에서 1 s로), `other_window_plays_between_pause_and_resume`(A의 음만 모음), `review2_control_plain_reconnect_during_playback_gets_program`(장치 복귀 대기 1.5 s를 5 s로), `cached_render_is_cheap_with_big_keymap`(한 번 잰 값 대신 다섯 번 중 가장 짧은 값)(모두 PR 1).
- 다른 시험이 러너에서 흔들리면 먼저 경과 시간 기준 등으로 고칠 수 있는지 본다. 고칠 수 없을 때만 이 문서 「반영」에 근거를 적고 목록에 더한다(A의 범위 안, 사람이 고른 분류 1·2에 드는 시험만).
