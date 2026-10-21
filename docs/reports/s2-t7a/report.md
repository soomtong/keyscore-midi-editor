# Task 보고: S2-T7a 새로 만들기·열기·저장·닫기·최근 파일

- PR: 브랜치 `s2/t7a-save-open`
- 계획 파일: `docs/reports/s2-t7a/plan.md`
- 계획 대비 달라진 점
  - `FileError::NewerFormat`은 두지 않았다. 상위 버전은 오류가 아니라 문서의 읽기 전용 사유(`ReadOnly::NewerFormat`)이고, 저장 흐름이 시트로 처리한다.
  - `ProjectDocument::is_pristine`, `window::open_project_windows`, `ProjectWindow::window_handle`을 더했다(열기의 창 바꾸기, 시험). 계획의 인터페이스 블록을 고쳤다.
  - 테스트는 계획의 25개 항목을 33개로 나눴다(아래 표).

## 구현 요약

- core `ksq::Warning`: 문구 외에 상위 형식 버전을 값으로 가진다(`newer_format_version`). 앱은 문구가 아니라 이 값으로 읽기 전용을 정한다.
- `src/document.rs`: 문서가 정규화한 파일 경로와 읽기 전용 사유를 가진다. `display_name`(「제목 없음」), `is_pristine`, `set_file`, `mark_saved`(바뀜 끄기, 읽기 전용 풀기).
- `src/files.rs`(새 모듈)
  - `save_project`: 있는 일반 파일은 쓰기로 열어 권한을 확인한 뒤(0444면 `PermissionDenied`, 원본 그대로) `ksq::write_file`(임시 파일 + fsync + rename). `.ksqz`면 gzip.
  - 저장 흐름: 읽기 전용(상위 버전) → 시트 [다른 이름으로 저장]/[취소]. 경로 없음 → 다른 이름으로 저장. 권한 없음 → 시트 [다른 이름으로 저장]/[취소]. 그 밖의 실패 → 오류 시트, 바뀜 유지.
  - 다른 이름으로 저장: 새 경로 대화상자(지금 파일의 폴더·이름, 새 프로젝트는 `~/Documents`와 「제목 없음.ksq」). 다른 창이 연 파일이면 거부. 성공하면 경로를 바꾸고 최근 파일에 넣는다.
  - 열기: 여러 파일 대화상자, 파일마다 창. 이미 연 파일(정규화 경로)은 그 창을 앞으로. 손대지 않은 새 창에서 열면 그 창을 닫는다. 읽기 경고는 상태줄, 실패는 시트 하나로 모은다(띄울 창이 없으면 새 창).
  - 닫기: `request_close`를 창 닫기 단추(`on_window_should_close`)와 `window::Close`가 함께 쓴다. 바뀐 창은 시트 [저장]/[취소]/[저장 안 함], 닫을 때 재생을 멈춘다.
  - 끝내기: `confirm_close_all`이 바뀐 창마다 차례로 묻고 답한 창은 바로 닫는다. 하나라도 취소면 끝내지 않는다.
  - 한 창에 흐름은 하나(`ProjectWindow::file_flow`). 시트가 떠 있는 동안 저장·닫기를 다시 눌러도 시트를 겹치지 않는다.
  - 최근 파일: 설정 `recent_files`(최대 10, 중복 없음, 정규화 경로)와 macOS 최근 문서. 메뉴 하위 항목·팔레트 줄은 `recent_labels`(같은 이름이면 「이름 — 부모 폴더」). 「최근 파일」에서 열지 못한 파일은 목록에서 뺀다.
- `src/window.rs`: 창 제목(파일 이름, 「(읽기 전용)」), 편집됨 표시, 문서 경로를 문서가 바뀔 때마다 맞춘다. 저장·다른 이름·닫기 처리기, 닫기 훅.
- `src/commands/mod.rs`: 「최근 파일」 하위 메뉴와 팔레트 줄을 지금 목록으로 만든다. 목록이 바뀌면 메뉴를 다시 건다.
- `src/settings.rs`: `recent_files` 읽기·쓰기(문자열만, 중복·초과 버림).
- `src/lib.rs`: `app::Quit`은 `confirm_close_all`이 참일 때만 끝낸다. 전역 `window::Close`는 프로젝트 창 밖(설정 창)만 맡는다.

## 테스트 결과

~~~
cargo test --workspace: 테스트 바이너리 192개 모두 ok, 통과 1600(이전 1568 + 새 32), 실패 0, 무시 0
새 테스트: tests/files.rs 30, tests/files_playback.rs 2(루프백), tests/save_kill.rs 1(20회 kill), core tests/ksq.rs 1
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `warning_newer_format_version_reports_only_newer_versions`(core `tests/ksq.rs`) | 통과 |
| 2 | `save_untitled_prompts_for_path_and_writes_file` | 통과 |
| 3 | `save_existing_path_writes_without_prompt` | 통과 |
| 4 | `save_as_moves_window_to_new_path` | 통과 |
| 5 | `save_as_cancelled_keeps_state` | 통과 |
| 6 | `save_ksqz_extension_writes_gzip` | 통과 |
| 7 | `open_reads_file_into_window_with_title_and_warnings`, `open_several_files_opens_window_each` | 통과 |
| 8 | `open_replaces_pristine_untitled_window` | 통과 |
| 9 | `open_same_path_twice_activates_existing_window`(`..` 경로 포함) | 통과 |
| 10 | `open_unreadable_file_shows_error_sheet`, `open_recent_missing_file_without_window_shows_error_in_new_window` | 통과 |
| 11 | `newer_version_opens_read_only_and_save_offers_save_as`, `newer_version_save_cancel_keeps_original` | 통과 |
| 12 | `save_read_only_permission_file_refuses_and_keeps_original`, `save_project_refuses_read_only_file` | 통과 |
| 13 | `save_write_failure_shows_error_and_stays_dirty` | 통과 |
| 14 | `save_as_to_path_open_in_other_window_is_refused` | 통과 |
| 15 | `close_clean_window_closes_without_sheet` | 통과 |
| 16 | `close_dirty_window_cancel_keeps_window` | 통과 |
| 17 | `close_dirty_window_dont_save_closes` | 통과 |
| 18 | `close_dirty_window_save_writes_then_closes`, `close_dirty_untitled_save_cancelled_keeps_window` | 통과 |
| 19 | `close_action_uses_same_sheet`, `second_close_while_sheet_open_does_not_stack_sheets` | 통과 |
| 20 | `closing_playing_window_stops_transport_in_close_hook`, `closing_dirty_playing_window_stops_after_dont_save`(`tests/files_playback.rs`, 루프백) | 통과 |
| 21 | `quit_prompts_each_dirty_window_and_cancel_aborts`, `quit_after_all_answered_resolves_true`, `quit_action_with_dirty_window_asks_first` | 통과 |
| 22 | `recent_files_are_recorded_deduped_and_capped` | 통과 |
| 23 | `recent_files_menu_and_palette_list_entries` | 통과 |
| 24 | `open_recent_missing_file_without_window_shows_error_in_new_window`, `open_recent_action_opens_file` | 통과 |
| 25 | `killed_during_save_keeps_original_intact`(`tests/save_kill.rs`, 20회) | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(1600 통과)
- [x] deny(advisories·bans·licenses·sources ok)
- [x] 추가 게이트: `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh` 통과. 단계 게이트 「저장 중 kill 후 원본 보존」은 `killed_during_save_keeps_original_intact`
- 첫 실행에서 `commands.rs`의 `action_counts_match_palette_and_menus`가 실패했다: 최근 목록이 비면 팔레트에서 `file::OpenRecent` 줄이 사라졌다(UI-11). 목록이 비면 실행할 수 없는 줄 하나를 남기게 고쳤다. 기존 테스트는 고치지 않았다.

## 알려진 한계

- 되돌려 저장 시점으로 돌아와도 「바뀜」은 켜진 채다(계획의 범위 밖). core 히스토리에 저장 지점 표식이 필요하다.
- `app::Quit` 밖의 종료(Dock 메뉴, 로그아웃)는 GPUI가 취소할 수 없는 훅만 준다. 이 경로의 저장 안 된 변경은 T7b 자동 저장본이 지킨다.
- 저장은 메인 스레드에서 동기로 쓴다(노트 10만 개 약 3 MB, fsync 포함). 자동 저장(T7b)에서 백그라운드 쓰기를 함께 본다.
- 크래시로 남은 임시 파일(`.이름.tmp-<pid>-<n>`)은 지우지 않는다(kill 게이트도 남긴다). T7b에서 본다.
- 열기 대화상자는 파일 종류를 거르지 않는다(GPUI `PathPromptOptions`에 형식 필터가 없다). 다른 파일을 고르면 오류 시트다.
- Finder 더블클릭·끌어 놓기 열기, vim 명령줄 `:w`·`:e`·`:q`는 범위 밖(계획).
- 다른 흐름의 시트가 떠 있는 창이 있으면 끝내기는 묻지 않고 멈춘다(그 시트의 답을 모른다).

## 리뷰어에게

- `files.rs`의 `file_flow` 표시: 한 창에서 흐름이 겹치지 않는가, 흐름이 끝나면(창이 사라진 경우 포함) 반드시 풀리는가.
- 권한 확인(`check_writable`)은 쓰기로 열어 보기다. 링크·폴더 권한·ACL 조합에서 원본이 바뀌는 경로가 남았는지.
- 같은 파일 두 창 금지는 정규화 경로 비교다. 열기 뒤 파일이 옮겨지거나 이름이 바뀐 경우는 보지 않는다.
- 닫기 훅은 바뀌지 않은 창에서 재생을 멈추고 참을 돌린다. 바뀐 창은 거짓을 돌리고 흐름이 `remove_window`로 닫는다.
