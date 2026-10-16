# 적대적 리뷰: S2-T1 GPUI 앱과 창 모델 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다. 1·2회차 리뷰어와도 다른 세션이다.
- 대상 커밋: `c22abee` (리뷰 2 이후 `993b3ee..c22abee`)
- 범위: 마지막 확인 리뷰. A) 리뷰 2 지적 1이 근본에서 고쳐졌는가, B) 리뷰어 테스트 한 곳(인덱스 2 → 1) 수정이 구조 변경만 따르고 의도를 지키는가, C) 제안 2(UTF-8이 아닌 파일 `.bak`)와 회귀, fmt·clippy. 그 밖은 제안까지만.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 993b3ee..HEAD`의 `src/`와 gpui-base 0.7.2 `resizable/mod.rs`·`panel.rs`를 읽고, 이번 리뷰 테스트를 쓰기 전에 적었다.

1. `render`가 펴진 패널만 그룹 자식으로 넣으므로 gpui-base `sizes` 길이는 1 + 펴진 가로 패널 수다. `record_sizes`는 길이가 다르면 버리고, 트랙 목록은 `sizes[0]`, 인스펙터는 `sizes[len - 1]`로 읽는다. 트랙 목록만 접힌 경우([메인, 인스펙터])와 인스펙터만 접힌 경우([트랙 목록, 메인])에 매핑이 맞는가.
2. 접기·펴기마다 `fresh_states`로 두 그룹 상태를 새로 만든다. 옛 상태를 누가 쥐고 있다가 `Resized`를 내도 구독이 끊겨 쓰이지 않는가(옛 상태의 인덱스는 접힌 패널을 포함한다).
3. 새 상태는 `size: None`으로 시작해 `flex_basis(저장 크기)`로 첫 프레임을 그린 뒤 측정값을 받는다. 연속 토글 사이에 크기가 흘러가지 않는가(세 패널을 모두 접은 채 열고 여러 순서로 편다).
4. 두 창: 한 창의 토글이 상태를 새로 만들어도 다른 창의 상태·레이아웃은 그대로인가. 설정의 각 필드는 마지막 명시적 변경 값인가.
5. 조절은 그 패널 값만 바꾸는가. `record_sizes`는 펴진 가로 패널 둘을 함께 쓰므로, gpui-base가 형제 크기를 바꾼 경우(넘침 깎기, 창 크기 비율 조정) 조절하지 않은 패널 값이 저장될 수 있다.
6. 제안 2: `fs::read` 뒤 `String::from_utf8(..).unwrap_or_default()`는 빈 문자열이 되어 JSON 실패 분기로 가므로 `.bak`이 원본 바이트 그대로여야 한다. 원본 파일은 읽기에서 지워지지 않아야 한다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 접힌 패널 조합별 인덱스 매핑 | 부분 | 리뷰 2 테스트는 조합 몇 개만 고정 순서로 본다 |
| 2. 옛 상태의 이벤트 | 없음 | |
| 3. 모두 접힌 채 열고 여러 순서로 펴기 | 부분 | `two_collapsed_panels_reopen_one_by_one_at_their_widths`는 둘, 한 순서 |
| 4. 두 창 무작위 섞기 | 부분 | 리뷰 2는 끌기·토글 한 순서 |
| 5. 조절하지 않은 패널 값 보존 | 부분 | 창 크기 변경은 없다 |
| 6. UTF-8 아닌 파일 백업 바이트 | 있음 | `non_object_settings_files_are_backed_up`(리뷰 2). 바이트 동일과 원본 보존은 이번에 더 본다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `crates/keyscore-app/tests`의 `window.rs`·`review_s2_t1.rs`·`review_s2_t1_r2.rs`를 3회 실행 | 못 깨뜨림. 3회 모두 12·9·13 통과 | 없음(기존 테스트) |
| 2 | 시작 레이아웃(접힘 여부·크기)을 무작위로 두고 두 창에서 세 패널의 토글과 조절(트랙 목록·인스펙터 130–379, 하단 130–279)을 40단계 무작위로 섞는다. seed 12개. 매 단계 뒤 두 창 모두 「창 레이아웃 = 모델」, 펴진 패널의 그린 크기 = 저장 크기(±2 px), 설정 = 필드별 마지막 명시적 변경 | 못 깨뜨림. 같은 테스트를 `993b3ee`의 `src/`(수정 전)에 돌리면 seed 하나의 3단계에서 깨진다(트랙 목록 그린 300.5, 저장 140). 테스트가 원래 결함을 잡는다는 확인 | `random_collapse_resize_reopen_sequences_keep_drawn_and_stored_sizes` |
| 3 | 세 패널을 모두 접은 채(333·177·244) 열고 인스펙터 → 하단 → 트랙 목록 → 인스펙터 → 트랙 목록 → 인스펙터 → 트랙 목록 순으로 토글. 매번 그린 크기와 레이아웃 값 | 못 깨뜨림. 크기가 흐르지 않는다 | `all_collapsed_at_open_reopen_in_any_order_at_saved_sizes` |
| 4 | 트랙 목록을 접기 전에 가로 상태를 쥐어 두고, 접은 뒤 옛 상태에 인덱스 0·2 조절 | 못 깨뜨림. 레이아웃·설정 그대로(구독이 새 상태로 옮겨졌다) | `resize_on_stale_state_after_collapse_is_ignored` |
| 5 | UTF-8이 아닌 바이트(`{"\xff\xfe":1}\x80`)로 `Settings::load` | 못 깨뜨림. 기본값, 경고 있음, `.bak`이 원본 바이트와 같고 원본도 남는다. `993b3ee`에서는 실패(`.bak` 없음) | `non_utf8_settings_file_is_backed_up_byte_for_byte` |
| 6 | 범위 밖 확인: 창 크기를 1000×700으로 바꾼다 | 깨짐(제안). 트랙 목록 그린 너비 143, 저장 220. gpui-base `adjust_to_container_size`가 모든 패널을 비율로 줄인다. 레이아웃 값은 그대로다 | 저장소에 넣지 않음(부록) |
| 7 | 범위 밖 확인: 시도 6 뒤 인스펙터만 200으로 조절 | 깨짐(제안). 트랙 목록의 창 레이아웃·설정 값이 220 → 143.2로 바뀐다. `record_sizes`가 펴진 가로 패널 둘을 함께 쓰기 때문이다. 이 Task의 리뷰 2 수정 전부터 있던 동작이다 | 저장소에 넣지 않음(부록) |
| 8 | `cargo test --workspace --no-fail-fast`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check` | 못 깨뜨림. 861 통과(구현자 보고 857 + 이번 4), 실패 0. 나머지 게이트 통과 | 없음 |

시도 6·7은 지시된 범위(접기·조절·펴기 순서) 밖이고 제안이라, 게이트를 막지 않도록 실패 테스트를 저장소에 넣지 않았다. 본문은 부록에 둔다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 접기·조절·펴기 뒤 펴진 패널의 그린 크기 = 창 레이아웃 값(±2 px) | 아니오 | 시도 2, 3 |
| 저장 크기는 그 패널의 명시적 조절로만 바뀐다(접기·조절·펴기 순서 안에서) | 아니오 | 시도 2, 4 |
| 같은 불변식, 창 크기 변경을 섞을 때 | 예(제안) | 시도 6, 7 |
| 한 창의 토글·조절이 다른 창의 레이아웃을 바꾸지 않는다 | 아니오 | 시도 2 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.1 화면 구성(접히는 패널, 마지막 레이아웃 복원) | 충족 | `crates/keyscore-app/src/window.rs:122-175, 226-265`. 접기·펴기 순서에서 그린 크기와 저장 크기가 같다(시도 2, 3). 창 크기 변경 때의 비율 조정은 REQ가 정하지 않은 동작이라 제안으로 둔다 |
| UI-12 인스펙터 접기, `Cmd+I` | 충족 | 인스펙터를 접고 펴도 트랙 목록·인스펙터 크기가 그대로다(시도 2, 3, 리뷰 2 테스트) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(앱은 아직 MIDI를 쓰지 않는다).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. `settings.rs:148`의 `unwrap_or_default`는 UTF-8 실패를 빈 문자열로 바꿔 깨진 파일 분기로 보내는 의도된 처리다.
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. `record_sizes`가 크기 목록 길이가 다를 때 저장하지 않는 것은 주석대로다(시도 4에서 옛 상태 이벤트는 구독이 없어 여기까지 오지 않는다).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 창 크기를 바꾸면 gpui-base가 모든 패널을 비율로 늘리거나 줄여 그린 크기가 저장 크기와 어긋나고, 이어 한 패널을 조절하면 `record_sizes`가 다른 펴진 패널의 비율 조정된 값까지 저장한다(1000×700에서 트랙 목록 220 → 143.2). 사이드 패널에 `.flex_none()`을 주어 비율 조정 대신 메인이 늘고 줄게 하거나, `record_sizes`가 조절된 패널 값만 쓰도록(이벤트 전후 비교) 바꾸는 방법이 있다. 다음 Task로 미뤄도 된다. 테스트: 부록의 `window_resize_keeps_side_panels_at_stored_widths`, `inspector_resize_after_window_resize_keeps_track_list_width` | `crates/keyscore-app/src/window.rs:144-175` |

리뷰 2 지적 1은 근본에서 해결됐다. 접힌 패널은 그룹 자식에서 빠지고(`window.rs:226-265`), 접기·펴기 때 상태를 새로 만들며(`window.rs:129-137`), 크기는 펴진 패널 순서로 매핑된다(`window.rs:144-175`). 리뷰 2 제안 2도 보고서대로 동작한다.

리뷰어 테스트 변경(B): `track_list_collapsed_after_open_reopens_after_inspector_resize`의 인덱스 2 → 1은 트랙 목록이 접혀 자식이 [메인, 인스펙터]가 된 구조만 따른다. 인덱스 1은 마지막 패널이라 이전과 같은 「마지막 패널 조절」 경로를 탄다. 단언(트랙 목록 260, 인스펙터 330)은 그대로라 의도를 지킨다. `993b3ee..HEAD`에서 리뷰 테스트 파일 변경은 이 5줄뿐이다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

범위 A·B·C 모두 통과했다. 지적 1은 범위 밖 제안이며 구현자가 거절하거나 뒤 Task로 미룰 수 있다(보고서에 이유를 적는다).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(구현자 수정의 diff와 gpui-base 소스만 보고 적었다. 리뷰 2 테스트의 도우미 함수는 재사용을 위해 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(현재 코드와 `993b3ee` 두 곳에서)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`993b3ee..HEAD`는 `settings.rs`, `window.rs`, 리뷰 2 테스트 5줄, 보고서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음, `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다

자동 게이트: `cargo fmt --all -- --check` 통과, `cargo clippy --workspace --all-targets -- -D warnings` 통과(리뷰 테스트 포함), `cargo test --workspace --no-fail-fast` 861 통과·실패 0, `cargo deny check` 통과. 앱 테스트 4파일(`window.rs` 12, `review_s2_t1.rs` 9, `review_s2_t1_r2.rs` 13, `review_s2_t1_r3.rs` 4)을 3회 돌려 모두 통과.

## 부록: 지적 1의 재현 테스트

`crates/keyscore-app/tests/review_s2_t1_r3.rs`에 붙이고 `use`에 `size`를 더하면 돈다. 현재 코드에서 둘 다 실패한다.

~~~rust
/// 제안(범위 밖 확인): 창 크기를 바꾸면 gpui-base `adjust_to_container_size`가 모든 패널을 비율로
/// 늘리거나 줄인다. 레이아웃 값은 그대로라 그린 크기와 저장 크기가 어긋난다.
#[gpui_kit::test]
fn window_resize_keeps_side_panels_at_stored_widths(cx: &mut TestAppContext) {
    start_with(cx, "window-resize", |_| {});
    let (handle, view) = open(cx);
    let layout = cx.read(|cx| view.read(cx).layout());
    check_window(cx, handle, &view, layout, "창 크기 바꾸기 전");
    cx.simulate_window_resize(handle, size(px(1000.0), px(700.0)));
    draw(cx, handle);
    let after = cx.read(|cx| view.read(cx).layout());
    check_window(cx, handle, &view, after, "창 크기 바꾼 뒤");
}

/// 창 크기를 바꾼 뒤 인스펙터만 조절하면 트랙 목록의 저장 너비가 바뀌는가.
#[gpui_kit::test]
fn inspector_resize_after_window_resize_keeps_track_list_width(cx: &mut TestAppContext) {
    start_with(cx, "window-resize-then-inspector", |_| {});
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1000.0), px(700.0)));
    draw(cx, handle);
    resize(cx, handle, &view, false, 2, 200.0);
    let layout = cx.read(|cx| view.read(cx).layout());
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(
        close(layout.track_list_width, 220.0) && close(saved.track_list_width, 220.0),
        "창 {layout:?}, 설정 {saved:?}"
    );
}
~~~
