# Task 보고: S1-T6 SMF 입출력

- PR: 브랜치 `s1/t6-smf`
- 계획 파일: `docs/reports/s1-t6/plan.md`
- 구현: 주 세션. 공개 샘플 수집은 경량 모델 서브 에이전트
- 계획 대비 달라진 점
  - core에 공개 API 셋을 더했다.
    - `TempoMap::from_changes`, `MeterMap::from_changes`: `set`을 차례로 부르면 O(n²)이라, 템포 변화가 많은 악의적 SMF에서 멈출 수 있었다.
    - `MeterMap::next_change_after`: S1-T5 리뷰 1 제안 4의 이월. 메트로놈 박 찾기를 이분 탐색으로 바꿨다.
  - 가사만 담은 트랙(LilyPond, 노래방 파일)의 가사는 가사 틱에 노트가 가장 많이 시작하는 곡 트랙에 붙인다. 계획에는 「붙일 노트가 없으면 버림」만 있었다. 샘플(bach)에서 가사 110개가 모두 사라지는 것을 보고 바꿨다.
  - 테스트 1의 Type 0 비교는 노트 단위가 아니라 켜기·끄기 이벤트 집합으로 한다(아래 한계 1).
  - 테스트 2개를 더했다: `many_tempo_and_lyric_events_import_without_quadratic_time`, `lyrics_only_track_attaches_to_matching_voice`.
  - S6 단계 파일(S6-T3)에 레인 보존 이월 줄을 더했다.

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `src/smf/raw.rs` | `SmfFile::read_with`/`write`, `SmfLimits`(트랙 1024, 이벤트 200만), `SmfError`. running status(메타·SysEx 뒤에도 이음), 가변 길이 수 4바이트 상한, 모르는 청크 건너뛰기. 쓰기는 running status 없이 결정적, 28비트를 넘는 델타는 빈 텍스트 메타로 나눔 |
| `src/smf/song.rs` | `Song`·`SongTrack`·`SongNote`. 재배율(반올림 정수), 트랙 나누기, FIFO 노트 짝짓기, 7·14비트 → 32비트 min-center-max 확장, 가사 디코딩(UTF-8, 아니면 Latin-1), 같은 틱 쓰기 순서(Note Off, CC, 프로그램, 벤드, 프레셔, 가사, Note On), 마디 단위 클립 묶기, 클립 펼치기와 꼬리 자르기 |
| `keyscore-core` `tempo.rs`, `meter.rs` | `from_changes`, `next_change_after` |
| `src/scheduler.rs` | `beat_at_or_after`가 `next_change_after`를 쓴다 |
| `tests/data/smf/` | 퍼블릭 도메인 SMF 10개와 `SOURCES.md` |

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 717 passed, 0 failed
SMF 테스트 16개 통과. 템포·가사 20만 개 파일 가져오기 약 0.5 s(디버그)
~~~

| 계획 # | 실제 이름 | 상태 |
| --- | --- | --- |
| 1 | `public_samples_round_trip_event_sets` | 통과(Type 1은 곡 전체 같음, Type 0은 채널별 이벤트 집합 같음) |
| 2 | `lyrics_round_trip_through_smf_and_project` | 통과 |
| 3 | `velocity_and_cc_survive_round_trip` | 통과 |
| 4 | `note_on_velocity_zero_reads_as_note_off` | 통과 |
| 5 | `running_status_and_meta_between_reads` | 통과 |
| 6 | `ppq_rescale_to_960_is_rounded_integer` | 통과 |
| 7 | `type0_splits_by_channel_and_type1_by_track` | 통과 |
| 8 | `to_project_groups_notes_into_bar_clips` | 통과 |
| 9 | `from_project_flattens_clips_and_cuts_hidden_tails` | 통과 |
| 10 | `malicious_smf_returns_err_without_panic`, `malicious_smf_random_bytes_never_panic` | 통과 |
| 11 | `smpte_division_and_type2_are_rejected` | 통과 |
| 12 | `written_smf_is_byte_stable` | 통과 |
| 13 | `song_round_trip_is_lossless_property` | 통과 |
| 추가 | `many_tempo_and_lyric_events_import_without_quadratic_time`, `lyrics_only_track_attaches_to_matching_voice` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (717 passed)
- [x] deny
- [x] 단계별 추가 게이트: `check-deps` 셋 통과

## 알려진 한계

1. Type 0으로 합칠 때의 노트 짝이 바뀔 수 있다.
   - 서로 다른 트랙이 같은 채널·음높이 노트를 겹쳐 쓰면(샘플 `gm-128-sounds-demo`), Type 0 한 줄에는 어느 Note Off가 어느 Note On의 짝인지 남지 않는다. 다시 읽으면 짝이 바뀐다.
   - 켜기·끄기 이벤트 집합은 그대로다. MIDI 1.0 스트림의 한계라 고칠 수 없다.
2. 모델로 옮길 때 CC·벤드·프레셔(레인)는 버린다.
   - 곡 계층(`Song`) 왕복은 보존한다.
   - 레인 저장은 S6-T3에서 만든다(이월 줄 추가).
3. 다음 메타·메시지는 곡 계층에서 버린다. `SmfFile` 계층에는 남는다.
   - 폴리 프레셔, SysEx
   - 이름·템포·박자표·가사 밖의 메타(텍스트, 마커, 조성, 저작권)
   - 키 서명 → 프로젝트 조성 규칙은 REQ에 없다.
4. Type 0 가져오기는 트랙 이름을 채널 트랙에 붙이지 않는다(대개 곡 제목이다). 이름 없는 트랙은 모델에서 `Track N`이다.
5. 노트 짝짓기와 붙이기 규칙
   - 같은 (채널, 음높이)의 노트는 먼저 켠 것을 먼저 끈다.
   - 끝나지 않은 노트는 트랙 끝에서 끝난다. 짝 없는 Note Off는 버린다.
   - 가사는 같은 틱에 시작하는 가장 높은 음에 붙인다. 그런 노트가 없는 가사는 모델로 옮길 때 버린다.
6. 메타·SysEx 뒤에도 running status를 잇는다. 명세보다 너그럽게 읽는 쪽을 택했다.

## 리뷰어에게

스스로 불안한 곳:

1. `raw.rs` `read_track`
   - running status 때 커서를 한 칸 물리는 방식(`c.pos -= 1`)
   - 오프셋 계산
2. `song.rs` `bar_groups`의 클립 끝 계산.
   - `bar_start(last + 1)`이 없으면(틱 범위 끝) 노트 끝까지다.
   - 마디 안에서 끝나는 노트가 다음 마디를 「찬 마디」로 만들지 않는지(`end - 1`로 마지막 마디를 셈).
3. `split_track`의 가사 붙이기와 `attach_lyrics`가 Type 1 왕복에서 같은 트랙을 고르는지. 속성 테스트는 트랙 하나짜리 SMF만 만든다.
4. `upscale`이 MIDI 2.0 명세 알고리즘과 같은지. 테스트는 왕복과 꼭대기·가운데만 본다.
5. `from_project`의 뱅크 → CC 0/32 변환과 `to_project`의 `bank_before`. 첫 프로그램 틱까지의 마지막 CC 0·32를 쓴다.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | Note Off 순서가 어긋나면 노트 정렬 삽입이 O(n²) | 노트를 모아 한 번에 정렬한다(`split_track`, `from_project`). 커밋 `midi: Bound SMF import time …` |
| 2 | 가사만 담은 트랙의 붙이기가 (가사 × 트랙) | 가사 틱별 개수를 세고 트랙의 노트 시작을 한 번씩만 본다. (노트 + 가사)에 비례. 같은 커밋 |
| 3 | `to_project`의 클립 넣기가 O(클립²)(core 겹침 검사) | core `check_free`를 이분 탐색으로 바꿨다. 클립이 정렬·비겹침이라 끝도 정렬되어 있다. 동작은 같다. 커밋 `core: Check clip overlap by binary search` |
| 4 | 낮은 PPQ의 큰 델타로 쓰기 출력이 960배 | 곡 길이 상한 `MAX_SONG_TICKS = u32::MAX`(PPQ 960)를 두었다. 넘으면 읽기·쓰기 모두 `SmfError::TooLong`이다. 채움 메타는 트랙당 최대 16개다. `to_smf`는 `Result`를 돌려준다 |
| 5 | 화음 위 여러 가사가 왕복에서 옮겨지거나 사라짐 (제안) | 반영. 같은 틱의 가사를 높은 음부터 쓴다. 가사가 있는 가장 낮은 음까지, 가사 없는 위 음 자리는 빈 가사로 채운다. 읽을 때 같은 순서로 붙이고 빈 가사는 자리만 차지한다. 테스트 `chord_lyrics_round_trip_through_smf_to_same_notes` |
| 6 | 노트·프로그램이 없는 모델 트랙은 왕복에서 사라짐 (제안) | 반영하지 않았다. SMF에는 채널 메시지 없는 트랙의 채널을 적을 자리가 없다. 빈 트랙을 내보내는 일은 S2 내보내기 메뉴에서 경고로 다룬다 |
| 7 | 노트 길이 u32 잘림, 65535트랙 넘으면 조용히 버림 (제안) | 반영. 길이 상한이 u32라 잘림이 없다. `to_smf`는 트랙이 헤더에 담기지 않으면 `TooManyTracks`다 |

리뷰어 테스트 변경
- `to_smf`가 `Result`를 돌려주게 되어 호출에 `.unwrap()`을 붙였다.
- `low_ppq_huge_delta_does_not_amplify_written_size`는 리뷰가 제안한 길이 상한에 따라 읽기의 `TooLong`도 받아들인다. 출력 크기 검사는 그대로다.

리뷰 테스트 11개 모두 통과(실패하던 4개는 합쳐 1.2 s). 워크스페이스 730개 통과.
