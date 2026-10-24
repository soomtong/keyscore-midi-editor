# Task 계획: S2-T7e S2-T7b·T7c·T7d 리뷰 이월 줄 정리

- 브랜치: `s2/t7e-carry-overs`
- 관련 REQ: IO-5·8.4(자동 저장·복구), IO-2·IO-3·SEQ-9·5.14(가져오기·내보내기), OUT-3·OUT-5·OUT-8·MET-1(포트 재연결, 대체 매핑)
- 선행 Task: S2-T7b(`autosave.rs`), S2-T7c(`import_export.rs`, `smf/report.rs`), S2-T7d(`engine.rs`·`ports.rs`의 대체 매핑)
- 관련 결정: `2026-10-22-s2-t7c-import-export.md` 구현 세부(같은 이름 트랙 여럿), `2026-10-22-out8-port-mapping-sheet.md` 3 A, `2026-10-15-port-name-visual-duplicates.md`
- 범위 원문: `plan/stages/12-S2-app-shell.md` S2-T7 절의 이월 줄 셋(S2-T7b 리뷰 4, S2-T7c 리뷰 2, S2-T7d 리뷰 3)

## 끝의 정의 (내 말로)

A. 자동 저장본·복구(`crates/keyscore-app/src/autosave.rs`, S2-T7b 리뷰 4)

1. 끝난 세션의 「내용 없는 메타」(`<키>.json`만 있고 `<키>.ksq`가 없음)는 정리 때 지운다. 그 메타가 잠금 파일 정리의 「쓰는 중」 판정에 끼지 않아 그 세션의 잠금 파일도 함께 지워진다(리뷰 4 부록 D1).
2. `discard`는 다른 실행(또는 이 실행의 다른 창)이 복구하며 잠근(`claimed`) 저장본을 지우지 않는다.
3. `recover`는 창을 열기 전에 내용 파일을 잠그고, 잠그지 못하면 건너뛴다. 잠근 파일을 그 창의 `claim`으로 넘긴다(검사와 잠금 사이 틈 제거).
4. 시작 때(`offer_recovery`) 복구 대상이 없어도 끝난 세션의 오래된 잠금 파일·내용 없는 메타를 정리한다. 바뀐 창 없이 크래시한 세션의 잠금 파일이 쌓이지 않는다.
5. `init`은 `try_lock` 실패를 짧게 다시 시도한다(최대 3회).

B. 가져오기·내보내기 보고(`crates/keyscore-midi/src/smf/report.rs`, `crates/keyscore-app/src/import_export.rs`, S2-T7c 리뷰 2)

1. `ExportReport::overlapping_notes`는 「다시 읽으면 길이가 바뀌는 노트 수」다. SMF 읽기의 짝 맞춤(틱 순 Note On을 틱 순 Note Off와 차례로 짝지음)을 (SMF 트랙, 채널, 음높이)마다 흉내 내어, 원래 (시작, 끝) 다중집합에 없는 노트를 센다. Type 0은 모든 트랙이 한 SMF 트랙이므로 같은 채널끼리 섞인다.
2. `ImportReport::skipped_events`에 (a) 같은 재배율 틱의 템포·박자표 가운데 마지막과 값이 다른 것, (b) 곡 트랙이 남지 않는 Type 1 트랙(채널 메시지가 폴리 프레셔·짝 없는 Note Off뿐)의 이름을 더한다. (a)는 같은 값이면 세지 않는다.
3. `import_text_into`는 같은 이름 트랙이 여럿이면 조건(채널·프로그램·뱅크·클립 겹침)에 맞는 첫 트랙을 고른다.

C. 대체 매핑 포트(`crates/keyscore-app/src/engine.rs`, S2-T7d 리뷰 3)

1. 장치가 돌아와 정규화 이름의 대체 포트를 지울 때(`release_mapped`), 메트로놈 포트·클럭 포트 설정이 그 대체 포트를 가리키면 장치 이름으로 고친다(`rename_virtual_port`와 같은 규칙). 시작 때 장치가 이미 있어 대체 포트를 설정에서 뺄 때(`install`)도 같다.
2. 보이는 모양이 같은 장치가 여럿이면 고유 ID가 가장 작은 것을 고른다. `open_port`(대체 포트가 비킬 장치)와 `release_mapped`(트랙을 되돌릴 장치 이름)가 같은 도우미를 쓴다. 정확히 같은 이름의 재연결은 이미 `best_match`(고유 ID 최소)다.
3. 사용자가 가상 포트를 어떤 이름으로 만들거나(`add_virtual_port`, `add_named_virtual_port`) 바꾸면(`rename_virtual_port`) 설정 `mapped_ports`에서 그 새 이름의 표시를 지운다. 시작 때 만들지 못한 대체 포트 이름이 설정에 남아 있어도 사용자의 포트에 대체 표시가 붙지 않는다.

## 공개 인터페이스 초안

공개 시그니처는 바뀌지 않는다. 바뀌는 것은 동작과 내부 도우미다.

~~~rust
// keyscore_app::autosave (내부)
fn remove_dead_locks(dir: &Path, own: &str);        // 내용 없는 메타도 지운다
fn claim(content: &Path) -> Option<fs::File>;        // 잠그면 Some, 아니면 None
pub fn offer_recovery(cx: &mut App);                 // 대상이 없어도 정리

// keyscore_midi::smf::report (내부)
fn changed_note_count(groups: ...) -> usize;         // SMF 짝 맞춤 흉내
fn yields_song_track(track: &SmfTrack) -> bool;      // split_track이 트랙을 남기는 조건

// keyscore_app::engine (내부)
fn visual_match<'a>(list: &'a [PortInfo], name: &str) -> Option<&'a PortInfo>;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| A1 | `orphan_meta_of_dead_session_is_removed_with_its_lock` | UI | 리뷰 4 부록 D1: 메타만 남은 끝난 세션의 메타와 잠금 파일이 [버리기] 뒤 지워진다 |
| A2 | `discard_skips_autosave_claimed_by_another_run` | UI | 다른 실행이 잠근 저장본은 [버리기]가 지우지 않는다 |
| A3 | `recover_skips_autosave_claimed_meanwhile` | UI | 시트가 떠 있는 동안 다른 실행이 잠근 저장본은 [복구]가 창을 열지 않는다 |
| A4 | `recovered_window_holds_claim_even_if_first_write_fails` | UI | 복구 창은 첫 쓰기가 실패해도 잠금을 쥐고, 다른 실행은 복구 대상으로 보지 않는다 |
| A5 | `startup_without_recoverables_removes_old_dead_locks` | UI | 복구 대상 없이 띄워도 오래된 끝난 세션 잠금 파일·내용 없는 메타를 지우고 막 생긴 것은 둔다 |
| A6 | `live_session_meta_without_content_is_kept` | UI | 살아 있는 세션의 내용 없는 메타(rename 사이)는 지우지 않는다 |
| B1 | `same_end_overlap_is_unchanged_and_not_counted` | 단위 | 리뷰 2 부록 B: 끝이 같은 겹침은 0 |
| B2 | `same_start_overlap_is_unchanged_and_not_counted` | 단위 | 부록 B: 시작이 같은 겹침은 0 |
| B3 | `changed_lengths_match_overlap_count` | 단위 | 부록 B: 두 노트가 바뀌면 2 |
| B4 | `type0_pools_same_channel_tracks_for_overlap_count` | 단위 | Type 0은 같은 채널의 다른 트랙과 섞여 바뀌는 노트를 센다, Type 1은 0 |
| B5 | `same_tick_tempos_drop_one_and_are_counted` | 단위 | 부록 B: 같은 틱 다른 템포 둘 → 1 |
| B6 | `same_tick_equal_tempos_are_not_counted` | 단위 | 같은 틱 같은 템포(Type 1 흔한 경우)는 손실 아님. PPQ 재배율로 같은 틱이 되는 경우도 |
| B7 | `name_of_vanished_track_is_counted` | 단위 | 부록 B: 폴리 프레셔만 있는 트랙의 이름은 손실 |
| B8 | `block_goes_to_matching_track_among_same_names` | 단위 | 리뷰 2 부록 A: 같은 이름 둘째 트랙에 들어간다 |
| C1 | `metronome_and_clock_on_mapped_port_follow_device` | UI(CoreMIDI, 자식 프로세스) | 장치가 돌아오면 메트로놈·클럭 설정이 장치 이름이 된다 |
| C2 | `startup_release_fixes_metronome_and_clock` | UI(CoreMIDI) | 시작 때 장치가 있어 대체 포트를 뺄 때도 설정을 장치 이름으로 고친다 |
| C3 | `two_visually_same_devices_pick_smallest_unique_id` | UI(CoreMIDI) | 보이는 모양이 같은 장치 둘이 함께 나타나면 고유 ID가 작은 쪽으로 트랙이 간다 |
| C4 | `rename_and_add_clear_stale_mapped_mark` | UI(CoreMIDI) | 설정에 남은 대체 표시 이름으로 포트를 만들거나 바꾸면 표시가 지워진다 |
| C5 | 기존 `review_s2_t7d_r3.rs`의 관찰 출력 | 통합 | 관찰을 단언으로 바꾼다(리뷰가 그렇게 지시) |

## 불변식

- 살아 있는 세션의 저장본·메타·잠금 파일은 지우지 않는다(`session_alive`가 먼저). 내용 없는 메타도 그 세션이 살아 있으면 둔다.
- 저장본 하나는 한 번에 한 창만 복구한다: 잠금을 쥔 쪽만 창을 연다.
- 보고 함수는 변환 결과를 바꾸지 않는다(`smf_export`·`smf_import`의 결과는 `Song` 변환과 같다).
- 트랙·설정 변경은 기존 경로(`edit_document`, `AppSettings::update`)만 쓴다. 트랙 변경은 창마다 undo 한 단위.

## 범위 밖

- 자동 저장본 키를 경로 해시로 바꾸는 일(리뷰 1 지적으로 창마다 고유 키가 맞다. 단계 파일 문구만 다르다).
- 보이는 모양이 같은 장치 여럿에서 일반 재연결(`Reconnector`)의 규칙 변경. 정확히 같은 이름끼리는 이미 고유 ID 최소다.
- T7a 이월 줄(FAT32 inode 재사용, `.bak`, 최근 목록 파일 시스템 호출)과 S1-T6 이월 줄(가사 붙이기 비용의 백그라운드 처리). S2 출구 전 별도 판단.
- 결정 문서 수정. 이번 수정은 모두 결정의 구현 세부 안이다.

## 리스크·결정 요청

- 결정 요청 없음.
- `init` 재시도는 공개 API로 재현할 수 없어 코드 대조로만 확인한다.
- CoreMIDI 시험은 목적지 목록·16개 상한을 다른 시험과 나누지 않게 자식 프로세스에서 혼자 돈다(기존 `run_isolated` 틀).
