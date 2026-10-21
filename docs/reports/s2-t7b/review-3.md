# 적대적 리뷰: S2-T7b 자동 저장본과 크래시 후 복구 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음). 리뷰 1·2의 문서·시험만 읽었다
- 대상 커밋: `f6e74b9`(브랜치 `s2/t7b-autosave`). 리뷰 2 뒤 수정은 `df3d4b0..f6e74b9`, 핵심은 `88695a7`
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7b_r3.rs`(깨뜨리지 못한 시도 7개, 모두 통과). 깨뜨린 시도 3개는 부록 A
- 작업 트리의 `tests/review_s2_t7b_r2.rs` 미커밋 변경(B1·B2의 `cargo fmt` 줄바꿈)은 이 리뷰 전부터 있었다. 건드리지 않았다

## 1. 코드만 읽고 적은 「내가 검사할 것」

`88695a7`의 `autosave.rs`만 읽고 적었다.

1. 세대·바닥: 세대는 준비 때(`prepare_autosave`) 받고, 성공한 쓰기는 바닥을 `세대 + 1`로, 지우기는 바닥을 `next`로 올린다. 늦게 끝난 옛 쓰기가 덮거나 되살리는 순서가 남는가. 버려진 쓰기(`Superseded`)를 `Ok`로 받아 다음 타이머·실패 알림이 꼬이는가.
2. 잠금 안 rename과 지우기의 순서: 내용 rename 뒤 메타 rename 실패 때 첫 쓰기면 내용을 지운다(`had_meta`). 복구한 저장본(메타가 이미 있음)에서 같은 실패가 나면 잠금(`claim`)은 옛 inode에 남고 새 내용은 잠기지 않는다.
3. `claim`: 내용 파일 inode에 flock. 놓는 때는 `Written`, `remove_autosave`, 창이 사라질 때. 같은 프로세스의 다른 앱에서 `claimed()`가 잠김을 보는가(flock은 열린 파일마다). `recover`의 `still_dead`는 `claimed`를 보지 않는다.
4. `remove_dead_locks`는 모든 `session-*.lock`을 훑고, 메타가 가리키지 않고 `session_alive`가 거짓이면 지운다. 다른 실행의 `init`은 `File::create` 뒤 `try_lock`이다. 그 사이의 잠금 파일은 「잠글 수 있음 = 끝난 세션」으로 보인다.
5. 세대별 임시 파일(`<키>.ksq.tmp-<세대>`): 쓰기의 모든 갈래에서 지운다. 쓰기 도중 크래시로 남은 것은 누가 지우는가.
6. 창이 사라진 뒤 끝난 쓰기: `finished.is_err()` 정리를 없앴다. 닫기가 바닥을 올렸으니 버려지는가. 닫기를 거치지 않고 뷰가 사라지는 경로가 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 세대·바닥 | 부분: 리뷰 2 B1(`review_r2_b1_...`) | 훅 두 번, 훅 + 저장, 저장 + 훅, 훅 + 닫기 순서 없음 |
| 2 메타 rename 실패 | 없음 | 키를 예측할 수 없어 공개 경로로 만들기 어렵다 |
| 3 `claim` | 부분: 리뷰 2 B2 | 재시도 성공 뒤 잠금 놓기, 같은 프로세스 판정, 시트가 이미 뜬 다른 실행 없음 |
| 4 잠금 파일 정리 | 부분: 리뷰 2 B4(끝난 세션만) | 살아 있거나 막 생긴 세션 없음 |
| 5 임시 파일 | 없음 | |
| 6 창이 사라진 뒤 쓰기 | 리뷰 2 `review_r2_close_answer_interleaved_with_write_leaves_nothing` | 닫기 경로는 `request_close`, `close_now_sync` 둘 다 `close_autosave`를 부른다(`files.rs:254`, `842`) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 쓰기 도중 k(0~11)단계 → 고침 → 훅 → 고침 → 훅 → 남은 작업 | 못 깨뜨림(두 번째 훅의 최신 내용) | `review_r3_two_flushes_with_inflight_write_keep_latest` |
| 2 | k단계 → 고침 → 훅 → `Cmd+S` → 60 s | 못 깨뜨림(저장본·임시 파일 없음, 다음 실행 제안 없음) | `review_r3_save_after_flush_with_inflight_write_leaves_nothing` |
| 3 | k단계 → `Cmd+S` → 고침 → 훅 → 남은 작업 → 또 고침 → 31 s | 못 깨뜨림(최신 유지, 버려진 쓰기 뒤 새 타이머로 30 s 안에 씀) | `review_r3_flush_after_save_with_inflight_write_keeps_latest_and_rearms` |
| 4 | k단계 → 고침 → 훅 → [저장 안 함] 닫기 → 60 s | 못 깨뜨림(저장본·임시 파일 없음) | `review_r3_close_after_flush_with_inflight_write_leaves_nothing` |
| 5 | 폴더를 막고 [복구] → 다른 실행 → 막힘 풀고 31 s → 또 다른 실행 → [저장 안 함] → 또 다른 실행 | 못 깨뜨림(잠금 동안·재시도 뒤 모두 복구 대상 아님, 닫은 뒤 아무것도 없음) | `review_r3_claim_released_after_retry_and_close_leaves_nothing` |
| 6 | 같은 프로세스에서 복구 창의 잠금을 다른 열린 파일·다른 앱으로 판정 | 못 깨뜨림(잠김으로 보임) | `review_r3_claim_is_seen_from_same_process` |
| 7 | 살아 있는 다른 실행, 이 실행, 읽을 수 없는 저장본이 남은 끝난 세션의 잠금 파일이 있을 때 [버리기] | 못 깨뜨림(셋 다 남고 버린 세션 것만 지움, 살아 있는 실행의 저장본은 복구 대상 아님) | `review_r3_dead_lock_cleanup_keeps_live_and_in_use_locks` |
| 8 | 다른 실행 C가 `init`에서 잠금 파일을 만들고 잠그기 전에 이 실행이 [버리기] → C가 잠그고 저장본을 씀 → 또 다른 실행 | 깨짐: 정리가 C의 잠금 파일을 지워 C는 잠금 파일 없이 살고, 그 뒤 실행이 C의 저장본을 복구 대상에 넣는다 | 부록 `review_r3_c1_fresh_session_lock_removed_by_dead_lock_cleanup` |
| 9 | A·B가 같은 끝난 저장본을 함께 제안받음 → 폴더를 막고 A가 [복구](잠금만 쥠) → B가 [복구] | 깨짐: B가 같은 키로 두 번째 복구 창을 연다 | 부록 `review_r3_c2_claimed_autosave_recovered_again_by_open_sheet` |
| 10 | 끝난 세션이 남긴 `k.ksq.tmp-3`, `k.json.tmp-3`이 있는 저장본을 [버리기] | 깨짐(경미): 임시 파일이 영영 남는다 | 부록 `review_r3_c3_crash_left_generation_temps_survive_discard` |

회귀 확인: `cargo test -p keyscore-app --test autosave --test review_s2_t7b --test review_s2_t7b_r2 --test review_s2_t7b_r3` 모두 통과(26, 16, 13, 7). 리뷰 2 부록 B1·B2·B4도 통과한다. 리뷰 테스트 파일은 `cargo fmt --all -- --check`, `cargo clippy -p keyscore-app --all-targets -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 자동 저장본은 `autosave/` 밖에 쓰지 않는다. 원래 파일은 복구 중 읽거나 쓰지 않는다 | 아니오 | 바뀐 곳 없음(리뷰 2 시도 7) |
| 살아 있는 세션의 저장본을 지우거나 복구하지 않는다 | 예(드묾) | 시도 8(막 뜬 실행의 잠금 파일 삭제, 이번 수정의 회귀), 시도 9(폴더에 쓸 수 없을 때) |
| 바뀐 문서에는 30 s 안에 최신 저장본이 있다(IO-5) | 아니오 | 시도 1·3, 리뷰 2 B1 |
| 저장했거나 버린 문서는 다음 실행에서 복구 제안되지 않는다 | 아니오 | 시도 2·4·5 |
| 창마다 저장본 키가 하나이고 다른 창·실행과 겹치지 않는다 | 예(드묾) | 시도 9 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-5 자동 저장(변경 후 30 s) | 충족 | `autosave.rs:73-97`, `267-303`. 시도 1~4, 리뷰 2 B1 |
| IO-5 충돌 시 복구 | 부분 충족 | `autosave.rs:502-530`. 막 뜬 다른 실행의 잠금 파일을 지워 그 실행이 살아 있는 동안 그 저장본이 남에게 복구·삭제 대상이 될 수 있다(시도 8) |
| 2절 표 「저장」: 자동 저장본은 `~/Library/Application Support/KeyScore/` 아래, 프로젝트 폴더를 더럽히지 않음 | 충족 | 바뀐 곳은 `autosave/` 안뿐. 임시 파일이 남는 것은 그 폴더 안이다(시도 10) |
| 8.4 크래시 시 `autosave/` 복구본에서 복구 | 충족 | 시도 5·6 |
| 8.4 이전 프로젝트 파일은 덮어쓰지 않는다 | 충족 | 바뀐 곳 없음 |
| 단계 게이트 「자동 저장본이 프로젝트 폴더에 생기지 않음」 | 충족 | 바뀐 곳 없음 |

「부분 충족」은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 새 `Mutex`(`Slot::floor`)는 UI 스레드와 백그라운드 실행기 사이뿐이고, 잠금 안에서는 rename·지우기만 한다. 안에서 다른 잠금을 잡지 않아 교착은 없다.
- 제품 코드의 `unwrap`/`expect`: 더한 것 없음. 독이 든 잠금은 `into_inner`로 받는다.
- 에러가 삼켜지는 곳
  - `remove_files`, 임시 파일 지우기, 잠금 파일 지우기의 실패 무시(리뷰 1·2와 같음).
  - 관찰(재현 없음): 복구한 저장본(메타가 이미 있음)을 이 세션 것으로 다시 쓸 때 내용 rename은 되고 메타 rename만 실패하면, 새 내용 파일(새 inode)은 잠기지 않고 `claim`은 지워진 옛 inode에 남는다. 메타는 끝난 세션 그대로라 다른 실행이 다시 복구 대상으로 본다. 리뷰 2 제안 2와 같은 갈래이고 rename 하나만 실패해야 해 제안 수준이다. `claim`을 내용 파일 대신 키 이름의 별도 표지 파일(예: `<키>.claim`)에 걸면 rename과 무관해진다.
  - 관찰: 폴더에 쓸 수 없는 동안 복구 창을 [저장 안 함]으로 닫으면 `remove_files`가 실패하고 `claim`을 놓아, 다음 실행이 버린 문서를 제안한다. 리뷰 1부터 있던 「폴더에 쓸 수 없음」 갈래이고 그동안 실패를 알렸으므로 지적하지 않는다.
  - 버려진 쓰기(`Superseded`)도 `failure_shown`을 거짓으로 되돌린다. 버려지려면 임시 파일 쓰기가 성공해야 하므로 실패 알림이 되풀이되지는 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `remove_dead_locks`(리뷰 2 제안 3 수정)가 막 뜬 다른 실행의 잠금 파일을 지운다. 다른 실행 C의 `init`은 `File::create` 뒤 `try_lock`하므로, 그 틈의 잠금 파일은 메타가 가리키지 않고 잠글 수 있어 「끝난 세션」으로 보인다. 이 실행이 그때 [복구]·[버리기]를 마치면 파일을 지우고, C는 지워진 inode를 잠근 채 끝까지 산다. 잠금 파일이 없으니 그 뒤 뜬 실행은 C의 바뀐 창 저장본을 끝난 세션 것으로 보고 복구를 제안하며, [복구]는 같은 키를 가져가고 [버리기]는 C의 저장본을 지운다(종류 d, 이번 수정의 회귀 f). 같은 정리의 `session_alive`가 C의 파일을 잠깐 잠그는 순간에 C의 `try_lock`이 겹치면 C는 잠금 없이 뜨고 파일은 지워진다(같은 결과). 틈은 짧지만 여러 실행을 동시에 띄우는 경우(`open -n`, 여러 파일 열기)에 생기고 결과가 남의 저장본 삭제다. 방안: 정리는 수정 시각이 오래된(예: `STALE_TEMP_AGE` 이상) 잠금 파일만 지운다. 함께 `init`은 잠근 뒤 경로가 같은 inode를 가리키는지 확인하고 아니면 다시 만든다(`try_lock` 실패 때도 몇 번 다시 시도). 재현 `review_r3_c1_fresh_session_lock_removed_by_dead_lock_cleanup`(C의 `init`을 파일 조작으로 흉내 내므로 `init` 쪽만 고치면 이 시험은 그대로 실패한다. 그때는 시험을 「오래된 잠금 파일만 지움」에 맞게 바꿔 재리뷰에서 확인한다) | `crates/keyscore-app/src/autosave.rs:138-143`, `502-530` |
| 2 | 제안 | `recover`는 `still_dead`(메타 세션, 세션 잠금)만 보고 `claimed`를 보지 않는다. 두 실행이 같은 끝난 저장본을 함께 제안받고 A가 [복구]했지만 다시 쓰기에 실패해 잠금만 쥐었으면, B의 [복구]는 같은 키로 두 번째 창을 연다(잠금 실패는 무시하고 계속한다). 두 창이 같은 키를 쥐어 한쪽 저장·닫기가 다른 쪽 저장본을 지운다(종류 d). 저장본 폴더에 쓸 수 없는 상태가 겹쳐야 해 제안으로 둔다. 방안: `recover`에서 `claim`의 `try_lock`이 실패하면 그 저장본을 건너뛰고(창을 열기 전에 잠근다), `discard`도 잠긴 저장본은 지우지 않는다. 재현 `review_r3_c2_claimed_autosave_recovered_again_by_open_sheet` | `crates/keyscore-app/src/autosave.rs:537-557`, `578-581`, `588-594` |
| 3 | 제안 | 쓰기 도중 끝난 세션의 세대별 임시 파일(`<키>.ksq.tmp-<세대>`, `<키>.json.tmp-<세대>`)은 [복구]·[버리기] 뒤에도 저장본 폴더에 남는다. 세대가 창마다 0부터라 이어받은 창이 같은 번호를 다시 쓸 때만 지워진다. 폴더 안의 찌꺼기일 뿐 판정에는 영향이 없다. 방안: [복구]·[버리기] 때 그 키의 `.tmp-*`를 지운다. 재현 `review_r3_c3_crash_left_generation_temps_survive_discard` | `crates/keyscore-app/src/autosave.rs:271-276`, `574-585` |
| 4 | 제안 | 6절 관찰: 복구 저장본의 메타 rename만 실패하면 `claim`이 옛 inode에 남는다. `claim`을 rename과 무관한 표지에 거는 방안 | `crates/keyscore-app/src/autosave.rs:288-296`, `555-557` |

리뷰 2 지적의 수정 확인

| 리뷰 2 | 확인 |
| --- | --- |
| 지적 1(끝내기 훅의 최신 저장본을 옛 쓰기가 덮음) | 고쳐짐. B1 통과, 시도 1~4로 훅·저장·닫기 순서를 넓혀도 회귀 없음. 임시 파일도 세대별로 갈라져 섞이지 않는다 |
| 제안 2(이어받기 실패 때 다른 실행이 가져감) | 고쳐짐(B2 통과, 시도 5·6). 시트가 이미 떠 있던 실행은 남는다(지적 2) |
| 제안 3(끝난 세션 잠금 파일이 쌓임) | 고쳐짐(B4 통과, 시도 7). 다만 막 생긴 세션을 지우는 회귀(지적 1) |
| 제안 4(리뷰 1 A1 시험, 계획 문구) | 계획 문구 고쳐짐(`plan.md:13`). 시험 대응은 받아들일 만하다 |
| 제안 5(메타만 실패한 첫 쓰기, 창이 사라진 뒤 쓰기) | 고쳐짐. 첫 쓰기는 `had_meta`로 내용을 지우고, 사라진 창의 쓰기는 닫기가 올린 바닥으로 버려진다(리뷰 2 시도 10, 이번 시도 4). 복구 저장본에서는 지적 4 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2의 지적 1과 제안 2·3·5는 고쳐졌고, 세대·바닥 규칙에서 빈틈을 찾지 못했다(시도 1~4). 남은 「수정」 1건(지적 1)은 이번 수정이 새로 들인 잠금 파일 정리가 막 뜬 다른 실행의 잠금 파일을 지우는 경합이다(종류 d, f). 고칠 곳은 `remove_dead_locks`의 지우기 조건(오래된 것만) 한 줄과 `init`의 잠금 확인으로 좁다. 재리뷰는 지적 1과 부록 C1에 한정해도 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(수정 커밋은 제품 코드만 바꿨고 새 시험이 없다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 A 3개 실패, 리뷰 테스트 7개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`df3d4b0..f6e74b9`: `autosave.rs`, `plan.md`, `report.md`)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 재현 테스트

`crates/keyscore-app/tests/review_s2_t7b_r3.rs` 끝에 그대로 붙여 넣으면 같은 파일의 도우미(`start`, `autosave_dir`, `autosaves`, `temps`, `plant`, `plant_dead`, `edited`, `answer`, `windows`, `dirty`, `launch_with_recovery`, `set_mode`)와 `use`로 컴파일된다. `cargo test -p keyscore-app --test review_s2_t7b_r3 review_r3_c`로 `f6e74b9`에서 3개 모두 실패하는 것을 확인했다. C1은 두 단언(복구 대상 수, 잠금 파일 존재)이 각각 실패하는 것을 확인했다. C1의 `c_lock`은 다른 실행 C의 `init`이 `File::create`와 `try_lock` 사이에 있는 상태를 흉내 낸다.

~~~rust
// ---------------------------------------------------------------- 부록 A: 깨뜨린 시도


/// C1. 막 뜬 다른 실행이 잠금 파일을 만들고 아직 잠그지 않은 틈에 이 실행이 [버리기]·[복구]를 마치면, 끝난 세션
/// 잠금 정리가 그 잠금 파일을 지운다. 그 실행은 잠금 파일 없이 살아, 그 뒤 뜬 실행이 그 실행의 저장본을
/// 끝난 세션 것으로 보고 복구 대상에 넣는다.
#[gpui_kit::test]
fn review_r3_c1_fresh_session_lock_removed_by_dead_lock_cleanup(cx: &mut TestAppContext) {
    let dir = start(cx, "fresh-lock");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Old"), None);
    launch_with_recovery(cx);
    // 다른 실행 C의 `init`: `File::create`까지 마치고 `try_lock` 전이다.
    let fresh = auto.join("session-fresh.lock");
    let c_lock = std::fs::File::create(&fresh).unwrap();
    answer(cx, "버리기");
    // C가 잠그고, C의 바뀐 창이 저장본을 쓴다.
    c_lock.try_lock().unwrap();
    plant(&auto, "c-window", &edited("Live"), None, "fresh");
    let lock_survived = fresh.exists();
    let later = cx.new_app();
    later.update(|cx| init(cx, dir.join("settings.json")));
    let found = later.read(autosave::find_recoverable).len();
    drop(c_lock);
    assert_eq!(found, 0, "살아 있는 실행 C의 저장본을 복구 대상으로 본다");
    assert!(lock_survived, "막 뜬 실행의 잠금 파일을 지웠다");
}

/// C2. 두 실행이 같은 끝난 저장본을 함께 제안받는다. A가 [복구]했지만 이 세션 것으로 다시 쓰지 못해(저장본
/// 폴더에 쓸 수 없음) 내용 파일 잠금만 쥔 채다. B의 [복구]는 `still_dead`만 보고 잠금을 보지 않아 같은 키로
/// 두 번째 복구 창을 연다.
#[gpui_kit::test]
fn review_r3_c2_claimed_autosave_recovered_again_by_open_sheet(cx: &mut TestAppContext) {
    let dir = start(cx, "claim-race");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("Claimed"), None);
    launch_with_recovery(cx);
    let mut b = cx.new_app();
    b.update(|cx| init(cx, dir.join("settings.json")));
    launch_with_recovery(&mut b);
    set_mode(&auto, 0o555);
    answer(cx, "복구");
    let a_held = windows(cx).iter().any(|w| dirty(cx, w));
    answer(&mut b, "복구");
    let b_dirty = windows(&mut b).iter().filter(|w| dirty(&mut b, w)).count();
    set_mode(&auto, 0o755);
    assert!(a_held, "A가 복구 창을 열지 않았다");
    assert_eq!(b_dirty, 0, "A가 잠근 저장본으로 B가 또 복구 창을 열었다");
}

/// C3. 쓰기 도중 끝난 세션이 남긴 세대별 임시 파일은 [버리기] 뒤에도 저장본 폴더에 남는다.
#[gpui_kit::test]
fn review_r3_c3_crash_left_generation_temps_survive_discard(cx: &mut TestAppContext) {
    start(cx, "crash-temps");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("T"), None);
    std::fs::write(auto.join("k.ksq.tmp-3"), "partial").unwrap();
    std::fs::write(auto.join("k.json.tmp-3"), "{").unwrap();
    launch_with_recovery(cx);
    answer(cx, "버리기");
    assert!(autosaves(cx).is_empty());
    assert!(temps(&auto).is_empty(), "남은 임시 파일: {:?}", temps(&auto));
}
~~~
