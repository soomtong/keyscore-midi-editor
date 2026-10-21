# 적대적 리뷰: S2-T7b 자동 저장본과 크래시 후 복구 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `d40411c`(브랜치 `s2/t7b-autosave`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7b.rs`(깨뜨리지 못한 시도 10개, 모두 통과). 깨뜨린 시도 6개는 부록 A

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 저장본 키가 원래 경로의 해시뿐이다(`autosave.rs:270-272`, `501-508`). 같은 키를 두 주인이 쓰는 경우: 경로 없이 복구한 창이 옛 키를 물려받는 경우(`autosave.rs:426-428`), 복구 시트 중에 같은 파일을 연 창, 같은 파일을 연 다른 실행. 남의 저장본을 덮거나, 저장·닫기·[버리기]가 남의 저장본을 지우는가.
2. 명시 저장이 `pending`을 놓는다(`files.rs:485` → `autosave.rs:229-231`). 놓은 작업이 이미 백그라운드 쓰기를 끝냈으면 그 결과(`finish_autosave`)를 받는 쪽이 사라져 저장본이 남는가. 남으면 다음 실행이 저장한 문서의 복구를 제안한다.
3. 닫기 도중 쓰기(`closed`)가 실제로 쓰기 도중을 시험하는가. 시험 실행기의 `advance_clock`이 작업을 모두 돌리는지.
4. 쓰기 실패 뒤 재시도는 「다음 변경」뿐이다(`autosave.rs:308-314`). 막힘이 풀린 뒤 더 고치지 않으면 바뀐 문서가 저장본 없이 남는가(IO-5 「변경 후 30 s」).
5. 세션 판정: 같은 프로세스의 두 앱(같은 pid)이 서로를 살아 있다고 보는가. 잠금을 지우면(끝내기 훅) 끝난 세션이 되는가.
6. 임시 파일 정리(`autosave.rs:456-472`)는 pid만 본다. 끝난 세션의 pid가 지금 살아 있는 다른 프로세스에 다시 쓰였으면 그 프로세스의 임시 파일을 지우는가.
7. 복구한 창이 원래 파일·프로젝트 폴더에 쓰는 경로(30 s 쓰기, `flush_all`, 키 옮김), 원래 파일의 수정 시각.
8. 복구한 창을 [저장 안 함]으로 닫거나 다른 이름으로 저장한 뒤 옛 저장본이 남아 다음 실행에서 다시 묻는가.
9. [버리기]가 끝난 세션의 읽을 수 있는 저장본만 지우는가(살아 있는 세션, 깨진 저장본, 잠금 파일).
10. T7a 이월: 저장 실패 뒤 저장본 유지, 최근 목록 캐시가 열기·다른 이름 저장을 바로 반영하는가, 링크 경로와 실제 경로의 두 저장본.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 키 공유 | 부분: `recover_path_already_open_opens_untitled` | 저장본 키를 경로 해시가 아닌 `"song"`으로 심어 실제 키 충돌이 생기지 않는다. 열린 창이 쓰거나 저장하는 뒤 단계를 보지 않는다 |
| 2 저장과 쓰기 경합 | 없음 | |
| 3 닫기 도중 쓰기 | 있음: `close_during_autosave_write_leaves_no_autosave` | `advance_clock`이 스케줄러를 끝까지 돌리므로(`gpui-pre-scheduler` `advance_clock`은 루프에서 `run()`) 창을 닫을 때 쓰기는 이미 끝났다. 「쓰기 도중」을 시험하지 않는다. 리뷰 테스트가 시계만 옮기고 한 단계씩 돌려 다시 확인했다(통과) |
| 4 실패 뒤 재시도 | 부분: `autosave_failure_is_reported_once_and_retried` | 막힘을 푼 뒤 고쳐서(touch) 재시도를 본다. 고치지 않는 경우 없음 |
| 5 세션 판정 | 있음(다른 잠금 핸들, 자식 프로세스 SIGKILL) | 같은 프로세스의 두 앱 없음 |
| 6 pid 재사용 | 없음(다른 pid의 임시 파일만) | |
| 7 원래 파일·폴더 | 부분: 복구 직후 원래 바이트 | 복구 뒤 30 s 쓰기·`flush_all`·수정 시각 없음 |
| 8 복구 창 닫기·다른 이름 | 없음 | |
| 9 [버리기] 범위 | 부분: 깨진 저장본 | 살아 있는 세션·잠금 파일 없음 |
| 10 T7a 이월 | 있음(이월 시험 4개) | 저장 실패 뒤 저장본 유지, 캐시 무효화 없음 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 30 s 쓰기가 백그라운드에서 끝난 직후, 결과를 받기 전에 `Cmd+S` | 깨짐: 문서는 깨끗한데 저장본이 남고, 잠금을 놓은 뒤 새 앱이 그 파일을 복구 대상으로 찾는다 | 부록 `review_a1_save_racing_autosave_write_leaves_saved_doc_recoverable` |
| 2 | F를 연 창이 있을 때 F의 끝난 저장본(실제 키 = 경로 해시) 복구 → F 창 고치고 30 s → F 창 저장 | 깨짐: 경로 없이 연 복구 창이 F의 키를 물려받아, F 창의 30 s 쓰기가 복구 창 저장본을 덮는다(저장하면 지운다) | 부록 `review_a2_recovered_untitled_shares_key_with_open_file_window` |
| 3 | 복구 시트가 떠 있는 동안 F를 열고 고쳐 30 s → [버리기] | 깨짐: 이 실행의 살아 있는 F 저장본이 지워진다(시트 전 `scan`의 키로 지움) | 부록 `review_a3_discard_deletes_live_autosave_of_file_opened_during_sheet` |
| 4 | 같은 파일을 연 살아 있는 다른 실행(잠금 쥠)의 저장본이 있을 때 이 실행이 그 파일을 고쳐 30 s | 깨짐: 다른 실행의 저장본·메타를 이 세션 것으로 덮는다 | 부록 `review_a4_live_other_session_autosave_of_same_file_overwritten` |
| 5 | 저장본 폴더를 막아 쓰기 실패 → 막힘 풀기 → 고치지 않고 5분 | 깨짐: 바뀐 문서인데 저장본이 끝내 생기지 않는다 | 부록 `review_a5_failed_autosave_retried_without_new_edit` |
| 6 | 끝난 세션 메타의 pid가 지금 살아 있는 프로세스(`sleep`)의 pid, 그 pid의 임시 파일 → [버리기] | 깨짐: 살아 있는 프로세스의 임시 파일을 지운다 | 부록 `review_a6_stale_temp_cleanup_hits_reused_live_pid` |
| 7 | 시도 1과 같은 단계 진행으로 닫기 [저장 안 함]을 쓰기 도중 0~11단계에 끼움 | 못 깨뜨림(저장본 남지 않음) | `review_close_stepped_into_autosave_write_leaves_nothing` |
| 8 | 쓸 권한 없는 원본에 `Cmd+S` → [취소] | 못 깨뜨림(바뀜·저장본·내용 유지) | `review_failed_save_keeps_autosave` |
| 9 | 같은 프로세스의 두 번째 앱이 같은 폴더를 볼 때 첫 앱의 저장본 | 못 깨뜨림(살아 있음으로 판정. 첫 앱 잠금을 지우면 복구 대상) | `review_second_live_app_does_not_see_first_apps_autosaves` |
| 10 | 경로 없는 저장본 복구 → 다른 이름으로 저장 | 못 깨뜨림(저장본 남지 않음) | `review_recovered_untitled_save_as_leaves_nothing` |
| 11 | 복구 창 [저장 안 함]으로 닫기 → 잠금 해제 뒤 복구 대상 | 못 깨뜨림(없음) | `review_recovered_window_discarded_not_offered_again` |
| 12 | 복구 창을 고쳐 30 s 세 번, `flush_all` | 못 깨뜨림(프로젝트 폴더에 원래 파일 하나, 원래 바이트·수정 시각 그대로, 옛 키 지움) | `review_recovered_window_never_writes_project_folder` |
| 13 | 같은 파일을 가리키는 두 저장본(실제 경로, 심볼릭 링크 경로) 복구 | 못 깨뜨림(경로 가진 창 하나) | `review_two_autosaves_same_file_one_window_with_path` |
| 14 | [버리기]: 살아 있는 세션 저장본, 깨진 저장본, 잠금 파일과 섞기 | 못 깨뜨림(끝난 세션의 읽을 수 있는 저장본만 지움) | `review_discard_touches_only_dead_good_autosaves` |
| 15 | 고치고 10 s 뒤 저장(타이머 놓음) → 5 s 뒤 다시 고침 | 못 깨뜨림(새 타이머로 30 s 뒤 씀) | `review_edit_after_save_autosaves_again` |
| 16 | 최근 목록 캐시: 열기 두 번, 다른 이름 저장, 그 뒤 메뉴 생성 | 못 깨뜨림(목록 즉시 반영, 메뉴 생성은 다시 계산하지 않음) | `review_recent_cache_follows_list_changes` |

회귀 확인: `cargo test -p keyscore-app`(이 브랜치, 리뷰 테스트 포함)에서 `review_s2_t7a*`, `files`, `autosave` 등 모든 테스트 바이너리 통과(부록 A 제외). 리뷰 테스트 파일은 `cargo fmt --check`, `cargo clippy -p keyscore-app --all-targets -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 자동 저장본은 `autosave/` 밖에 쓰지 않는다. 원래 파일은 복구 중 읽거나 쓰지 않는다 | 아니오 | 시도 12, 기존 1·10. 단 임시 파일 정리는 원래 파일 옆에서 지운다(시도 6) |
| 살아 있는 세션의 저장본을 지우거나 복구하지 않는다 | 예 | 복구 대상으로 보지는 않지만(시도 9·14), 지우거나 덮는다: 시도 3(이 실행의 저장본을 [버리기]가 지움), 시도 4(다른 실행의 저장본을 덮음), 시도 2(이 실행의 다른 창 저장본을 덮고 지움) |
| 바뀐 문서에는 30 s 안에 저장본이 있다(IO-5) | 예 | 시도 2·3(저장본이 남의 것으로 덮이거나 지워짐), 시도 5(실패 뒤 재시도 없음) |
| 저장했거나 버린 문서는 다음 실행에서 복구 제안되지 않는다 | 예 | 시도 1(저장과 쓰기 경합). 닫기는 못 깨뜨림(시도 7·11) |
| 복구한 창은 저장하기 전까지 「바뀜」 | 아니오 | 기존 10·11 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-5 자동 저장(변경 후 30 s) | 부분 충족 | `autosave.rs:228-262`. 실패 뒤 고치지 않으면 다시 쓰지 않는다(`autosave.rs:308-314`, 시도 5). 다른 창·다른 실행이 같은 키를 덮어 바뀐 문서의 저장본이 사라진다(시도 2·3·4) |
| IO-5 충돌 시 복구 | 부분 충족 | `autosave.rs:133-192`, `411-452`. 복구한 작업의 저장본이 다른 창에 덮이거나 지워진다(시도 2·3). 저장한 문서를 복구 제안한다(시도 1) |
| 2절 표 「저장」: 자동 저장본은 `~/Library/Application Support/KeyScore/` 아래, 프로젝트 폴더를 더럽히지 않음 | 충족 | `autosave.rs:121-123`, `218-222`. 시도 12, 기존 1(단계 게이트) |
| 8.4 크래시 시 `autosave/` 복구본에서 복구 | 부분 충족 | IO-5 복구와 같은 근거 |
| 8.4 이전 프로젝트 파일은 덮어쓰지 않는다 | 충족 | `autosave.rs:415-438`(원래 파일을 읽거나 쓰지 않음), `document.rs` `mark_recovered`(메타데이터만). 시도 12 |
| 단계 게이트 「자동 저장본이 프로젝트 폴더에 생기지 않음」 | 충족 | 기존 1, 시도 12 |

「부분 충족」은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 변경은 `keyscore-app`의 UI 스레드·백그라운드 실행기뿐이다.
- 제품 코드의 `unwrap`/`expect`: 더한 것 없음(`git diff main...HEAD -- crates/keyscore-app/src`).
- 에러가 삼켜지는 곳
  - `remove_files`·`remove_stale_temps`·잠금 파일 지우기는 실패를 무시한다(`autosave.rs:476-480`, `456-472`, `111-113`). 저장본 지우기가 실패하면 다음 실행이 저장·버린 문서의 복구를 제안한다. 지우기 실패를 일으킬 공개 경로를 찾지 못해 지적으로 올리지 않는다.
  - 세션 잠금을 잡지 못하면(`autosave.rs:100-105`) 알리지 않는다. 그 실행의 저장본은 다른 실행에 끝난 세션으로 보인다(모듈 주석에 적힘).
  - `flush_all`(끝내기 훅)의 쓰기 실패는 상태줄로만 알리는데 앱은 곧 끝난다.
  - 관찰(재현 없음, 지적 아님): `flush_all`은 진행 중인 백그라운드 쓰기와 같은 임시 경로(`<키>.ksq.tmp`)를 동시에 쓸 수 있다(`autosave.rs:483-487`). 시험 실행기는 한 스레드라 재현할 수 없었다. 지적 1을 고칠 때 함께 보길 권한다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 명시 저장(`files.rs:485`)이 `autosave_document_changed`를 불러 `pending`을 놓는다. 그 작업의 백그라운드 쓰기가 이미 끝났으면(또는 실제 스레드에서 쓰는 중이면) 결과를 받는 `finish_autosave`가 돌지 않아 `written`이 갱신되지 않고 저장본이 남는다. 문서는 깨끗해 끝내기가 묻지 않고, 끝내기 훅이 잠금을 지우면 다음 실행이 저장한 문서의 복구를 제안한다. 사람이 그 사이 원본을 고쳤다면 [복구] → 저장이 더 새 내용을 옛 내용으로 덮는다(데이터 손실 가능성). 진행 중인 작업을 놓지 말고 결과 처리(`!dirty`면 지움)에 맡기거나, 놓을 때 그 작업의 키를 지우는 방안. 재현 `review_a1_save_racing_autosave_write_leaves_saved_doc_recoverable` | `crates/keyscore-app/src/autosave.rs:228-233`, `files.rs:485` |
| 2 | 차단 | 경로 없이 연 복구 창이 저장본 키로 원래 경로의 해시(`item.key`)를 그대로 물려받는다. 같은 파일을 연 창(복구 전에 열었거나 Finder에서 그 파일로 앱을 다시 띄운 경우)이 30 s 뒤 같은 키로 써서 복구 창의 저장본을 덮고, 그 창을 저장·닫으면 그 키를 지운다. 복구 창은 바뀐 채 저장본이 없어 다음 크래시에 복구한 작업을 잃는다. 경로 없이 열 때는 새 키(`untitled_key()`)를 쓰고 옛 키를 지우는 방안. 재현 `review_a2_recovered_untitled_shares_key_with_open_file_window` | `crates/keyscore-app/src/autosave.rs:426-428` |
| 3 | 차단 | [버리기]·[복구]는 시트를 띄우기 전 `scan`의 키로 지운다. 시트가 떠 있는 동안(창 모달이라 다른 창에서 열기·편집 가능) 같은 파일을 연 창이 같은 키로 저장본을 쓰면, [버리기]가 이 실행의 살아 있는 저장본을 지운다. 지우기 직전에 메타의 세션이 여전히 스캔한 끝난 세션인지 확인하는 방안(키 충돌 자체는 지적 2·4와 같은 뿌리). 재현 `review_a3_discard_deletes_live_autosave_of_file_opened_during_sheet` | `crates/keyscore-app/src/autosave.rs:446-452` |
| 4 | 수정 | 키가 경로만의 해시라 같은 파일을 연 살아 있는 다른 KeyScore 실행(`open -n`)의 저장본을 덮고, 이 창을 닫거나 저장하면 지운다. 계획의 불변식 「살아 있는 세션의 저장본을 지우지 않는다」와 어긋난다. 같은 파일을 두 실행에서 여는 일 자체가 드물어 수정으로 둔다. 키에 세션을 넣거나(복구 판정은 메타의 경로로) 쓰기·지우기 전에 메타의 세션을 확인하는 방안. 재현 `review_a4_live_other_session_autosave_of_same_file_overwritten` | `crates/keyscore-app/src/autosave.rs:270-272`, `319-329` |
| 5 | 수정 | 쓰기 실패 뒤에는 「다음 변경」에만 다시 시도한다. 일시적 실패(디스크 가득 참, 권한) 뒤 사람이 더 고치지 않으면 바뀐 문서가 끝내 저장본 없이 남는다(IO-5 「변경 후 30 s」를 그 동안 만족하지 않는다). 실패하면 다음 30 s 타이머를 다시 거는 방안(알림은 지금처럼 한 번). 재현 `review_a5_failed_autosave_retried_without_new_edit` | `crates/keyscore-app/src/autosave.rs:308-314` |
| 6 | 수정 | 기존 `close_during_autosave_write_leaves_no_autosave`는 이름과 달리 쓰기 도중을 시험하지 않는다. `advance_clock`이 진행하며 스케줄러를 끝까지 돌리므로 창을 닫을 때 쓰기와 결과 처리가 이미 끝났다. 시계만 옮기고(`cx.dispatcher.scheduler().clock().advance`) `cx.executor().tick()`으로 한 단계씩 끼워 넣어야 한다(리뷰 테스트 `review_close_stepped_into_autosave_write_leaves_nothing`이 그 방식이고 닫기 경로는 통과한다. 지적 1을 고친 뒤 저장 경로도 같은 방식으로 시험한다) | `crates/keyscore-app/tests/autosave.rs:675-693` |
| 7 | 제안 | 임시 파일 정리는 메타의 pid만 본다. 재부팅 등으로 그 pid가 지금 살아 있는 다른 프로세스(`ksq-cli` 등)에 다시 쓰였으면 그 프로세스가 쓰는 중인 임시 파일을 지운다(그 쓰기는 rename에서 실패한다. 원본은 그대로). 계획의 「끝난 KeyScore 세션의 pid만 지우므로 겹치지 않는다」는 pid 재사용에서 성립하지 않는다. 임시 파일의 수정 시각이 세션 끝(잠금 파일 시각) 이전인 것만 지우거나, 그 pid가 살아 있으면 건너뛰는 방안. 재현 `review_a6_stale_temp_cleanup_hits_reused_live_pid` | `crates/keyscore-app/src/autosave.rs:456-472` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 3건(지적 1~3)은 모두 저장 안 된 작업을 잃거나 저장한 문서의 낡은 복구본을 제안할 수 있는 경로다. 지적 2·3·4는 「저장본 키 = 경로 해시」 한 뿌리라 키 설계를 함께 고치길 권한다. 지적 1은 `pending`을 놓는 저장 경로 하나다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 A 6개 실패, 리뷰 테스트 10개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`keyscore-app`의 `autosave.rs`·`files.rs`·`document.rs`·`window.rs`·`lib.rs`·`app_state.rs`, 시험, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음, `Cargo.toml`·`Cargo.lock` 변경 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 재현 테스트

`crates/keyscore-app/tests/review_s2_t7b.rs` 끝에 그대로 붙여 넣으면 같은 파일의 도우미(`start`, `open`, `open_file`, `touch`, `run`, `advance`, `answer`, `windows`, `text_of`, `dirty`, `autosave_dir`, `autosaves`, `plant_dead`, `edited`, `write_sample`, `sample`, `launch_with_recovery`, `release_session_locks`)와 `use`로 컴파일된다. `cargo test -p keyscore-app --test review_s2_t7b review_a`로 `d40411c`에서 6개 모두 실패하는 것을 확인했다. A1은 시계만 옮기고(`cx.dispatcher.scheduler().clock().advance`) 한 단계씩(`cx.executor().tick()`) 돌려 경합 순서를 찾는다. 고친 뒤에는 어떤 단계에서도 저장본이 남지 않아 통과한다.

~~~rust
// ---------------------------------------------------------------- 부록 A: 깨뜨린 시도

/// 앱의 저장본 키(원래 경로의 FNV-1a 64비트 16진, `autosave.rs`의 `path_key`와 같은 계산).
fn path_key(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn autosave_texts(cx: &mut TestAppContext) -> Vec<String> {
    autosaves(cx)
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

/// A1. 30 s 타이머의 백그라운드 쓰기가 끝난 직후, 그 결과를 받기 전에 `Cmd+S`가 먼저 돌면 저장본이 남는다.
/// 문서는 깨끗한데 저장본이 남고, 정상 종료(잠금 해제) 뒤 다음 실행이 저장한 문서의 복구를 제안한다.
/// 시험 실행기는 한 단계씩(`tick`) 돌려 「쓰기 끝 → 저장 → 결과 받기」 순서를 찾는다.
#[gpui_kit::test]
fn review_a1_save_racing_autosave_write_leaves_saved_doc_recoverable(cx: &mut TestAppContext) {
    let dir = start(cx, "race-save");
    let mut leaked = None;
    for k in 0..40 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        // `advance_clock`은 진행하며 모든 작업을 돌린다. 시계만 옮기고 한 단계씩 돌린다.
        cx.dispatcher
            .scheduler()
            .clock()
            .advance(AUTOSAVE_DELAY + Duration::from_secs(1));
        for _ in 0..k {
            if !cx.executor().tick() {
                break;
            }
        }
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(Box::new(file::Save), cx);
        })
        .unwrap();
        cx.run_until_parked();
        advance(cx, AUTOSAVE_DELAY * 2);
        assert!(!dirty(cx, &view));
        let key = path_key(&path);
        if autosave_dir(cx).join(format!("{key}.ksq")).exists() {
            leaked = Some(path);
            break;
        }
    }
    let Some(path) = leaked else {
        return;
    };
    // 정상 종료 뒤 다음 실행.
    let auto = autosave_dir(cx);
    release_session_locks(&auto);
    let next = cx.new_app();
    next.update(|cx| init(cx, dir.join("settings.json")));
    let found = next.read(autosave::find_recoverable);
    panic!(
        "저장한 {}의 저장본이 남아 다음 실행이 복구를 제안한다: {:?}",
        path.display(),
        found
            .iter()
            .map(|r| r.path().map(Path::to_path_buf))
            .collect::<Vec<_>>()
    );
}

/// A2. 파일 F를 연 창이 있을 때 F의 끝난 저장본을 복구하면, 경로 없이 연 복구 창이 F의 키(경로 해시)를 물려받는다.
/// F 창이 30 s 뒤 저장본을 쓰면 복구 창의 저장본을 덮고, F 창을 저장하면 그 키를 지운다. 복구 창은 바뀐 채
/// 저장본이 없다(두 번째 크래시에서 복구한 작업을 잃는다).
#[gpui_kit::test]
fn review_a2_recovered_untitled_shares_key_with_open_file_window(cx: &mut TestAppContext) {
    let dir = start(cx, "shared-key");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let (file_handle, _) = open_file(cx, &path);
    let auto = autosave_dir(cx);
    plant_dead(&auto, &path_key(&path), &edited(), Some(&path), 999_999);
    cx.update(autosave::offer_recovery);
    cx.run_until_parked();
    answer(cx, "복구");
    let recovered = windows(cx)
        .into_iter()
        .find(|w| cx.read(|cx| w.read(cx).document().read(cx).path().is_none()))
        .expect("경로 없이 연 복구 창");
    let recovered_text = text_of(cx, &recovered);
    touch(cx, file_handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    assert!(
        autosave_texts(cx).contains(&recovered_text),
        "복구 창의 저장본이 F 창의 저장본에 덮였다"
    );
    run(cx, file_handle, file::Save);
    assert!(dirty(cx, &recovered));
    assert!(
        autosave_texts(cx).contains(&recovered_text),
        "F 창을 저장하자 바뀐 복구 창의 저장본이 지워졌다"
    );
}

/// A3. 복구 시트가 떠 있는 동안 F를 열어 고치면(30 s 뒤 저장본이 생김) [버리기]가 그 살아 있는 저장본을 지운다.
#[gpui_kit::test]
fn review_a3_discard_deletes_live_autosave_of_file_opened_during_sheet(cx: &mut TestAppContext) {
    let dir = start(cx, "discard-live");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let auto = autosave_dir(cx);
    plant_dead(&auto, &path_key(&path), &edited(), Some(&path), 999_999);
    launch_with_recovery(cx);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let live = text_of(cx, &view);
    assert!(autosave_texts(cx).contains(&live));
    answer(cx, "버리기");
    assert!(dirty(cx, &view));
    assert!(
        autosave_texts(cx).contains(&live),
        "[버리기]가 이 실행에서 고친 창의 저장본을 지웠다"
    );
}

/// A4. 같은 파일을 연 살아 있는 다른 실행의 저장본(같은 경로 해시 키)을 이 실행이 덮어쓰고, 창을 닫을 때 지운다.
#[gpui_kit::test]
fn review_a4_live_other_session_autosave_of_same_file_overwritten(cx: &mut TestAppContext) {
    let dir = start(cx, "other-live");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    let lock = std::fs::File::create(auto.join("session-other.lock")).unwrap();
    lock.try_lock().unwrap();
    let key = path_key(&path);
    std::fs::write(auto.join(format!("{key}.ksq")), edited()).unwrap();
    std::fs::write(
        auto.join(format!("{key}.json")),
        serde_json::json!({"path": path.display().to_string(), "read_only": null,
            "session": "other", "pid": 1})
        .to_string(),
    )
    .unwrap();
    let (handle, _) = open_file(cx, &path);
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(auto.join(format!("{key}.json"))).unwrap())
            .unwrap();
    drop(lock);
    assert_eq!(
        meta["session"], "other",
        "살아 있는 다른 실행의 저장본을 덮어썼다"
    );
}

/// A5. 저장본 쓰기가 한 번 실패한 뒤 막힘이 풀려도, 더 고치지 않으면 다시 쓰지 않는다. 바뀐 문서가 저장본 없이
/// 남는다(그 뒤 크래시면 실패 전 30 s 넘는 작업을 잃는다).
#[gpui_kit::test]
fn review_a5_failed_autosave_retried_without_new_edit(cx: &mut TestAppContext) {
    start(cx, "retry");
    let blocker = autosave_dir(cx);
    let _ = std::fs::remove_dir_all(&blocker);
    std::fs::write(&blocker, "not a directory").unwrap();
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    advance(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
    std::fs::remove_file(&blocker).unwrap();
    advance(cx, AUTOSAVE_DELAY * 10);
    assert!(dirty(cx, &view));
    assert_eq!(
        autosaves(cx).len(),
        1,
        "막힘이 풀린 뒤 5분이 지나도 바뀐 문서의 저장본이 없다"
    );
}

/// A6. 끝난 세션의 pid가 지금 살아 있는 다른 프로세스에 다시 쓰였으면, 복구·버리기가 그 프로세스의 임시 파일을 지운다.
#[gpui_kit::test]
fn review_a6_stale_temp_cleanup_hits_reused_live_pid(cx: &mut TestAppContext) {
    let dir = start(cx, "pid-reuse");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid = child.id();
    let temp = dir.join(format!(".song.ksq.tmp-{pid}-0"));
    std::fs::write(&temp, "in flight").unwrap();
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited(), Some(&path), pid);
    launch_with_recovery(cx);
    answer(cx, "버리기");
    let kept = temp.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(kept, "살아 있는 프로세스(pid {pid})의 임시 파일을 지웠다");
}
~~~
