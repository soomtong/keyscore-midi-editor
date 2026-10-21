# Task 계획: S2-T7a 새로 만들기·열기·저장·닫기·최근 파일

- 브랜치: `s2/t7a-save-open`
- 관련 REQ: 6.6 IO-1, 7.5(`Cmd+N/O/S/Shift+S`, `Cmd+W/Q`, 「저장 안 된 변경은 macOS 표준 시트로 묻기」), 7.7 메뉴바(파일 메뉴의 최근 파일), 8.4
- 선행 Task: S0-T6(`.ksq` 읽기·쓰기), S2-T1(창 모델), S2-T4(명령 표·메뉴·팔레트), S2-T5a(트랜스포트)
- 관련 결정: `docs/decisions/2026-10-12-ksq-newer-version-save.md` A, `docs/decisions/2026-10-21-s2-t7-save-policy-and-split.md` 1·2
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

S2-T7을 셋으로 나눈 첫 Task다(결정 2). 자동 저장본·복구는 T7b, 가져오기·내보내기는 T7c다.

1. 문서의 파일 상태
   - `ProjectDocument`가 파일 경로(`Option<PathBuf>`)와 읽기 전용 사유(상위 `format_version`)를 가진다.
   - 저장하면 「바뀜」이 꺼진다. 바뀜은 저장 뒤 편집·되돌리기·다시 실행이 하나라도 있었는가다(되돌려 저장 시점으로 돌아와도 켜진 채다. 범위 밖 참고).
   - 창 제목은 파일 이름(새 프로젝트는 「제목 없음」), 읽기 전용이면 「(읽기 전용)」을 붙인다. macOS 창의 「편집됨」 표시(`set_window_edited`)와 문서 경로(`set_document_path`)를 맞춘다.
2. 새로 만들기 `file::New`: 빈 프로젝트의 새 창(지금과 같다).
3. 열기 `file::Open`
   - 파일 선택 대화상자(여럿 고르기 가능)로 `.ksq`·`.ksqz`를 고른다. 파일마다 창 하나.
   - 같은 파일(정규화한 경로)이 이미 열려 있으면 새 창을 만들지 않고 그 창을 앞으로 가져온다.
   - 지금 창이 손대지 않은 새 프로젝트(경로 없음, 바뀜 없음, undo 기록 없음)면 연 파일 창으로 바꾼다(그 창을 닫는다, macOS 문서 앱 관례).
   - 읽기 경고(모르는 키 등)는 새 창 상태줄에 남긴다. 상위 `format_version`이면 읽기 전용으로 열고 상태줄에 알린다.
   - 읽지 못하면 오류 시트를 띄운다. 띄울 창이 없으면 새 프로젝트 창을 하나 열고 그 창에 띄운다.
4. 저장 `file::Save`
   - 경로가 없으면 「다른 이름으로 저장」과 같다.
   - 읽기 전용(상위 버전)이면 쓰지 않고 시트 「새 버전 앱이 만든 파일이라 덮어쓸 수 없다」 [다른 이름으로 저장] [취소].
   - 파일에 쓸 권한이 없으면(쓰기로 열어 보기, 내용은 바꾸지 않는다) 쓰지 않고 시트 「권한이 없다」 [다른 이름으로 저장] [취소]. 원본은 바뀌지 않는다(결정 1 A).
   - 쓰기는 `ksq::write_file`(임시 파일 + fsync + rename)이다. 실패하면 오류 시트, 바뀜은 켜진 채다.
   - 압축은 확장자로 정한다(`.ksqz`면 gzip, 그 밖은 평문).
5. 다른 이름으로 저장 `file::SaveAs`
   - 새 경로 대화상자(지금 파일의 폴더와 이름, 새 프로젝트는 「제목 없음.ksq」). 고르면 쓰고 창의 경로를 바꾸고 읽기 전용을 푼다. 새 파일은 `format_version: 1`이고 모르는 키를 보존한다(결정 A).
   - 다른 창이 이미 그 경로를 열고 있으면 쓰지 않고 오류 시트(같은 파일 두 창 금지).
6. 창 닫기(`window::Close`, 빨간 버튼 모두 `on_window_should_close` 한 곳)
   - 바뀜이 없으면 닫는다. 있으면 표준 시트 「“이름”의 변경 사항을 저장하겠는가?」 [저장] [취소] [저장 안 함].
   - 저장 → 4의 흐름(경로 없으면 새 경로 대화상자). 성공해야 닫는다. 대화상자를 취소하거나 실패하면 창은 남는다.
   - 창이 닫히면 그 창의 재생을 멈춘다. 지금은 뷰 해제(`on_release`)에 기대는데, 닫기 훅에서 먼저 멈춘다(S2-T5a 이월).
7. 끝내기 `app::Quit`
   - 바뀐 창마다 차례로 6의 시트를 띄운다. 하나라도 [취소]면 끝내기를 멈춘다(이미 저장·닫은 창은 그대로). 모두 통과하면 `cx.quit()`.
8. 최근 파일
   - `settings.json`의 `recent_files`(최근이 먼저, 최대 10, 중복 없음). 열기·다른 이름으로 저장에 성공하면 앞에 넣고 macOS 최근 문서(`add_recent_document`)에도 넣는다.
   - 파일 > 최근 파일 하위 메뉴에 파일마다 `file::OpenRecent(경로)` 항목(제목은 파일 이름, 같은 이름이 여럿이면 부모 폴더를 붙인다). 목록이 바뀌면 메뉴를 다시 건다. 목록이 비면 지금처럼 꺼진 빈 하위 메뉴다.
   - 팔레트는 최근 파일마다 「최근 파일: 이름」 줄을 보인다.
   - 열 수 없는 최근 파일(지워짐 등)은 오류 시트를 보이고 목록에서 뺀다.
9. 테스트 게이트: 저장 중 kill
   - 테스트가 자기 실행 파일을 자식 프로세스로 띄워 큰 프로젝트를 같은 경로에 거듭 저장하게 하고, 임시 파일이 생긴 순간 `SIGKILL`로 죽인다. 원본은 저장 전 내용 또는 완전한 새 내용으로 읽혀야 한다(반쯤 쓴 내용 금지). 여러 번 되풀이한다.

## 공개 인터페이스 초안

~~~rust
// keyscore_core::ksq
impl Warning {
    /// 상위 형식 버전 경고면 그 버전.
    pub fn newer_format_version(&self) -> Option<u64>;
}

// keyscore_app::document
pub enum ReadOnly { NewerFormat(u64) }
impl ProjectDocument {
    pub fn path(&self) -> Option<&Path>;
    pub fn read_only(&self) -> Option<ReadOnly>;
    pub fn display_name(&self) -> String;          // 파일 이름 또는 「제목 없음」
    pub(crate) fn mark_saved(&mut self, path: PathBuf);
}

// keyscore_app::files (새 모듈)
pub fn open_path(path: &Path, cx: &mut App) -> Result<Entity<ProjectWindow>, FileError>;
pub fn save_project(path: &Path, project: &Project) -> Result<(), FileError>; // 확장자로 압축, ksq::write_file
pub enum FileError { Read(KsqError), Write(KsqError), PermissionDenied, NewerFormat(u64), AlreadyOpen(PathBuf), Window(String) }

// keyscore_app::settings
pub struct Settings { …, pub recent_files: Vec<String> }
pub const MAX_RECENT_FILES: usize = 10;
~~~

흐름(대화상자·시트)은 `ProjectWindow`의 비동기 작업(`cx.spawn_in`)이고 GPUI 시험 플랫폼의 `simulate_prompt_answer`·`simulate_new_path_selection`·`simulate_path_prompt_response`·`simulate_close`로 시험한다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `warning_reports_newer_format_version` | 단위(core) | 상위 버전 경고만 `Some(v)`, 다른 경고는 `None` |
| 2 | `save_untitled_prompts_for_path_and_writes_file` | UI | 새 프로젝트 `file::Save` → 새 경로 대화상자 → 파일이 생기고 다시 읽으면 같은 프로젝트, 바뀜 꺼짐, 제목이 파일 이름 |
| 3 | `save_existing_path_writes_without_prompt` | UI | 연 파일을 고치고 저장하면 대화상자 없이 쓰고 바뀜이 꺼진다 |
| 4 | `save_as_moves_window_to_new_path` | UI | 다른 이름으로 저장 뒤 경로·제목이 바뀌고 원래 파일은 그대로 |
| 5 | `save_as_cancelled_keeps_state` | UI | 대화상자 취소면 파일 없음, 바뀜 그대로 |
| 6 | `save_ksqz_extension_writes_gzip` | UI | `.ksqz`로 저장하면 gzip, 다시 읽으면 같음 |
| 7 | `open_reads_file_into_window_with_title_and_warnings` | UI | 열기 대화상자 → 새 창, 제목·경로, 모르는 키 경고가 상태줄에 |
| 8 | `open_replaces_pristine_untitled_window` | UI | 손대지 않은 새 창은 닫히고, 고친 새 창은 남는다 |
| 9 | `open_same_path_twice_activates_existing_window` | UI | 창 수가 늘지 않는다 |
| 10 | `open_unreadable_file_shows_error_sheet` | UI | 깨진 JSON → 오류 시트, 창이 늘지 않는다. 창이 없을 때는 새 창에 시트 |
| 11 | `newer_version_opens_read_only_and_save_offers_save_as` | UI | 상위 버전 파일: 제목에 읽기 전용, `Save` → 시트, [다른 이름으로 저장] → 새 파일은 버전 1 + 모르는 키 보존, 원본 바이트 그대로, 이후 읽기 전용 풀림 |
| 12 | `save_read_only_permission_file_refuses_and_keeps_original` | UI | 0444 파일 `Save` → 권한 시트, [취소]면 원본 바이트·권한 그대로, 바뀜 그대로 |
| 13 | `save_write_failure_shows_error_and_stays_dirty` | UI | 폴더를 지운 뒤 저장 → 오류 시트, 바뀜 켜짐 |
| 14 | `save_as_to_path_open_in_other_window_is_refused` | UI | 같은 파일 두 창 금지 |
| 15 | `close_clean_window_closes_without_sheet` | UI | 시트 없음, 창 닫힘 |
| 16 | `close_dirty_window_cancel_keeps_window` | UI | [취소] → 창 남음 |
| 17 | `close_dirty_window_dont_save_closes` | UI | [저장 안 함] → 파일 그대로, 창 닫힘 |
| 18 | `close_dirty_window_save_writes_then_closes` | UI | [저장] → 파일 갱신 후 닫힘. 새 프로젝트면 대화상자, 취소하면 창 남음 |
| 19 | `close_action_uses_same_sheet` | UI | `window::Close` Action도 같은 시트 |
| 20 | `closing_playing_window_stops_transport` | 루프백 | 재생 중 창을 닫으면 닫기 훅에서 멈춘다(All Notes Off를 받는다) |
| 21 | `quit_prompts_each_dirty_window_and_cancel_aborts` | UI | 바뀐 창 둘: 첫째 [저장 안 함], 둘째 [취소] → 끝내지 않음, 둘째 창 남음 |
| 22 | `recent_files_are_recorded_deduped_and_capped` | UI | 열기·다른 이름 저장이 앞에 넣고, 중복 없고, 10개, 설정 파일에 남는다 |
| 23 | `recent_files_menu_and_palette_list_entries` | UI | 메뉴 하위 항목과 팔레트 줄, 같은 이름은 폴더 붙임 |
| 24 | `open_recent_missing_file_shows_error_and_drops_entry` | UI | 지워진 파일은 목록에서 빠진다 |
| 25 | `killed_during_save_keeps_original_intact` | 통합(자식 프로세스) | 9의 게이트, 20회 |

## 불변식

- 모델은 여전히 `ProjectDocument`의 undo 엔진만 거쳐 바뀐다. 열기는 새 문서를 만들 뿐이다.
- 저장은 원본을 「완전한 옛 내용 또는 완전한 새 내용」으로만 남긴다(`ksq::write_file`).
- 읽기 전용 사유가 있거나 쓸 권한이 없으면 원본 바이트를 바꾸는 호출이 없다.
- 한 파일은 한 창에서만 열린다.

## 범위 밖

- 자동 저장본·크래시 복구(T7b), 온보딩 프로젝트의 자동 저장본(REQ 7.8, S8).
- SMF·계이름 텍스트 가져오기·내보내기(T7c).
- 되돌려 저장 시점으로 돌아오면 바뀜을 끄는 일: core 히스토리에 저장 지점 표식이 필요하다. 지금은 켜진 채로 둔다(macOS 일부 앱과 같다).
- Dock 메뉴·로그아웃 같은 `app::Quit` 밖의 종료: GPUI가 취소할 수 없는 종료 훅만 준다(`on_app_quit`). 이 경로의 저장 안 된 변경은 T7b 자동 저장본이 지킨다.
- Finder에서 파일을 끌어 놓거나 더블클릭해 열기(`on_open_urls`): 앱 번들·UTI 등록이 필요해 S8 배포 단계에서 다룬다.
- vim 명령줄 `:w`·`:e`·`:q`(명령줄 UI가 없다).
- 크래시로 남은 임시 파일(`.이름.tmp-*`) 정리: T7b에서 자동 저장본과 함께 본다.

## 리스크·결정 요청

- 결정은 모두 받았다(상위 버전 A, 읽기 전용 거부 A, 나누기). 위 세부(열기 창 바꾸기, 같은 파일 두 창 금지, 최근 파일 10개)는 macOS 관례를 따른 것이고 REQ 의미를 바꾸지 않는다.
- GPUI `window.prompt`는 재진입을 지원하지 않는다(한 창에 시트 하나). 끝내기는 창마다 차례로 묻는다.
