# 적대적 리뷰: S2-T7a 새로 만들기·열기·저장·닫기·최근 파일 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음). 리뷰 1 세션과도 다르다
- 대상 커밋: `5159f7b`(브랜치 `s2/t7a-save-open`). 리뷰 1 뒤 수정은 `b6a742e..5159f7b`(`e9508ce`, `d3dce83`, `9a80c8b`, `b81be61`, `d56474e`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7a_r2.rs`(깨뜨리지 못한 시도 18개, 모두 통과). 깨뜨린 시도 5개는 부록 A

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 지적 1 수정(`save_as`의 읽기 전용 검사)은 「지금 경로와 같은 파일」만 본다. 연 뒤 원본이 옮겨지거나 이름이 바뀌면 원본을 다시 고를 수 있는가. 연 뒤에 만든 링크·하드 링크, 다른 창에서의 다른 이름 저장, 닫기·끝내기 시트를 거친 경로도 막히는가.
2. 지적 2 수정(`confirm_close_all` 반복 + `Quit` 재발송): `app::Quit` Action 경로에서 끝나지 않는 반복이나 시트 재요청이 있는가. 끝내기 두 번, 끝내기 중 다른 창의 닫기 시트, 대화상자가 떠 있는 바뀐 창에서 묻지 않고 끝나는가.
3. 지적 3 수정(`open_paths`의 `file_flow` 검사): 남겨진 창의 대화상자 답이 그 창에 반영되고 흐름이 풀리는가.
4. 지적 4·6 수정(`same_file`의 `(dev, ino)`): 내용만 같은 복사본을 같은 파일로 잘못 묶는가(열기 거부, 최근 목록 병합). 하드 링크 두 이름이 최근 목록에 하나인가.
5. `file_flow`를 세우지 않는 시트(열기 실패 시트, `open_paths`의 `ask`)가 있는 창에 닫기·끝내기 시트가 겹치는가.
6. 같은 파일 두 창: 두 창이 같은 새 경로를 차례로 고를 때, 대화상자 중 같은 파일을 다른 창에 열었을 때, 연 파일을 바깥에서 이름을 바꿨을 때.
7. 최근 파일: 설정에 같은 파일이 두 모양으로 있을 때 시작 직후 메뉴, 이름이 같은 셋의 라벨.
8. 포커스가 없는 창에서 `window::Close`가 전역 처리기(설정 창용, 바로 `remove_window`)로 빠져 묻지 않고 닫히는가(gpui-kit `Root`가 뷰를 감싼다).
9. UI-11: 파일·창·끝내기 Action이 모두 메뉴에 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 상위 버전 원본 보호의 우회 | 원본 경로 그대로 고르기만(`review_save_as_onto_newer_version_original_keeps_original`) | 링크·하드 링크·닫기/끝내기 경유·옮김 없음 |
| 2 `Quit` 재발송·끝내기 두 번 | `confirm_close_all` 직접 호출만. `app::Quit`은 `quit_action_with_dirty_window_asks_first`(취소) 하나 | 재발송 분기(`lib.rs:76`)는 시험 플랫폼에서 끼어들 틈을 만들 수 없어 코드로만 봤다 |
| 3 남겨진 창의 대화상자 완료 | 지적 3 재현 테스트는 「닫힌 창이 쓰지 않음」만 본다 | 남은 창에 반영되는지·흐름이 풀리는지 없음 |
| 4 복사본 오병합 | 없음 | |
| 5 열기 실패 시트와 겹침 | 없음 | |
| 6 같은 파일 두 창 경합 | 다른 창이 연 파일로 다른 이름 저장만 | |
| 7 시작 직후 최근 목록 중복 | 없음(리뷰 1 재현은 파일을 연 뒤를 본다) | |
| 8 포커스 없는 닫기 | 없음 | |
| 9 UI-11 | `commands.rs`의 `action_counts_match_palette_and_menus` 등 | 파일 Action만 따로 확인 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 상위 버전 파일을 연 뒤 원본을 바깥에서 옮기고, 저장 → [다른 이름으로 저장] → 옮긴 원본 선택 | 깨짐: 옮긴 원본이 `format_version: 1`로 덮인다 | 부록 `review_r2_save_as_onto_moved_newer_original_keeps_original` |
| 2 | 연 파일을 바깥에서 이름 바꾼 뒤 새 이름으로 열기 | 깨짐: 같은 inode가 두 창에 열린다 | 부록 `review_r2_open_renamed_file_activates_existing_window` |
| 3 | 바뀐 창에 열기 실패 시트가 떠 있을 때 끝내기 | 깨짐: 같은 창에 끝내기 시트가 겹친다 | 부록 `review_r2_quit_during_open_error_sheet_does_not_stack` |
| 4 | 바뀐 창에 열기 실패 시트가 떠 있을 때 `window::Close` | 깨짐: 같은 창에 닫기 시트가 겹친다 | 부록 `review_r2_close_during_open_error_sheet_does_not_stack` |
| 5 | 설정에 같은 파일을 정규 경로와 `sub/../`로 넣고 시작 | 깨짐: 처음 파일을 열거나 저장할 때까지 메뉴·팔레트에 두 줄 | 부록 `review_r2_recent_noncanonical_duplicate_at_startup_shown_once` |
| 6 | 상위 버전 원본을 연 뒤 만든 심볼릭 링크로 다른 이름 저장 | 못 깨뜨림(거부, 링크 유지, 바뀜 유지) | `review_r2_save_as_newer_original_via_symlink_made_after_open_is_refused` |
| 7 | 상위 버전 원본의 하드 링크 다른 이름으로 다른 이름 저장 | 못 깨뜨림 | `review_r2_save_as_newer_original_via_hard_link_is_refused` |
| 8 | 닫기 [저장] → 상위 버전 시트 → 원본 경로 | 못 깨뜨림(창·바뀜 유지, 흐름 풀림) | `review_r2_close_save_newer_onto_original_keeps_window_and_original` |
| 9 | 끝내기 [저장] → 상위 버전 시트 → 원본 경로 | 못 깨뜨림(끝내기 거짓) | `review_r2_quit_save_newer_onto_original_aborts` |
| 10 | 다른 새 창이 상위 버전 원본의 하드 링크로 다른 이름 저장 | 못 깨뜨림(「이미 열고 있다」) | `review_r2_other_window_save_as_onto_newer_original_hard_link_is_refused` |
| 11 | `app::Quit` Action, 바뀐 창 둘 모두 [저장 안 함] | 못 깨뜨림(둘 다 닫힘, 재발송이 시트를 더 띄우지 않음) | `review_r2_quit_action_all_dont_save_closes_all_without_extra_sheet` |
| 12 | 끝내기 시트 중 `app::Quit` 한 번 더 | 못 깨뜨림(둘째는 바쁜 창을 보고 멈춤, 시트 겹침 없음) | `review_r2_quit_action_twice_does_not_stack_sheets` |
| 13 | 바뀐 창에 다른 이름 저장 대화상자가 떠 있을 때 끝내기 | 못 깨뜨림(거짓, 대화상자 답은 정상, 그 뒤 끝내기 참) | `review_r2_quit_with_pending_save_as_dialog_aborts_and_dialog_still_works` |
| 14 | 끝내기 시트(창 a) 중 창 b에 닫기 시트 | 못 깨뜨림(끝내기는 b에서 멈춤, 묻지 않고 끝나는 경로 없음) | `review_r2_quit_while_other_window_close_sheet_never_quits_unasked` |
| 15 | 대화상자가 떠 있는 손대지 않은 창에서 열기 → 대화상자에 답 | 못 깨뜨림(창 남음, 경로 반영, 흐름 풀림) | `review_r2_open_keeps_busy_pristine_window_and_its_dialog_completes` |
| 16 | 두 창이 차례로 같은 새 경로로 다른 이름 저장 | 못 깨뜨림(둘째 거부, 첫째 내용 유지) | `review_r2_two_windows_save_as_same_new_path_second_refused` |
| 17 | 대화상자 중 같은 파일을 다른 창에 연 뒤 그 경로 선택 | 못 깨뜨림(거부) | `review_r2_save_as_onto_file_opened_meanwhile_is_refused` |
| 18 | 내용이 같은 복사본 두 개 열기 | 못 깨뜨림(창 둘, 최근 둘. `(dev, ino)`가 오병합하지 않음) | `review_r2_copies_with_same_content_open_in_separate_windows` |
| 19 | 하드 링크 두 이름을 차례로 열기 → 최근 목록 | 못 깨뜨림(하나) | `review_r2_recent_hard_link_names_kept_once` |
| 20 | 이름이 같은 셋(`x/proj`, `y/proj`, `x/other`) 라벨 | 못 깨뜨림(모두 다름) | `review_r2_recent_labels_distinct_three_way` |
| 21 | 포커스를 뺀(`window.blur`) 바뀐 창에 `window::Close` | 못 깨뜨림(시트로 묻는다) | `review_r2_close_without_focus_still_asks` |
| 22 | 포커스를 뺀 창에 `file::Save` | 못 깨뜨림(저장됨) | `review_r2_save_without_focus_saves` |
| 23 | UI-11: `file::New/Open/Save/SaveAs/OpenRecent`, `window::Close`, `app::Quit`이 메뉴에 | 못 깨뜨림 | `review_r2_file_actions_are_all_in_menus` |

회귀 확인: `review_s2_t7a`(22), `files`(30), `commands`(18) 모두 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 읽기 전용 사유가 있으면 원본 바이트를 바꾸지 않는다(계획 불변식, 결정 2026-10-12 A) | 예(좁아졌지만 남음) | 연 뒤 원본이 옮겨지면 옮긴 원본을 다른 이름 저장으로 덮는다(지적 1). 링크·하드 링크·닫기·끝내기 경유는 막힌다 |
| 쓸 권한이 없으면 원본 바이트를 바꾸지 않는다 | 아니오 | 리뷰 1 시도 8~11 그대로 |
| 저장은 옛 내용 또는 완전한 새 내용 | 아니오 | 변경 없음(kill 게이트) |
| 한 파일은 한 창에서만 | 예(이름 바꿈) | 지적 2. 링크·하드 링크·대소문자·NFD는 막힌다 |
| 한 창에 시트 하나, 흐름이 끝나면 풀림 | 예(겹침) | 열기 실패 시트는 `file_flow`를 세우지 않아 닫기·끝내기 시트가 겹친다(지적 3). 흐름은 모두 풀린다 |
| 바뀐 창은 묻지 않고 닫히거나 끝나지 않는다(7.5) | 아니오 | 시도 11~14, 21. `Quit` 재발송은 바뀐 창이 남았을 때만 다시 묻고, 바쁜 창이 있으면 멈춘다. 사용자 입력 없이 반복하는 경로는 없다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-1 + 결정 2026-10-12 A(원본은 바뀌지 않는다) | 부분 충족 | 상위 버전 읽기 전용(`files.rs:122`), 같은 파일 검사(`files.rs:360`). 연 뒤 옮긴 원본은 막지 못한다(지적 1) |
| 7.5 `Cmd+N/O/S/Shift+S` | 충족 | `lib.rs:55`, `files.rs:60`, `files.rs:221-236` |
| 7.5 `Cmd+W/Q` 「저장 안 된 변경은 macOS 표준 시트로 묻기」 | 충족 | `files.rs:145-169`(매번 다시 찾기), `lib.rs:70-86`(끝내기 직전 재확인). 시트 겹침(지적 3)은 묻지 않는 경로가 아니다 |
| 7.6 메뉴바 파일 > 최근 파일, UI-11 | 충족 | `commands/mod.rs`의 `recent_menu`·팔레트 줄, 시도 23. 시작 직후 중복 줄은 지적 4(제안) |
| 8.4 이전 프로젝트 파일을 덮어쓰지 않음(원자적 쓰기) | 충족 | `ksq::write_file`, `tests/save_kill.rs` |
| 8.4 실시간 규율 | 해당 없음 | 스케줄러 코드 변경 없음 |

「부분 충족」은 미충족으로 센다: IO-1(결정 A).

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 리뷰 1 뒤 변경은 `files.rs`, `lib.rs`(UI 스레드)뿐이다.
- 제품 코드의 `unwrap`/`expect`: 리뷰 1 뒤 더한 것 없음.
- 에러가 삼켜지는 곳
  - `same_file`은 `fs::metadata` 실패를 「다른 파일」로 본다(`files.rs:688`). 없는 경로끼리는 정규형 비교로 이미 가려지므로 타당하다. 다만 이 때문에 「연 경로가 사라진」 경우가 지적 1·2로 이어진다.
  - `confirm_close_all`이 바쁜 창을 만나면 아무 알림 없이 거짓을 돌린다(`files.rs:156`). 보고서 「알려진 한계」에 적혀 있고 데이터는 안전하다(시도 13). 지적으로 올리지 않는다.
  - `lib.rs:77`의 재발송은 `App::dispatch_action`이라 활성 창이 있으면 그 창의 디스패치를 거친다. 프로젝트 창·설정 창 모두 `app::Quit`을 가로채지 않아 전역 처리기에 닿는다(시도 11에서 같은 경로로 시트가 뜨는 것을 확인).

## 7. 지적 사항

리뷰 1 지적 대응 확인: 1(차단) 원본 경로·링크·하드 링크 고침 확인, 단 옮긴 원본은 아래 지적 1. 2(차단) 고침 확인(시도 11~14, 리뷰 1 재현 둘 녹색). 3(수정) 고침 확인(시도 15). 4(수정) 넣기 시점은 고침, 시작 직후는 아래 지적 4. 5(제안) 받아들여 고침(시도 20). 6(제안) 받아들여 고침, 오병합 없음(시도 18·19).

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 상위 버전 파일을 연 뒤 원본이 옮겨지거나 이름이 바뀌면(Finder, 동기화 도구), [다른 이름으로 저장]에서 옮긴 원본을 골라 `format_version: 1`로 덮어쓴다. 읽기 전용 검사가 「문서가 기억하는 경로」와 고른 경로를 `same_file`로 비교하는데, 옛 경로가 없으면 `metadata`가 실패해 다른 파일로 본다. 결정 2026-10-12 A 「원본은 바뀌지 않는다」와 계획 불변식 위반이고, 리뷰 1 지적 1과 같은 사용자 행동(원본 고르기)이다. 열 때 원본의 `(dev, ino)`를 문서에 남겨 그것과 비교하면 막힌다(아래 지적 2와 같은 수단). 재현 `review_r2_save_as_onto_moved_newer_original_keeps_original` | `crates/keyscore-app/src/files.rs:360`, `files.rs:681-693` |
| 2 | 제안 | 연 파일의 이름이 바깥에서 바뀐 뒤 새 이름으로 열면 같은 inode가 두 창에 열린다(보고서 「리뷰어에게」가 예고한 한계). 저장은 각자 경로로 rename하므로 서로 덮지는 않는다. 지적 1의 `(dev, ino)` 기록을 `window_with_path`에도 쓰면 함께 막힌다. 재현 `review_r2_open_renamed_file_activates_existing_window` | `crates/keyscore-app/src/files.rs:662-677` |
| 3 | 수정 | 열기 실패 시트(`open_paths`의 `ask`)는 `file_flow`를 세우지 않는다. 그 시트가 떠 있는 바뀐 창에 `window::Close`나 끝내기를 내면 같은 창에 닫기 시트가 겹친다. 모듈 문서·계획 리스크의 「한 창에 시트 하나」 위반이다. 코드상 `open_paths`가 이미 열린 바쁜 창(닫기 시트 중인 창이 `opened`가 될 때)을 시트 자리로 고를 때도 같다(시험하지 않음). 데이터는 잃지 않는다(macOS는 둘째 시트를 줄 세운다). 시트 자리 창의 `file_flow`를 세우고 풀거나, 바쁜 창이면 다른 창을 고르는 방안. 재현 `review_r2_quit_during_open_error_sheet_does_not_stack`, `review_r2_close_during_open_error_sheet_does_not_stack` | `crates/keyscore-app/src/files.rs:524-553` |
| 4 | 제안 | 설정 파일에 같은 파일이 두 모양(정규 경로와 `sub/../`)으로 있으면, 파일을 열거나 저장하기 전까지 「최근 파일」 메뉴·팔레트에 두 줄이다. 리뷰 1 대응이 「설정 읽기는 문자열 겹침만 빼고 다음 넣기에서 정리」로 정한 범위다. 읽을 때(`read_recent_files`) 또는 메뉴를 만들 때 `same_file`로 한 번 거르는 방안. 재현 `review_r2_recent_noncanonical_duplicate_at_startup_shown_once` | `crates/keyscore-app/src/settings.rs`의 `read_recent_files`, `files.rs:586` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 1건(지적 1)과 수정 1건(지적 3)을 고친 뒤 재리뷰한다. 부록의 해당 재현 테스트를 저장소 테스트로 옮겨 녹색이 되어야 한다. 제안 2·4를 거절하면 그 재현은 빼고 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 5개 실패, 리뷰 테스트 18개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 1 뒤 변경은 `files.rs`, `lib.rs`, 리뷰 1 테스트, 보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 재현 테스트

`crates/keyscore-app/tests/review_s2_t7a_r2.rs` 끝에 그대로 붙여 넣으면 같은 파일의 도우미(`start`, `open`, `open_file`, `open_newer`, `run`, `touch`, `answer`, `answer_new_path`, `prompt_text`, `start_quit`, `write_sample`)와 `use`로 컴파일된다. `cargo test -p keyscore-app --test review_s2_t7a_r2`로 리뷰 시점(`5159f7b`)에 5개 모두 실패하는 것을 확인했다.

~~~rust
/// 상위 버전 원본을 연 뒤 Finder에서 옮기면, 옮긴 원본을 「다른 이름으로 저장」으로 골라 덮을 수 있다.
#[gpui_kit::test]
fn review_r2_save_as_onto_moved_newer_original_keeps_original(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-moved");
    let path = dir.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let moved = dir.join("moved.ksq");
    std::fs::rename(&path, &moved).unwrap();
    run(cx, handle, file::Save);
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(moved.clone()));
    if cx.has_pending_prompt() {
        answer(cx, "확인");
    }
    assert_eq!(
        std::fs::read(&moved).unwrap(),
        before,
        "옮긴 상위 버전 원본이 버전 1로 덮였다"
    );
}

/// 연 파일을 바깥에서 이름을 바꾼 뒤 새 이름으로 열면 같은 파일이 두 창에 열린다.
#[gpui_kit::test]
fn review_r2_open_renamed_file_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "renamed");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (first, _) = open_file(cx, &path);
    let renamed = dir.join("b.ksq");
    std::fs::rename(&path, &renamed).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&renamed, cx)).unwrap();
    assert_eq!(again, first, "같은 파일(inode)이 두 창에 열렸다");
}

/// 열기 실패 시트가 떠 있는 바뀐 창에 끝내기를 내면 같은 창에 시트가 겹친다.
#[gpui_kit::test]
fn review_r2_quit_during_open_error_sheet_does_not_stack(cx: &mut TestAppContext) {
    let dir = start(cx, "quit-open-error");
    let bad = dir.join("broken.ksq");
    std::fs::write(&bad, "{ not json").unwrap();
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, file::Open);
    cx.simulate_path_prompt_response(move |_| Some(vec![bad]));
    cx.run_until_parked();
    assert!(prompt_text(cx).contains("열지 못했다"));
    let quit = start_quit(cx);
    answer(cx, "확인");
    assert!(
        !(cx.has_pending_prompt() && quit.get().is_none()),
        "열기 실패 시트 뒤에 끝내기 시트가 같은 창에 겹쳐 있었다"
    );
}

/// 열기 실패 시트가 떠 있는 바뀐 창에 닫기를 내면 같은 창에 시트가 겹친다.
#[gpui_kit::test]
fn review_r2_close_during_open_error_sheet_does_not_stack(cx: &mut TestAppContext) {
    let dir = start(cx, "close-open-error");
    let bad = dir.join("broken.ksq");
    std::fs::write(&bad, "{ not json").unwrap();
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, file::Open);
    cx.simulate_path_prompt_response(move |_| Some(vec![bad]));
    cx.run_until_parked();
    run(cx, handle, window_actions::Close);
    answer(cx, "확인");
    assert!(
        !cx.has_pending_prompt(),
        "열기 실패 시트 뒤에 닫기 시트가 같은 창에 겹쳐 있었다"
    );
}

/// 설정에 같은 파일이 두 모양으로 있으면 시작하자마자 메뉴에 두 줄이다.
#[gpui_kit::test]
fn review_r2_recent_noncanonical_duplicate_at_startup_shown_once(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!(
        "ks-review-t7a-r2-{}-startup-dup",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let path = dir.join("song.ksq");
    write_sample(&path);
    let roundabout = dir.join("sub/../song.ksq");
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": [
            path.canonicalize().unwrap().display().to_string(),
            roundabout.display().to_string()
        ] })
        .to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let n = cx.update(|cx| {
        keyscore_app::commands::palette_entries(cx)
            .iter()
            .filter(|e| e.name() == "file::OpenRecent")
            .count()
    });
    assert_eq!(n, 1, "같은 파일이 최근 파일에 두 줄");
}
~~~
