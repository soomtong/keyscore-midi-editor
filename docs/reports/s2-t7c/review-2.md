# 적대적 리뷰: S2-T7c SMF·계이름 텍스트 가져오기·내보내기 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `ec36bc2`(브랜치 `s2/t7c-import-export`). 리뷰 1 뒤 수정은 `git diff 195db0d..HEAD`
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7c_r2.rs`(10개), `crates/keyscore-midi/tests/review_s2_t7c_r2.rs`(7개). 깨뜨리지 못한 시도만 남겼고 모두 통과한다. 깨뜨린 시도 6개는 부록 A·B
- 리뷰 1 시험(`review_s2_t7c.rs` 앱 17개, midi 7개)과 구현자 시험(`import_export.rs` 14개, `smf_report.rs` 7개), `cargo test --workspace` 모두 통과. 리뷰 시험 파일은 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과

## 1. 코드만 읽고 적은 「내가 검사할 것」

`195db0d..HEAD`의 제품 코드(`import_export.rs`, `report.rs`)를 읽고 적었다.

1. `refuse_open_project_file`이 다른 이름으로 저장의 같은 파일 규칙(자기 창, 링크, 하드 링크, 바깥에서 옮긴 파일)을 모두 타는가. 거부 뒤 파일 흐름이 풀리는가. 닫힌 창의 파일·가져온(경로 없는) 창에서 과하게 막지 않는가.
2. `cx.notify()` 추가로 가져온 창 자동 저장이 걸리는가. 그 창을 버리고 닫으면 저장본이 지워지는가.
3. 텍스트 가져오기의 새 채널·프로그램·뱅크 조건: 우리 내보내기 텍스트의 재가져오기가 같은 트랙으로 가는가(회귀), 프로그램을 적지 않은 블록, 다른 뱅크, 이름 같은 트랙이 여럿일 때, undo 하나.
4. `overlapping_notes` 셈이 실제로 길이가 바뀌는 노트 수와 맞는가: 끝이 같은 겹침, 시작이 같은 겹침, 한 겹침에 두 노트가 바뀌는 경우, 숨김 꼬리, 맞닿음, 다른 트랙.
5. `kept_meta`가 `Song::from_smf`의 실제 버림과 맞는가: 같은 틱 템포 둘(`TempoMap::from_changes`가 마지막만 남긴다), 사라지는 트랙의 이름, 둘째 이름, 가사 트랙 이름.
6. 빈 가사 제외가 빈 것 아닌 붙지 않은 가사의 셈을 지우지 않는가.
7. 벨로시티 `max(1)` 비교가 1(최소)을 손실로 세지 않는가.
8. `Mismatch` 거부가 쓰기 전에 오는가, 결정 문서와 어긋나는 정책 변경(지적 4의 새 트랙 조건)이 기록되었는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 열린 파일 거부 | 일부 | 리뷰 1 부록 A 두 시험(상위 버전 원본, 다른 창 파일)만. 자기 창·링크·옮긴 파일·거부 뒤 흐름은 리뷰 시험으로 확인(통과) |
| 2 가져온 창 자동 저장 | 있음 | 리뷰 1 부록 A. 버리고 닫기의 저장본 정리는 리뷰 시험으로 확인(통과) |
| 3 블록 채널 조건 | 일부 | 리뷰 1 부록 A 하나. 재가져오기 회귀, 프로그램 생략, 뱅크, undo는 리뷰 시험으로 확인(통과). 이름 같은 트랙 여럿은 깨짐(제안 3) |
| 4 겹침 셈 | 일부 | 리뷰 1 부록 B 하나(바뀌는 경우). 바뀌지 않는 겹침·두 노트 변화는 깨짐(제안 1) |
| 5 메타 셈 | 일부 | 리뷰 1 부록 B 하나. 같은 틱 템포·사라지는 트랙 이름은 깨짐(제안 2) |
| 6 빈 가사 | 일부 | 리뷰 시험 `nonempty_unattached_lyric_is_still_counted`(통과) |
| 7 벨로시티 1 | 없음 | 리뷰 시험(통과) |
| 8 `Mismatch`·결정 기록 | 시험 없음 | 공개 API로 `Mismatch`를 만들 수 없다. 결정 문서는 바뀌지 않았다(제안 4) |
| 팔레트 실행(리뷰 1 제안 8) | 있음 | `menu_and_palette_run_import_export`가 팔레트 줄과 실행을 본다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 리뷰 1 시험 24개와 구현자 시험 다시 실행 | 못 깨뜨림(모두 통과) | 기존 |
| 2 | 저장된 창이 자기 `.ksq`로 SMF·텍스트 내보내기, 거부 뒤 다른 경로로 다시 내보내기 | 못 깨뜨림: 두 번 거부, 파일 그대로, 흐름이 풀려 셋째는 쓴다 | `export_to_own_file_is_refused_then_flow_continues` |
| 3 | 다른 창이 연 파일의 심볼릭 링크(`.mid`)·하드 링크(`.txt`)로 내보내기 | 못 깨뜨림(둘 다 거부) | `export_through_link_to_open_file_is_refused` |
| 4 | 연 뒤 바깥에서 `a.ksq`→`b.ksq`로 옮기고 `b.ksq`로 내보내기 | 못 깨뜨림(기억한 파일 정체로 거부) | `export_to_externally_moved_open_file_is_refused` |
| 5 | 닫힌 창의 `.ksq`, 가져온(경로 없는) 창에서 내보내기 | 못 깨뜨림(과하게 막지 않는다) | `export_to_closed_project_file_is_allowed`, `imported_window_can_export` |
| 6 | 가져온 창 31 s → 저장본 1개 → 닫기 「저장 안 함」 | 못 깨뜨림: 묻고, 저장본을 지운다 | `imported_window_autosave_is_removed_on_discard` |
| 7 | 우리 텍스트 내보내기(ch1 / ch5 prog40 / ch10 prog0 bank300)를 같은 문서에 다시 가져오기 | 못 깨뜨림: 새 트랙 없이 같은 트랙(회귀 없음) | `reimport_of_own_text_goes_to_same_track` |
| 8 | 프로그램 생략 블록 → 같은 트랙, `prog41` → 「Lead 2」 ch5 prog41, undo 하나로 원래대로. 다른 뱅크 → 새 트랙. 앱 흐름 undo | 못 깨뜨림 | `block_program_decides_track_and_undo_is_one`, `block_bank_mismatch_goes_to_new_track`, `import_text_into_new_track_undoes_to_clean` |
| 9 | 이름 같은 트랙 「Lead」 ch1·「Lead」 ch5에 `@track Lead ch5` | 깨짐: 첫 「Lead」만 보고 셋째 트랙 「Lead 2」를 만든다 | 부록 A `block_goes_to_matching_track_among_same_names` |
| 10 | 같은 음 겹침 셈: 바뀌는 겹침, 숨김 꼬리를 자른 뒤 겹치지 않음, 맞닿음, 다른 트랙 같은 채널 | 못 깨뜨림 | midi `overlapping_same_pitch_that_changes_is_reported`, `hidden_tail_cut_at_clip_end_is_not_overlap`, `adjacent_same_pitch_is_not_overlap`, `same_pitch_in_other_track_is_not_overlap` |
| 11 | 끝이 같은 겹침(0–960, 480–960), 시작이 같은 겹침(0–480, 0–1920) | 깨짐: 다시 읽어도 그대로인데 「길이가 바뀔 노트 1개」 | 부록 B `same_end_overlap_is_unchanged_and_not_counted`, `same_start_overlap_is_unchanged_and_not_counted` |
| 12 | 0–1920·480–480 겹침 | 깨짐(수): 두 노트 길이가 바뀌는데 1로 센다 | 부록 B `changed_lengths_match_overlap_count` |
| 13 | 벨로시티 1과 0 | 못 깨뜨림(0만 센다) | midi `velocity_one_is_lossless_zero_is_counted` |
| 14 | 빈 가사와 빈 것 아닌 엇박 가사가 함께 있는 트랙 | 못 깨뜨림(엇박 가사 1개만 센다) | midi `nonempty_unattached_lyric_is_still_counted` |
| 15 | Type 1 곡 트랙 둘째 이름, 가사만 담은 트랙의 이름 | 못 깨뜨림(둘 다 버림으로 센다, 첫 이름은 남김) | midi `second_name_and_lyric_track_name_are_counted` |
| 16 | 두 트랙에 같은 틱 0의 서로 다른 템포(125 BPM, 180 BPM) | 깨짐: 하나만 남는데 손실 없음 | 부록 B `same_tick_tempos_drop_one_and_are_counted` |
| 17 | 이름과 폴리 프레셔만 있는 Type 1 트랙 | 깨짐(수): 트랙이 사라져 이름도 버리는데 이름은 세지 않는다(폴리 프레셔는 세므로 손실 있음 보고는 맞다) | 부록 B `name_of_vanished_track_is_counted` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | 채널 조건은 대상 트랙을 줄이기만 한다. 겹침 판정은 그대로(시도 8·9) |
| 노트는 클립 안, 채널 1–16 | 못 깨뜨림 | 변경 없음 |
| 텍스트 가져오기는 undo 하나 | 못 깨뜨림 | 새 트랙 + 클립도 `apply` 한 번(시도 8) |
| 내보내기가 열린 프로젝트 파일을 바꾸지 않음 | 못 깨뜨림 | `is_open_anywhere`가 경로·링크·하드 링크·옮긴 파일을 본다(시도 2–4) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-2 SMF Type 0/1 가져오기·내보내기 | 충족. 손실 보고의 수가 드문 조합에서 어긋난다(제안 1·2) | `import_export.rs:191-245`, `report.rs` |
| IO-3 텍스트 가져오기·내보내기 | 충족. 블록 ch·prog·bank를 버리지 않는다 | `import_export.rs:74-104` |
| SEQ-9 트랙 나누기 | 충족(변경 없음) | `report.rs` |
| 5.14 `@track` 블록 = 클립 | 충족. 같은 이름 트랙 여럿에서 첫 트랙만 본다(제안 3) | `import_export.rs:76-85` |
| LYR-6 SMF Lyric 왕복 | 충족. 빈 채움 가사를 세지 않는다 | `report.rs:113-166` |
| IO-5 자동 저장 | 충족(가져온 창도 편집 없이 저장본, 버리면 지움) | `import_export.rs:203-207` |
| 결정 2026-10-12·2026-10-21 1 A·T7a 같은 파일 규칙 | 충족 | `import_export.rs:457-465`, `files.rs:782-822` |
| 결정 2026-10-22 2 A(같은 이름이면 그 트랙) | 부분 충족: 채널·프로그램·뱅크가 다르면 새 트랙으로 바꿨는데 결정 문서에 적지 않았다(제안 4) | `import_export.rs:74-85` |
| 결정 2026-10-22 3 A(손실 종류와 수) | 충족(종류). 수는 제안 1·2의 드문 경우에 어긋난다 | `report.rs:184-220` |

결정 2 A의 「부분 충족」은 동작이 아니라 기록의 문제다. 리뷰 1이 「정책은 구현자 판단, 필요하면 결정 요청」으로 열어 두었고, 새 동작은 손실을 숨기지 않는 쪽이다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(스케줄러를 건드리지 않는다).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. `refuse_open_project_file`·`Mismatch`는 시트로 올라간다.

## 7. 지적 사항

차단·수정 없음. 아래는 모두 제안(거절할 수 있다).

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `overlapping_notes`가 「앞 노트 끝 전에 시작하는 노트」를 세므로 요약 「같은 음높이와 겹쳐 길이가 바뀔 노트 N개」와 수가 어긋난다. (a) 끝이 같거나 시작이 같은 겹침은 SMF FIFO로 다시 읽어도 그대로인데 1로 센다(과대). (b) 0–1920·480–480은 두 노트가 바뀌는데 1로 센다(과소). 경고 자체는 나오고(사람이 겹침을 알게 된다), 같은 트랙 같은 음 겹침이 드물어 제안으로 둔다. 고칠 길: 실제 FIFO 짝을 흉내 내 바뀌는 노트를 세거나, 문구를 「같은 음높이가 겹친 곳 N곳(다시 읽으면 길이가 바뀔 수 있다)」로. 재현: 부록 B 세 시험 | `report.rs:211-220`, `import_export.rs:371-375` |
| 2 | 제안 | 메타 셈이 `Song::from_smf`의 버림과 두 곳에서 어긋난다. (a) 같은 틱의 템포(박자표도 같다) 여럿은 `from_changes`가 마지막만 남기는데 모두 남긴 것으로 센다. 값이 다른 경우만 세야 한다(Type 1 파일은 같은 값을 여러 트랙에 겹쳐 적는 일이 흔하다). PPQ가 960보다 크면 재배율로 다른 틱이 같은 틱이 되어도 같다. (b) 채널 메시지가 폴리 프레셔뿐이라 사라지는 트랙의 이름을 남긴 것으로 센다(폴리 프레셔는 세므로 「손실 있음」은 맞다). 재현: 부록 B `same_tick_tempos_drop_one_and_are_counted`, `name_of_vanished_track_is_counted` | `report.rs:12-31` |
| 3 | 제안 | 이름 같은 트랙이 여럿이면 `find`가 첫 트랙만 보고, 그 트랙이 채널이 다르거나 겹치면 맞는 둘째 트랙을 두고 새 트랙을 만든다. 손실은 없고(요약에 「새 트랙 1개」) 드문 조합이다. 고칠 길: `filter`를 `find` 안으로 합쳐 조건을 모두 만족하는 첫 트랙. 재현: 부록 A | `import_export.rs:76-90` |
| 4 | 제안 | 지적 4 대응으로 「같은 이름이면 그 트랙」(결정 2026-10-22 2 A)에 채널·프로그램·뱅크 조건이 붙었는데 결정 문서에는 없다. 블록이 `ch`를 적지 않으면 ch1로 읽혀, ch5 「Lead」가 있는 문서에 `@track Lead`를 가져오면 「Lead 2」 ch1이 생기는 것도 이 정책의 결과다. 결정 문서에 한 줄 덧붙이거나 보고서에 근거를 남긴다 | `docs/decisions/2026-10-22-s2-t7c-import-export.md:14-21` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1의 차단 1과 수정 4개(2–5), 받아들인 제안 3개(6–8)가 모두 고쳐졌고 회귀를 찾지 못했다. 새로 찾은 것은 드문 조합의 셈 정밀도와 기록 문제(제안 4개)뿐이다. 원본·열린 파일을 바꾸거나, 저장 안 된 변경을 잃거나, 불변식·undo를 깨거나, 손실을 숨기는 경로는 찾지 못했다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(재리뷰라 리뷰 1 문서와 대응 표는 먼저 읽었다. 구현자 시험은 열지 않았고 리뷰 1 시험 파일은 도움 함수를 빌리려 열었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 6개 모두 리뷰 시험 파일 끝에 붙여 실패를 확인했다)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`195db0d..HEAD`는 `import_export.rs`, `report.rs`, 시험, 보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`menu_and_palette_run_import_export`는 늘기만 했다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 앱 재현 시험(실패)

`crates/keyscore-app/tests/review_s2_t7c_r2.rs` 끝에 그대로 붙이면 돌아간다(`use`는 그 파일 것을 쓴다). 지금 실패한다.

~~~rust
/// 이름이 같은 트랙이 둘(ch1, ch5)이면 ch5 블록은 맞는 둘째 트랙에 들어가야 한다. 첫 트랙만 보고 새 트랙을 만든다.
#[test]
fn block_goes_to_matching_track_among_same_names() {
    let base = ksq::from_str(
        r#"{"tracks": [
            {"name": "Lead", "channel": 1, "clips": []},
            {"name": "Lead", "channel": 5, "clips": []}]}"#,
    )
    .unwrap()
    .0;
    let mut doc = ProjectDocument::new(base).unwrap();
    let parsed = keyscore_text::parse("@track Lead ch5\nc\n", Mode::Lenient).unwrap();
    let s = import_text_into(&mut doc, &parsed, Tick::ZERO).unwrap();
    let names: Vec<_> = doc
        .project()
        .tracks()
        .iter()
        .map(|t| (t.name().to_owned(), t.channel().get(), t.clips().count()))
        .collect();
    assert_eq!(s.new_tracks, 0, "{names:?}");
}
~~~

## 부록 B. SMF 보고 재현 시험(실패)

`crates/keyscore-midi/tests/review_s2_t7c_r2.rs` 끝에 그대로 붙이면 돌아간다(`smf`·`track`·`clips`·`note`·`v100`·`round_trip` 도움 함수는 그 파일 것을 쓴다). 다섯 시험 모두 지금 실패한다.

~~~rust
/// 끝이 같은 같은 음 겹침은 다시 읽어도 그대로인데 「길이가 바뀔 노트」로 센다(과대).
#[test]
fn same_end_overlap_is_unchanged_and_not_counted() {
    let p = clips(&[(0, 3840, vec![note(0, 960, 60, v100()), note(480, 480, 60, v100())])]);
    let (notes, _, ex) = round_trip(&p);
    assert_eq!(notes, vec![(0, 960), (480, 480)], "다시 읽은 노트");
    assert_eq!(ex.overlapping_notes, 0, "바뀌지 않는데 셌다: {ex:?}");
}

/// 시작이 같은 같은 음(길이만 다름)도 그대로인데 센다(과대).
#[test]
fn same_start_overlap_is_unchanged_and_not_counted() {
    let p = clips(&[(0, 3840, vec![note(0, 480, 60, v100()), note(0, 1920, 60, v100())])]);
    let (notes, _, ex) = round_trip(&p);
    assert_eq!(notes, vec![(0, 480), (0, 1920)], "다시 읽은 노트");
    assert_eq!(ex.overlapping_notes, 0, "바뀌지 않는데 셌다: {ex:?}");
}

/// 겹침 하나로 두 노트의 길이가 바뀌는데 1로 센다(과소).
#[test]
fn changed_lengths_match_overlap_count() {
    let p = clips(&[(0, 3840, vec![note(0, 1920, 60, v100()), note(480, 480, 60, v100())])]);
    let (notes, _, ex) = round_trip(&p);
    let changed = [(0u64, 1920u32), (480, 480)]
        .iter()
        .filter(|n| !notes.contains(n))
        .count();
    assert_eq!(changed, 2, "{notes:?}");
    assert_eq!(ex.overlapping_notes, changed, "{ex:?}");
}

/// 같은 틱의 서로 다른 템포 둘은 하나만 남는데 손실 없음.
#[test]
fn same_tick_tempos_drop_one_and_are_counted() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20])]),
            track(&[
                (0, &[0xFF, 0x51, 0x03, 0x05, 0x16, 0x15]),
                (0, &[0x90, 60, 100]),
                (480, &[0x80, 60, 0]),
            ]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tempo_map().changes().count(), 1);
    assert!(!report.is_lossless(), "템포 하나를 버렸는데 손실 없음: {report:?}");
}

/// 폴리 프레셔만 있는 Type 1 트랙의 이름은 트랙이 사라져 버려지는데 남긴 것으로 센다.
#[test]
fn name_of_vanished_track_is_counted() {
    let file = smf(
        1,
        &[
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
            track(&[(0, &[0xFF, 0x03, 0x01, b'P']), (0, &[0xA0, 60, 10])]),
        ],
    );
    let (p, report) = smf_import(&file).unwrap();
    assert_eq!(p.tracks().len(), 1);
    assert_eq!(report.skipped_events, 2, "폴리 프레셔 1 + 이름 1: {report:?}");
}
~~~
