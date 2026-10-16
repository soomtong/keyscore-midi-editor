# 적대적 리뷰: S1-T7 `.ksq` 재생 도구 (리뷰 1회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 새 서브 에이전트 세션(구현자와 다름)
- 대상 커밋: `1780bc9` (브랜치 `s1/t7-ksq-play`, `main..HEAD` 4커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`render.rs`, `tools/ksq-play/src/lib.rs`, `src/bin/ksq-play.rs`, `Cargo.toml`, `check-deps.sh` 변경만 읽고, 테스트를 열기 전에 적었다.

1. 끝내는 모든 경로(곡 끝, `q`, `p` 뒤 `q`, Ctrl-C, `--wait` 중 Ctrl-C, 재생 전에 이미 와 있는 명령, 닫힌 입력)에서 걸린 노트가 남는가.
2. 곡 끝 계산 `tick_to_time(end) - tick_to_time(from) + TAIL`이 템포 변화 곡에서 마지막 노트를 정지로 자르지 않는가.
3. `--from`: 앞 마디 노트를 보내지 않고, 틱 0의 뱅크·프로그램이 chase로 노트보다 먼저 오는가(OUT-4).
4. 포트 매핑: 있는 이름 포트는 그 목적지로만, 같은 이름 트랙 둘은 같은 포트 하나로. 없는 이름 포트를 기본 가상 포트(0번)로 합치면 다른 트랙의 (포트, 채널)과 섞이지 않는가(SEQ-7). 없는 `--metronome-port`의 클릭이 멜로디 포트로 가지 않는가(MET-3).
5. `render`: 뮤트·솔로 16가지 조합, 숨김 꼬리 끝과 다음 클립 첫 틱의 같은 음높이(Off가 먼저), 클립 끝에 딱 맞는 노트, 겹치는 같은 음높이, 같은 틱에서 트랙별 MSB·LSB·프로그램 순서, 빈 프로젝트, 작은 벨로시티(0 Note On 금지), `port_of`의 트랙 인덱스.
6. 인자: `--wait -0`(범위 검사 통과 뒤 `Duration::from_secs_f64` 패닉 여부), `inf`, 상한 초과, 값 없는 옵션, 같은 옵션 두 번, `--from u64::MAX`.
7. 오류 경로: 파일 없음·범위 밖 마디는 `CoreMIDI` 포트를 만들기 전에 1로 끝나는가. 스케줄러 시작 뒤 `?`로 빠지는 곳이 정지·배수 없이 끝나는가.
8. 바이너리: 실제 SIGINT, 표준 입력이 처음부터 닫힌 경우(`Stdio::null`).
9. 구조: `ksq-cli`가 여전히 `CoreMIDI` 없이 빌드되는가(`check-deps`), REQ에 없는 결정(뮤트+솔로)이 결정 문서 없이 들어갔는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 끝내는 경로 | 부분 | `q`, `p` 뒤 `q`, 재생 중 Ctrl-C만. 재생 전 명령, `--wait` 중 Ctrl-C, 닫힌 입력, 실제 SIGINT 없음 |
| 2 템포 변화 곡 끝 | 없음 | 픽스처는 단일 템포로 바꿔 재생 |
| 3 `--from`·chase | 없음 | 인자 해석만 |
| 4 포트 매핑 | 부분 | 없는 이름 포트 경고만. 있는 목적지, 합쳐짐, 메트로놈 포트 없음 |
| 5 `render` 경계 | 부분 | 뮤트·솔로 일부 조합, 꼬리, 뱅크 순서(트랙 하나). 같은 틱 꼬리/새 노트, 겹침, 빈 프로젝트, proptest 없음 |
| 6 인자 경계 | 부분 | `-1`, `nan`은 있음. `-0`, `inf`, 상한 초과 없음 |
| 7 오류 경로 | 없음 | |
| 8 바이너리 | 없음 | 보고서의 수동 확인만 |
| 9 구조 | 있음 | `check-deps.test.sh`에 ksq-play 사례 하나 |

## 3. 깨뜨리기 시도

모두 실행했다. 루프백 테스트는 `cargo test -p ksq-play` 전체를 6회, 리뷰 파일을 3회 더 돌렸고 결과가 매번 같았다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 뮤트·솔로 트랙 셋의 64조합 | 못 깨뜨림(「솔로가 있으면 솔로만, 없으면 뮤트 아닌 것」) | `render_mute_solo_all_combinations_follow_solo_then_mute_rule` |
| 2 | 꼬리 Note Off와 다음 클립 첫 틱의 같은 음높이 Note On | 못 깨뜨림(Off 먼저) | `render_tail_off_precedes_same_pitch_on_at_next_clip_start` |
| 3 | 클립 끝에 딱 맞는 노트 | 못 깨뜨림 | `render_note_ending_exactly_at_clip_end_is_not_cut` |
| 4 | 같은 음높이 겹침(켜짐 수 균형) | 못 깨뜨림 | `render_overlapping_same_pitch_notes_stay_balanced` |
| 5 | 빈 프로젝트, 프로그램만 있는 프로젝트 | 못 깨뜨림(끝 0) | `render_empty_and_program_only_projects_end_at_zero` |
| 6 | 두 트랙의 MSB·LSB·프로그램 묶음 순서 | 못 깨뜨림 | `render_bank_program_per_track_stay_grouped_before_tick_zero_notes` |
| 7 | 앞 트랙이 뮤트일 때 `port_of` 인덱스 | 못 깨뜨림(절대 인덱스) | `render_port_of_gets_absolute_track_index_when_earlier_tracks_are_muted` |
| 8 | 16비트 벨로시티 1 | 못 깨뜨림(Note On 벨로시티 ≥ 1) | `render_smallest_velocity_is_still_note_on` |
| 9 | 임의 노트 + 분할 꼬리 proptest(균형, 클립 안, 정렬, 끝) | 못 깨뜨림 | `render_any_project_is_balanced_and_inside_clips` |
| 10 | `--wait -0`, `inf`, `3600.5`, `--from -1`·빈 값, 파일 없이 옵션만, 같은 옵션 두 번 | 못 깨뜨림 | `parse_args_edge_values_do_not_panic_and_reject_out_of_range` |
| 11 | 없는 파일, `--from 18446744073709551615` | 못 깨뜨림(1, 「범위 밖」) | `run_missing_file_and_unreachable_bar_exit_one_without_midi` |
| 12 | 960 → 60 BPM 템포 변화 곡의 마지막 노트 길이 | 못 깨뜨림(약 1초 유지, 곧 끝남) | `song_end_waits_for_last_note_across_tempo_change` |
| 13 | `--from 2`: 앞 마디 노트 제외, 프로그램 chase가 노트 앞 | 못 깨뜨림 | `from_bar_skips_earlier_notes_and_chases_program` |
| 14 | 빈 프로젝트 재생 | 못 깨뜨림(0, 2초 안) | `empty_project_finishes_quickly_with_zero` |
| 15 | 재생 전에 이미 와 있는 `q`(`--wait` 0) | 못 깨뜨림(Note On 0개) | `quit_queued_before_play_sends_no_note` |
| 16 | 재생 전에 이미 와 있는 Ctrl-C | 못 깨뜨림(130, Note On 0개) | `interrupt_queued_before_play_sends_no_note_and_exits_130` |
| 17 | `--wait 5` 중 Ctrl-C | 못 깨뜨림(130, 곧 끝남) | `interrupt_during_wait_exits_130_without_notes` |
| 18 | `p`와 `q`를 붙여 보냄, 닫힌 명령 채널로 곡 끝까지 | 못 깨뜨림 | `panic_then_quit_and_closed_input_leave_nothing_sounding` |
| 19 | 있는 OS 목적지 이름 트랙 둘 + 가상 포트 트랙 | 못 깨뜨림(목적지 2노트, 가상 1노트, 경고 없음) | `named_port_present_routes_track_to_os_destination_only` |
| 20 | 있는 `--metronome-port` | 못 깨뜨림(클릭은 그 목적지에만) | `metronome_port_present_gets_clicks_and_main_port_none` |
| 21 | 없는 `--metronome-port` | 깨짐: 클릭(채널 10, 37/42)이 기본 가상 포트로 간다 | `missing_metronome_port_does_not_click_on_main_port` |
| 22 | 같은 채널 1의 가상 포트 트랙(프로그램 0)과 없는 이름 포트 트랙(프로그램 48) | 깨짐: 가상 포트 채널 1에 `C0 00`, `C0 30`이 함께 오고 같은 음높이 60이 두 번 켜진다 | `missing_named_port_does_not_merge_into_other_tracks_channel` |
| 23 | 실제 바이너리, 표준 입력 `null`, 재생 중 SIGINT(테스트가 띄운 자식에만) | 못 깨뜨림(130, 패닉, 걸린 노트 없음) | `binary_sigint_with_closed_stdin_releases_notes_and_exits_130` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `render` 결과에서 키마다 Note On 수 = Note Off 수, Off가 On을 앞지르지 않음 | 아니오 | proptest #9 |
| 모든 이벤트가 클립 `[start, end]` 안(숨김 꼬리 잘림, 결정 Q1) | 아니오 | proptest #9, #2 |
| `end()` = 마지막 이벤트 틱 | 아니오 | proptest #9, #5 |
| 도구는 끝날 때 정지를 보내고 걸린 노트를 남기지 않음 | 아니오 | #12, #15–18, #23 |
| 트랙 = (포트, 채널) 하나(SEQ-7) | 예 | 없는 이름 포트 트랙이 기본 가상 포트로 합쳐진다(#22) |
| `ksq-cli`는 core·text만, `CoreMIDI` 없음 | 아니오 | `scripts/check-deps.sh` 통과, `cargo tree -p ksq-cli`에 midi·coremidi·ctrlc 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 가상 출력 「KeyScore Out」 | 충족 | `tools/ksq-play/src/lib.rs:251`, 기본 이름 `DEFAULT_PORT_NAME` |
| OUT-4 트랙 → (포트, 채널), 시작 시 뱅크·프로그램 | 부분 충족 | 프로그램·뱅크는 `crates/keyscore-midi/src/render.rs:65-71`과 chase(#13)로 충족. 없는 이름 포트 트랙은 다른 트랙의 (포트, 채널)과 섞인다(`lib.rs:227`, #22) |
| OUT-6 패닉 | 충족 | `lib.rs:327-331`, `p`·Ctrl-C 모두 CC 120·123·64(#18, #23) |
| OUT-8 없는 포트는 가상 포트로 대체 매핑 제안 | 부분 충족 | `suggest_mapping`이 `Suggestion::Virtual { name }`을 내는데 도구는 그것을 버리고 기본 포트 0번으로 보낸다(`lib.rs:224-229`) |
| MET-1 클릭(채널 10, 37/42) | 충족 | `lib.rs:285-295`, 기존 `metronome_option_clicks_on_channel_ten` |
| MET-3 메트로놈 전용 포트 | 부분 충족 | 있는 포트는 충족(#20). 없는 포트면 클릭이 멜로디 포트로 간다(`lib.rs:266-269`, #21). MET-3의 목적(「멜로디 신디에는 안 보냄」)을 거스른다 |
| SEQ-7 트랙 = MIDI 목적지 하나 | 부분 충족 | #22 |
| 5.10 뮤트·솔로 | 충족(뮤트+솔로 동시 규칙은 REQ에 없음, 지적 3) | `render.rs:48-56`, #1 |
| 결정 2026-10-12 Q1 숨김 꼬리 | 충족 | `render.rs:78`, #2, #9 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이번 diff는 스케줄러 스레드 코드를 바꾸지 않았다(`order_class`를 `pub(crate)`로 바꾼 것뿐). `render`는 제어 스레드에서 돈다.
- 제품 코드의 `unwrap`/`expect`: `render.rs`, `tools/ksq-play/src`에 없다.
- 에러가 삼켜지는 곳
  - `lib.rs:350` `finish`의 `let _ = scheduler.stop()`: 스레드가 죽었을 때뿐이라 정당하다. 그때는 drop도 보낼 수 없다.
  - `bin/ksq-play.rs:20` `ctrlc::set_handler` 실패를 버린다. 그러면 Ctrl-C가 기본 동작(즉시 종료)이 되어 노트가 남을 수 있다. 주석으로 인정한 한계다.
  - `lib.rs:327`, `330`의 `scheduler.panic()?`는 정지·배수 없이 빠지지만, 실패 조건이 「스레드가 끝남」이라 보낼 수단이 없다. 문제 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 없는 이름 포트(`OutputPort::Named`) 트랙을 기본 가상 포트 0번으로 합친다. 같은 채널을 쓰는 다른 트랙과 섞여 프로그램이 덮이고(`C0 00` 뒤 `C0 30`) 같은 음높이가 서로를 끊는다. SEQ-7·OUT-4 위반. `suggest_mapping`이 이미 내는 `Suggestion::Virtual { name }`대로 그 이름의 가상 포트를 따로 만들면 OUT-8과도 맞는다(`Unavailable`이면 오류 또는 경고 후 그 트랙 제외). 테스트 `missing_named_port_does_not_merge_into_other_tracks_channel` | `tools/ksq-play/src/lib.rs:224-229` |
| 2 | 수정 | 없는 `--metronome-port`의 클릭을 기본 가상 포트로 보낸다. MET-3은 「멜로디 신디에는 안 보냄」이 목적이다. 지적 1과 같은 방식(같은 이름의 별도 가상 포트)으로 고치거나, 사용자가 명시한 포트이므로 사용법 오류로 끝내야 한다. 테스트 `missing_metronome_port_does_not_click_on_main_port` | `tools/ksq-play/src/lib.rs:266-269` |
| 3 | 수정 | 뮤트이면서 솔로인 트랙을 「솔로가 이김」으로 정했다. REQ 5.10·SEQ-7은 이 조합을 정하지 않는다(보고서도 인정). CLAUDE.md 규칙상 REQ에 없는 결정은 `docs/decisions/`에 먼저 적어야 한다. 코드는 그대로 두고 결정 문서를 더하면 된다. 회귀 테스트 `render_mute_solo_all_combinations_follow_solo_then_mute_rule`가 현재 규칙을 고정한다 | `crates/keyscore-midi/src/render.rs:50-55` |
| 4 | 제안 | `--metronome-port`만 주고 `--metronome`을 빼면 포트를 열고도 클릭이 없다. 경고하거나 `--metronome`을 함께 켜는 편이 덜 놀랍다 | `tools/ksq-play/src/lib.rs:266`, `285` |
| 5 | 제안 | `--from`이 곡 끝 뒤 마디여도 받아들이고 100 ms 뒤 0으로 끝난다. 오류 문구(「곡 범위 밖」)는 u64 넘침에만 쓰인다. 곡 끝 뒤면 경고하는 편이 낫다 | `tools/ksq-play/src/lib.rs:245-249` |
| 6 | 제안 | 두 번째 Ctrl-C도 채널에 넣기만 한다. 정리 중 멈추면 사용자가 Ctrl-C로 끝낼 수 없다. 두 번째 신호는 즉시 종료(130)가 흔한 관례다 | `tools/ksq-play/src/bin/ksq-play.rs:20-22` |
| 7 | 제안 | 단계 파일 `plan/stages/11-S1-midi-engine.md`의 S1-T7 제목은 여전히 「`ksq-cli play`」다. 분리 결정은 Task 계획에만 있다. 단계 파일에 한 줄 적어 두면 출구 게이트 확인 때 헷갈리지 않는다 | `plan/stages/11-S1-midi-engine.md:52` |

구조 결정(`ksq-play` 분리)은 타당하다. `check-deps.sh` 통과, `cargo tree -p ksq-cli`에 midi·coremidi·ctrlc가 없다. `ksq-play`는 금지 목록 검사 대상이 아닌데, 이 crate가 `CoreMIDI`를 쓰는 것이 목적이라 맞다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2는 실패하는 테스트가 있고 SEQ-7·MET-3(부분 충족 = 미충족)에 걸린다. 지적 3은 결정 문서만 더하면 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`order_class` 공개 범위 변경은 `render`가 쓰므로 범위 안)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`git diff main..HEAD`에 삭제된 테스트·`#[ignore]` 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(`ctrlc` MIT/Apache-2.0, `cargo deny`는 보고서 기준 통과)
- [x] 스타일 지적을 적지 않았다
