# Task 보고: S1-T7 `.ksq` 재생 도구

- PR: 브랜치 `s1/t7-ksq-play`
- 계획 파일: `docs/reports/s1-t7/plan.md`
- 구현: 주 세션
- 계획 대비 달라진 점
  - 단계 파일의 「`ksq-cli play`」를 별도 도구 crate `tools/ksq-play`로 만들었다(계획에 적은 구조 결정).
    - 00-conventions 저장소 구조와 `check-deps.sh` 내부 의존 표에 `ksq-play`(core + midi)를 더했다.
    - `check-deps.test.sh`에 잘못된 의존 사례 하나를 더했다.
  - `ksq-play`만 `ctrlc` crate(MIT/Apache-2.0)를 쓴다.
  - 스케줄러의 같은 틱 정렬 함수 `order_class`를 crate 안 공개(`pub(crate)`)로 바꿔 `render`가 같은 순서로 정렬한다.

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `crates/keyscore-midi/src/render.rs` | `render(project, port_of) -> Rendered`. 뮤트·솔로(솔로가 있으면 솔로만, 뮤트+솔로는 솔로가 이김), 클립 펼치기, 숨김 꼬리는 클립 끝에서 끔, 뱅크(CC 0·32)·프로그램은 틱 0, 스냅샷과 같은 정렬, 곡 끝 틱 |
| `tools/ksq-play/src/lib.rs` | `parse_args`, `run`. 포트 매핑(`suggest_mapping`: OS 목적지가 있으면 열고, 없으면 기본 가상 포트로 보내고 경고), 메트로놈 옵션, `--from` 마디, `--wait`, 명령 채널(`p`, `q`, Ctrl-C), 곡 끝 정지, 끝낼 때 정지 → 50 ms → 스케줄러 닫기 → 50 ms |
| `tools/ksq-play/src/bin/ksq-play.rs` | 표준 입력 줄과 Ctrl-C를 명령 채널로 넘긴다 |

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 764 passed, 0 failed
ksq-play 테스트 6개 5회 반복: 모두 통과
수동 실행: 04-band.ksq에 p·q 입력 → 종료 코드 0, 잘못된 옵션 → 2, SIGINT → 곧바로 정리하고 끝남
~~~

| 계획 # | 실제 이름 | 상태 |
| --- | --- | --- |
| 1 | `render_skips_muted_tracks_and_honours_solo` | 통과 |
| 2 | `render_cuts_hidden_tail_at_clip_end` | 통과 |
| 3 | `render_sends_bank_then_program_at_tick_zero` | 통과 |
| 4 | `render_uses_track_channel_and_port_mapping` | 통과 |
| 5 | `render_end_is_last_note_off` | 통과 |
| 6 | `parse_args_accepts_options_and_rejects_bad_usage` | 통과 |
| 7 | `play_fixture_sends_every_note_on_and_off_to_virtual_port` | 통과(픽스처 01-melody를 960 BPM으로) |
| 8 | `panic_command_from_stdin_silences_all_channels` | 통과 |
| 9 | `quit_command_stops_and_releases_notes` | 통과(Ctrl-C 130 포함) |
| 10 | `named_port_missing_falls_back_to_virtual_with_warning` | 통과 |
| 11 | `metronome_option_clicks_on_channel_ten` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (764 passed)
- [x] deny(`ctrlc` 추가 뒤)
- [x] 단계별 추가 게이트: `check-deps`(새 crate 포함), `check-deps.test`(16개), `bench-check.test`

## 수동 확인 절차 (S1 출구 게이트용, 사람이 듣는다)

1. Surge XT(스탠드얼론)를 켜고 MIDI 입력에서 「KeyScore Out」을 고를 준비를 한다.
2. `cargo run -p ksq-play --release -- assets/fixtures/04-band.ksq --wait 10`
   - 10초 안에 Surge XT에서 「KeyScore Out」을 고른다.
3. 곡이 끝까지 재생되고 프로그램이 저절로 끝나는지 듣는다.
4. 다시 실행해 재생 중에 `p` + Enter를 눌러 소리가 모두 멈추는지, 그 뒤 걸린 노트가 없는지 본다. `q` + Enter로 끝낸다.
5. `--metronome`으로 클릭(채널 10, 37/42)이 박마다 들리는지 본다. Surge XT는 채널 무관(omni)으로 받는다.
6. Dexed로 1–3을 되풀이한다.
7. 재생 중 `kill -9 <ksq-play pid>` 뒤 「KeyScore Out」이 목록에서 사라지는지 본다. CoreMIDI가 프로세스 종료 때 포트를 정리하는지 확인한다.

## 알려진 한계

- 이름 지정 출력이 없으면 같은 이름의 가상 포트를 만들지 않는다. 기본 가상 포트로 보내고 경고만 한다. 대체 매핑을 제안·선택하는 일(OUT-8)은 S2 UI다.
- 피치 오프셋(소수 음높이)은 보내지 않는다. 벤드 레인은 S6이다.
- 루프 재생, 템포 바꾸기 옵션, `.mid` 입력 재생은 없다.
- 끝날 때 기다리는 시간(정지 뒤 50 ms × 2, 곡 끝 뒤 100 ms)은 lookahead 10 ms 기준의 경험값이다.

## 리뷰어에게

스스로 불안한 곳:

1. `finish`의 순서: 정지 → 50 ms → 스케줄러 닫기 → 50 ms. 가상 포트는 스냅샷과 함께 닫힌다. 예약한 Note Off(최대 10 ms 앞)가 포트가 사라지기 전에 닿는지.
2. `wait_for_commands`가 `Disconnected`(표준 입력이 닫힘)일 때 남은 시간을 그냥 잔다. 그동안 Ctrl-C는 채널로 오지 않는다.
   - 바이너리는 송신자 둘(표준 입력 스레드, Ctrl-C 처리기)을 가진다. Ctrl-C 쪽이 살아 있으면 끊기지 않는다.
3. 곡 길이 계산: `tick_to_time(end) - tick_to_time(from)` + 100 ms. 템포 변화가 있는 곡에서 맞는지.
4. `render`의 뮤트+솔로 규칙. REQ 5.10은 뮤트·솔로만 정하고 둘 다 켜진 경우를 정하지 않는다. 흔한 DAW 규칙(솔로가 이김)을 택했다.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 없는 이름 포트의 트랙을 기본 가상 포트에 합쳐 같은 채널의 다른 트랙과 섞임 | `suggest_mapping`의 제안대로 그 이름의 가상 포트를 따로 만든다. 가상 포트 이름으로 못 쓰는 이름이면 「기본 이름 N」으로 만든다. 계획의 「기본 가상 포트로 보냄」을 바꿨고, 내 테스트 10은 `named_port_missing_gets_its_own_virtual_port_with_warning`으로 바꿨다 |
| 2 | 없는 메트로놈 포트면 클릭이 기본 가상 포트로 감 | 1과 같은 경로로 따로 만든다 |
| 3 | 뮤트+솔로 규칙에 결정 문서 없음 | `docs/decisions/2026-10-16-mute-solo-precedence.md`(솔로가 이김, 흔한 DAW 규칙, 사람이 바꿀 수 있음) |
| 4 | `--metronome` 없는 `--metronome-port` (제안) | 반영. 경고한다 |
| 5 | 곡 끝 뒤 `--from` (제안) | 반영. 종료 코드 1과 오류 |
| 6 | 두 번째 Ctrl-C로 즉시 끝낼 수 없음 (제안) | 반영. 두 번째는 바로 130으로 끝낸다 |
| 7 | 단계 파일 S1-T7에 구조 결정 메모 (제안) | 반영 |

리뷰어 테스트는 바뀌지 않았다. 실패하던 2개가 통과한다. T7 테스트 3회 반복 통과, 워크스페이스 787개 통과.
