# Task 계획: S2-T7b 자동 저장본과 크래시 후 복구

- 브랜치: `s2/t7b-autosave`
- 관련 REQ: IO-5(자동 저장, 변경 후 30 s, 충돌 시 복구), 2절 표 「저장」(자동 저장본은 복구 전용, `~/Library/Application Support/KeyScore/` 아래, 프로젝트 폴더를 더럽히지 않음), 8.4(크래시 시 autosave 복구본에서 복구, 이전 프로젝트 파일은 덮어쓰지 않음)
- 선행 Task: S2-T7a(파일 흐름, `ProjectDocument`의 경로·바뀜)
- 관련 결정: `docs/decisions/2026-10-21-s2-t7-save-policy-and-split.md` 2
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

1. 자동 저장본
   - 위치: 설정 파일 옆 `autosave/`(앱에서는 `~/Library/Application Support/KeyScore/autosave/`). 시험은 설정 경로를 바꿔 격리한다.
   - 창마다 파일 둘: `<키>.ksq`(문서 내용)와 `<키>.json`(메타: 원래 경로, 읽기 전용 사유, 세션, pid). 키는 창마다 만든 고유 값(pid·시각·순번)이다. 원래 경로는 메타에만 둔다. 처음 계획은 경로 해시 키였으나 리뷰 1 지적 2·3·4로 바꿨다(단계 파일의 `<경로 해시>.ksq`와 다르다. REQ는 위치만 정한다). 쓰기는 세대별 임시 파일 + 차례 잠금 안의 rename이다(리뷰 2).
   - 시점: 깨끗한 문서가 처음 바뀌면 30 s 타이머를 건다. 30 s가 되면 그때 내용을 쓴다. 쓰는 동안이나 뒤에 또 바뀌면 다음 타이머를 건다. 그래서 계속 고쳐도 잃는 작업은 최대 약 30 s다(REQ 「변경 후 30 s」를 「바뀐 뒤 30 s 안에 저장본이 생긴다」로 읽는다).
   - 지우기: 명시 저장으로 깨끗해지면, 창을 닫으면(저장·저장 안 함 모두), 복구를 버리면 지운다.
   - 쓰기 실패는 상태줄에 한 번 알리고 다음 변경 때 다시 시도한다. 원래 파일과 프로젝트 폴더에는 아무것도 쓰지 않는다.
   - `app::Quit`을 거치지 않는 종료(Dock 메뉴·로그아웃, T7a 한계)에서는 `on_app_quit`에서 바뀐 창의 저장본을 곧바로 쓴다. 다음 실행에서 복구를 제안한다.
2. 세션과 크래시 판정
   - 실행 중인 앱은 `autosave/session-<pid>.lock`을 배타 잠금(`File::try_lock`)으로 쥔다. 메타는 세션 pid를 적는다.
   - 시작할 때, 메타의 세션 잠금을 잡을 수 있으면(그 프로세스가 끝났다) 그 저장본은 복구 대상이다. 잡을 수 없으면 살아 있는 다른 KeyScore의 것이라 건드리지 않는다.
3. 복구 제안
   - 앱 시작(`run`) 때 복구 대상이 있으면 첫 창에 시트: 「KeyScore가 예기치 않게 끝났다. 저장하지 않은 프로젝트 N개를 복구하겠는가?」 [복구] [버리기].
   - [복구]: 저장본마다 창을 연다. 원래 경로와 읽기 전용 사유를 그대로 갖고, 「바뀜」 상태로 연다(제목에 「(복구됨)」). 원래 파일은 읽지도 쓰지도 않는다(8.4). 사람이 저장하면 그때 원래 파일에 쓴다. 복구한 창은 저장본을 새 세션 것으로 가져간다. 손대지 않은 첫 창은 닫는다.
   - [버리기]: 저장본과 메타를 지운다.
   - 원래 경로를 이미 연 창이 있으면(다른 경로로 같은 파일) 그 저장본은 새 프로젝트처럼 경로 없이 연다(같은 파일 두 창 금지).
   - 메타가 없거나 깨진 저장본, 내용이 깨진 저장본은 복구하지 않고 남겨 둔다(사람이 손으로 볼 수 있게). 시트 본문에 그 수를 적는다.
4. 크래시로 남은 임시 파일
   - 복구 대상 세션의 원래 경로 옆에 그 pid의 임시 파일(`.<이름>.tmp-<pid>-<n>`)이 남아 있으면 지운다. 끝난 KeyScore 세션의 pid만 지우므로 다른 프로그램(`ksq-cli`)의 쓰기와 겹치지 않는다.
5. S2-T7a 이월(리뷰 3 제안)
   - 문서가 기억한 파일 정체(`FileId`)는 문서 경로의 파일이 없어졌을 때만 쓴다. 경로에 다른 파일이 있으면(바깥에서 다시 썼거나 `.bak`으로 옮기고 새로 썼다) 기억한 정체는 낡았다. 백업을 열면 따로 열리고, FAT의 inode 재사용으로 다른 파일이 옛 창에 묶이지 않는다.
   - `remove_recent`는 표시와 같은 같은 파일 판정을 쓴다(하드 링크 쌍을 함께 뺀다).
   - 메뉴를 만들 때마다 파일 시스템을 부르지 않는다: 최근 목록의 표시 형태(합친 목록, 라벨)를 목록이 바뀔 때 한 번 계산해 둔다.

## 공개 인터페이스 초안

~~~rust
// keyscore_app::autosave (새 모듈)
pub const AUTOSAVE_DELAY: Duration = Duration::from_secs(30);
pub fn autosave_dir(cx: &App) -> PathBuf;                      // 설정 파일 옆 autosave/
pub struct Recoverable { pub path: Option<PathBuf>, pub read_only: Option<ReadOnly>, … }
pub fn find_recoverable(cx: &App) -> Vec<Recoverable>;         // 끝난 세션의 저장본
pub fn offer_recovery(cx: &mut App);                            // run()이 부른다(시험도). 흐름은 스스로 detach
pub fn flush_all(cx: &mut App);                                 // on_app_quit: 바뀐 창의 저장본을 지금 쓴다

// keyscore_app::document
impl ProjectDocument { pub fn revision(&self) -> u64; pub fn is_recovered(&self) -> bool; pub(crate) fn mark_recovered(&mut self, path, read_only); }
// keyscore_app::app_state
impl AppSettings { pub fn path(cx: &App) -> &Path; }
// keyscore_app::files
#[doc(hidden)] pub fn recent_label_computations(cx: &App) -> u64;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `autosave_written_30s_after_change_outside_project_folder` | UI(시계 진행) | 고친 뒤 29 s에는 없고 30 s 뒤 `autosave/`에 저장본·메타가 생긴다. 프로젝트 폴더에는 원래 파일 하나뿐(단계 게이트) |
| 2 | `autosave_content_matches_document` | UI | 저장본을 읽으면 문서와 같다. 메타의 원래 경로가 맞다 |
| 3 | `continuous_editing_still_autosaves_within_30s` | UI | 1 s마다 고쳐도 30 s 안에 저장본이 생기고, 그 뒤 고친 내용도 다음 30 s 안에 반영된다 |
| 4 | `explicit_save_removes_autosave` | UI | `Cmd+S` 뒤 저장본이 지워진다 |
| 5 | `closing_window_removes_autosave` | UI | [저장 안 함]으로 닫아도 지워진다 |
| 6 | `untitled_windows_get_separate_autosaves` | UI | 새 프로젝트 둘은 저장본 둘 |
| 7 | `autosave_failure_is_reported_once_and_retried` | UI | 저장본 폴더를 파일로 막으면 상태줄 알림 한 번, 막힘을 풀면 다음 변경에 쓴다 |
| 8 | `live_session_autosaves_are_not_recoverable` | 단위 | 다른 세션이 잠금을 쥐고 있으면 복구 대상이 아니다 |
| 9 | `dead_session_autosaves_are_recoverable` | 통합(자식 프로세스) | 자식 프로세스가 저장본을 쓰고 SIGKILL로 죽으면 부모가 복구 대상으로 찾는다 |
| 10 | `recover_restores_dirty_windows_without_touching_original` | UI | [복구]: 창이 원래 경로·바뀜·「(복구됨)」으로 열리고 원래 파일 바이트는 그대로, 손대지 않은 첫 창은 닫힘, 저장본은 새 세션 것 |
| 11 | `recovered_window_save_writes_original_and_clears_autosave` | UI | 복구한 창 `Cmd+S` → 원래 파일 갱신, 저장본 지움 |
| 12 | `discard_removes_recoverable_autosaves` | UI | [버리기] → 파일 지움, 창 없음 |
| 13 | `recover_newer_version_keeps_read_only` | UI | 상위 버전 원본의 저장본은 읽기 전용 그대로 |
| 14 | `recover_path_already_open_opens_untitled` | UI | 같은 파일을 연 창이 있으면 경로 없이 연다 |
| 15 | `corrupt_autosave_is_left_and_counted` | UI | 깨진 저장본은 남기고 시트 본문에 센다 |
| 16 | `recovery_removes_stale_temp_files_of_dead_session` | UI | 끝난 세션 pid의 임시 파일만 지우고 다른 pid의 것은 남긴다 |
| 17 | `app_quit_hook_flushes_dirty_autosaves` | UI | `flush_all`이 타이머를 기다리지 않고 바뀐 창의 저장본을 쓴다 |
| 18 | 리뷰 3 부록 A의 `review_r3_recent_broken_hard_link_pair_removed_together`, `review_r3_backup_rename_then_open_backup_opens_separately`, 그리고 `stale_identity_ignored_when_path_holds_other_file`(같은 inode 번호를 흉내 내지 않고, 경로에 다른 파일이 있으면 기억한 정체를 쓰지 않음을 본다) | UI | 이월 5. FAT 재사용 시험은 디스크 이미지를 붙여야 해 저장소 테스트에 넣지 않는다. 같은 원인(낡은 정체)을 백업 시험과 이 시험이 덮는다 |
| 19 | `menu_build_uses_cached_recent_labels` | UI | 목록이 바뀌지 않으면 라벨 계산을 다시 하지 않는다(계산 횟수) |

## 불변식

- 자동 저장본은 `autosave/` 밖에 아무것도 쓰지 않는다. 원래 파일은 복구 중에도 읽거나 쓰지 않는다.
- 살아 있는 세션의 저장본을 지우거나 복구하지 않는다.
- 복구한 창은 사람이 저장하기 전까지 원래 파일과 다를 수 있음을 「바뀜」으로 보인다.

## 범위 밖

- SMF·텍스트 가져오기·내보내기(T7c).
- 되돌려 저장 시점으로 돌아오면 바뀜 끄기(core 저장 지점 표식). T7a 한계 그대로.
- 명시 저장의 백그라운드 쓰기. 자동 저장본은 직렬화만 메인 스레드에서 하고 쓰기는 백그라운드에서 한다.
- 온보딩 프로젝트를 자동 저장본으로만 남기기(REQ 7.8, S8).

## 리스크·결정 요청

- 「변경 후 30 s」를 「바뀐 뒤 30 s 안에」(첫 변경부터 재는 타이머)로 읽는다. 마지막 변경부터 재면 계속 고치는 동안 저장본이 생기지 않는다. REQ 의미를 바꾸지 않는 해석으로 보고 결정 요청은 쓰지 않는다.
- 복구 시트의 문구·단추는 macOS 관례를 따른다.
