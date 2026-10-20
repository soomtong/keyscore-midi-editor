# 적대적 리뷰: S2-T6f 이월 줄 정리 (리뷰 1회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 서브 에이전트 세션(적대적 리뷰어 역할)
- 대상 커밋: `ebc8fd0`(브랜치 `s2/t6f-carry-overs`, `git diff main...s2/t6f-carry-overs`)
- 추가한 테스트: `crates/keyscore-app/tests/review_s2_t6f.rs`(11개), `crates/keyscore-core/tests/review_s2_t6f.rs`(1개)

## 1. 코드만 읽고 적은 「내가 검사할 것」

제품 코드 diff(`history.rs`, `document.rs`, `window.rs`, `overlay.rs`, `ports.rs`, `lib.rs`, `settings_window.rs`)와 gpui-base 0.7.2 `resizable`을 먼저 읽고 적었다. 구현자 테스트는 diff에 함께 보여 훑었지만 아래 목록은 제품 코드에서 뽑았다.

1. `History::enforce_limit`만 `dropped`를 센다. apply·undo·redo 세 경로 모두 거기를 지나는가. 특히 undo가 redo 쪽을 키워(작은 역편집 → 큰 정편집) 버리는 경로에서 「넣은 단위 = undo + redo + 버린 수」가 정확한가. 코얼레싱으로 단위가 늘지 않고 커지기만 할 때도.
2. 상한 알림은 문서 observe 콜백에서만 난다. 편집·undo·redo가 모두 문서를 `notify`하는가. 편집 없이 undo로만 처음 버릴 때도 뜨는가. 알림 뒤에 같은 사이클에서 다른 메시지가 덮어 상태줄(마지막 메시지)에서 바로 사라지지 않는가. 창마다 한 번인가.
3. `observe_window_bounds`에서 크기 상태를 새로 만든다. 새 상태는 첫 프레임에 모든 패널 크기를 재고(`update_panel_size`가 자리표시 `PANEL_MIN_SIZE`를 실측으로 바꾸며 `size = Some`) 그룹 prepaint의 `adjust_to_container_size`가 합이 컨테이너보다 크면 비율로 줄인다. 저장된 사이드 패널 합이 창보다 크면 이어지는 조절에서 `record_sizes`가 두 사이드 패널 값을 다 쓰므로 조절하지 않은 패널의 저장 너비가 바뀔 수 있다.
4. 접힌 패널 + 창 크기 변경 + 펴기, 세로(아래 패널) 조절과 가로 조절의 교차, 두 창(창별 `layout`과 `merge_changes`).
5. 최소 크기 720×480에서 기본 세 패널(220, 260, 160)과 메인 영역이 실제로 들어가는가.
6. 오버레이 라벨: 키를 보지 않고 이름+`build_action`+`partial_eq`로 찾는다. 같은 값이 다른 표기(`2`, `"02"`)로 여러 키에 걸리면 줄이 갈라지거나 라벨이 틀리는가.
7. `windows_using`이 앱 포트 이름이면 빈 목록을 돌려 `Lost`·`Reconnect` 처리(`transport_port_lost`·`transport_port_returned`)까지 건너뛴다. 감시가 옛 목적지 이름을 기억하는 동안 앱 포트가 그 이름을 가져가는 경우(트랙 경로) 소리가 끊기거나 「다시 연결했다」가 잘못 나오는가.
8. 바탕 레이어 디버그 키 시험: 편집기·충돌 검사는 애초에 바탕 레이어를 담지 않아 단언이 항진명제다. 의미 있는 질문은 「충돌 없음」으로 보이는 `alt-cmd-i`를 사용자가 잡으면 실제로 사용자 Action이 이기는가다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. undo·redo 경로의 버린 수 정확성 | 부분 | `document_counts_dropped_undo_units`는 이름 바꾸기만 써서 역편집과 정편집 크기가 같다. undo·redo가 실제로 버리지 않고 단언도 `>=`다. undo 경로는 사실상 시험되지 않는다 |
| 2. undo로만 처음 버릴 때 알림, 마지막 메시지, 창마다 | 부분 | `undo_limit_notice_shows_once`는 편집으로 버린 뒤 undo·redo에서 다시 안 뜨는지만 본다 |
| 3. 저장 합이 창보다 클 때 조절 | 없음 | 부록 두 테스트는 기본 크기(합이 창보다 작다)만 본다 |
| 4. 접기·세로·두 창 | 없음 | |
| 5. 최소 크기 창의 실제 배치 | 없음 | `project_window_has_min_size`는 상수가 옵션에 들어갔는지만 본다 |
| 6. 같은 값 다른 표기 | 없음 | C1·C2는 다른 값·다른 컨텍스트 |
| 7. 앱 포트가 잃은 트랙 목적지 이름을 가져감 | 없음 | 대조군 기대값만 바꿨다(클럭 경로는 S2-T6e 리뷰 3에 있음) |
| 8. 디버그 키를 사용자가 잡았을 때 실제 디스패치 | 없음 | 있는 시험은 바탕 레이어에 키가 있음만 실질적으로 확인한다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 저장 너비 500·500, 창 1600 → 900으로 줄인 뒤 인스펙터만 450으로 조절. 트랙 목록 저장 너비가 500으로 남는가 | 깨짐. 창·설정 모두 `track_list_width: 350.0`(인스펙터 450) | `overflowing_panels_inspector_resize_keeps_track_list_width` |
| 2 | 같은 설정에서 창을 줄였을 때 인스펙터가 창 밖으로 밀려나는가 | 못 깨뜨림(비율로 줄어 창 안에 든다. 즉 그린 너비는 저장 너비와 다르다) | `shrinking_window_keeps_inspector_inside_window` |
| 3 | 트랙 목록 접기 → 창 크기 변경 → 인스펙터 조절 → 펴기. 저장 220으로 펴지는가 | 못 깨뜨림 | `collapsed_track_list_survives_window_resize` |
| 4 | 창 높이 변경 → 아래 패널 조절 → 창 크기 변경 → 트랙 목록 조절. 아래·인스펙터 저장값 유지 | 못 깨뜨림 | `vertical_then_horizontal_after_window_resize` |
| 5 | 두 창: B에서 트랙 목록 320, A 창 크기 변경 뒤 A 인스펙터 조절. 설정의 트랙 목록 320 유지, A의 창 레이아웃 220 유지 | 못 깨뜨림 | `window_resize_in_one_window_keeps_other_windows_width` |
| 6 | 최소 크기(720×480)에서 기본 세 패널이 저장 크기로, 메인 100 이상, 인스펙터가 창 안 | 못 깨뜨림 | `min_size_window_fits_default_panels` |
| 7 | 편집으로는 안 버리고(작은 이름 넷 + 3000자 이름) undo 한 번으로 처음 버림. 알림 1회, 마지막 메시지가 알림, 이어 redo·편집으로 더 버려도 1회 | 못 깨뜨림 | `undo_only_drop_notices_once` |
| 8 | 두 창이 각각 버림. 창마다 한 번씩, 다른 창에 새지 않음 | 못 깨뜨림 | `undo_limit_notice_is_per_window` |
| 9 | core: undo(정편집이 큼)·redo 반복 중 매 단계 `버린 수 + undo + redo = 넣은 수`, 코얼레싱 다섯 편집이 한 단위로 세어짐 | 못 깨뜨림 | `dropped_count_is_exact_on_undo_and_redo_paths`(core) |
| 10 | 사용자 키맵 `alt-cmd-i` → `track::ToggleMute`. 키를 치면 뮤트되고 오버레이에 그 줄이 보이는가(바탕 레이어 디버그 키가 가로채지 않는가) | 못 깨뜨림 | `user_binding_on_debug_key_wins_over_base_layer` |
| 11 | `f1: play::SelectTrack(2)`, `f2: ["play::SelectTrack", "02"]`. Play 오버레이에서 한 줄로 묶이고 두 키가 다 보이는가 | 못 깨뜨림 | `same_argument_different_spelling_groups_into_one_line` |
| 12 | (자식 프로세스) 트랙이 목적지 Y로 재생 중 Y 사라짐 → 앱 추가 포트 Z를 Y로 이름 바꿈 → 노트가 앱 포트 Y로 감 → 목적지 Y 돌아옴. 목적지로 노트 0, 앱 포트 노트 계속, 「다시 연결했다」 0, 「새로 보인다」 1 | 못 깨뜨림 | `ports_isolated::track_on_lost_destination_then_own_port_takes_name` |
| 13 | 바뀐 대조군과 S2-T6e 리뷰 3 포트 테스트 4개 회귀 | 못 깨뜨림(4 통과) | 기존 `review_s2_t6e_r3.rs` |

실행: `cargo test -p keyscore-app --test review_s2_t6f` → 10 통과, 1 실패(시도 1). `cargo test -p keyscore-core --test review_s2_t6f` → 1 통과. 두 파일 모두 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| core `Document` 공개 API로 히스토리를 바꿀 수 없다 | 아니오 | `dropped_undo_units`는 읽기만 한다(`&self`, `u64` 복사) |
| 버린 수 + undo + redo = 넣은 단위 | 아니오 | 시도 9 |
| 레이아웃 설정 값은 사람이 조절한 패널만 바뀐다 | 예 | 시도 1. 저장 합이 창보다 크면 인스펙터만 조절해도 트랙 목록 저장 너비가 바뀐다(`window.rs` `record_sizes` 855–886, `observe_window_bounds` 119–121) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-1(결정 A: 256 MiB, 처음 한 번 상태줄 알림) | 충족 | `crates/keyscore-core/src/history.rs:154-160`, `crates/keyscore-app/src/window.rs:308-322`, 시도 7·8·9 |
| 7.1(패널 구성, 창 크기 변경 때 사이드·아래 패널 크기 유지) | 부분 충족(미충족으로 셈) | 저장 합이 창보다 작을 때는 맞다(부록 두 테스트, 시도 3–6). 클 때는 조절하지 않은 패널 저장값이 바뀐다(시도 1) |
| UI-6 | 충족 | `crates/keyscore-app/src/overlay.rs:111-133`, C1·C2, 시도 11 |
| KM-10 | 충족 | 시도 10(사용자가 잡은 디버그 키가 오버레이에 보이고 실제로 동작) |
| OUT-3 | 충족 | `crates/keyscore-app/src/ports.rs:161-186`, 시도 12·13 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이번 변경은 UI 스레드와 core 히스토리뿐).
- 제품 코드의 `unwrap`/`expect`: 이번 diff에 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `overlay.rs`의 `build_action(...).is_ok_and(...)`는 만들 수 없는 항목을 건너뛴다. 엔진도 그 바인딩을 갖지 못하므로 정당하다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 저장된 사이드 패널 합(+ 메인 최소 100)이 창 너비보다 크면 창 크기 변경 뒤 새 크기 상태가 첫 프레임에 모든 패널을 재고(`update_panel_size`가 메인까지 `size = Some`) `adjust_to_container_size`가 비율로 줄인다. 이어 한 패널만 조절해도 `record_sizes`가 가로 그룹의 두 사이드 패널 값을 모두 쓰므로, 조절하지 않은 패널의 저장 너비가 줄어든 값으로 바뀐다. 재현: `overflowing_panels_inspector_resize_keeps_track_list_width` — 500·500 저장, 창 900, 인스펙터 450 조절 뒤 `track_list_width: 350.0`(기대 500). 계획의 「이어 한 패널을 조절해도 다른 패널의 저장 크기가 바뀌지 않는다」가 이 경우에 깨진다. 최소 크기 720에서도 사이드 패널은 각각 1200까지 저장되므로 넓은 창에서 늘린 뒤 창을 줄이면 흔히 닿는다. 계획 리스크 절의 대안(`record_sizes`가 끈 손잡이에 붙은 패널만 쓴다)이 이 경우도 막는다 | `crates/keyscore-app/src/window.rs:855-886`, `119-121` |
| 2 | 제안 | `document_counts_dropped_undo_units`는 이름 바꾸기만 써서 undo·redo가 실제로 버리지 않는다(정·역편집 크기가 같다). 「undo·redo로 버려도 센다」를 시험하지 못한다. 시도 9처럼 작은 역편집 뒤 큰 정편집을 되돌리는 경우를 넣는다 | `crates/keyscore-core/tests/undo_limit.rs:54-64` |
| 3 | 제안 | `base_debug_key_is_hidden_from_editor_overlay_and_menus`의 편집기·충돌·팔레트 단언은 그 목록이 바탕 레이어를 애초에 담지 않아 항상 참이다. 의미 있는 것은 「충돌 없음」인 그 키를 사용자가 잡으면 실제로 이긴다는 것이다. 시도 10 같은 디스패치 단언을 더한다 | `crates/keyscore-app/tests/carry_overs.rs:100-146` |
| 4 | 제안 | `project_window_has_min_size`의 뒤 두 단언은 상수를 자기 자신과 비교한다. 최소 크기 창에서 실제 배치를 보는 시도 6 같은 단언이 계획의 근거(세 패널 + 메인이 들어간다)를 지킨다 | `crates/keyscore-app/tests/carry_overs.rs:149-159` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)을 고치고 `overflowing_panels_inspector_resize_keeps_track_list_width`가 통과하면 재리뷰한다. 제안 2–4는 거절할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(구현자 테스트는 diff에 함께 보였으나 목록은 제품 코드에서 뽑았다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(대조군 하나의 기대값 변경은 계획 7대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
