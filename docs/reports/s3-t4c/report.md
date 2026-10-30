# Task 보고: S3-T4c 미리듣기 가벼운 경로와 배치 뷰 Step 표시

- PR: 브랜치 `s3/t4c-preview-arrangement-step`
- 계획 파일: docs/reports/s3-t4c/plan.md
- 계획 대비 달라진 점
  - 표시 API 이름은 `arrangement_step_text`가 아니라 `ProjectWindow::arrangement_step(&App) -> Option<ArrangementStep>`이고 `ArrangementStep`은 `step.rs`에 있다.
  - 엔진 쪽 인터페이스는 계획 그대로다. 추가로 뱅크·프로그램 기록의 열쇠는 기본 가상 포트를 이름으로 가리켜도 빈 문자열로 모은다(`Engine::setup_key`). 같은 포트를 두 열쇠로 적으면 번갈아 들을 때 음색을 건너뛸 수 있기 때문이다.
  - 진행 순서: 시험 두 파일을 먼저 썼지만, 실패를 본 것은 구현 뒤에 기능을 일부러 끈 상태에서다(아래 「테스트 결과」). 프로토콜의 「실패 먼저 확인」 순서를 지키지 못했다.

## 구현 요약

- `crates/keyscore-app/src/engine.rs`
  - `Engine`에 `preview`(정지 중 미리듣기가 넣은 스냅샷의 포트 구성과 경로표), `setups_sent`((목적지, 채널) → 마지막으로 보낸 뱅크·프로그램 묶음), `preview_prepares`(시험·진단용 횟수)를 더했다.
  - `forget_preview`가 둘을 잊는다. 부르는 곳: `prepare` 맨 앞(재생 시작·재생 중 편집·정지 준비), `ports_edited`(가상 포트 만들기·이름 바꾸기·대체 매핑), `remove_virtual_port`, `send_port_test_note`의 스냅샷 교체, `port_lost`·`port_returned`, `reregister_main`, `reregister_extras`의 교체, 포트 감시가 목적지 변화를 본 `housekeeping`(`ports.rs`).
  - `preview_route`(포트 구성이 같을 때만 경로표에서 번호), `remember_preview`, `setup_sent`·`note_setup_sent`, 공개 `preview_prepares(cx)`.
- `crates/keyscore-app/src/preview.rs`: `Purpose { Preview, TestNote }`. 정지 중 이름 포트는 `preview_ports`(트랙 포트 차례, 메트로놈, 클럭)로 기억한 경로를 먼저 찾고, 없으면 예전처럼 준비·교체한 뒤 포트를 찾았을 때만 기억한다. `send_note`는 뱅크·프로그램 묶음을 만들어 미리듣기면 같은 묶음을 건너뛰고, 보냈으면 기록한다. 재생 중 경로와 다른 창 재생 중 경로는 그대로다.
- `inspector.rs`(테스트 노트 `TestNote`), `selection.rs`·`step_input.rs`(미리듣기 `Preview`): 호출에 까닭을 넘긴다.
- `crates/keyscore-app/src/step.rs`·`step_input.rs`·`window.rs`: `ArrangementStep`, `arrangement_step`(배치 뷰 Step일 때만, 클립 안 → 클립 끝 → 새 클립 마디(`clip_for_notes`, 지금 길이와 앞의 `n`) → 쓸 수 없는 이유), `render_arrangement_step`(자리표시 `main-view` 안에 `arrangement-step-cursor`·`arrangement-step-clip`).
- `step_input.rs` `step_triplet`: 이미 셋잇단 대기면 `n`을 표시에 더 붙이지 않는다.
- 문서: 결정 2026-10-29 1절에 「쓰는 클립」 줄, 결정 2026-10-30 2절의 경계 문장을 동작별로 나눔, 결정 2026-10-17-mode-entry-view 이월에 S3-T4c 읽기, S4-T1 이월 줄.

## 테스트 결과

~~~
keyscore-app --test preview_light: 9 passed
keyscore-app --test arrangement_step: 9 passed
관련 시험 묶음(미리듣기·테스트 노트·엔진·Step을 쓰는 시험 파일 72개, --no-fail-fast): 773 passed, 0 failed
전체 cargo test --workspace --no-fail-fast: <리뷰 뒤 채움>
~~~

실패 확인(구현 뒤 기능을 일부러 끈 상태):
- `preview_route`가 늘 `None`, 음색 건너뛰기 끔: `preview_repeat_on_named_port_prepares_once`, `preview_sends_program_once_while_unchanged` 실패.
- `forget_preview`를 빈 함수로: `preview_after_play_stop_prepares_again_and_resends_program` 실패.
- `arrangement_step`이 늘 `None`, `n` 표시 되돌림: `arrangement_step` 9개 가운데 8개 실패(`step_in_piano_roll_has_no_arrangement_display`는 표시가 없음을 보는 시험이라 통과).

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `preview_repeat_on_named_port_prepares_once` | 통과 |
| 2 | `preview_after_track_port_change_prepares_again` | 통과 |
| 3 | `preview_after_play_stop_prepares_again_and_resends_program` | 통과 |
| 4 | `preview_sends_program_once_while_unchanged` | 통과(뱅크 CC 0·32도 한 번) |
| 5 | `preview_resends_program_after_track_program_edit` | 통과 |
| 6 | `preview_alternating_tracks_on_shared_channel_resend_program` | 통과 |
| 7 | `test_note_always_sends_program` | 통과 |
| 8 | `preview_to_missing_port_finds_port_added_later` | 통과 |
| 9 | `preview_after_virtual_port_rename_follows_new_port` | 통과 |
| 10 | `step_in_arrangement_shows_track_and_cursor` | 통과 |
| 11 | `step_in_arrangement_inside_clip_names_clip` | 통과 |
| 12 | `step_in_arrangement_at_clip_end_says_extend` | 통과 |
| 13 | `step_in_arrangement_outside_clip_shows_new_clip_bars` | 통과 |
| 14 | `step_in_arrangement_follows_input` | 통과 |
| 15 | `step_in_arrangement_blocked_by_next_clip_says_why` | 통과 |
| 16 | `normal_in_arrangement_has_no_step_display` | 통과 |
| 17 | `step_in_piano_roll_has_no_arrangement_display` | 통과 |
| 18 | `triplet_twice_shows_single_n` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체는 리뷰 뒤에 처음부터 돌린다)
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh` 통과

## 알려진 한계

- 신디에서 사람이 음색을 손으로 바꾸면 다음 미리듣기는 프로그램을 다시 보내지 않는다. 재생 시작(chase)과 테스트 노트는 늘 보낸다.
- 기억한 OS 목적지 핸들은 미리듣기마다 목적지 목록과 대조하지 않는다(그것이 줄이려던 비용이다). 사라짐·돌아옴·목적지 목록 변화는 포트 감시가 알려 잊게 한다. 감시 주기 사이의 사라짐은 예전에도 CoreMIDI가 조용히 버렸다.
- 배치 뷰 표시는 글이다. 격자 위 커서·고스트 클립은 S4-T1 이월.
- 사람 확인: 실제 신디(Surge XT 등)로 Step 음 키를 빠르게 칠 때 끊김·음색 재설정이 줄었는지(단계 파일 「실제 신디로 키 반복을 먼저 재 본다」). 에이전트는 소리를 들을 수 없어 헤드리스 시험의 준비 횟수와 메시지 열로 대신했다.

## 리뷰어에게

- 무효화 누락이 가장 불안하다. 엔진 밖에서 스냅샷을 넣는 길은 `prepare` 뒤뿐이라고 보았다(`transport.rs`의 두 `swap`). 엔진 안의 `Snapshot::new` 교체 다섯 곳에는 모두 `forget_preview`가 닿는지 확인해 달라. 같은 포트 구성인데 스냅샷이 바뀌는 길이 있으면 경로표 번호가 엉뚱한 포트를 가리킨다.
- 열쇠에 문서 revision을 넣지 않았다. 정지 중 미리듣기는 스냅샷의 이벤트를 쓰지 않는다는 판단이다. 정지 중 스냅샷의 이벤트를 읽는 길(클럭·메트로놈 등)이 있으면 깨진다.
- 음색 기록의 열쇠는 (목적지 이름, 채널)이다. 같은 장치를 다른 이름으로 가리키는 길(대체 매핑과 같은 모양 이름)에서 건너뛰기가 틀릴 수 있는지 봐 달라.
