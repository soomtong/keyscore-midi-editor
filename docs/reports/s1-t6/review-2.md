# 적대적 리뷰: S1-T6 SMF 입출력 (리뷰 2회차)

- 리뷰어 세션: 구현 세션·리뷰 1 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: 7ef5d8d (브랜치 `s1/t6-smf`, 리뷰 1 이후 변경 `ff5bccf..HEAD`)
- 범위: 확인 리뷰. (A) 리뷰 1 지적 1–4가 부분 수정 없이 해결되었는가, (B) core `check_free` 변경이 불변식과 동작을 그대로 두는가, (C) 반영한 제안 5·7이 문서대로 동작하고 거절한 제안 6이 REQ를 어기지 않는가, (D) 회귀 없음.
- 깨뜨리는 테스트: `crates/keyscore-midi/tests/review_s1_t6_r2.rs`, `crates/keyscore-core/tests/review_s1_t6_r2.rs`

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff ff5bccf..HEAD`의 제품 코드(`track.rs`, `song.rs`, `raw.rs`)만 보고, 리뷰어 테스트를 쓰기 전에 적었다.

1. 지적 1: 노트를 모아 한 번에 정렬하는 것은 `split_track`과 `from_project`뿐이다. 노트가 곡 계층 순서 (시작, 길이, 음높이)로 정렬된 뒤 `to_project`가 core 클립 순서 (시작, 음높이, 길이)의 `Clip::insert_note`로 하나씩 넣는다. 한 틱에 노트가 많으면 매번 앞쪽에 끼워 넣어 이차인가.
2. 지적 2: 트랙 고르기는 선형이 되었지만, 고른 트랙에 붙이는 `add_lyric`은 여전히 `partition_point` + `Vec::insert`다. 가사만 든 트랙이 둘 이상(1절·2절)이면 둘째가 매번 가운데에 끼어 이차인가.
3. 지적 3: 이분 탐색은 클립의 끝도 정렬되어 있다는 가정에 기댄다. 길이 0 클립이 있거나 같은 시작의 클립이 둘이면 가정이 깨진다. `Clip::new`가 길이 0을 막는가. `ignore`를 쓰는 `unsplit_clip`(undo)에서도 같은가.
4. 지적 3의 이웃: 박자가 마디마다 바뀌는 곡에서 `MeterMap::position`·`bar_start`가 선형이면 클립 수 × 변화 수인가.
5. 지적 4: 읽기는 트랙 끝 틱만 검사한다. 끝나지 않은 노트는 트랙 끝에서 끝나므로 길이 상한도 지켜지는가. 상한에서 정확히 끝나는 곡, 하나 넘는 곡, PPQ 1의 반올림 경계.
6. 지적 4(쓰기): 노트 끝 = 시작 + 길이가 상한을 넘는 경우, 시작이 u64 끝 가까이인 경우의 넘침, 템포·박자 변화만 상한 뒤인 경우.
7. 읽을 수 있는 곡은 다시 쓸 수 있는가(상한에서 시작하는 끝나지 않은 노트는 쓰면 끝이 상한 + 1).
8. 제안 5: 같은 틱의 같은 음높이 노트가 둘이면 `from_project`는 자리 채움 가사를 노트마다 내고 `bar_groups`는 음높이를 중복 제거해 짝짓는다. 개수가 어긋나는가. Type 0으로 합쳐도 같은 틱 가사 순서가 유지되는가(`finish_track`의 정렬이 안정인가).
9. 제안 7: `TooManyTracks` 경계가 정확히 65534개(지휘 트랙 포함 65535)인가. Type 0은 트랙 수와 상관없는가.
10. 리뷰어 테스트 수정은 `.unwrap()`과 `TooLong` 받아들이기뿐인가. 받아들인 뒤 출력 크기 검사가 여전히 무언가를 검사하는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 한 틱 큰 화음의 클립 넣기 | 없음 | 리뷰 1 테스트는 서로 다른 틱의 노트만 쓴다 |
| 2 가사 트랙 여럿 | 없음 | `orphan_lyrics_with_many_song_tracks_attach_in_bounded_time`은 가사 트랙 하나 |
| 3 끝 정렬 가정 | 일부 | core 기존 속성 테스트가 넣기·옮기기·분할의 불변식을 본다. 전수 비교와 판정 일치는 없음 |
| 4 박자 변화가 잦은 곡 | 없음 | |
| 5 읽기 경계 | 일부 | `song_longer_than_limit_is_rejected_on_read_and_write`는 한참 넘는 곡만 |
| 6 쓰기 경계 | 일부 | 시작 = 상한인 노트 하나만. 끝이 넘는 노트, 템포·박자 변화 없음 |
| 7 읽고 다시 쓰기 | 없음 | |
| 8 같은 음높이 화음 가사, Type 0 | 일부 | `chord_lyrics_round_trip_through_smf_to_same_notes`는 Type 1, 서로 다른 음높이만 |
| 9 트랙 수 경계 | 없음 | |
| 10 리뷰어 테스트 수정 | 해당 | 수정은 두 종류뿐이다. 다만 `low_ppq_huge_delta_does_not_amplify_written_size`의 입력은 상한을 넘어 이제 늘 `TooLong`으로 일찍 끝난다. 상한 안의 출력 크기는 이 리뷰의 `to_smf_checks_note_end_and_conductor_against_limit`가 본다 |

## 3. 깨뜨리기 시도

모두 디버그 빌드에서 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | SMF 트랙 7개 × 채널 16, 노트 7500개씩을 켠 역순으로 끄기(음높이 128개를 돌아 같은 키가 여러 개 열림). 이벤트 168만 개 | 못 깨뜨림. 가져오기 + `to_project` 1.3 s | `reverse_note_off_order_across_channels_and_tracks_imports_in_bounded_time` |
| 2 | 한 틱에 노트 N개(음높이는 내려가고 길이는 늘어남) → `to_project` | 깨짐. N = 6만 2.1 s, 24만 35 s(4배 입력에 17배). 곡 계층 정렬 (시작, 길이, 음높이)과 core 클립 정렬 (시작, 음높이, 길이)이 달라 `Clip::insert_note`가 매번 앞에 끼운다. 이벤트 상한이면 노트 90만 개 | `huge_chord_at_one_tick_builds_clip_in_bounded_time` |
| 3 | CC 트랙 1021 × 16, 틱 0에 5000음 화음과 L개 노트가 있는 트랙, 같은 틱의 가사만 든 트랙 둘(1·2절) | 깨짐. L = 15만 7.3 s, 30만 28 s(2배 입력에 3.8배). 트랙 고르기는 선형이지만 `add_lyric`이 둘째 가사 트랙을 하나씩 가운데에 끼운다 | `two_lyric_only_verses_with_many_tracks_attach_in_bounded_time` |
| 4 | 마디마다 3/4·4/4가 바뀌고(변화 12만 개) 3/4 마디마다 1틱 노트(클립 6만 개) | 못 깨뜨림. 0.2 s, 클립 길이 모두 2880 | `sparse_clips_with_meter_change_every_bar_build_in_bounded_time` |
| 5 | 읽기 경계: 끝 = 2^32 − 1(받음), 2^32(`TooLong { tick: 2^32 }`), PPQ 1에서 4473924(받음)·4473925(`TooLong`) | 못 깨뜨림 | `read_accepts_song_ending_exactly_at_limit_and_rejects_one_past` |
| 6 | 상한 틱에 켜고 끄지 않은 노트: 읽기는 받고(길이 1), 그 곡을 다시 쓰면 `TooLong` | 깨짐(경미). 읽은 곡을 그대로 저장할 수 없다. 조용한 손실은 아니고 `Err`다 | 같은 테스트(현재 동작을 고정) |
| 7 | 쓰기 경계: 시작 0·길이 u32::MAX(끝 = 상한, Type 0·1 모두 300 B 미만, 다시 읽어 같음), (1, u32::MAX)·(상한 − 5, 6)·(상한, 1)·(u64::MAX − 1, u32::MAX)는 `TooLong`, 템포·박자 변화가 상한이면 씀, 상한 + 1이면 `TooLong` | 못 깨뜨림 | `to_smf_checks_note_end_and_conductor_against_limit` |
| 8 | 화음 가사 네 모양(셋 모두 가사, 맨 아래만, 가운데만, 한 음)을 Type 1·Type 0으로 왕복 | 못 깨뜨림 | `chord_lyrics_shapes_round_trip_in_both_formats` |
| 9 | 같은 틱에 G(길이 960), G(길이 480), C「lo」 → Type 1 왕복 | 깨짐. 「lo」가 사라진다. `from_project`가 빈 가사 둘 + 「lo」를 내고 `bar_groups`는 음높이 [G, C]에 앞의 둘(빈 가사)만 짝짓는다 | `chord_lyrics_with_duplicate_pitch_round_trip` |
| 10 | 곡 트랙 65534개 Type 1(트랙 65535개), 65535개 Type 1(`TooManyTracks`), Type 0(트랙 1개) | 못 깨뜨림 | `to_smf_track_count_limit_is_exact` |
| 11 | core: 넣기·옮기기·분할·지우기 무작위 열(512 케이스, 맞닿음·겹침이 잦은 좁은 구간)에서 성공 여부를 전수 비교와 대조, 실패 시 상태 불변, 매 단계 시작 순·비겹침 | 못 깨뜨림 | `check_free_matches_brute_force_for_random_track_ops` |
| 12 | core: 맞닿은 클립들을 `Document`로 무작위 분할한 뒤 모두 undo(`unsplit_clip`이 `ignore` 두 개로 `check_free`), 모두 redo, 단계마다 스냅샷과 같음 | 못 깨뜨림 | `split_undo_redo_with_adjacent_clips_restores_state` |
| 13 | 리뷰 1 테스트 `review_s1_t6.rs` 11개 | 모두 통과(1.2 s) | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 새 `check_free`는 끝 정렬 가정에 기대는데, `Clip::new`·`set_length`가 길이 0을 막아(`clip.rs:47`, `279`) 시작 순 + 비겹침이면 끝도 정렬된다. 시도 11·12에서 전수 비교와 판정이 같다. `move_clip`·`resize_clip`·`replace_notes`는 여전히 선형 `overlaps`를 쓴다(동작 같음) |
| 클립 시작 순 | 아니오 | `place`가 `partition_point`로 넣는다. 시도 11 |
| 노트는 클립 안 | 아니오 | `to_project`는 `Clip::insert_note`를 그대로 거친다 |
| 곡 길이 ≤ `MAX_SONG_TICKS` | 읽기·쓰기 모두 아니오 | `song.rs:228-236`, `348-355`. 시도 5–7 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-2 | 충족(기능). 악의적 입력의 시간 상한은 지적 1·2 | 쓰기 `song.rs:326-368`, 길이 상한 `song.rs:13-16` |
| SEQ-9 | 충족 | 마디 클립 `song.rs:765-834`. 빈 모델 트랙이 왕복에서 사라지는 것(거절한 제안 6)은 SEQ-9 문장 「노트가 있는 구간을 마디 단위 클립으로 묶는다」와 어긋나지 않는다 |
| LYR-6 | 부분 충족 | SMF ↔ 곡의 Lyric 메타 왕복은 된다. 모델 왕복의 같은 음높이 화음에서 가사가 사라진다(지적 3) |

「부분 충족」은 미충족으로 센다. 다만 LYR-6의 모델 왕복 손실은 리뷰 1에서 제안이던 영역의 새 코드 결함이라 심각도는 수정으로 둔다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이번 변경에 없음.
- 제품 코드의 `unwrap`/`expect`: 없음. `text.unwrap_or("")`(`song.rs:456`)는 자리 채움이라 정당하다.
- 에러가 삼켜지는 곳: 새로 생긴 곳 없음. `to_smf`의 `#[must_use]` 제거는 `Result`가 대신한다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 지적 1·3의 부분 수정. 곡 계층 노트는 (시작, 길이, 음높이) 순인데 `to_project`가 (시작, 음높이, 길이) 순인 core 클립에 `Clip::insert_note`로 하나씩 넣는다. 한 틱 화음 24만 음에서 35 s(6만 음의 17배). 클립을 노트 목록으로 한 번에 만들거나(core에 정렬 후 붙이는 공개 경로가 필요하면 결정 요청) 넣기 전에 core 순서로 정렬해야 한다. 테스트 `huge_chord_at_one_tick_builds_clip_in_bounded_time` | `song.rs:394-401`, core `clip.rs:120-121` |
| 2 | 수정 | 지적 2의 부분 수정. `attach_lyrics`의 트랙 고르기는 선형이지만 붙이기가 `add_lyric`(정렬 삽입)이라, 가사만 든 트랙이 둘 이상이면 둘째부터 가운데에 끼워 이차다. 30만 × 2에서 28 s. 붙인 뒤 한 번 안정 정렬하면 된다(같은 틱 순서 유지 필요). 테스트 `two_lyric_only_verses_with_many_tracks_attach_in_bounded_time` | `song.rs:315-317`, `118-121` |
| 3 | 수정 | 제안 5 반영분의 결함. 같은 틱에 같은 음높이 노트가 둘 이상이면 `from_project`는 노트마다 자리 채움 가사를 내고 `bar_groups`는 음높이를 중복 제거해 짝지어, 아래 음의 가사가 밀려 사라진다. `from_project`도 음높이별로 하나만 내야 한다. 테스트 `chord_lyrics_with_duplicate_pitch_round_trip` | `song.rs:424-459`, `766-784` |
| 4 | 제안 | 상한 틱에서 켜고 끄지 않은 노트는 읽기가 받지만(길이 1, 끝 = 상한 + 1) 그 곡을 다시 쓰면 `TooLong`이다. 읽기에서 노트 끝까지 보거나 보고서 한계에 적기. 테스트 `read_accepts_song_ending_exactly_at_limit_and_rejects_one_past`(현재 동작 고정) | `song.rs:228-236`, `601-607` |
| 5 | 제안 | `low_ppq_huge_delta_does_not_amplify_written_size`는 입력이 상한을 넘어 이제 늘 `TooLong`으로 일찍 돌아가 출력 크기 검사를 하지 않는다. 수정 자체는 리뷰 1이 제안한 방향이라 정당하다. 상한 안의 출력 크기는 이 리뷰 테스트가 본다 | `tests/review_s1_t6.rs:162-169` |

거절한 제안 6(빈 모델 트랙이 왕복에서 사라짐)은 REQ ID를 명시적으로 어기지 않아 받아들인다. 리뷰어 테스트 수정은 `.unwrap()` 추가와 `TooLong` 받아들이기뿐이다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1 지적 1–3은 처음 테스트의 모양에서는 해결되었지만, 이웃 입력(한 틱 큰 화음, 가사 트랙 둘)에서 같은 종류의 이차 시간이 남는다(지적 1·2). 반영한 제안 5에 가사 손실 결함이 있다(지적 3). 지적 4(곡 길이 상한)와 core `check_free`는 경계·전수 비교에서 깨지지 않았다. 실패하는 테스트 3개는 그대로 저장소에 남긴다. 그 밖의 워크스페이스 테스트는 모두 통과한다(`cargo test --workspace --no-fail-fast`: 738 통과, 위 3개 실패).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 diff만 보고 적음. 구현자가 `smf.rs`에 추가한 두 테스트는 diff에 함께 보였다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다(IO-2, SEQ-9, LYR-6)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(core `track.rs`는 리뷰 1 지적 3 대응으로 보고서에 적힘)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
