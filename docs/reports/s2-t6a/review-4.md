# 적대적 리뷰: S2-T6a 설정 창·키맵 편집기 (리뷰 4회차, 좁은 확인 리뷰)

- 리뷰어 세션: 구현자와 다른 새 세션(구현 컨텍스트를 물려받지 않음). 결정 `docs/decisions/2026-10-18-s2-t6a-review-limit.md` (A)에 따라 리뷰 3 지적 1의 수정만 확인하는 좁은 리뷰다. 그 밖의 범위는 보지 않았다.
- 대상 커밋: d313366 (`s2/t6-settings-keymap`, 현재 HEAD 2fabddc)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show d313366`, `keymap::set_swap_alt_cmd`, `UserKeymap::retarget_standard_unbinds`, `save_user`, `current_layers`만 읽고 테스트를 열기 전에 적었다.

1. 교환 꺼짐에서 `edit::Undo`(cmd-z), `app::Quit`(cmd-q), `settings::Open`(cmd-,)을 지우고 교환을 켜기·끄기·켜기로 여러 번 바꿔도 목록·엔진 디스패치·메뉴 키가 계속 비어 있는가. cmd 키와 그 교환 키(alt) 둘 다에서.
2. 교환 켜짐에서 지운 뒤 끄기·켜기. 경로는 `set_swap_alt_cmd` 직접, `keymap::SwapModifiers` 액션, 설정 창 키 alt-m/cmd-m.
3. `r`(복원)이 두 교환 상태 어디서나 표준 단축키를 되돌리고 파일에서 `Unbind`가 사라지는가.
4. 프리셋 지정 해제와 사용자 바인딩은 `retarget`가 건드리지 않는가(표준 레이어에 같은 키·같은 Action이 있을 때만 고친다). 프리셋과 표준이 같은 키·같은 Action을 가진 경우의 모호성은 현재 기본 키맵에는 없다고 읽었다.
5. 깨진 파일이면 `current_layers`가 `Err`를 주어 `if let Ok` 분기를 건너뛰고 `apply`만 하는가. 파일을 쓰지 않는가.
6. 손으로 쓴 유효한 파일: 표준 지정 해제가 없으면 `retarget`가 `false`라 쓰지 않는가. 있으면 `save_user`가 내용 기준으로 `.bak`을 남기고, 다음 토글(편집기가 쓴 내용)은 `.bak`을 덮지 않는가.
7. 토글이 파일을 쓴 뒤 핫리로드가 같은 파일을 다시 읽어도 `keymap::errors`에 오류가 생기지 않는가. `AppSettings::update`가 `save_user` 전에 호출되어 순서 문제가 없는가.
8. `retarget`의 `old == new` 단축, 인자가 문자열이 아닌 `Unbind`(건너뜀), 교환 켜짐 원래 키 `swap_alt_cmd` 복원 후 재교환이 `canonical_keys`로 일관되는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 표준 3종 반복 토글 | 부분 | d313366의 `review_s2_t6a_r3.rs` 테스트는 `edit::Undo`, `settings::Open` 한 번씩 한 방향만 |
| 2 액션·키 경로 | 없음 | `set_swap_alt_cmd` 직접 호출만 있었다 |
| 3 토글 뒤 `r` 복원 | 부분 | 교환 켜짐 상태의 복원은 있으나 토글 이력 뒤 복원은 없음 |
| 4 프리셋·사용자 바인딩 | 부분 | `swap_toggle_keeps_preset_unbind`(프리셋만) |
| 5 깨진 파일 토글 | 없음 | |
| 6 `.bak` 내용 기준 + 토글 | 없음 | |
| 7 오류 항목 | 없음 | |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s2_t6a_r4.rs`에 있고 통과한다. 실패해서 부록으로 옮긴 시도는 없다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 교환 꺼짐에서 표준 3종을 지우고 켜기·끄기·켜기·끄기·켜기. 매 단계 목록, 메뉴, cmd 키와 alt 키 디스패치, `keymap::errors` 확인. 이후 `r`로 3종 복원, 교환 끈 뒤 메뉴 키 확인, 파일에 `Unbind` 없음 | 못 깨뜨림 | `swap_toggles_keep_standard_cleared_from_off` |
| 2 | 교환 켜짐에서 3종을 지우고 `keymap::SwapModifiers` 액션을 4번, 설정 창 alt-m/cmd-m 키로 1번 토글(교환 상태가 실제로 바뀌는지 단언). 매 단계 같은 확인, 이후 `r` 복원 | 못 깨뜨림 | `swap_toggles_keep_standard_cleared_from_on_via_action` |
| 3 | 프리셋 `mode::Play` 지움, `edit::Undo` 지움, 사용자 `alt-o`를 `track::ToggleSolo`에 건 뒤 4번 토글. 지운 항목 유지, 편집기 모델과 엔진 불일치 0, 사용자 바인딩 1개 유지, Undo 디스패치 없음 | 못 깨뜨림 | `swap_toggle_keeps_preset_unbind_and_user_bindings` |
| 4 | 문법이 깨진 `keymap.json`으로 시작해 3번 토글. 파일 바이트 동일, `.bak` 없음 | 못 깨뜨림 | `swap_toggle_does_not_touch_broken_file` |
| 5 | 손으로 쓴 유효한 파일(주석, 표준 지정 해제 없음): 토글해도 파일 그대로, `.bak` 없음. 표준 지정 해제가 있는 손글씨 파일: 토글이 다시 쓰고 원본을 `.bak`으로 남김, 두 번째 토글은 `.bak`을 덮지 않음, 엔진에서 cmd-z·alt-z 모두 Undo 아님 | 못 깨뜨림 | `swap_toggle_bak_policy_is_content_based` |

실행: `cargo test -p keyscore-app --test review_s2_t6a_r4` 5개 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 지운 표준 단축키는 교환 토글로 되살아나지 않는다 | 아니오 | 시도 1, 2 |
| 깨진 `keymap.json`은 편집 동작이 덮어쓰지 않는다 | 아니오 | 시도 4 |
| `.bak`은 사람이 쓴 내용만 | 아니오 | 시도 5 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| KM-11 (수식키 교환) | 충족 | `keymap/mod.rs` `set_swap_alt_cmd`, 시도 1~3 |
| KM-12 (사용자 지정·해제의 보존) | 충족 | `keymap/user.rs` `retarget_standard_unbinds`, 시도 1~5 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(UI 스레드 코드).
- 제품 코드의 `unwrap`/`expect`: 이 수정에는 없음.
- 에러가 삼켜지는 곳: `set_swap_alt_cmd`에서 `save_user`가 `Err`이면(디스크 쓰기 실패) 조용히 `apply`로 떨어진다. 이 경우 지운 표준 단축키가 되살아날 수 있으나 쓰기 실패라는 외부 조건이며 `save_user`를 쓰는 다른 편집과 같은 처리다. 제안 1 참고.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `save_user` 실패 시 알림 없이 `apply`로 넘어간다. 디스크 쓰기 실패 같은 드문 경우에만 해당하고 `main` 대비 회귀가 아니다. 거절 가능 | `crates/keyscore-app/src/keymap/mod.rs` `set_swap_alt_cmd` |
| 2 | 제안 | `retarget`는 표준 레이어와 같은 키·같은 Action 이름이 프리셋에도 있으면 프리셋 지정 해제까지 표준으로 취급한다. 현재 기본 키맵에 그런 쌍이 없어 도달하지 못한다. 키맵 데이터가 바뀌면 불변식 테스트로 막을 것 | `crates/keyscore-app/src/keymap/user.rs` `retarget_standard_unbinds` |

수정에 해당하는 지적은 없다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다(좁은 범위의 KM-11, KM-12)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`src/`, `assets/` 미수정)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
