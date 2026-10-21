# 적대적 리뷰: S2-T7a 새로 만들기·열기·저장·닫기·최근 파일 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `5771943`(브랜치 `s2/t7a-save-open`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7a.rs`(깨뜨리지 못한 시도 15개, 모두 통과). 깨뜨린 시도 7개는 부록 A

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 상위 `format_version` 파일에서 「다른 이름으로 저장」으로 원본과 같은 경로를 고르면 원본이 덮이는가(`save_as`의 같은 파일 검사는 자기 창을 뺀다).
2. 끝내기(`confirm_close_all`)가 시작할 때 찍은 창 목록과 「바뀜」만 보므로, 시트가 떠 있는 동안 다른 창을 고치거나 새 창을 열면 묻지 않고 끝나는가.
3. 같은 파일 두 창 금지가 정규화 경로 비교뿐이라 대소문자·NFC/NFD·심볼릭 링크·하드 링크 별칭으로 우회되는가.
4. 권한 확인(`check_writable`)이 링크 대상 0444, 폴더 0555, `uchg` 잠금, 폴더 경로에서 원본을 지키는가.
5. 손대지 않은 새 창을 열기가 닫을 때 그 창의 `file_flow`(진행 중 대화상자)를 보는가.
6. 시트 겹침: 대화상자·시트가 떠 있을 때 닫기 단추·끝내기를 눌러도 겹치지 않고, 흐름이 끝나면 `file_flow`가 풀리는가.
7. 최근 파일: 손상된 설정(문자열 아닌 항목, 중복, 초과, 정규화하지 않은 경로)과 같은 이름 라벨 구별.
8. 저장 중 kill 게이트가 실제로 「쓰는 도중」 죽였는지 검사하는가.
9. 닫기 시트 [저장] → 권한·상위 버전 시트 [취소]에서 창이 남고 바뀜이 유지되는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 상위 버전 원본 경로로 다른 이름 저장 | 없음 | `newer_version_opens_read_only_and_save_offers_save_as`는 다른 경로(`copy.ksq`)만 고른다 |
| 2 끝내기 도중 편집·새 창 | 없음 | `quit_*` 셋은 시작 전에 상태를 다 만든다 |
| 3 별칭 경로 | `..` 경로만 | 링크·대소문자·NFD·하드 링크 없음 |
| 4 권한 조합 | 0444 직접, 폴더 삭제 | 링크·폴더 0555·`uchg` 없음 |
| 5 열기의 창 바꾸기와 흐름 | 없음 | `open_replaces_pristine_untitled_window`는 흐름 없는 창만 |
| 6 시트 겹침 | 닫기 두 번만 | 대화상자 중 닫기, 닫기 시트 중 끝내기 없음 |
| 7 최근 파일 손상 | 없음(설정 단위 시험 여부 미확인) | 앱 수준 시험 없음 |
| 8 kill 게이트 | 있음 | `killed_mid_write > 0`을 단언한다. 게이트를 실제로 본다 |
| 9 닫기 → 권한 시트 | 없음 | |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 상위 버전 파일 → 저장 → [다른 이름으로 저장] → 원본 경로 선택 | 깨짐: 원본이 `format_version: 1`로 덮인다 | 부록 `review_save_as_onto_newer_version_original_keeps_original` |
| 2 | 바뀐 창 B의 끝내기 시트가 떠 있는 동안 이미 지나간 깨끗한 창 A를 고친 뒤 B에 [저장 안 함] | 깨짐: 끝내기가 참, A는 묻지 않음 | 부록 `review_quit_window_edited_during_other_sheet_is_asked` |
| 3 | 끝내기 시트가 떠 있는 동안 새 창을 열어 고침 | 깨짐: 끝내기가 참, 새 창은 묻지 않음 | 부록 `review_quit_window_opened_during_sheet_is_asked` |
| 4 | 같은 파일을 하드 링크 두 이름으로 열기 | 깨짐: 두 창 | 부록 `review_open_hard_link_activates_existing_window` |
| 5 | 설정에 `sub/../song.ksq`가 있는 상태에서 `song.ksq` 열기 | 깨짐: 최근 목록에 같은 파일 둘 | 부록 `review_recent_noncanonical_settings_entry_is_deduped` |
| 6 | `x/proj/song.ksq`, `y/proj/song.ksq`를 최근 목록에 | 깨짐: 메뉴·팔레트 라벨 둘이 같음 | 부록 `review_recent_labels_distinct_for_same_name_and_parent_name` |
| 7 | 손대지 않은 새 창에서 다른 이름 저장 대화상자를 연 채 열기 → 그 창이 닫힌 뒤 대화상자에 답 | 깨짐: 닫힌 창의 문서가 파일로 써진다 | 부록 `review_open_replacing_window_with_pending_save_dialog` |
| 8 | `save_project`를 0444 대상의 심볼릭 링크로 | 못 깨뜨림(권한 거부, 링크 유지) | `review_save_project_through_symlink_to_read_only_target_refuses` |
| 9 | 폴더 0555, 파일 0644 | 못 깨뜨림(실패, 원본·임시 파일 없음) | `review_save_project_in_read_only_folder_keeps_original` |
| 10 | `chflags uchg` 잠금 | 못 깨뜨림(권한 거부) | `review_save_project_immutable_flag_refuses_and_keeps_original` |
| 11 | 폴더 경로에 저장 | 못 깨뜨림 | `review_save_project_onto_directory_fails` |
| 12 | 심볼릭 링크로 같은 파일 열기 | 못 깨뜨림 | `review_open_through_symlink_activates_existing_window` |
| 13 | 다른 창 파일을 가리키는 링크로 다른 이름 저장 | 못 깨뜨림(거부) | `review_save_as_onto_symlink_to_other_windows_file_is_refused` |
| 14 | 대소문자만 다른 경로로 열기·다른 이름 저장 | 못 깨뜨림(`canonicalize`가 디스크의 대소문자로 돌린다) | `review_open_case_variant_activates_existing_window`, `review_save_as_case_variant_of_other_window_is_refused` |
| 15 | 한글 이름 NFD로 열기 | 못 깨뜨림 | `review_open_nfd_name_activates_existing_window` |
| 16 | 다른 이름 저장 대화상자 중 닫기 단추 | 못 깨뜨림(무시, 대화상자 취소 뒤 정상 시트) | `review_close_during_save_as_dialog_is_ignored_then_flow_clears` |
| 17 | 닫기 [저장] → 0444 권한 시트 [취소] | 못 깨뜨림(창·바뀜 유지, 다시 닫기 가능) | `review_close_save_on_read_only_file_cancel_keeps_window` |
| 18 | 닫기 시트 중 끝내기, 그 뒤 다시 끝내기 | 못 깨뜨림(첫째 거짓, 둘째 정상) | `review_quit_during_close_sheet_aborts_then_works` |
| 19 | 끝내기 [저장] → 새 프로젝트 경로 대화상자 취소 | 못 깨뜨림(끝내지 않음, 흐름 풀림) | `review_quit_save_untitled_cancel_dialog_aborts` |
| 20 | 손상된 `recent_files`(숫자, 중복, 16개) | 못 깨뜨림(10개, 문자열 중복 없음) | `review_recent_files_from_corrupt_settings_are_sanitised` |
| 21 | 깨진 최근 파일을 「최근 파일」로 열기 | 못 깨뜨림(시트, 목록에서 빠짐, 창 수 그대로) | `review_open_recent_corrupt_file_drops_entry` |
| 22 | kill 게이트 재실행 | 못 깨뜨림(통과 2.8 s). 코드상 `killed_mid_write > 0` 단언으로 「쓰는 도중」을 실제로 검사한다 | 기존 `killed_during_save_keeps_original_intact` |

참고: GPUI 시험 플랫폼의 `simulate_close`는 「닫아도 되는가」만 돌려주고 창을 없애지 않는다. 실제로 닫히는지는 `window::Close` Action이나 흐름의 `remove_window`로 본다(시도 21에서 확인).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 읽기 전용 사유가 있으면 원본 바이트를 바꾸지 않는다(계획 「불변식」, 결정 2026-10-12 A) | 예 | 저장 → [다른 이름으로 저장] → 원본 경로(지적 1) |
| 쓸 권한이 없으면 원본 바이트를 바꾸지 않는다(결정 2026-10-21 1 A) | 아니오 | 0444·링크·`uchg`·폴더 0555 모두 거부 |
| 저장은 옛 내용 또는 완전한 새 내용 | 아니오 | kill 게이트 |
| 한 파일은 한 창에서만 | 예(하드 링크) | 지적 4 |
| 한 창에 흐름 하나, 흐름이 끝나면 풀림 | 아니오(풀림). 다만 흐름 중인 창을 다른 경로가 닫는다 | 지적 6 |
| 모델은 undo 엔진만 거쳐 바뀐다 | 아니오 | 열기는 새 문서, `ProjectDocument`에 `&mut Project` 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-1(`.ksq`/`.ksqz`, `format_version`, 모르는 키 보존) + 결정 2026-10-12 A | 부분 충족 | 확장자로 압축(`files.rs:70`), 상위 버전 읽기 전용(`files.rs:120`). 그러나 다른 이름 저장으로 원본이 덮인다(`files.rs:346`) |
| 7.5 `Cmd+N/O/S/Shift+S` | 충족 | `commands/table.rs:578-583`, `window.rs`의 처리기 |
| 7.5 `Cmd+W/Q` 「저장 안 된 변경은 macOS 표준 시트로 묻기」 | 부분 충족 | 닫기는 충족. 끝내기는 시트 도중 바뀐·새 창을 묻지 않는다(`files.rs:140`) |
| 7.6 메뉴바 파일 > 최근 파일, KM-13, UI-11 | 충족 | `commands/mod.rs`의 `recent_menu`, 팔레트 줄(빈 목록이면 실행 없는 줄). 라벨 구별은 지적 5(제안) |
| 8.4 크래시 시 이전 프로젝트 파일을 덮어쓰지 않음(이 Task 몫: 원자적 쓰기) | 충족 | `ksq::write_file`, `tests/save_kill.rs` |
| 8.4 실시간 규율 | 해당 없음 | 스케줄러 코드 변경 없음 |

「부분 충족」은 미충족으로 센다: IO-1(결정 A), 7.5 끝내기.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 변경은 UI 스레드(`files.rs`, `window.rs`, `commands/mod.rs`, `settings.rs`)와 `transport.rs`의 `stop_playback` 감싸기뿐이다.
- 제품 코드의 `unwrap`/`expect`: 이 브랜치가 더한 것은 없다(`git diff main...s2/t7a-save-open -- crates/*/src`).
- 에러가 삼켜지는 곳
  - `check_writable`(`files.rs:88`)은 `PermissionDenied` 밖의 열기 오류를 `Ok`로 넘긴다. 그 뒤 `write_file`이 실제 오류를 내므로 원본은 안전하다(시도 9·11).
  - `handle.update(...)`의 `let _`(창 앞으로, `remove_window`): 창이 이미 없을 때뿐이라 타당.
  - `open_paths`가 시트를 띄울 창도 못 만들면 `eprintln!`만 한다(`files.rs` 끝 부분). 창 열기 실패 상황이라 타당.
  - `save` 권한 시트 문구가 「읽기 전용 파일이다」로 고정이라 폴더 0555(임시 파일 생성 실패, 시도 9)에도 같은 문구다. 원본은 안전하다. 지적으로 올리지 않는다(문구).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 상위 `format_version` 파일(읽기 전용)에서 [다른 이름으로 저장]으로 원본 경로를 고르면 원본을 버전 1로 덮어쓴다. `save_as`의 같은 파일 검사가 자기 창을 빼고(`Some(view)`), 쓰기 전에 대상이 읽기 전용 사유의 원본인지 보지 않는다. 결정 2026-10-12 A 「원본은 바뀌지 않는다」와 계획 불변식 위반, 새 앱 데이터 손실 가능. 재현 `review_save_as_onto_newer_version_original_keeps_original` | `crates/keyscore-app/src/files.rs:346` |
| 2 | 차단 | 끝내기가 시작할 때의 창 목록만 돌고, 각 창의 바뀜을 지나갈 때 한 번만 본다. 시트(창 단위)가 떠 있는 동안 이미 지나간 창을 고치거나 새 창을 열어 고치면 묻지 않고 `true`가 되어 `cx.quit()`이 그 변경을 버린다. 7.5 「저장 안 된 변경은 시트로 묻기」 미충족, 데이터 손실. 재현 `review_quit_window_edited_during_other_sheet_is_asked`, `review_quit_window_opened_during_sheet_is_asked` | `crates/keyscore-app/src/files.rs:140`, `lib.rs`의 `Quit` 처리기 |
| 3 | 수정 | 열기가 손대지 않은 새 창을 닫을 때(`close_now_sync(origin)`) 그 창의 `file_flow`를 보지 않는다. 다른 이름 저장 대화상자가 떠 있던 창이 사라진 뒤 대화상자에 답하면 보이지 않는 문서를 파일로 쓰고 최근 목록에 넣는다. 「한 창에 흐름 하나」 설계의 빈틈. 재현 `review_open_replacing_window_with_pending_save_dialog` | `crates/keyscore-app/src/files.rs:496-498` |
| 4 | 수정 | 손으로 고친 설정에 정규화하지 않은 경로가 있으면 같은 파일이 최근 목록에 둘 남는다. `read_recent_files`·`add_recent`가 문자열로만 겹침을 본다. 계획 8 「중복 없음」 미충족. 재현 `review_recent_noncanonical_settings_entry_is_deduped` | `crates/keyscore-app/src/files.rs:541`, `settings.rs`의 `read_recent_files` |
| 5 | 제안 | 같은 이름의 파일이 부모 폴더 이름까지 같으면(`x/proj/song.ksq`, `y/proj/song.ksq`) 메뉴·팔레트 라벨이 같아 구별할 수 없다. 구별될 때까지 위 폴더를 더 붙이거나 전체 경로를 붙이는 방안. 재현 `review_recent_labels_distinct_for_same_name_and_parent_name` | `crates/keyscore-app/src/files.rs:575` |
| 6 | 제안 | 같은 파일 두 창 금지가 경로 비교라 하드 링크 별칭으로 두 창이 같은 inode를 연다(보고서 「리뷰어에게」가 예고한 범위). `(dev, ino)` 비교를 함께 쓰는 방안. 재현 `review_open_hard_link_activates_existing_window` | `crates/keyscore-app/src/files.rs:619` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 2건(지적 1·2), 수정 2건(지적 3·4)을 고친 뒤 재리뷰한다. 부록 테스트를 그대로 저장소 테스트로 옮겨 녹색이 되어야 한다(제안 5·6을 거절하면 그 둘은 빼고 거절 이유를 보고서에 적는다).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/12-S2-app-shell.md`는 결정 2의 나누기 반영뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음, `Cargo.toml`·`Cargo.lock` 변경 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 재현 테스트

`crates/keyscore-app/tests/review_s2_t7a.rs` 끝에 그대로 붙여 넣으면 같은 파일의 도우미(`start`, `open`, `activate`, `run`, `touch`, `dirty`, `is_open`, `answer`, `answer_new_path`, `start_quit`, `write_sample`)로 컴파일된다. `cargo test -p keyscore-app --test review_s2_t7a`로 리뷰 시점(`5771943`)에 7개 모두 실패하는 것을 확인했다.

~~~rust
/// 상위 버전 파일을 「다른 이름으로 저장」에서 원본과 같은 경로로 고르면 원본이 버전 1로 덮인다.
#[gpui_kit::test]
fn review_save_as_onto_newer_version_original_keeps_original(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-same-path");
    let path = dir.join("future.ksq");
    std::fs::write(
        &path,
        r#"{"format_version": 7, "future": {"x": 1}, "tracks": []}"#,
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    activate(cx, handle);
    touch(cx, handle);
    run(cx, handle, file::Save);
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(path.clone()));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "읽기 전용(상위 버전) 원본이 버전 1로 덮였다"
    );
}

/// 끝내기 시트가 다른 창에 떠 있는 동안 이미 지나간(깨끗했던) 창을 고치면, 묻지 않고 끝내기가 참이 된다.
#[gpui_kit::test]
fn review_quit_window_edited_during_other_sheet_is_asked(cx: &mut TestAppContext) {
    start(cx, "quit-race-edit");
    let (clean, clean_view) = open(cx, sample());
    let (dirty_handle, _) = open(cx, sample());
    touch(cx, dirty_handle);
    let quit = start_quit(cx);
    assert!(cx.has_pending_prompt());
    // 시트는 창 단위다. 다른 창은 편집할 수 있다.
    touch(cx, clean);
    assert!(dirty(cx, &clean_view));
    answer(cx, "저장 안 함");
    assert!(
        !(quit.get() == Some(true) && dirty(cx, &clean_view)),
        "바뀐 창을 묻지 않고 끝내기가 참이 됐다(변경 사항을 잃는다)"
    );
}

/// 끝내기 시트가 떠 있는 동안 새 창을 열어 고치면, 묻지 않고 끝내기가 참이 된다.
#[gpui_kit::test]
fn review_quit_window_opened_during_sheet_is_asked(cx: &mut TestAppContext) {
    start(cx, "quit-race-new");
    let (first, _) = open(cx, sample());
    touch(cx, first);
    let quit = start_quit(cx);
    let (late, late_view) = open(cx, sample());
    touch(cx, late);
    activate(cx, first);
    answer(cx, "저장 안 함");
    assert!(
        !(quit.get() == Some(true) && dirty(cx, &late_view)),
        "끝내기 도중 연 창의 변경 사항을 묻지 않았다"
    );
}

/// 하드 링크 두 이름으로 열면 두 창이 같은 파일을 가진다.
#[gpui_kit::test]
fn review_open_hard_link_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "hardlink");
    let path = dir.join("tune.ksq");
    write_sample(&path);
    let other = dir.join("other.ksq");
    std::fs::hard_link(&path, &other).unwrap();
    let (first, _) = cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&other, cx)).unwrap();
    assert_eq!(again, first, "같은 파일(inode)이 두 창에 열렸다");
}

/// 설정에 정규화하지 않은 경로가 있으면 같은 파일이 최근 목록에 두 번 남는다.
#[gpui_kit::test]
fn review_recent_noncanonical_settings_entry_is_deduped(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-{}-noncanon", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let path = dir.join("song.ksq");
    write_sample(&path);
    let roundabout = dir.join("sub/../song.ksq");
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": [roundabout.display().to_string()] }).to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let recent = cx.update(|cx| files::recent_files(cx));
    assert_eq!(recent.len(), 1, "같은 파일이 두 번: {recent:?}");
}

/// 이름과 부모 폴더 이름이 모두 같은 두 최근 파일은 메뉴·팔레트에서 구별되지 않는다.
#[gpui_kit::test]
fn review_recent_labels_distinct_for_same_name_and_parent_name(cx: &mut TestAppContext) {
    let dir = start(cx, "labels");
    let a = dir.join("x/proj/song.ksq");
    let b = dir.join("y/proj/song.ksq");
    for p in [&a, &b] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        write_sample(p);
        cx.update(|cx| files::open_path(p, cx)).unwrap();
    }
    let labels: Vec<String> = cx.update(|cx| {
        keyscore_app::commands::palette_entries(cx)
            .iter()
            .filter(|e| e.label().starts_with("최근 파일:"))
            .map(|e| e.label().to_string())
            .collect()
    });
    assert_eq!(labels.len(), 2);
    assert_ne!(labels[0], labels[1], "{labels:?}");
}

/// 손대지 않은 새 창에서 「다른 이름으로 저장」 대화상자를 연 채 다른 파일을 열면 그 창이 닫힌다. 그 뒤

/// 대화상자에 답하면 닫힌 창의 문서가 파일로 써진다.
#[gpui_kit::test]
fn review_open_replacing_window_with_pending_save_dialog(cx: &mut TestAppContext) {
    let dir = start(cx, "replace-busy");
    let other = dir.join("other.ksq");
    write_sample(&other);
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::SaveAs);
    assert!(cx.did_prompt_for_new_path());
    run(cx, handle, file::Open);
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(move |_| Some(vec![other]));
    cx.run_until_parked();
    let closed = !is_open(cx, handle);
    let ghost = dir.join("ghost.ksq");
    cx.simulate_new_path_selection({
        let g = ghost.clone();
        move |_| Some(g)
    });
    cx.run_until_parked();
    assert!(
        !(closed && ghost.exists()),
        "닫힌 창의 문서가 파일로 써졌다"
    );
}
~~~
