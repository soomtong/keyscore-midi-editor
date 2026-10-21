# 적대적 리뷰: S2-T7b 자동 저장본과 크래시 후 복구 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음). 리뷰 1의 문서·시험만 읽었다
- 대상 커밋: `a92fdc4`(브랜치 `s2/t7b-autosave`). 리뷰 1 뒤 수정은 `bf854b5..a92fdc4`, 핵심은 `141213e`
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7b_r2.rs`(깨뜨리지 못한 시도 10개, 모두 통과). 깨뜨린 시도 3개는 부록 A

## 1. 코드만 읽고 적은 「내가 검사할 것」

수정 커밋의 시험(`tests/autosave.rs`, `tests/review_s2_t7b.rs`의 바뀐 부분)을 열기 전에 `autosave.rs`, `files.rs`의 바뀐 곳만 읽고 적었다.

1. 키가 창마다 고정(`new_key`)이고 `written`은 쓰기가 성공했을 때만 채워진다. 첫 쓰기의 내용은 썼는데 메타 쓰기가 실패하면, 또는 쓰기 결과를 받기 전에 창·앱이 사라지면 그 키를 지우는 쪽이 있는가.
2. 진행 중 작업(`pending`)을 놓지 않게 바뀌었다. 저장 → 곧 다시 편집에서 첫 타이머가 남아 「30 s 안」을 지키는가. 쓰기 도중 저장 → 다시 편집에서 마지막 내용을 다시 쓰는가.
3. 쓰기 실패 재시도: 실패가 계속되면 알림이 되풀이되거나, 저장·닫기 뒤에도 계속 도는가. 닫은 창이 막힘이 풀린 뒤 버린 문서를 쓰는가.
4. 복구 창은 저장본의 키를 이어받는다(`recover`). 다른 이름으로 저장, 두 복구 창, 같은 경로 복구에서 옛 키가 남거나 서로 지우는가.
5. 이어받기(메타를 이 세션 것으로 다시 쓰기)가 실패하면 메타는 끝난 세션 그대로다. 그동안 뜬 다른 실행이 같은 저장본을 가져가는가.
6. `still_dead`: 다른 실행이 먼저 [복구]한 저장본을 이 실행의 [복구]·[버리기]가 건드리지 않는가.
7. `flush_all`(Dock 종료)은 진행 중인 백그라운드 쓰기와 같은 키·같은 임시 파일을 쓴다(리뷰 1의 관찰이 그대로다). 옛 내용의 쓰기가 끝내기 훅이 쓴 새 내용을 덮는가.
8. `schedule_autosave`의 「뷰가 사라짐」 정리(`finished.is_err()`)는 작업을 뷰가 쥐고 있어 뷰가 사라지면 작업도 버려진다. 실제로 도는 경로인가.
9. 끝난 세션의 잠금 파일·임시 파일 정리(1분 기준).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 지우기 누락 | 부분: 닫기 단계 진행(`review_close_stepped_into_autosave_write_leaves_nothing`) | 메타 쓰기만 실패하는 경우 없음 |
| 2 저장 → 다시 편집 | 부분: `review_edit_after_save_autosaves_again` | 리뷰 1 부록 A1(`review_a1_...`)은 저장본을 경로 해시 이름으로 찾는다. 키가 바뀐 지금은 그 이름의 파일이 생길 수 없어 늘 통과한다(지적 4). 지적 1 수정에 의미 있는 회귀 시험이 없다 |
| 3 실패 재시도 | 부분: `autosave_failure_is_reported_once_and_retried`, `review_a5_...` | 오래 실패할 때 알림 횟수, 저장·닫기 뒤 멈춤 없음 |
| 4 복구 창 키 | 부분: `review_a2_...`, `review_recovered_untitled_save_as_leaves_nothing` | 경로를 가진 복구 창의 다른 이름 저장, 두 복구 창 없음 |
| 5 이어받기 실패 | 없음 | |
| 6 `still_dead` | 없음(`review_a3_...`은 키가 달라 `still_dead`까지 가지 않는다) | |
| 7 `flush_all` 경합 | 없음 | |
| 8 뷰가 사라짐 정리 | 없음 | 시험 실행기는 한 스레드라 「쓰는 중」을 만들 수 없다 |
| 9 잠금 파일 정리 | 없음 | |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 리뷰 1 A1을 저장본 폴더 전체로 다시 봄: 시계만 옮기고 0~39단계 뒤 `Cmd+S`, 다음 실행의 복구 대상 | 못 깨뜨림(어느 단계에서도 저장본이 남지 않음) | `review_r2_save_stepped_into_autosave_write_leaves_nothing` |
| 2 | 0~29단계 뒤 `Cmd+S` → 곧 다시 고침 → 30 s | 못 깨뜨림(30 s 안에 마지막 내용의 저장본) | `review_r2_save_then_edit_during_write_autosaves_latest` |
| 3 | 고침 → 5 s 뒤 저장 → 20 s 뒤 다시 고침 → 6 s(놓지 않은 첫 타이머), 또 고침 → 31 s | 못 깨뜨림(첫 타이머가 마지막 내용을 쓰고, 다음 변경에 새 타이머) | `review_r2_kept_timer_after_save_writes_latest_then_rearms` |
| 4 | 저장본 폴더를 막고 10분, 저장, 막힘 풀고 2분, 다시 고침 | 못 깨뜨림(알림 한 번, 깨끗한 동안 쓰지 않음, 다시 고치면 씀) | `review_r2_failing_retry_is_quiet_and_stops_when_clean` |
| 5 | 쓰기 실패 중 [저장 안 함]으로 닫기 → 막힘 풀고 2분 | 못 깨뜨림(버린 문서를 쓰지 않음) | `review_r2_failing_retry_stops_after_close` |
| 6 | 경로 없는 저장본 둘 복구 → 하나를 다른 이름으로 저장 → 다른 창 고침 → 저장한 창 다시 고침 → 둘 다 [저장 안 함] | 못 깨뜨림(서로 지우지 않고, 새 경로 메타로 쓰고, 마지막에 하나도 안 남음) | `review_r2_two_recovered_windows_keep_separate_keys` |
| 7 | 경로를 가진 복구 창을 다른 이름으로 저장 | 못 깨뜨림(옛 키 없음, 원래 파일 바이트 그대로, 복구 대상 없음) | `review_r2_recovered_with_path_save_as_leaves_nothing` |
| 8 | 같은 폴더의 두 실행 A·B가 같은 끝난 저장본을 제안받고 B가 먼저 [복구] → A가 [버리기] 또는 [복구] | 못 깨뜨림(A는 창을 열지 않고 B의 저장본을 지우거나 바꾸지 않음) | `review_r2_other_run_recovered_first_is_left_alone` |
| 9 | 끝내기 [저장]을 쓰기 도중 0~15단계에 끼움 → `flush_all` | 못 깨뜨림(저장본 없음) | `review_r2_quit_save_stepped_into_write_leaves_nothing` |
| 10 | 닫기 시트를 띄운 채 시계를 옮기고 k(0~7)단계 → [저장 안 함] → j(0~3)단계 | 못 깨뜨림(시험 실행기에서는 어느 순서도 남지 않음. 실제 스레드는 6절) | `review_r2_close_answer_interleaved_with_write_leaves_nothing` |
| 11 | 시계만 옮기고 k단계 → 또 고침 → `flush_all`(Dock 종료) → 남은 작업 진행 | 깨짐(k=8): 끝내기 훅이 쓴 최신 저장본을 그 전에 준비한 백그라운드 쓰기가 옛 내용으로 덮는다 | 부록 `review_r2_b1_flush_overwritten_by_older_inflight_write` |
| 12 | 복구할 때 저장본 폴더에 쓸 수 없어 이어받기 실패 → 다른 실행 시작 | 깨짐: A가 연 복구 창의 저장본을 B가 끝난 세션 것으로 보고 복구 대상에 넣는다 | 부록 `review_r2_b2_failed_claim_lets_other_run_take_same_autosave` |
| 13 | 끝난 세션의 잠금 파일이 있는 저장본을 [버리기] | 깨짐(경미): 잠금 파일이 남는다. 크래시마다 하나씩 쌓인다 | 부록 `review_r2_b4_dead_session_lock_files_pile_up` |

회귀 확인: `cargo test -p keyscore-app`(이 브랜치, 리뷰 테스트 두 파일 포함)에서 부록 A를 빼고 모든 시험 통과. 리뷰 테스트 파일은 `cargo fmt --check`, `cargo clippy -p keyscore-app --all-targets -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 자동 저장본은 `autosave/` 밖에 쓰지 않는다. 원래 파일은 복구 중 읽거나 쓰지 않는다 | 아니오 | 시도 7, 리뷰 1 시도 12 |
| 살아 있는 세션의 저장본을 지우거나 복구하지 않는다 | 예(드묾) | 시도 12: 이어받기에 실패한 동안. 다른 실행이 먼저 가져간 것은 건드리지 않는다(시도 8) |
| 바뀐 문서에는 30 s 안에 최신 저장본이 있다(IO-5) | 예(드묾) | 시도 11: Dock 종료 직전의 변경이 옛 내용에 덮인다. 실패 뒤 재시도는 고쳐졌다(시도 4) |
| 저장했거나 버린 문서는 다음 실행에서 복구 제안되지 않는다 | 시험 실행기에서는 아니오 | 시도 1·9·10. 실제 스레드의 좁은 경합은 6절 |
| 창마다 저장본 키가 하나이고 다른 창·실행과 겹치지 않는다 | 아니오(이어받기 실패 제외) | 시도 6·8, 리뷰 1 A2~A4 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-5 자동 저장(변경 후 30 s) | 부분 충족 | `autosave.rs:234-322`. 실패 재시도·키 분리는 고쳐졌다. Dock 종료 때 끝내기 훅의 최신 저장본이 진행 중이던 옛 쓰기에 덮일 수 있다(`autosave.rs:201-209`, `248-259`, 시도 11) |
| IO-5 충돌 시 복구 | 충족 | `autosave.rs:418-475`. 시도 6·7·8. 이어받기 실패 경우(시도 12)는 저장본 폴더에 쓸 수 없는 상태에서만 생겨 제안으로 둔다 |
| 2절 표 「저장」: 자동 저장본은 `~/Library/Application Support/KeyScore/` 아래, 프로젝트 폴더를 더럽히지 않음 | 충족 | `autosave.rs:127-129`. 시도 7. 프로젝트 폴더에서는 끝난 세션의 1분 넘은 임시 파일만 지운다(`autosave.rs:480-502`) |
| 8.4 크래시 시 `autosave/` 복구본에서 복구 | 부분 충족 | IO-5와 같은 근거(Dock 종료 경로의 최신성) |
| 8.4 이전 프로젝트 파일은 덮어쓰지 않는다 | 충족 | `document.rs` `mark_recovered`, 시도 7 |
| 단계 게이트 「자동 저장본이 프로젝트 폴더에 생기지 않음」 | 충족 | 시도 7, 기존 1 |

「부분 충족」은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 변경은 `keyscore-app`의 UI 스레드·백그라운드 실행기뿐이다.
- 제품 코드의 `unwrap`/`expect`: 더한 것 없음(`git diff main...HEAD -- crates/keyscore-app/src`).
- 에러가 삼켜지는 곳
  - `remove_files`·`remove_stale_temps`·잠금 파일 지우기의 실패 무시(리뷰 1과 같음).
  - 첫 쓰기에서 내용(`<키>.ksq`)은 썼고 메타 쓰기가 실패하면 `written`이 비어(`autosave.rs:296-297`은 성공 때만) 저장·닫기가 그 내용 파일을 지우지 않는다. 메타 없는 내용 파일은 다음 실행부터 「읽을 수 없는 저장본」으로 세어져 영영 남는다. 키를 예측할 수 없어 공개 경로로 재현하지 못했다. 키가 창마다 고정이므로 `remove_autosave`가 `written`과 상관없이 `self.autosave.key`를 지우게 하면 함께 풀린다.
  - 관찰(재현 없음, 실제 스레드 경합): `schedule_autosave`의 작업은 뷰의 `pending`이 쥔다(`autosave.rs:78`, `248`). 창을 닫아 뷰가 사라지면 작업도 버려지므로 `finished.is_err()` 정리(`autosave.rs:264-267`)는 돌 수 없다. 실제 스레드에서 백그라운드 쓰기가 도는 동안 [저장 안 함]으로 닫거나, 끝내기 [저장] 직후 앱이 끝나면 쓰기는 마저 끝나 파일을 남기고 결과를 받는 쪽은 없다(첫 쓰기면 `written`도 비어 있다). 다음 실행이 버리거나 저장한 문서의 복구를 제안한다. 창은 쓰기 한 번(수 ms)이라 시험 실행기(한 스레드, 시도 9·10)에서는 재현하지 못했다. 지적 1을 고칠 때 「쓰기가 스스로 뒷정리」하는 방식(백그라운드 쓰기가 끝난 뒤 공유 플래그로 닫힘·깨끗함을 보고 스스로 지움)을 함께 권한다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `flush_all`(Dock 메뉴·로그아웃 종료 훅)은 진행 중인 백그라운드 쓰기와 같은 키·같은 임시 경로(`<키>.ksq.tmp`)에 동기로 쓴다. 타이머가 옛 내용(r1)으로 쓰기를 준비한 뒤 사람이 또 고치고(r2) Dock에서 끝내면, 훅이 r2를 쓴 다음 이미 시작한 r1 쓰기가 끝나 저장본을 r1로 덮는다. 다음 실행은 r1을 복구해 마지막 변경을 잃는다(종류 a). 두 쓰기가 같은 임시 파일을 동시에 열면 섞인 내용이 될 수도 있다(리뷰 1 6절 관찰이 그대로 남았다). 실제로는 백그라운드 쓰기가 편집·종료보다 늦게 끝나야 해(스레드 풀 대기, 디스크 지연) 드물다. 방안: 창마다 쓰기 세대(`Arc<Mutex<u64>>`)를 두고 임시 파일 이름에 세대를 넣어 겹치지 않게 하며, rename 직전에 잠금 안에서 자기 세대가 마지막으로 쓴 세대보다 새로울 때만 바꾼다. 재현 `review_r2_b1_flush_overwritten_by_older_inflight_write` | `crates/keyscore-app/src/autosave.rs:201-209`, `248-259`, `512-528` |
| 2 | 제안 | 복구 때 이어받기(이 세션 메타로 다시 쓰기)가 실패하면 메타는 끝난 세션 그대로다(`autosave.rs:439-444`). 그동안 뜬 다른 실행(`open -n`)이 같은 저장본을 복구 대상으로 보고 가져갈 수 있고, 두 창이 같은 키를 쥐어 서로 덮거나 한쪽 저장이 다른 쪽 저장본을 지운다(종류 d). 저장본 폴더에 쓸 수 없는(가득 참, 권한) 상태 + 그 사이 두 번째 실행이 겹쳐야 하고, 그 상태에서는 IO-5 자체를 지킬 수 없으며 실패는 이미 알린다. 그래서 제안으로 둔다. 방안: 이어받기에 실패하면 [복구]가 그 저장본을 잠금 파일 같은 표지로 쥐거나, 실패를 시트로 알리고 복구를 미룬다. 재현 `review_r2_b2_failed_claim_lets_other_run_take_same_autosave` | `crates/keyscore-app/src/autosave.rs:435-445` |
| 3 | 제안 | 끝난 세션의 `session-<id>.lock`은 [복구]·[버리기] 뒤에도 지우지 않아 크래시마다 저장본 폴더에 하나씩 쌓인다. 지워도 판정은 같다(파일 없음 = 끝난 세션). 그 세션의 저장본이 더 없을 때 지우는 방안. 재현 `review_r2_b4_dead_session_lock_files_pile_up` | `crates/keyscore-app/src/autosave.rs:456-466` |
| 4 | 제안 | 지적 1(리뷰 1) 회귀 시험 `review_a1_save_racing_autosave_write_leaves_saved_doc_recoverable`은 저장본을 경로 해시 이름(`path_key`)으로 찾는다. 키가 창마다 바뀐 지금은 그 파일이 생길 수 없어 수정 전 코드로 되돌려도 통과한다. 저장본 폴더 전체를 보게 고치거나 `review_r2_save_stepped_into_autosave_write_leaves_nothing`을 그 회귀 시험으로 삼는다. 함께: `docs/reports/s2-t7b/plan.md:13`은 아직 「키는 원래 경로의 해시」, `review_recovered_window_never_writes_project_folder`의 주석은 「새 키(경로 해시)」라 실제 설계와 다르다. 단계 파일(`plan/stages/12-S2-app-shell.md:83`)의 `autosave/<경로 해시>.ksq`와 다른 것은 보고서에 적혀 있다 | `crates/keyscore-app/tests/review_s2_t7b.rs`, `docs/reports/s2-t7b/plan.md:13` |
| 5 | 제안 | 6절 관찰 두 가지(메타만 실패한 첫 쓰기의 내용 파일이 남음, 뷰가 사라지면 쓰기 뒷정리를 못 함). `remove_autosave`가 고정 키를 늘 지우게 하고, 백그라운드 쓰기가 스스로 뒷정리하게 하는 방안 | `crates/keyscore-app/src/autosave.rs:264-267`, `294-297`, `331-335` |

리뷰 1 지적의 수정 확인

| 리뷰 1 | 확인 |
| --- | --- |
| 지적 1(저장과 쓰기 경합) | 고쳐짐. 시도 1·2·9, 리뷰 1 시도 7 |
| 지적 2(복구 창이 같은 파일 창과 키 공유) | 고쳐짐. `review_a2_...` 통과, 시도 6·7 |
| 지적 3(시트 중 연 파일의 저장본을 [버리기]가 지움) | 고쳐짐. `review_a3_...` 통과, `still_dead`는 시도 8로 확인 |
| 지적 4(다른 실행의 같은 파일 저장본을 덮음) | 고쳐짐. `review_a4_...` 통과. 이어받기 실패 때만 남는다(지적 2) |
| 지적 5(실패 뒤 재시도) | 고쳐짐. `review_a5_...` 통과, 시도 4·5(알림 한 번, 깨끗함·닫힘에서 멈춤) |
| 지적 6(쓰기 도중 닫기 시험) | 리뷰어 시험을 남김. 시도 10이 두 축으로 넓혀 통과 |
| 제안 7(pid 재사용 임시 파일) | 받아들임. `review_a6_...` 통과 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1의 차단 3건과 수정 2건은 모두 고쳐졌고 그 수정의 회귀는 찾지 못했다(시도 1~10). 남은 「수정」 1건(지적 1)은 Dock 종료 경로에서 마지막 변경을 잃을 수 있는 경합이다(종류 a). 드물지만 고칠 곳이 `flush_all`·`Job::write` 둘로 좁고, 리뷰 1이 같은 곳을 관찰로 남겼으므로 이번에 닫길 권한다. 지적 1만 고치고 재리뷰는 그 부분과 부록 B1에 한정해도 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(수정 커밋의 시험을 열기 전)
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 A 3개 실패, 리뷰 테스트 10개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`bf854b5..a92fdc4`: `autosave.rs`, `tests/autosave.rs`, `tests/review_s2_t7b.rs`, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`tests/autosave.rs`는 임시 파일 시각 맞추기만 더함)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 재현 테스트

`crates/keyscore-app/tests/review_s2_t7b_r2.rs` 끝에 그대로 붙여 넣으면 같은 파일의 도우미(`start`, `open_file`, `dispatch`, `run`, `touch`, `advance_clock_only`, `ticks`, `text_of`, `dirty`, `autosave_dir`, `autosaves`, `autosave_text_for`, `plant_dead`, `edited`, `write_sample`, `answer`, `windows`, `launch_with_recovery`)와 `use`로 컴파일된다. `cargo test -p keyscore-app --test review_s2_t7b_r2 review_r2_b`로 `a92fdc4`에서 3개 모두 실패하는 것을 확인했다. B1은 k=8에서 깨진다. B1의 마지막 `run_until_parked`는 「끝내기 훅 전에 이미 시작한 백그라운드 쓰기가 프로세스가 끝나기 전에 마저 끝남」을 흉내 낸다(시계는 옮기지 않아 새 타이머는 울리지 않는다).

~~~rust
// ---------------------------------------------------------------- 부록 A: 깨뜨린 시도

/// B1. 끝내기 훅(`flush_all`)이 최신 내용을 쓴 뒤, 그 전에 준비한 30 s 타이머의 백그라운드 쓰기가 옛 내용으로 덮는다.
#[gpui_kit::test]
fn review_r2_b1_flush_overwritten_by_older_inflight_write(cx: &mut TestAppContext) {
    let dir = start(cx, "flush-stale");
    let mut broken = Vec::new();
    for k in 0..12 {
        let path = dir.join(format!("song{k}.ksq"));
        write_sample(&path);
        let (handle, view) = open_file(cx, &path);
        touch(cx, handle);
        advance_clock_only(cx, AUTOSAVE_DELAY + Duration::from_secs(1));
        ticks(cx, k);
        // 쓰기가 끝나기 전에 또 고치고 Dock에서 끝낸다(끝내기 훅).
        dispatch(cx, handle, track::New);
        cx.update(autosave::flush_all);
        let latest = text_of(cx, &view);
        assert_eq!(autosave_text_for(cx, &path).as_deref(), Some(latest.as_str()));
        // 이미 시작한 백그라운드 쓰기는 프로세스가 끝나기 전에 마저 끝난다(시계는 옮기지 않는다).
        cx.run_until_parked();
        if autosave_text_for(cx, &path).as_deref() != Some(latest.as_str()) {
            broken.push(k);
        }
        run(cx, handle, file::Save);
    }
    assert!(
        broken.is_empty(),
        "끝내기 훅이 쓴 최신 저장본을 옛 쓰기가 덮었다: k={broken:?}"
    );
}

/// B2. 복구 창이 저장본을 이 세션 것으로 바꾸지 못하면(저장본 폴더에 쓸 수 없음) 메타는 끝난 세션 그대로라, 그
/// 동안 뜬 다른 실행이 같은 저장본을 또 복구 대상으로 본다.
#[gpui_kit::test]
fn review_r2_b2_failed_claim_lets_other_run_take_same_autosave(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = start(cx, "claim-fail");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Claimed"), None);
    launch_with_recovery(cx);
    std::fs::set_permissions(&auto, std::fs::Permissions::from_mode(0o555)).unwrap();
    answer(cx, "복구");
    let held = windows(cx).iter().any(|w| dirty(cx, w));
    let other = cx.new_app();
    other.update(|cx| init(cx, dir.join("settings.json")));
    let found = other.read(autosave::find_recoverable).len();
    std::fs::set_permissions(&auto, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(held, "A가 복구 창을 열지 않았다");
    assert_eq!(found, 0, "A가 연 복구 창의 저장본을 다른 실행이 또 복구 대상으로 본다");
}

/// B4. 끝난 세션의 잠금 파일은 복구·버리기 뒤에도 남아 크래시마다 쌓인다.
#[gpui_kit::test]
fn review_r2_b4_dead_session_lock_files_pile_up(cx: &mut TestAppContext) {
    start(cx, "dead-locks");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("L"), None);
    let dead_lock = auto.join("session-dead-k.lock");
    std::fs::write(&dead_lock, "").unwrap();
    launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(autosaves(cx).is_empty());
    assert!(
        !dead_lock.exists(),
        "끝난 세션의 잠금 파일이 남았다(저장본 폴더에 크래시마다 하나씩 쌓인다)"
    );
}
~~~
