# 적대적 리뷰: S2-T7e S2-T7b·T7c·T7d 리뷰 이월 줄 정리 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 세션(리뷰 1회차와 같은 리뷰 전용 에이전트)
- 대상 커밋: `205a613`(`615fbb5..205a613`, 브랜치 `s2/t7e-carry-overs`). 리뷰 1 제안 6건의 대응만 보는 좁은 확인 리뷰다. 제품 코드 변경은 `6ee6df1`(`autosave.rs`), `a97f382`(`engine.rs`)뿐이고 `205a613`은 보고서·단계 파일이다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

(제품 코드 diff만 읽고 적었다. 구현자 시험 `generation_temps_of_session_dead_before_first_rename_are_removed`는 이 절을 쓴 뒤 열었다.)

1. 임시 파일 정리(`remove_dead_locks` 셋째 반복)가 살아 있는 창의 것을 지우는 경로: 첫 쓰기 중인 창(메타 없음, 임시 파일은 막 생김), 두 rename 사이의 창(메타 있음), 다시 쓰는 창(메타 있음). `old`(1분) 조건과 `<키>.json` 존재 조건이 각 경우를 막는가. 수정 시각이 미래이면(`elapsed` Err) 지우지 않는가.
2. 끝난 세션의 메타만 있는 키: 첫째 반복이 메타를 지운 뒤 셋째 반복이 같은 키의 임시 파일을 `exists()`로 다시 보므로 함께 지워진다. 살아 있는 세션의 메타만 있는 키는 메타가 남아 임시 파일도 남는가.
3. 메타·내용이 있는 복구 대상 키의 오래된 임시 파일은 시트 전에 지우지 않는가([버리기]·[복구]의 `remove_generation_temps`가 맡는다).
4. 키 파싱: `split_once(".ksq.tmp-")`·`".json.tmp-"`가 잠금 파일·메타·내용·다른 파일에 잘못 닿는가. 키(`<pid>-<nanos>-<n>`)에 점이 없어 첫 `split_once`가 키를 자르지 않는가.
5. 제안 6의 알림: `taken`은 `claim` 실패만 센다. `still_dead`가 거짓이라 건너뛴 것(다른 실행이 이미 복구해 다시 쓴 저장본)과 `open_project_window` 실패는 세지 않는다. 시트에 이름이 떴는데 창이 열리지 않는 같은 증상이 알림 없이 남는가. 알림은 `remove_dead_locks` 뒤·호스트 닫기 전에 모든 창에 가는가(호스트가 닫히면 복구 창이 받는다).
6. 제안 4의 `same_port_name`: 살아 있는 다른 포트(특히 대체 매핑 포트)의 표시를 지울 수 있는가. 포트 만들기·이름 바꾸기가 살아 있는 포트와 보이는 모양이 같은 이름을 거부하므로(`create_virtual_output`·`set_name`), `remember_user_port`·`rename_virtual_port`에 닿는 이름은 살아 있는 포트와 모양이 다르다. 규칙에 맞지 않는(손으로 고친) 남은 표시는 `same_port_name`이 거짓이라 남는다(전에는 정확히 같으면 지웠다). `add_mapped_virtual_port`가 `remember_user_port` 뒤에 표시를 더하므로 보이는 모양이 같은 옛 표시와 새 표시가 둘이 되지 않는가.
7. 제안 2·3·5의 거절·이월 이유가 사실과 맞는가(보고서 「알려진 한계」·「리뷰 1 대응」, 단계 파일 이월 줄).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 막 생긴 임시 파일 | 있음(구현자 시험의 `fresh.ksq.tmp-3`) | 리뷰 시험 `fresh_temps_without_meta_are_kept_until_old`가 늙은 뒤 지워지는 것까지 본다(통과). 실제 쓰기 중인 창은 `live_window_autosave_survives_other_runs_startup_cleanup`(통과). 미래 수정 시각은 코드 대조(`elapsed().ok()` → `old` 거짓) |
| 1·2 메타만 있는 키 | 있음(구현자 시험의 `kept`, 메타·내용 있음) | 메타만 있는 키는 없었다. 리뷰 시험 `old_temps_of_meta_only_key_follow_the_meta`(살아 있는 세션은 남고 끝난 세션은 메타와 함께 지워짐, 통과) |
| 3 복구 대상 키의 임시 파일 | 없음 | 리뷰 시험 `old_temps_of_recoverable_key_wait_for_the_sheet`(통과) |
| 4 키 파싱 | 없음 | 코드 대조: 잠금 파일·메타·내용에는 `.tmp-`가 없다. 키는 `new_key`(`autosave.rs:761-768`) 형식이라 점이 없다 |
| 5 알림 | 없음(구현자 시험 없음) | 리뷰 시험 `recover_reports_autosaves_claimed_by_another_run`, `recover_says_nothing_when_nothing_is_skipped`(통과). 이미 다시 쓴 저장본을 건너뛰는 경우는 부록 A(알림 없음) |
| 6 보이는 모양 비교 | 없음(구현자 C4는 같은 글자) | 리뷰 시험 `visually_same_stale_mark_is_cleared_on_add_and_rename`, `live_mapped_mark_survives_adding_and_renaming_other_ports`(자식 프로세스, 통과) |
| 7 거절·이월 이유 | 해당 없음 | 3절 끝에 적었다 |

## 3. 깨뜨리기 시도

리뷰 시험 파일: `crates/keyscore-app/tests/review_s2_t7e_r2.rs`(8개). `CARGO_TARGET_DIR=target/alt`로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 살아 있는 다른 실행의 메타만 있는 키 + 오래된 임시 파일 둘, 끝난 세션의 메타만 있는 키 + 오래된 임시 파일 둘을 한 폴더에 두고 시작 정리 | 못 깨뜨림(살아 있는 것은 메타·임시 파일 모두 남고, 끝난 것은 모두 지워짐) | `old_temps_of_meta_only_key_follow_the_meta` |
| 2 | 복구 대상 키(메타·내용 있음, 끝난 세션)의 오래된 임시 파일을 두고 시트까지, 그 뒤 [버리기] | 못 깨뜨림(시트 전에는 남고 [버리기]가 키째 지움) | `old_temps_of_recoverable_key_wait_for_the_sheet` |
| 3 | 창을 바꿔 30 s 뒤 실제 저장본을 만든 뒤 다른 실행의 시작 정리 | 못 깨뜨림(시트 없음, 파일 그대로, 임시 파일 없음) | `live_window_autosave_survives_other_runs_startup_cleanup` |
| 4 | 메타 없는 막 생긴 임시 파일 둘 → 시작 정리 → 늙힌 뒤 다시 정리 | 못 깨뜨림(막 생긴 것은 남고 늙으면 지워짐) | `fresh_temps_without_meta_are_kept_until_old` |
| 5 | 복구 대상 둘 가운데 하나를 다른 쪽이 잠근 채 [복구] | 못 깨뜨림(「1개는 여기서 열지 않았다」가 호스트 창에, 다른 하나는 열림) | `recover_reports_autosaves_claimed_by_another_run` |
| 6 | 건너뛴 것 없이 [복구] | 못 깨뜨림(알림 없음) | `recover_says_nothing_when_nothing_is_skipped` |
| 7 | 두 실행이 같은 시트를 받고 A가 먼저 [복구]해 다시 쓴 뒤 B가 [복구] | 깨짐: B는 창도 열지 않고 알리지도 않는다(`still_dead` 거짓 경로는 `taken`에 들지 않는다) | 부록 A(파일에 넣지 않음) |
| 8 | 남은 표시 셋(결합자를 끼운 X, 다른 이름, 결합자를 끼운 Z)을 설정에 두고 X를 만들고 X→Z로 바꿈 | 못 깨뜨림(X·Z 표시만 지워지고 다른 이름은 남음) | `visually_same_stale_mark_is_cleared_on_add_and_rename` |
| 9 | 살아 있는 대체 매핑 포트 D의 표시를 두고, 결합자를 끼운 D로 포트 만들기, 다른 포트를 결합자를 끼운 D로 바꾸기, 다른 이름의 포트 만들기 | 못 깨뜨림(앞 둘은 거부, 표시는 그대로, `virtual_ports`의 D는 하나) | `live_mapped_mark_survives_adding_and_renaming_other_ports` |

구현자 시험과 1회차 리뷰 시험도 다시 돌렸다: `t7e_autosave`(7), `review_s2_t7e`(8), `t7e_port_mapping`(4, 자식 프로세스), `review_s2_t7b_r4`(5) 통과. `cargo fmt --all -- --check`, `cargo clippy -p keyscore-app --all-targets -- -D warnings` 통과.

거절·이월 이유 확인

- 제안 2([복구]·[버리기] 경합의 남은 틈): 「수정 전부터 있던 틈, 방안이 rename 둘과 완전히 배타적이지 않다」는 1회차 리뷰의 분석과 같다. 거절 이유 타당. 단계 파일 이월 줄에 있다.
- 제안 3(`init` 재시도): 보고서 「알려진 한계」가 「닿는 경로가 지금 없다」로 고쳐졌고 코드는 방어로 남겼다. 해가 없으므로 타당. 다만 「리뷰어에게」 절의 「`init`의 재시도가 그 경우를 받는다」 문장은 고쳐지지 않아 「알려진 한계」와 어긋난다(7절 2).
- 제안 5(`virtual_ports`에 같은 이름 둘): 이번 Task 범위 밖이 맞다(`rename_virtual_port`의 이름 검사는 T7e의 변경이 아니다). 단계 파일 이월 줄에 있다. 타당.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 살아 있는 세션의 저장본·메타·잠금·임시 파일은 지우지 않는다 | 아니오 | 임시 파일은 `old`(1분, `autosave.rs:596-600`)와 메타 없음(`601`)이 모두 참일 때만. 첫 쓰기의 임시 파일은 1분을 넘기지 않고(쓰기 끝에 `write`가 지운다, `317-318`), 그 뒤 쓰기는 메타가 있다. 시도 1·3·4 |
| 복구 대상 키의 파일은 시트의 답 없이 지우지 않는다 | 아니오 | 메타가 있어 셋째 반복이 건너뛴다. 시도 2 |
| 대체 매핑 포트의 표시는 그 장치가 돌아오거나 사용자가 그 포트를 바꿀 때만 지운다 | 아니오 | 살아 있는 포트와 보이는 모양이 같은 이름은 만들거나 바꿀 수 없어 `same_port_name` 지우기가 다른 살아 있는 포트에 닿지 않는다. 시도 9 |
| 설정 `mapped_ports`에 같은 모양이 둘 생기지 않는다 | 아니오 | `add_mapped_virtual_port`가 `remember_user_port`(보이는 모양으로 지움) 뒤에 하나를 더한다(`engine.rs:273-281`, `309-313`) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-5(자동 저장·복구) | 충족 | `autosave.rs:583-604`(임시 파일 정리), `610`·`619`·`651-657`(건너뛴 복구 알림). 시도 1~6 |
| 8.4(복구 시트) | 충족 | 시트 흐름은 그대로. 건너뛴 수 알림이 더해졌다. 이미 다시 쓴 저장본은 7절 1 |
| OUT-8(대체 매핑) | 충족 | `engine.rs:276-279`, `424-426`. 시도 8·9 |

그 밖의 REQ(IO-2·IO-3·SEQ-9·OUT-3·OUT-5·MET-1)는 이 범위의 변경이 없다. 1회차 대조가 유효하다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음.
- 제품 코드의 `unwrap`/`expect`: diff에 없음.
- 에러가 삼켜지는 곳: `autosave.rs:602` `let _ = fs::remove_file(path)`(임시 파일 정리 실패는 다음 시작에 다시). `autosave.rs:614-617` `still_dead` 거짓 → `continue`(알림 없음, 7절 1).

## 7. 지적 사항

차단·수정 없음.

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 제안 6의 알림이 `claim` 실패만 센다. 다른 실행이 먼저 [복구]해 이미 다시 쓴 저장본(`still_dead` 거짓)도 시트에 이름이 떴는데 창이 열리지 않는 같은 증상인데 알림이 없다(손실은 없다, 그 실행이 열었다). `still_dead` 거짓 경로도 `taken`에 넣으면 된다(`open_project_window` 실패는 따로 세거나 그대로). 재현 부록 A | `autosave.rs:614-620`, `651-657` |
| 2 | 제안 | 보고서 「리뷰어에게」 절의 「`init`의 재시도가 그 경우를 받는다」가 「알려진 한계」의 고친 설명(닿는 경로가 지금 없다)과 어긋난다. 한 줄 고친다 | `docs/reports/s2-t7e/report.md:92` |

## 이월

판정에 넣지 않는다.

- 임시 파일 정리는 수정 시각 1분을 기준으로 한다. 살아 있는 창의 첫 쓰기가 1분 넘게 멈추면(`sync_all`이 멎은 볼륨) 다른 실행의 시작 정리가 그 임시 파일을 지워 rename이 실패하고 30 s 뒤 다시 쓴다. 손실은 없고 재현하지 못했다(원래 파일 옆 임시 파일의 `STALE_TEMP_AGE`와 같은 성질).

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

제안 1(임시 파일), 4(보이는 모양 비교), 6(건너뛴 복구 알림)이 고쳐졌고, 살아 있는 세션의 메타만 있는 키·쓰는 중인 창·복구 대상 키·살아 있는 다른 포트의 표시를 건드리는 회귀를 찾지 못했다. 제안 2·3·5의 거절·이월 이유는 1회차 분석과 맞다. 남은 것은 제안 둘(이미 다시 쓴 저장본을 건너뛸 때의 알림, 보고서 문장 하나)이며 받아들이거나 거절 이유를 적으면 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 diff만 읽고 적었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(리뷰 시험 8개 통과, 부록 A 1개 실패를 임시 파일로 확인)
- [x] REQ ID를 하나씩 대조했다(이 범위의 IO-5·8.4·OUT-8)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`615fbb5..205a613`: `autosave.rs`, `engine.rs`, `t7e_autosave.rs`, 보고서, 단계 파일 이월 줄)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
- [x] 리뷰 시험 파일이 `cargo fmt --all -- --check`와 `cargo clippy -p keyscore-app --all-targets -- -D warnings`를 통과한다

## 부록 A. 실패하는 재현 테스트(7절 1)

`crates/keyscore-app/tests/review_s2_t7e_r2.rs` 끝에 그대로 붙이면 같은 파일의 도우미(`start`, `autosave_dir`, `plant_dead`, `edited`, `launch_with_recovery`, `answer`, `messages`, `init`)로 컴파일된다. `205a613`에서 실패한다(B의 호스트 창 메시지 `[]`).

~~~rust
/// 두 실행이 같은 시트를 받고 A가 먼저 [복구]해 이 세션 것으로 다시 썼다. B의 [복구]는 `still_dead`가 거짓이라
/// 건너뛰는데, 시트에 이름이 떴던 프로젝트가 열리지 않은 이유를 알리지 않는다.
#[gpui_kit::test]
fn recover_reports_autosaves_already_taken_by_another_run(cx: &mut TestAppContext) {
    let dir = start(cx, "notify-taken");
    let auto = autosave_dir(cx);
    plant_dead(&auto, "k", &edited("K"));
    launch_with_recovery(cx);
    let mut b = cx.new_app();
    b.update(|cx| init(cx, dir.join("settings.json")));
    let (_, b_host) = launch_with_recovery(&mut b);
    answer(cx, "복구");
    answer(&mut b, "복구");
    let said = messages(&mut b, &b_host);
    assert!(
        said.iter().any(|m| m.contains("열지 않았다")),
        "이미 다른 실행이 복구한 저장본을 건너뛰고 알리지 않았다: {said:?}"
    );
}
~~~
