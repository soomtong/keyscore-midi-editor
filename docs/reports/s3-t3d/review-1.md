# 적대적 리뷰: S3-T3d 곡 끝 정지 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 컨텍스트를 물려받지 않은 새 리뷰 에이전트)
- 대상 커밋: `d26fd27`(브랜치 `s3/t3d-end-of-song`, `git diff main...s3/t3d-end-of-song`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`scheduler.rs`·`transport.rs`·`engine.rs`·`window.rs`·`preview.rs`의 변경만 읽고, 시험 파일을 열기 전에 적었다.

1. 곡 끝 틱 E에 Note Off·박·MIDI Clock이 겹칠 때: Note Off는 나가고, 그 틱의 클릭과 Clock(0xF8)은 나가지 않으며, Stop(0xFC)은 정확히 한 번이고 그 뒤로 Clock이 없다. E가 Clock 칸(40틱)에 맞지 않을 때도 E 앞의 Clock까지만.
2. 시작 위치 ≥ E(빈 프로젝트, 곡 끝 뒤 재개): chase 뒤 곧바로 멈춘다. 클럭 장치에는 무엇이 가는가(SPP·Continue가 나간 뒤 Stop인가).
3. 재생 중 교체로 곡 끝이 이미 보낸 구간 안(기준점 < E2 ≤ 보낸 끝)으로 당겨질 때 E2 뒤로 이미 넘긴 Note On이 꺼지는가. 곡 끝을 늘리는 교체에서 옛 E 틱의 Note Off·박이 빠지지 않는가.
4. `take_ended` 번호 비교: 곡 끝 정지를 읽지 않은 채 `stop`·`play`를 거듭해도 옛 정지를 보고하지 않고, 새 재생의 정지는 꼭 한 번 보고한다. 패닉(번호를 늘리지 않음)과 교체 막힘(`blocked`, 뒤 명령이 안 꺼내짐)에서 `ended >= last_start`가 거짓 양성·영구 음성이 되지 않는가.
5. 곡 끝 정지 뒤 `stop` 명령·스케줄러 drop이 Stop(0xFC)을 다시 보내지 않는가(`playing` 거짓).
6. 루프 중 곡 끝 무시, 루프를 끈 순간 이미 곡 끝을 지났으면 곧바로 멈추고 울리던 노트를 끄는가.
7. 미리듣기: 곡 끝 정리가 걸린 미리듣기 노트를 끄는가(정지 명령과 같음). 곡 끝 정지 뒤 미리듣기는 정상으로 켜지고 길이 뒤에 꺼지며 `ended`를 다시 세우지 않는가.
8. 실시간 스레드: `Next::SongEnd` 처리에 할당·락·로그가 없는가(원자값 저장, `release_notes`, `send_clock_message`만).
9. 창 감시 작업의 수명: 정지·일시정지·다른 창 재생·서버 재시작·창 닫기 뒤 스스로 끝나는가. 다른 창이 재생을 가져간 뒤 옛 창 감시가 새 주인의 정지를 가로채지 않는가. 재개가 새 감시를 붙이는가.
10. Sequence 전환: 재생 중 진입·이탈만 스냅샷을 다시 넣고, 정지 중이거나 주인이 아닌 창에서는 아무것도 하지 않는가. 모드를 바꾸는 다른 길(`apply_selection_event`)이 Sequence 여부를 바꾸지 않는가.
11. 곡 끝 정지와 창 표시 사이의 틈(lookahead 10 ms + `END_POLL` 20 ms): 그 사이 편집·모드 전환·일시정지가 어떻게 되는가.
12. 곡 끝 아닌 스냅샷 교체 경로(포트 삭제, 테스트 노트, 서버 복구, 정지 중 미리듣기)가 재생 중 곡 끝을 지우지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 곡 끝 틱 Note Off·박·Clock 순서, Stop 한 번 | 부분 | 클릭·박 보고는 `song_end_does_not_click_on_end_tick`. MIDI Clock(0xF8 개수, 0xFC 횟수·순서)은 시험이 없었다. 리뷰어 시험 추가 |
| 2 곡 끝 뒤 시작 | 있음(노트·클릭만) | 클럭 장치에 가는 메시지는 시험 없음. 리뷰어 시험이 관찰(제안 1) |
| 3 줄이는·늘리는 교체 | 있음 | `swap_to_end_before_position_stops_at_once`, `swap_extending_end_keeps_playing` |
| 4 `take_ended` 번호 | 부분 | `take_ended_is_false_after_new_play`. `stop`·`play` 거듭은 리뷰어 시험 추가. 패닉·교체 막힘은 코드로 확인(아래 4절) |
| 5 정지 뒤 Stop 중복 없음 | 없음 | 리뷰어 시험 추가 |
| 6 루프 | 부분 | 루프 중 무시만 있음. 루프 해제 시 즉시 정지는 리뷰어 시험 추가(S5 전이라 지금은 앱이 닿지 않는다) |
| 7 미리듣기 | 없음 | 리뷰어 시험 둘 추가 |
| 8 실시간 할당 | 간접 | 디버그 빌드 `assert_no_alloc`이 루프백 시험마다 걸린다 |
| 9 감시 작업 수명 | 부분 | 정지·재개는 `space_after_song_end_plays_from_start`·`resume_after_song_end_stops_at_once`. 다른 창 재생과 곡 안 일시정지→재개는 리뷰어 시험 추가 |
| 10 Sequence 전환 | 부분 | 재생 중 진입·이탈은 `sequence_mode_keeps_playing_past_song_end`. 정지 중 전환은 리뷰어 시험 추가 |
| 11 표시 틈 | 알려진 한계로 보고서에 적힘 | 모드 전환 쪽은 적혀 있지 않다(제안 2) |
| 12 다른 교체 경로 | 없음 | 코드로 확인: 모두 주인 없음(정지) 때만 빈 스냅샷을 넣는다(`engine.rs` 포트 삭제·테스트 노트·`reregister_extras`), 서버 복구는 먼저 주인을 놓는다. 재생 중이면 `transport_refresh`가 곡 끝을 다시 넣는다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 곡 끝 960에 Note Off·박·Clock을 겹치고 메트로놈·클럭 켬. 정지 뒤 `stop`, drop | 못 깨뜨림(Clock 24개, 클릭 1개, 0xFC 1번, Note Off < Stop, 마지막 Clock < Stop) | `keyscore-midi/tests/review_s3_t3d.rs::song_end_tick_sends_off_but_no_clock_or_click_and_one_stop` |
| 2 | 곡 끝 1001(Clock 칸 밖) | 못 깨뜨림(Clock 26개, 1040 없음) | `song_end_off_clock_grid_sends_clocks_before_end_only` |
| 3 | 곡 끝 뒤(2000 > 1920)에서 클럭 켜고 시작 | 못 깨뜨림(Clock 없음, 마지막은 Stop). 관찰: SPP(`F2 09 00`)·Continue(`FB`)·Stop(`FC`)이 같은 시각에 나간다(제안 1) | `play_past_end_with_clock_sends_no_clock_ticks` |
| 4 | 곡 끝 정지 뒤 미리듣기 200 ms | 못 깨뜨림(켜지고 ≥150 ms 뒤 꺼짐, `take_ended` 다시 참 아님) | `preview_after_song_end_sounds_and_releases` |
| 5 | 재생 중 2 s 미리듣기를 걸고 곡 끝 | 못 깨뜨림(곡 끝 정리가 끈다) | `preview_during_playback_is_released_at_song_end` |
| 6 | 루프 [0,960) 중 곡 끝 480을 지나 루프 해제 | 못 깨뜨림(곧바로 멈추고 64를 끔) | `clearing_loop_past_end_stops_at_once` |
| 7 | 곡 끝 정지를 읽지 않고 `stop`·`play`·`stop`·`play` | 못 깨뜨림(옛 정지 보고 없음, 새 정지 한 번) | `stale_end_then_stop_and_play_reports_new_end_once` |
| 8 | 창 A(2 s 곡) 재생 중 창 B(0.5 s 곡)가 재생을 가져감 → B 곡 끝 → A 다시 재생 | 못 깨뜨림(A 정지 유지, B 곡 끝 정지, A는 0.8 s 뒤에도 재생 중이고 곡 끝에서 정지) | `keyscore-app/tests/review_s3_t3d.rs::other_window_takes_over_and_each_stops_at_own_end` |
| 9 | 곡 안에서 일시정지, 옛 감시가 끝나게 둔 뒤 재개 | 못 깨뜨림(재개에 새 감시, 곡 끝 정지·틱 0) | `pause_inside_song_then_resume_still_stops_at_end` |
| 10 | 정지 중 Sequence 진입·이탈 | 못 깨뜨림(재생 시작 없음, 노트 없음) | `sequence_toggle_while_stopped_does_not_play` |

실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-midi --test review_s3_t3d`(7 통과), `-p keyscore-app --test review_s3_t3d`(3 통과). 함께 `--test song_end` 두 바이너리(8, 6 통과). 두 리뷰 파일 모두 `cargo fmt --check`, `cargo clippy -p keyscore-midi -p keyscore-app --tests -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 틱 순서: 이벤트 > 곡 끝 > 클릭·Clock(곡 끝 틱의 것은 보내지 않음) | 아니오 | `peek_next`(`scheduler.rs:985` 이하): 곡 끝은 `consider`로 먼저 들어가 Clock(엄격 `<`)을 이기고, 박은 `next_beat < end`로 거르고, 이벤트는 `<=`로 곡 끝을 이긴다. 시도 1·2 |
| 곡 끝 정지 보고는 그 재생의 것만, 한 번 | 아니오 | `take_ended`(`scheduler.rs:504`): `ended`는 정지 때의 `popped`, `last_start`는 `play`·`locate` 넣은 뒤의 `pushed`. `popped ≤ pushed`가 늘 성립하므로 새 `play` 뒤 옛 값은 작다. 패닉은 `popped`를 늘리지 않고 곡 끝 정지를 만들지 않는다. 교체 막힘은 뒤 `Play`를 꺼내지 않을 뿐이라 그동안 정지가 없다. 막힘이 풀려 `Play`를 꺼낸 뒤의 정지는 `popped ≥ last_start`. `ended`는 새 정지마다 덮어써 영구 음성이 없다. CAS 실패(그 사이 새 정지)는 다음 주기가 읽는다 |
| 곡 끝 뒤 소리 없음 | 아니오 | `Next::SongEnd`(`scheduler.rs:1060`)가 `release_notes`(미전송 끄기 대기도 버림)·0xFC·`playing = false` 뒤 `break`. 시도 1·5 |
| 정지 중 Stop 중복 없음 | 아니오 | `Command::Stop`과 `run` 끝은 `playing`일 때만 0xFC. 시도 1 |
| 창이 재생 중이 아니면 감시가 끝난다 | 아니오 | `check_song_end`(`transport.rs:433`): 상태·엔진·주인 확인. 창이 사라지면 `update` 실패 → `false`. 시도 8·9 |

`locate`는 정지 중이면 무시되면서 `last_start`만 올린다. 앱이 곡 끝 정지 뒤(창이 아직 「재생 중」인 틈에) `locate`를 부르면 그 정지를 영영 보고하지 않아 창이 재생 중으로 남는다. 지금 앱에는 `locate` 호출이 없어 닿지 않는다(`rg 'locate\(' crates/keyscore-app/src` 결과 없음). 위치 이동(TRN-1, S5)을 만들 때 볼 것(제안 3).

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 재생/정지(곡 끝 동작, 결정 A: 정지·틱 0) | 충족 | `scheduler.rs:1060`, `transport.rs:433`(`mark_stopped` → 위치 틱 0). 앱 시험 `playback_stops_at_song_end_and_rewinds`, 리뷰 시도 8·9 |
| 결정 A: 재생 중 편집으로 곡이 길어지면 새 끝까지 | 충족 | `engine.rs` `prepare`의 `with_end`, 교체 경로 `transport.rs` `transport_document_changed`. 곡 끝 직전 틈은 알려진 한계로 보고서에 있음 |
| 결정 A: Sequence 모드에서는 멈추지 않음 | 충족 | `transport.rs:414`, `window.rs:304`–`308`. 시험 `sequence_mode_keeps_playing_past_song_end` |
| 5.8 `Space`(곡 끝 정지 뒤 처음부터) | 충족 | 정지 상태의 `transport_play`가 틱 0. 시험 `space_after_song_end_plays_from_start` |
| 5.8 `Shift+Space`(커서가 곡 끝 뒤) | 충족 | 시작 위치 ≥ E면 곧바로 정지(`peek_next` 조기 반환). 시도 3 |
| UI-4 재생 헤드(정지하면 없음) | 충족 | `mark_stopped`가 `anchor = None`. 최대 `END_POLL` 늦게 사라지는 것은 보고서 한계 |
| MET-1 메트로놈(곡 끝에서 멈춤, 곡 끝 박 클릭 없음) | 충족 | `peek_next` 박 거르기. 시험 `song_end_does_not_click_on_end_tick`, 시도 1 |
| OUT-5 Clock Stop | 충족 | `Next::SongEnd`의 `send_clock_message(host, 0xFC)`. 시도 1(이전엔 시험 없음) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `Next::SongEnd`는 `release_notes`(고정 배열 순회), `send_clock_message`, `AtomicU64::store`만 쓴다. `peek_next`의 곡 끝 판정은 `Option` 복사뿐. 디버그 빌드 `assert_no_alloc`이 위 루프백 시험을 모두 통과했다.
- 제품 코드의 `unwrap`/`expect`: 새로 들어온 것 없음(`song_end`의 `unwrap_or(0)`, 감시의 `unwrap_or(false)`는 기본값).
- 에러가 삼켜지는 곳: `watch_song_end`의 `this.update(..).unwrap_or(false)`는 창이 사라진 경우만이고 그때 감시를 끝내는 것이 맞다. `apply_mode_event`의 `transport_refresh` 실패는 기존 경로대로 창 메시지에 남는다(「재생 중 편집을 반영하지 못했다」). 이때 Sequence에서 나왔어도 곡 끝이 들어가지 않아 계속 재생된다. 사람이 메시지를 보고 멈출 수 있어 지적하지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 곡 끝 뒤에서 시작하면 클럭 장치에 SPP·Continue를 보낸 뒤 같은 시각에 Stop을 보낸다(시도 3에서 `F2 09 00`, `FB`, `FC` 관찰). 계획 2절은 「chase만 하고 곧바로 같은 정리로 멈춘다」. 해롭지 않지만 외부 장치가 잠깐 Continue를 받는다. 시작 위치 ≥ 곡 끝이면 `resync_clock`·Stop을 건너뛰거나, 지금 동작을 계획·보고서에 적는다 | `crates/keyscore-midi/src/scheduler.rs:888`, `:985` |
| 2 | 제안 | 알려진 한계 「교체가 닿기 전에 옛 곡 끝을 지나면 멈춘다」는 편집만 적었다. 곡 끝 직전(lookahead 10 ms 안)에 Sequence로 들어가도 같은 이유로 멈춘다. 또 Sequence 진입·이탈은 `transport_document_changed`의 전체 교체라 기존 교체처럼 그 틈의 박 하나·Clock 한 칸이 빠질 수 있다. 보고서 한계에 적는다 | `crates/keyscore-app/src/window.rs:304`, `crates/keyscore-app/src/transport.rs:244` |
| 3 | 제안 | `locate`는 정지 중이면 무시되면서 `last_start`를 올린다. 앱이 곡 끝 정지 뒤 창이 아직 「재생 중」인 틈에 `locate`를 부르면 그 정지를 보고하지 않아 창이 재생 중으로 남는다. 지금은 앱에 호출이 없다. 위치 이동을 만드는 Task(S5)에 이월 줄로 남긴다 | `crates/keyscore-midi/src/scheduler.rs:398` |
| 4 | 제안 | 계획 불변식 「곡 끝 틱의 Clock은 보내지 않는다」와 「Stop(0xFC)을 보낸다」를 구현자 시험이 고정하지 않았다. 리뷰어 시험(`crates/keyscore-midi/tests/review_s3_t3d.rs`)이 이를 메우므로 그대로 커밋한다 | `crates/keyscore-midi/tests/song_end.rs` |

차단·수정 지적은 없다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

조건: 리뷰어 시험 두 파일(`crates/keyscore-midi/tests/review_s3_t3d.rs`, `crates/keyscore-app/tests/review_s3_t3d.rs`)을 저장소에 남긴다. 제안 1–4는 구현자가 받거나 거절 이유를 보고서에 적는다. 보고서에 적힌 대로 병합 전 전체 `cargo test --workspace --no-fail-fast`를 돌린다(리뷰어는 지시에 따라 전체 시험을 돌리지 않았다).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`preview.rs`는 `prepare` 인자 추가, `plan/stages/13-S3-step-and-pianoroll.md`는 구현 메모 한 줄)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
