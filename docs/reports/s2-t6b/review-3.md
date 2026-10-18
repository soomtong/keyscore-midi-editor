# 적대적 리뷰: S2-T6b `?` 키맵 오버레이 (리뷰 3회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 에이전트 세션(구현자, 리뷰 1·2 리뷰어와 다른 세션)
- 대상 커밋: `d00f357`(브랜치 `s2/t6b-keymap-overlay`, 수정 커밋 `d445aff`, `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

리뷰 2 문서, 보고서의 「리뷰 2 대응」, `git show d445aff`의 `overlay.rs`만 읽고 테스트 파일을 열기 전에 적었다.

1. 인자 라벨 찾기(`overlay.rs:115-118`)는 키맵 원문의 실효 바인딩에서 이름과 키만 맞춰 첫 항목을 고른다. 컨텍스트를 보지 않는다. 같은 키·같은 이름이 다른 컨텍스트에서 다른 인자로 걸려 있으면, 엔진이 고른 바인딩이 아닌 다른 항목의 인자를 보일 것이다.
2. 라벨이 엔진과 맞는가를 기본·vim 프리셋, 수식키 교환 켬/끔, 모든 모드·보기·선택·고스트 경로에서 바인딩마다 대조한다(라벨의 인자로 Action을 다시 만들어 엔진의 승자와 `partial_eq`).
3. 사용자 키맵: 같은 자리를 다른 인자로 덮기, 시퀀스 키(`g 3`), `zed::Unbind`로 끄기, 수식키 교환 뒤의 키(`alt-f9` → `cmd-f9`). 키맵 편집기 경로(`UserKeymap::bind` → `save_user`)로 키 옮기기.
4. 라벨을 찾지 못하면 묶음 키가 Action 이름으로 돌아가 다시 한 줄로 뭉칠 수 있다(`map_or_else(|| name.to_owned(), …)`). 언제 못 찾는가.
5. 배경 `occlude`: 안쪽 클릭(줄)은 닫지 않고 포커스(Tab)가 남는가. 바깥 클릭은 닫기만 하고 그 뒤 문서 키가 사는가. 닫힌 뒤 가림이 남지 않는가. 스크롤은 아래로 새는가.
6. `overlay_lines`가 캐시를 돌려줄 때, 키맵이 바뀌고 아직 다시 그리지 않은 순간에도 낡지 않는가.
7. `delete` 기호(`key_element`)가 수식키 조합에서 어떻게 보이는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 컨텍스트가 다른 같은 키·같은 이름 | 없음 | 깨짐(부록 C1, C2). 손으로 쓴 파일 또는 드문 편집기 조작에서만 |
| 2. 전 프리셋·교환·경로 라벨 대조 | 일부(`open_argument_actions_keep_their_arguments`는 기본 Play에서 뭉침·`dyn Action` 여부만) | 추가, 통과 |
| 3. 사용자 덮기·시퀀스·Unbind·교환·편집기 | 없음 | 추가, 통과 |
| 4. 라벨 못 찾을 때 뭉침 | 없음 | 2·3의 대조에서 못 찾는 경우가 나오지 않았다 |
| 5. `occlude` 안쪽·바깥·닫힌 뒤 | 일부(리뷰 2 부록 B1 두 테스트가 바깥 클릭만) | 추가, 통과. 메인 창에 스크롤 요소가 없어 스크롤은 시험할 대상이 없다 |
| 6. 다시 그리기 전 `overlay_lines` | 없음 | 추가, 통과 |
| 7. `delete` 기호 | 없음 | 수식키 조합은 `Kbd` 그대로(`⌫`). 겉모양이라 지적하지 않음 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s2_t6b_r3.rs`에서 실행했다. 깨진 시도는 파일에서 빼고 부록에 남겼다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 기본·vim × 교환 켬/끔 × Normal(Arrangement·PianoRoll, 선택 켬/끔), Step(고스트 켬/끔), Sequence, Play, AI, Lyric, Controller에서 값 열린 인자 바인딩마다 라벨 인자로 만든 Action이 엔진 승자와 같은가. `dyn Action` 표기가 없는가 | 못 깨뜨림(Play `f1`–`f8`, vim Step `{`/`}`, vim Normal 숫자 등 모두 맞음) | `argument_labels_match_engine_across_presets_and_swap` |
| 2 | vim Step `{`/`}`가 방향(-1/1)으로 나뉘고, vim Normal 숫자가 9줄인가 | 못 깨뜨림 | `vim_octave_once_and_count_keep_direction` |
| 3 | 사용자 키맵 Play: `f1` → 5로 덮기, `f2` `zed::Unbind`, `g 3` → 3, `alt-f9` → 7, 이어서 수식키 교환 | 못 깨뜨림(`f1`은 5 줄에, `f2`는 사라짐, `g 3`은 3 줄에, 교환 뒤 `cmd-f9`는 7 줄에) | `argument_labels_follow_user_overrides` |
| 4 | 편집기 경로로 `play::SelectTrack`을 `f9`로, 교환 켠 채 `cmd-f10`으로 옮김 | 못 깨뜨림 | `argument_labels_follow_editor_rebind` |
| 5 | 열린 채 줄 클릭 → 열려 있음, Tab이 쪽을 넘김, `m`이 문서에 닿지 않음. 바깥(뮤트 버튼 자리) 클릭 → 닫힘, 뮤트 안 됨, 그 뒤 `m`은 뮤트 | 못 깨뜨림 | `inside_click_keeps_overlay_and_focus` |
| 6 | 열린 채 뮤트 버튼 자리에 hover, Esc로 닫은 뒤 그 버튼 클릭 | 못 깨뜨림(뮤트됨, 가림이 남지 않음) | `occlusion_does_not_outlive_overlay` |
| 7 | 키맵 다시 읽은 직후(다시 그리기 전) `overlay_lines` | 못 깨뜨림(판이 달라 새로 구함) | `overlay_lines_is_fresh_before_redraw` |
| 8 | 손으로 쓴 전역 `f1` → `play::SelectTrack(2)`. Play에서 엔진은 전역(2)을 고르는데 줄은 `f1 · … · 1` | 깨짐(부록 C1) | 부록 C1 |
| 9 | vim에서 편집기로 Normal의 `operator::Count`를 `2`로 옮김(파일: Normal `1` Unbind, `2` → Count(1)). Normal > Arrangement에서 엔진은 Count(1)을 고르는데 줄은 `2 · 카운트 · 2` | 깨짐(부록 C2) | 부록 C2 |

시도 8·9의 원인은 같다. 라벨 찾기가 컨텍스트를 보지 않고 실효 바인딩 목록에서 이름·키가 같은 첫 항목을 고른다(`overlay.rs:115-118`). 시도 9에서는 실효 목록의 첫 `2`가 `Normal > PianoRoll`의 Count(2)라서 그것이 뽑힌다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음, 노트는 클립 안, 틱 ≥ 0, 채널 1–16 | 아니오 | 이 Task는 모델을 건드리지 않는다 |
| 팝업은 하나만 띄운다 | 아니오 | 리뷰 2 시도 7 그대로 통과 |
| 오버레이가 열린 동안 문서는 모드 키를 먹지 않는다(계획 끝의 정의 4) | 아니오 | 키는 오버레이 포커스가, 마우스는 배경 `occlude`가 막는다(시도 5, 리뷰 2 부록 B1 테스트) |
| 오버레이는 지금 모드의 키를 보인다(정의 2) | 아니오 | 시도 1, 리뷰 2 테스트 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| UI-6 `?`로 현재 모드 키맵 겹쳐 표시 | 충족 | 기본·vim 프리셋과 편집기로 만든 사용자 키맵에서 줄·인자가 엔진과 맞다(시도 1-4). 열린 인자 Action은 인자마다 한 줄이고 `dyn Action` 표기가 없다(`overlay.rs:108-132`). 시도 8·9의 어긋남은 손으로 쓴 파일이나 드문 편집기 조작에서만 나와 제안으로 둔다 |
| KM-10 오버레이는 항상 현재 키맵 | 충족 | 판이 바뀌면 다시 구한다. 다시 그리기 전에도(시도 7) |
| 5.8 `?` 행, Tab 표의 다음/이전 쪽 | 충족 | `assets/keymaps/default.json`, 시도 5 |
| 5.8.1 vim `?` | 충족 | 리뷰 1 시도 1 그대로 통과 |
| 7.6 도움말 > 키맵 보기 | 충족 | `commands/table.rs`의 `help::KeymapOverlay` |
| 11절 컴포넌트 표 `Sheet` 또는 `Popover` + `Kbd` | 충족 | 리뷰 2 판단 그대로. `delete`만 `⌦` 글자로 그린다(`overlay.rs:161-166`) |
| A13 Esc = 통상의 취소 | 충족 | 시도 6 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. UI 스레드만 건드린다.
- 제품 코드의 `unwrap`/`expect`: 새 코드에 없음.
- 에러가 삼켜지는 곳: `KeyContext::parse(c).ok()`(경로가 `&'static` 상수라 실패하지 않는다). 라벨을 못 찾으면 인자 없이 명령 제목만 쓰는 것(`map_or_else`)은 의도한 대체이고 시도 1-4에서 일어나지 않았다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 인자 라벨 찾기가 컨텍스트를 보지 않는다. 같은 키·같은 이름이 함께 맞는 다른 컨텍스트에 다른 인자로 있으면 엔진이 고르지 않은 항목의 인자를 보인다. 손으로 쓴 전역 `f1` → `play::SelectTrack(2)`(Play에서는 2가 실행되는데 줄은 1), vim에서 편집기로 카운트 키 `1`을 `2`로 옮긴 경우(Count(1)이 실행되는데 줄은 2). 기본·vim 프리셋 그대로와 흔한 편집(시도 3·4)에서는 나오지 않는다. 키·이름 대신 「실효 항목의 `args`로 `cx.build_action(name, args)`를 만들어 엔진 Action과 `partial_eq`」로 찾으면 컨텍스트와 무관하게 정확하다. 회귀 테스트: 부록 C1, C2 | `crates/keyscore-app/src/overlay.rs:115-118` |

리뷰 2 지적 1(배경 너머 클릭)과 지적 2(열린 인자 뭉침·디버그 표기)는 뿌리에서 고쳐졌다. 제안 3(`overlay_lines` 캐시)과 제안 4(`delete` 기호)도 반영됐고 회귀를 찾지 못했다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

남은 것은 제안 하나다. 구현자가 받아들이면 부록 C1·C2를 저장소 테스트로 옮기고, 거절하면 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(diff에 `ignore` 없음. `cargo test -p keyscore-app` 전부 통과, `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` 통과)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 깨진 테스트와 실패 출력

`review_s2_t6b_r3.rs`의 도우미(`start`, `open`, `run`, `event`, `arg_mismatches`)를 그대로 쓴다. `arg_mismatches`는 지금 경로에서 엔진이 그 키에 실제로 거는 값 열린 인자 바인딩마다, 그 키를 가진 그 이름의 줄의 제목 인자로 Action을 다시 만들어 엔진 Action과 같은지 보고, 어긋난 것을 돌려준다.

### C1. 손으로 쓴 전역 바인딩이 Play 바인딩을 이길 때 (지적 1)

~~~rust
/// 손으로 쓴 전역 `f1` → `play::SelectTrack(2)`가 Play의 `f1`(1)과 겹칠 때, 라벨은 엔진이 실제로 고른 것과 같다.
#[gpui_kit::test]
fn argument_label_with_global_override(cx: &mut TestAppContext) {
    start(
        cx,
        "global",
        Some(r#"[{"bindings": {"f1": "play::SelectTrack(2)"}}]"#),
    );
    let (handle, view) = open(cx);
    run(cx, handle, mode::Play);
    run(cx, handle, help::KeymapOverlay);
    let bad = arg_mismatches(cx, &view);
    assert!(bad.is_empty(), "{bad:#?}");
}
~~~

~~~
thread 'argument_label_with_global_override' panicked at crates/keyscore-app/tests/review_s2_t6b_r3.rs:428:5:
[
    "[\"Play\"] f1 -> play::SelectTrack: 줄 [\"f2 · 트랙 1–8 직접 선택 · 2 (play::SelectTrack)\", …, \"f1 · 트랙 1–8 직접 선택 · 1 (play::SelectTrack)\"]",
]
~~~

### C2. 편집기로 vim 카운트 키를 옮긴 뒤 Normal 경로 (지적 1)

~~~rust
/// vim: 편집기로 Normal의 카운트 키 `1`을 `2`로 옮기면 Normal의 `2`는 Count(1), PianoRoll의 `2`는 그대로
/// Count(2)다. 두 경로의 라벨이 엔진이 고른 것과 같다.
#[gpui_kit::test]
fn argument_label_editor_rebind_shadowed_by_deeper_context(cx: &mut TestAppContext) {
    start(cx, "deeper", None);
    let (handle, view) = open(cx);
    cx.update(|cx| keymap::set_preset(cx, keymap::Preset::Vim));
    cx.run_until_parked();
    cx.update(|cx| {
        let (layers, user) = keymap::current_layers(cx);
        let mut user = user.unwrap();
        let eff = keymap::user::effective_with(&layers, false);
        user.bind("operator::Count", Some("Normal"), "2", &eff);
        keymap::save_user(cx, &user).unwrap();
    });
    cx.run_until_parked();
    render(cx, handle);
    run(cx, handle, help::KeymapOverlay);
    let normal = arg_mismatches(cx, &view);
    event(cx, handle, &view, ModeEvent::ToggleView);
    let roll = arg_mismatches(cx, &view);
    assert!(normal.is_empty() && roll.is_empty(), "{normal:#?} {roll:#?}");
}
~~~

~~~
thread 'argument_label_editor_rebind_shadowed_by_deeper_context' panicked at crates/keyscore-app/tests/review_s2_t6b_r3.rs:452:5:
[
    "[\"Normal\", \"Arrangement\"] 2 -> operator::Count: 줄 [\"3 · 카운트 · 3 (operator::Count)\", …, \"2 · 카운트 · 2 (operator::Count)\"]",
] []
~~~

편집기가 쓴 파일은 `Normal`: `"1": ["zed::Unbind", "operator::Count"]`, `"2": ["operator::Count", "1"]`이다. 실효 목록에서 `operator::Count`·`2`의 첫 항목이 `Normal > PianoRoll`의 Count(2)라서 Normal 경로 줄이 2를 보인다. PianoRoll 경로는 맞다.
