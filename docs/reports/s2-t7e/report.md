# Task 보고: S2-T7e S2-T7b·T7c·T7d 리뷰 이월 줄 정리

- PR: 로컬 브랜치 `s2/t7e-carry-overs`
- 계획 파일: docs/reports/s2-t7e/plan.md
- 계획 대비 달라진 점: 계획의 내부 도우미 이름이 조금 다르다(`changed_by_pairing`, `dropped_same_tick`, `follow_device_in_settings`, `remember_user_port`). 시험 C5는 기존 리뷰 시험의 관찰 출력을 단언으로 바꾸는 것이라 새 이름이 없다. 그 밖에는 없음.

## 구현 요약

A. `crates/keyscore-app/src/autosave.rs`(S2-T7b 리뷰 4)

- `remove_dead_locks`: 메타마다 내용 파일이 있는지 본다. 내용 없는 메타가 끝난 세션(이 세션 아님, 잠금을 쥔 프로세스 없음)의 것이면 지우고 「쓰는 중」 세션에 넣지 않는다. 그래서 두 rename 사이에 끝난 세션의 잠금 파일도 함께 지워진다(지적 1). 살아 있는 세션의 내용 없는 메타(두 rename 사이)는 그대로 둔다.
- `claim(content) -> Option<File>`: 열고 잠그는 것을 한 번에 한다. `recover`는 창을 열기 전에 이것으로 잠그고, 잠그지 못하면 그 저장본을 건너뛴다. 잠근 파일을 창의 `claim`으로 넘긴다(지적 3).
- `discard`: `claimed`면 건너뛴다(지적 2). 리뷰 4 시험 `review_r4_discard_of_claimed_autosave_is_rewritten_by_holder`는 바뀌기 전 동작(「동작이 바뀜」이라고 적혀 있었다)을 기대했으므로 기대값을 1로 바꿨다. 데이터 손실이 없다는 끝은 같다.
- `offer_recovery`: 복구 대상이 없어도 `remove_dead_locks`를 돌린다. 바뀐 창 없이 끝난 세션의 잠금 파일과 1분 조건으로 남았던 잠금 파일이 다음 시작에 지워진다(이월).
- `init`: `try_lock` 실패를 20 ms 간격으로 최대 3회 다시 시도한다(이월). 공개 API로 재현할 수 없어 코드 대조만.

B. `crates/keyscore-midi/src/smf/report.rs`, `song.rs`, `crates/keyscore-app/src/import_export.rs`(S2-T7c 리뷰 2)

- `ExportReport::overlapping_notes`: (SMF 트랙, 채널, 음높이)마다 펼친 (시작, 끝)을 모아 `changed_by_pairing`으로 센다. SMF 읽기는 (채널, 음높이)마다 Note Off가 먼저 켠 노트를 끄므로 틱 순 시작을 틱 순 끝과 차례로 짝지은 것과 같다(노트 길이가 1 이상이라 짝 없는 Off가 없다). 그 짝에 없는 원래 노트가 「다시 읽으면 길이가 바뀌는 노트」다. Type 0은 모든 트랙이 한 SMF 트랙이라 같은 채널끼리 섞는다(제안 1).
- `ImportReport::skipped_events`: `dropped_same_tick`이 같은 재배율 틱의 템포·박자표 가운데 마지막과 값이 다른 것을 센다(제안 2a). `yields_song_track`이 `split_track`의 트랙 생성 조건(노트를 켜는 Note On, CC, 프로그램, 채널 프레셔, 벤드)을 흉내 내어, 곡 트랙이 되지 않는 Type 1 트랙의 이름을 버린 것으로 센다(제안 2b). `Rescale`을 `pub(super)`로 열고 `for_ppq`를 더했다.
- `import_text_into`: 같은 이름·채널·프로그램·뱅크·겹치지 않음을 한 `find`로 보아 같은 이름 트랙이 여럿이면 조건에 맞는 첫 트랙에 넣는다(제안 3).

C. `crates/keyscore-app/src/engine.rs`(S2-T7d 리뷰 3)

- `visual_match`: 보이는 모양이 같은 목적지 가운데 고유 ID가 가장 작은 것. `open_port`(대체 포트가 비킬 장치)와 `release_mapped`(트랙을 되돌릴 장치 이름)가 쓴다. 정확히 같은 이름의 `best_match`와 같은 기준이다(제안 2).
- `follow_device_in_settings`: 메트로놈 포트·클럭 포트 설정이 대체 포트를 가리키면 장치 이름으로 고친다. `release_mapped`(이름이 다를 때)와 `install`(시작 때 장치가 이미 있어 대체 포트를 뺄 때)이 부른다. 재생 창의 반영은 뒤따르는 `remove_virtual_port`의 다시 준비가 한다(제안 1).
- `remember_user_port`: `add_virtual_port`·`add_named_virtual_port`가 설정에 포트를 적을 때 그 이름의 `mapped_ports` 표시를 지운다. `rename_virtual_port`도 새 이름의 표시를 지운다(제안 4).

문서

- `plan/stages/12-S2-app-shell.md`: 이월 줄 셋에 처리 결과를 적었다.

## 테스트 결과

~~~
cargo test --workspace: 테스트 바이너리 212개 모두 ok, 1846 passed, 0 failed, 2 ignored(기존)
리뷰 1·2 대응 뒤(리뷰 시험 포함): 215개 모두 ok, 1876 passed, 0 failed, 2 ignored
cargo test -p keyscore-midi --test smf_report: 15 passed
cargo test -p keyscore-app --test t7e_autosave: 6 passed
cargo test -p keyscore-app --test t7e_port_mapping: 4 passed(자식 프로세스)
cargo test -p keyscore-app --test import_export: 15 passed
cargo test -p keyscore-app --test review_s2_t7d_r3: 3 passed(관찰 출력을 단언으로)
cargo test -p keyscore-app --test review_s2_t7b_r4: 5 passed(기대값 하나 수정)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| A1 | `orphan_meta_of_dead_session_is_removed_with_its_lock` | 통과(수정 전 실패) |
| A2 | `discard_skips_autosave_claimed_by_another_run` | 통과(수정 전 실패) |
| A3 | `recover_skips_autosave_claimed_meanwhile` | 통과(수정 전에도 통과: 시트 뒤의 `claimed` 검사가 이미 있었다. 검사·잠금 사이 틈은 공개 API로 끼울 수 없어 회귀 방지용) |
| A4 | `recovered_window_holds_claim_even_if_first_write_fails` | 통과(수정 전에도 통과, 회귀 방지용) |
| A5 | `startup_without_recoverables_removes_old_dead_locks` | 통과(수정 전 실패) |
| A6 | `live_session_meta_without_content_is_kept` | 통과(수정 전에도 통과, 과한 삭제 방지용) |
| B1 | `same_end_overlap_is_unchanged_and_not_counted` | 통과(수정 전 실패) |
| B2 | `same_start_overlap_is_unchanged_and_not_counted` | 통과(수정 전 실패) |
| B3 | `changed_lengths_match_overlap_count` | 통과(수정 전 실패) |
| B3' | `three_overlapping_notes_count_each_changed_note` | 통과(수정 전 실패: 2로 셌다) |
| B4 | `type0_pools_same_channel_tracks_for_overlap_count` | 통과(수정 전 실패) |
| B5 | `same_tick_tempos_drop_one_and_are_counted` | 통과(수정 전 실패) |
| B6 | `same_tick_equal_tempos_are_not_counted` | 통과(수정 전 실패: 재배율 경우) |
| B7 | `name_of_vanished_track_is_counted` | 통과(수정 전 실패) |
| B8 | `block_goes_to_matching_track_among_same_names` | 통과(수정 전 실패) |
| C1 | `metronome_and_clock_on_mapped_port_follow_device` | 통과(수정 전 실패) |
| C2 | `startup_release_fixes_metronome_and_clock` | 통과(수정 전 실패) |
| C3 | `two_visually_same_devices_pick_smallest_unique_id` | 통과(엔진 수정을 되돌리면 실패하는 것을 확인했다. 먼저 나타나는 장치에 큰 고유 ID를 `kMIDIPropertyUniqueID`로 준다) |
| C4 | `rename_and_add_clear_stale_mapped_mark` | 통과(수정 전 실패) |
| C5 | `review3_metronome_and_clock_on_mapped_port_follow_device`(기존) | 관찰 출력을 단언으로 바꿈, 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음. `init` 재시도(A 5)는 시험이 없다(계획대로 코드 대조).

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(`cargo test --workspace` 최종 1876 passed, 0 failed, 2 ignored. 리뷰어가 CoreMIDI 시험을 함께 돌리던 2회차 실행에서는 재생 시간 측정 시험 `pause_position_does_not_drift_with_many_live_edits`가 한 번 실패했고 혼자·경합 없는 3회차에서는 통과했다)
- [x] deny
- [x] `scripts/check-deps.sh`, `check-deps.test.sh`(16), `bench-check.test.sh`(8)

## 알려진 한계

- `init`의 잠금 재시도(20 ms × 2회)에 닿는 경로는 지금 없다(리뷰 1 제안 3): 새 세션 id를 가리키는 메타가 없고, 잠금 파일 정리는 1분이 안 된 파일을 잠그지 않는다. 앞으로 그 조건이 바뀔 때의 방어로 둔다.
- [복구]와 [버리기]가 다른 실행에서 겹칠 때, [버리기]의 `still_dead`(메타 읽기)와 `claimed`(내용 열기) 사이에 [복구] 쪽의 첫 쓰기 rename 둘이 모두 끝나면 [버리기]가 그 새 저장본을 지운다(리뷰 1 제안 2). 수정 전에도 있던 틈이고 사람 손으로 맞추기 어렵다. [버리기]가 내용을 잠근 채 메타를 다시 읽는 방안은 rename 둘과 완전히 배타적이지 않아 들이지 않았다. S2 출구 전 이월 후보.
- `rename_virtual_port`로 설정 `virtual_ports`에 남은(시작 때 만들지 못한) 이름으로 바꾸면 그 목록에 같은 이름이 둘이 된다(리뷰 1 제안 5, 이번 범위 밖). 다음 시작에 둘째 만들기가 거부돼 알림이 나고 `remove_virtual_port`가 둘 다 지운다. 단계 파일에 이월했다.
- 시작 때 정리(`offer_recovery`)는 메인 스레드에서 돈다. 저장본 폴더의 파일 수만큼 열고 잠가 본다. 파일이 수백 개가 아니면 보이지 않는다.
- `release_mapped`는 나타남 이벤트마다 `destinations()`를 한 번 더 읽는다(대체 포트가 있을 때만).
- 보이는 모양이 같은 장치가 여럿일 때 정확히 같은 이름끼리의 재연결(`Reconnector`)은 바뀌지 않았다. 대체 포트가 가리키는 장치는 이제 고유 ID 최소이고, 그 뒤의 재연결은 그 정확한 이름으로 `best_match`한다.
- `overlapping_notes` 셈은 노트 길이가 1 이상이라는 모델 규칙에 기댄다(짝 없는 Note Off 없음). 벨로시티 0 노트가 1로 나가는 규칙은 그대로다.
- T7a 이월 줄(FAT32 inode, `.bak`, 최근 목록의 파일 시스템 호출)과 S1-T6 이월 줄(가사 붙이기 비용)은 이 Task 범위 밖이다. S2 출구 전에 따로 판단한다.

## 리뷰어에게

- `changed_by_pairing`의 전제(틱 순 시작 ↔ 틱 순 끝 짝)가 `split_track`의 FIFO와 같다는 논증: k번째 Off(틱 순)는 그때까지 켜진 가장 오래된 On을 끈다. 모든 노트의 시작 < 끝이고 같은 틱에서는 Off가 On보다 앞서므로 k번째 Off 전에 적어도 k개의 On이 켜져 있어 짝 없는 Off가 없다. 따라서 k번째 Off는 k번째 On을 끈다. 이 전제가 깨지는 입력(길이 0 노트)은 모델이 막는다.
- `remove_dead_locks`가 이제 시작 때마다 돈다. 살아 있는 세션 판정이 잠금 파일을 잠깐 잠그는 것은 전과 같다(1분이 안 된 잠금 파일은 잠그지 않으므로 막 뜬 실행의 `init`과 겹치지 않는다).
- `recover`에서 잠금을 창보다 먼저 잡으므로 `open_project_window`가 실패하면 `claim`이 함수 끝에서 놓인다(그 저장본은 다음 실행의 복구 대상으로 남는다). 의도한 동작이다.
- C3 시험은 `kMIDIPropertyUniqueID`를 가상 목적지에 직접 정한다. CoreMIDI가 거부하면 시험이 그 자리에서 실패하게 두었다(조용히 지나가지 않는다).

## 리뷰 1 대응

판정 「병합 가능」(`review-1.md`), 제안 6건.

| 제안 | 대응 |
| --- | --- |
| 1: 첫 쓰기 rename 전에 끝난 세션의 세대별 임시 파일이 남는다 | 받아들임. `remove_dead_locks`가 메타 없는 키의 1분 넘은 임시 파일을 지운다. 시험 `generation_temps_of_session_dead_before_first_rename_are_removed`(부록 A + 메타 있는 키·막 생긴 파일은 둔다) |
| 2: [복구]·[버리기] 경합의 남은 틈 | 받아들이지 않음(「알려진 한계」). 방안이 완전하지 않고 수정 전부터 있던 틈이다 |
| 3: `init` 재시도에 닿는 경로 없음 | 보고서 「알려진 한계」 설명을 고쳤다. 코드는 방어로 둔다 |
| 4: 표시 지우기가 정확한 이름 비교 | 받아들임. `remember_user_port`·`rename_virtual_port`가 `same_port_name`으로 지운다 |
| 5: `virtual_ports`에 같은 이름 둘 | 범위 밖. 「알려진 한계」와 단계 파일 이월 줄 |
| 6: 잠그지 못해 건너뛴 저장본 알림 없음 | 받아들임. `recover`가 건너뛴 수를 모든 창에 알린다 |

## 리뷰 2 대응

판정 「병합 가능」(`review-2.md`, 리뷰 1 대응의 좁은 확인), 제안 2건.

| 제안 | 대응 |
| --- | --- |
| 1: 다른 실행이 먼저 복구해 다시 쓴 저장본(`still_dead` 거짓)도 알림 없이 건너뛴다 | 받아들임. 그 경로도 건너뛴 수에 넣는다. 시험 `recover_reports_autosaves_already_taken_by_another_run`(부록 A) |
| 2: 「리뷰어에게」의 `init` 재시도 설명이 「알려진 한계」와 어긋남 | 고쳤다 |

이월(리뷰 2): 살아 있는 창의 첫 쓰기가 1분 넘게 멈추면 다른 실행의 시작 정리가 그 임시 파일을 지워 쓰기가 실패하고 30 s 뒤 다시 쓴다. 손실 없음. `STALE_TEMP_AGE`를 쓰는 다른 정리와 같은 성질이라 따로 처리하지 않는다.
