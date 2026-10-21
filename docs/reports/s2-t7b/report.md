# Task 보고: S2-T7b 자동 저장본과 크래시 후 복구

- PR: 브랜치 `s2/t7b-autosave`
- 계획 파일: `docs/reports/s2-t7b/plan.md`
- 계획 대비 달라진 점
  - `offer_recovery`는 `Task`를 돌려주지 않고 흐름을 스스로 detach한다. 처음 뼈대대로 `Task`를 돌려주니 시험 도우미가 그것을 놓아 흐름이 취소됐다. 앱 호출자도 같은 실수를 할 수 있어 바꿨다. 계획의 인터페이스 블록을 고쳤다.
  - 문서에 판 번호(`ProjectDocument::revision`)를 더했다. 저장본을 쓰는 동안 또 바뀌었는지 본다.
  - 계획 8·9(살아 있는 세션, 끝난 세션)는 단위 시험 `live_session_autosaves_are_not_recoverable`와 자식 프로세스 시험 `dead_session_autosaves_are_recoverable`로 나눴다. 자기 세션 시험 `own_session_autosaves_are_not_recoverable`, 깨끗한 문서 시험 `clean_document_writes_no_autosave`, 다른 이름 저장 시험 `save_as_moves_autosave_key`, 복구 대상 없음 시험 `no_recoverable_shows_no_sheet`, 옮긴 파일 정체 시험 `moved_file_still_matches_remembered_identity`를 더했다.

## 구현 요약

- `src/autosave.rs`(새 모듈)
  - 세션: `init`이 `autosave/session-<pid>-<나노초>.lock`을 만들고 `File::try_lock`으로 쥔다. 종료 훅(`on_app_quit`)에서 바뀐 창의 저장본을 쓰고(`flush_all`) 잠금 파일을 지운다.
  - 창마다 `WindowAutosave`(새 프로젝트 키, 쓴 키, 진행 중 작업, 실패 알림 여부). 문서 관찰자가 `autosave_document_changed`를 부른다. 깨끗하면 저장본을 지우고, 바뀌었고 타이머가 없으면 30 s 타이머를 건다(첫 변경부터 잰다).
  - 쓰기: 메인 스레드에서 직렬화(`ksq::to_string`, 메타 JSON), 백그라운드에서 내용 → 메타 순으로 임시 파일 + fsync + rename. 끝나면 키가 바뀌었으면 옛 저장본을 지우고, 그새 저장했으면 지우고, 또 바뀌었으면 다음 타이머를 건다. 실패는 성공할 때까지 상태줄에 한 번 알린다.
  - 키: 원래 경로의 FNV-1a 64비트(실행마다 같다), 새 프로젝트는 pid·시각·순번.
  - 복구: `scan`이 끝난 세션(잠금 파일이 없거나 잡힌다)의 저장본만 고른다. 메타·내용을 읽을 수 없는 것과 메타 없는 내용은 「읽을 수 없는」 수로 세고 남겨 둔다. `offer_recovery`가 시트 [복구]/[버리기]. 복구는 저장본마다 창을 열고 원래 경로·읽기 전용 사유를 그대로, 「바뀜」·「(복구됨)」으로 표시한다. 원래 파일은 읽지 않는다. 같은 파일을 연 창이 있으면 경로 없이 연다. 저장본은 이 세션 것으로 다시 쓴다. 손대지 않은 첫 창은 닫는다. 복구·버리기 모두 끝난 세션 pid의 임시 파일(`.<이름>.tmp-<pid>-*`)을 지운다.
- `src/document.rs`: `revision`, `is_recovered`, `mark_recovered`. 저장하면 복구 표시가 풀린다.
- `src/files.rs`: 저장 뒤 자동 저장본을 지운다(저장은 문서 관찰자를 깨우지 않는다). 창을 닫으면 지운다. 제목에 「(복구됨)」. T7a 이월: 기억한 파일 정체는 문서 경로의 파일이 없어졌을 때만 쓴다(경로에 다른 파일이 있으면 낡은 정체). `remove_recent`는 같은 파일 판정(하드 링크 포함). 최근 목록의 합친 목록·라벨은 설정의 목록이 바뀔 때만 계산한다(`RecentCache`).
- `src/window.rs`: 창의 자동 저장 상태와 관찰자 연결. `src/lib.rs`: `autosave::init`, `run`이 첫 창 뒤 `offer_recovery`. `src/app_state.rs`: 설정 경로 접근자.

## 테스트 결과

~~~
cargo test --workspace: 테스트 바이너리 196개 모두 ok, 통과 1689(이전 main 1663 + 새 26), 실패 0, 무시 0
새 테스트: tests/autosave.rs 26(자식 프로세스 SIGKILL 1 포함)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `autosave_written_30s_after_change_outside_project_folder`(단계 게이트) | 통과 |
| 2 | 1에 포함(내용·메타 경로 확인), `clean_document_writes_no_autosave` | 통과 |
| 3 | `continuous_editing_still_autosaves_within_30s` | 통과 |
| 4 | `explicit_save_removes_autosave` | 통과 |
| 5 | `closing_window_removes_autosave` | 통과 |
| 6 | `untitled_windows_get_separate_autosaves`, `save_as_moves_autosave_key` | 통과 |
| 7 | `autosave_failure_is_reported_once_and_retried` | 통과 |
| 8 | `live_session_autosaves_are_not_recoverable`, `own_session_autosaves_are_not_recoverable` | 통과 |
| 9 | `dead_session_autosaves_are_recoverable`(자식 프로세스 SIGKILL) | 통과 |
| 10 | `recover_restores_dirty_windows_without_touching_original` | 통과 |
| 11 | `recovered_window_save_writes_original_and_clears_autosave` | 통과 |
| 12 | `discard_removes_recoverable_autosaves`, `no_recoverable_shows_no_sheet` | 통과 |
| 13 | `recover_newer_version_keeps_read_only` | 통과 |
| 14 | `recover_path_already_open_opens_untitled` | 통과 |
| 15 | `corrupt_autosave_is_left_and_counted` | 통과 |
| 16 | `recovery_removes_stale_temp_files_of_dead_session` | 통과 |
| 17 | `app_quit_hook_flushes_dirty_autosaves` | 통과 |
| 18 | `stale_identity_ignored_when_path_holds_other_file`, `moved_file_still_matches_remembered_identity`, `recent_broken_hard_link_pair_removed_together` | 통과 |
| 19 | `menu_build_uses_cached_recent_labels` | 통과 |
| (추가) | `close_during_autosave_write_leaves_no_autosave`, `closed_view_kept_alive_does_not_autosave_again` | 통과 |

S2-T7a 리뷰 3 부록 A의 FAT 재사용 시험은 디스크 이미지를 붙여야 해 넣지 않았다(계획). 같은 원인(낡은 정체)을 18의 시험이 덮는다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(1689 통과)
- [x] deny(advisories·bans·licenses·sources ok)
- [x] 추가 게이트: `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh` 통과. 단계 게이트 「자동 저장본이 프로젝트 폴더에 생기지 않음」은 `autosave_written_30s_after_change_outside_project_folder`

## 알려진 한계

- 자동 저장본의 직렬화는 메인 스레드에서 한다(노트 10만 개 약 3 MB 문자열). 쓰기만 백그라운드다.
- 저장본이 쓰이는 중에 앱이 죽으면 그 회차의 저장본은 이전 회차 것으로 남는다(임시 파일 + rename). 30 s 안의 작업은 잃을 수 있다(REQ IO-5의 범위).
- 복구 시트에서 창이 사라지면(답 없이) 다음 실행에서 다시 묻는다.
- 「읽을 수 없는」 저장본은 지우지 않는다. 사람이 `autosave/`를 직접 본다. 앱 안에서 정리하는 UI는 없다.
- 끝난 세션의 잠금 파일은 그 세션의 저장본을 버리거나 복구해도 남는다(빈 파일, 다음 판정에는 영향 없음).
- 되돌려 저장 시점으로 돌아오면 바뀜 끄기, 명시 저장의 백그라운드 쓰기는 범위 밖(계획).

## 리뷰어에게

- 세션 판정(`session_alive`): `flock` 잠금은 열린 파일마다라 같은 프로세스 안에서도 다른 핸들이면 막힌다. 판정할 수 없으면 「살아 있음」으로 본다.
- 쓰기와 지우기의 경합: 보고서를 쓰다 찾은 결함이 있었다. 백그라운드 쓰기 중이거나 뷰가 살아 있는 채로 창을 [저장 안 함]으로 닫으면 저장본이 다시 생겨, 다음 실행이 버린 문서의 복구를 제안할 수 있었다. 창에 「닫힘」(`WindowAutosave::closed`)을 두어 닫힌 뒤에는 쓰지 않고, 끝난 쓰기는 지우고, 뷰가 사라졌으면 작업이 직접 지운다. 시험 두 개를 더했다(먼저 실패를 확인했다).
- 복구한 창이 저장본을 「이 세션 것」으로 다시 쓰는 방식과 키 바뀜.

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1(차단): 쓰기 직후 결과를 받기 전 `Cmd+S`가 돌면 저장본이 남는다 | 문서가 깨끗해져도 진행 중인 작업(`pending`)을 놓지 않는다. 쓰기가 끝나면 `finish_autosave`가 깨끗한 문서의 저장본을 지운다 |
| 지적 2(차단): 경로 없이 복구한 창이 같은 파일 창과 키를 공유한다 | 키를 창마다 고유하게(`<pid>-<나노초>-<순번>`) 바꾸고 원래 경로는 메타에만 둔다. 복구한 창은 저장본의 키를 이어받는다. 단계 파일의 `autosave/<경로 해시>.ksq` 이름과 달라진다. REQ(2절 표, 8.4)는 위치만 정하므로 REQ 의미는 그대로다 |
| 지적 3(차단): 복구 시트 중 같은 파일을 열어 고치면 [버리기]가 살아 있는 저장본을 지운다 | 키가 창마다 달라 겹치지 않는다. 또 복구·버리기는 지우거나 가져가기 전에 메타의 세션이 여전히 찾았을 때의 끝난 세션인지 다시 본다(`still_dead`) |
| 지적 4: 같은 파일을 연 살아 있는 다른 실행의 저장본을 덮는다 | 2와 같은 수정(키가 실행·창마다 다르다) |
| 지적 5: 쓰기 실패 뒤 더 고치지 않으면 다시 시도하지 않는다 | 실패하면 바뀐 문서인 동안 30 s 뒤 다시 시도한다. 알림은 성공 전까지 한 번 |
| 지적 6: 기존 쓰기 도중 닫기 시험이 쓰기 도중을 시험하지 않는다 | 리뷰어의 단계 실행 시험 `review_close_stepped_into_autosave_write_leaves_nothing`(통과)을 남긴다. 기존 시험은 지우지 않는다 |
| 제안 7: 임시 파일 정리가 같은 pid를 다시 쓴 살아 있는 프로세스의 파일을 지운다 | 받아들임. 그 pid의 것이면서 1분보다 오래된 임시 파일만 지운다. 우리 시험 `recovery_removes_stale_temp_files_of_dead_session`은 임시 파일의 수정 시각을 10분 전으로 맞추게 고쳤다 |

지적 1–5와 제안 7은 같은 모듈의 키·작업 처리라 커밋 하나(`141213e`)로 묶었다. 리뷰 테스트 `tests/review_s2_t7b.rs`(부록 6개 포함 16개)는 남기고 모두 통과한다.

## 리뷰 2 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: Dock 종료 훅이 쓴 최신 저장본을 그 전에 준비된 백그라운드 쓰기가 옛 내용으로 덮는다. 두 쓰기가 같은 임시 경로를 쓴다 | 창마다 차례(`Slot`: 세대 번호와 바닥)를 둔다. 쓰기는 세대별 임시 파일(`<키>.ksq.tmp-<세대>`)에 쓰고, 잠금 안에서 세대가 바닥보다 낮으면 버리고, 아니면 rename한 뒤 바닥을 올린다(`88695a7`) |
| 제안 2: 복구 때 이 세션 것으로 다시 쓰기가 실패하면 다른 실행이 같은 저장본을 또 복구 대상으로 본다 | 받아들임. 복구한 창이 저장본 내용 파일을 배타 잠금으로 쥐고(폴더 쓰기 권한이 없어도 된다), 찾기는 잠긴 저장본을 건너뛴다. 이 세션 것으로 다시 쓰면 잠금을 놓는다 |
| 제안 3: 끝난 세션의 잠금 파일이 쌓인다 | 받아들임. 복구·버리기 뒤 저장본이 하나도 남지 않은 끝난 세션의 잠금 파일을 지운다 |
| 제안 4: 리뷰 1 A1 시험이 경로 해시 이름을 찾아 아무것도 검사하지 않는다. 계획에 아직 경로 해시 키 | 계획 문구를 고쳤다(`4a714c8`). 리뷰 1의 시험은 리뷰어 산출물이라 그대로 두고, 같은 경로를 폴더 전체로 보는 리뷰 2의 `review_r2_save_stepped_into_autosave_write_leaves_nothing`이 대신한다 |
| 제안 5: 첫 쓰기에서 메타만 실패하면 내용 파일이 남는다. 창이 사라진 뒤 끝난 쓰기의 정리가 돌지 않는다 | 받아들임. 처음 쓰는 저장본이면 메타 rename 실패 때 내용 파일을 지운다. 창이 사라진 뒤 끝난 쓰기는 닫기가 이미 바닥을 올려 버려진다(지우기도 같은 잠금 안에서 바닥을 올린다) |

리뷰 테스트 `tests/review_s2_t7b_r2.rs`(부록 3개 포함 13개)는 남기고 모두 통과한다.

## 리뷰 3 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 잠금 파일 정리가 막 뜬 다른 실행의 잠금 파일(만들고 잠그기 전)을 지운다 | 1분보다 오래된 잠금 파일만 지운다. 함께 `init`은 잠근 뒤 그 파일이 경로의 파일인지 확인하고, 아니면 다시 만든다(`fa5c697`). 리뷰 2 B4와 리뷰 3 잠금 정리 시험은 끝난 세션의 잠금 파일을 방금 만들어 이 수정과 충돌했다. 그 잠금 파일의 수정 시각을 10분 전으로 맞췄다(시험 의도는 그대로) |
| 제안 2: 복구가 `claimed`를 보지 않아 다른 실행이 잠긴 저장본으로 두 번째 창을 연다 | 받아들임. 복구는 잠긴 저장본을 건너뛴다 |
| 제안 3: 크래시가 남긴 세대별 임시 파일이 [버리기]·[복구] 뒤에도 남는다 | 받아들임. 그 키의 세대별 임시 파일을 지운다 |
| 제안 4: 메타 rename만 실패하면 `claim`이 지워진 옛 inode에 남는다 | 받아들임. 메타를 내용보다 먼저 rename한다. 메타가 이 세션을 가리키면 내용이 옛 회차여도 다른 실행이 가져가지 않는다. 첫 쓰기에서 내용 rename이 실패하면 메타를 지운다 |

리뷰 3회에 닿아 결정 요청 `docs/decisions/2026-10-22-s2-t7b-review-limit.md`를 썼다. 리뷰 테스트 `tests/review_s2_t7b_r3.rs`(부록 3개 포함 10개)는 남기고 모두 통과한다.
