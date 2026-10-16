# Task 보고: S1-T5 메트로놈과 미리듣기

- PR: 브랜치 `s1/t5-metronome-preview`
- 계획 파일: `docs/reports/s1-t5/plan.md`
- 구현: 주 세션(실시간 경로)
- 계획 대비 달라진 점
  - `Beat`의 박 번호 필드 이름을 `index`로 했다. clippy `struct_field_names` 때문이고, 공개 getter는 계획대로 `beat()`다.
  - 정지·되감기·재생 때 노트를 모두 끄면(`release_notes`), 아직 넘기지 않은 끄기 대기도 버린다. 계획에 없던 세부다. 이미 끈 노트를 다시 끄지 않으려는 것이다.

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `src/metronome.rs` | `MetronomeConfig`(비공개 필드, 기본값 채널 10·37·42·벨로시티 127/100·50 ms), `Beat`, `DEFAULT_PREVIEW_LENGTH`(200 ms), `MAX_NOTE_LENGTH`(10 s), 길이 맞추기 |
| `src/scheduler.rs` | `Snapshot::with_meter`·`meter`, `Scheduler::{set_metronome, set_metronome_enabled, set_metronome_forced, poll_beat, preview}`, 실시간 쪽 `Next::Click`, `click`, `play_note`, `flush_offs`, `free_earliest_off`, `forget_offs_on_changed_ports`, `beat_at_or_after` |
| `src/lib.rs` | 공개 내보내기 |

동작은 이렇다.
- 박은 스케줄 항목의 하나다. 같은 틱이면 이벤트, 클릭, Clock 순이고, 루프 끝과 같은 틱의 박은 다음 바퀴의 것이다.
- 박마다 `Beat`를 `rtrb` 큐(256)에 넣는다. 클릭이 꺼져 있어도 넣는다. 큐가 가득이면 버린다.
- 클릭과 미리듣기의 Note Off는 끄기 대기 표(64칸, 스택 배열)에 둔다. 시각이 lookahead 창에 들어오면 그 시각으로 예약 송신한다. 시각이 지나면 칸을 비운다.
- 같은 노트를 다시 켜는 경우
  - 아직 넘기지 않은 끄기는 새 Note On 시각(이하)에 앞당겨 보낸다.
  - 이미 넘긴 끄기는 취소할 수 없으므로 새 Note On을 그 뒤로 미룬다(최대 lookahead 10 ms).
- 표가 가득하면 가장 이른 대기를 지금 보내고 칸을 다시 쓴다. Note Off를 잃는 경로는 없다.
- 교체 때 다음 박은 「마지막으로 처리한 박 + 1」 이후의 첫 박을 새 박자표로 다시 찾는다.
  - 이미 지난 시각의 박이면 `floor`에 늦게 나간다.
  - 같은 번호의 포트가 바뀌면 그 번호의 끄기 대기는 버린다. 노트는 옛 포트로 이미 껐다.
- 위치 이동·재생 시작·루프 되감기는 그 위치 이상인 첫 박부터 센다. 박 위면 그 자리에서 클릭한다.

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 677 passed, 0 failed
keyscore-midi 3회 반복: 모두 통과
클릭·박·미리듣기 타임스탬프: 기대 시각과 1 µs 안
~~~

| 계획 # | 실제 이름 | 상태 |
| --- | --- | --- |
| 1 | `metronome_clicks_on_beat_grid_with_accent_on_bar_start` | 통과 |
| 2 | `metronome_follows_meter_change_and_short_bar` | 통과 |
| 3 | `metronome_click_note_off_after_configured_length` | 통과 |
| 4 | `metronome_off_sends_no_click_and_forced_overrides_button` | 통과 |
| 5 | `metronome_uses_its_own_port_and_channel` | 통과 |
| 6 | `metronome_restarts_grid_after_locate_and_loop_wrap` | 통과 |
| 7 | `beats_are_reported_for_every_beat_while_playing` | 통과 |
| 8 | `preview_sends_note_off_after_default_length` | 통과 |
| 9 | `preview_retrigger_same_note_restarts_length` | 통과 |
| 10 | `many_previews_never_leave_a_note_on` | 통과(노트 100개로 표 64칸을 넘김) |
| 11 | `metronome_and_preview_do_not_allocate` | 통과 |
| 12 | `metronome_config_default_matches_req` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (677 passed)
- [x] deny
- [x] 단계별 추가 게이트: `check-deps` 셋 통과, 할당 0(테스트 11)

## 알려진 한계

- 클릭 길이 50 ms와 벨로시티 127/100은 REQ에 값이 없어 흔한 값으로 정했다. 설정으로 바꿀 수 있다.
- 복합 박자(6/8)는 분모 박(8분음표)마다 클릭하고, 마디 첫 박만 강박이다.
- 이미 넘긴 Note Off와 겹치는 다시 켜기는 새 Note On이 최대 10 ms 늦는다.
- 끄기 대기 표가 가득하면(동시에 64개 넘게 기다림) 가장 이른 노트가 예정보다 일찍 끊긴다.
- 박 보고 큐를 UI가 비우지 않으면 박이 빠진다. UI 연결은 S2·S4다.
- 카운트인, 녹음 상태, 설정 화면은 범위 밖이다(S2·S4·S5).

## 리뷰어에게

스스로 불안한 곳:

1. `play_note`의 다시 켜기
   - 넘기지 않은 끄기는 `min(off.host, host)`에 보낸다.
   - 넘긴 끄기는 `host = max(host, off.host)`로 Note On을 미룬다.
   - 클릭 길이가 박 간격보다 길 때 클릭이 밀리는지 봐 달라.
2. `sent_through_now`에서 `Next::Click`을 Clock과 같이 「그 틱의 이벤트는 다 보냄」으로 둔 것.
   - 교체 때 클릭은 `sent_through`가 아니라 `last_click`으로 다시 찾는다.
3. `release_notes`가 넘기지 않은 끄기 대기를 버리는 것.
   - 루프 되감기에서도 불린다. 되감기 직전에 켠 미리듣기 노트는 되감기 때 꺼진다.
   - S1-T4부터 `send_now` 노트에 대해 같은 동작이었다.
4. `forget_offs_on_changed_ports`와 `release_changed_ports`·`release_unless_continued`의 순서. 둘 다 옛 스냅샷이 아직 `self.snap`일 때 불린다.
