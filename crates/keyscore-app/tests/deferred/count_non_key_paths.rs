//! 미뤄 둔 수용 테스트(S2-T2 리뷰 3, 판정 「범위 재정의」).
//!
//! 키가 아닌 경로(메뉴, 툴바, 팔레트의 `window.dispatch_action`)와 카운트 접두의 의미는 KM-5가 정하지
//! 않았다. 결정 요청 `docs/decisions/2026-10-17-count-and-non-key-dispatch.md`가 닫히면, 그 결정을
//! 구현하는 Task가 아래 테스트를 결정에 맞게 고쳐 `tests/` 바로 아래로 옮겨 켠다.
//!
//! 이 폴더는 cargo가 테스트로 빌드하지 않는다(`tests/*.rs`만 빌드한다). 지우거나 `#[ignore]`하지 않고
//! 보존하려는 것이다. 도우미(`start`, `open_probe` 등)는 `tests/review_s2_t2_r3.rs`에 있다.

/// 리뷰 2 지적 2의 시나리오 그대로: `3` → 메뉴로 실행 취소 → 한참 뒤 키 Action. 키 Action이 3을
/// 받으면 지적이 고쳐지지 않은 것이다. 리뷰 2 테스트는 처리기 밖 `take_count`가 늘 `None`이 되는
/// 의미 변경만으로 통과한다.
#[gpui_kit::test]
fn count_typed_before_menu_action_does_not_reach_next_key_action(cx: &mut TestAppContext) {
    start_vim(cx, "menu-then-key", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3");
    menu(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(names(&seen), [("edit::Undo", None)], "메뉴 처리기(전제)");
    advance(cx, 5000);
    cx.simulate_keystrokes(handle, "g g");
    assert_eq!(
        names(&seen).last().copied(),
        Some(("cursor::ClipStart", None)),
        "메뉴 Action 앞에 친 카운트가 다음 키 Action에 붙었다"
    );
}

/// 시퀀스 대기 중(`3 g`)에 메뉴로 실행된 처리기도 카운트를 받지 않는다(보고서: 「메뉴·팔레트 처리기는
/// 받지 않는다」). 대기 키는 키 관찰자를 부르지 않아 `in_key_dispatch`가 참으로 남는다.
#[gpui_kit::test]
fn menu_action_during_pending_sequence_gets_no_count(cx: &mut TestAppContext) {
    start_vim(cx, "menu-pending", None);
    let (handle, seen) = open_probe(cx, &["Normal"]);
    cx.simulate_keystrokes(handle, "3 g");
    menu(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(
        names(&seen),
        [("edit::Undo", None)],
        "대기 중 메뉴 처리기가 카운트를 받았다"
    );
}
