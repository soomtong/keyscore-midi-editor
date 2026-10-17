# Task 보고: S2-T2 키맵 엔진

- PR: 브랜치 `s2/t2-keymap`
- 계획 파일: `docs/reports/s2-t2/plan.md`
- 구현: 주 세션(엔진). Action 정의, 프리셋 JSON 생성, 충돌 결정 요청 초안은 경량 모델 서브 에이전트가 만들고 주 세션이 연결·검토했다.
- 계획 대비 달라진 점
  - 테스트 먼저 규칙에서 벗어났다. 엔진 초안(`keymap/mod.rs`)을 테스트와 거의 같이 썼고, 실패 테스트만의 커밋이 없다. 키맵 파일 읽기는 테스트를 먼저 돌려 실패(열 번호, 오류 순서)를 보고 고쳤다.
  - Action은 285 + 2 = 287개다. 카탈로그에 없던 `view::ToggleTrackList`, `view::ToggleBottomPanel`(S2-T1의 패널 토글)을 더했다.
  - S2-T1 Action 이름을 카탈로그 이름으로 바꿨다.
    - `NewWindow` → `file::New`, `CloseWindow` → `window::Close`, `Quit` → `app::Quit`, `ToggleInspector` → `view::ToggleInspector`
    - 테마 셋 → `view::SetTheme(ThemeKind)`
    - 동작은 같고, 리뷰어 테스트에서도 이름만 바꿨다.
  - 「프리셋은 Cmd 키 불변」을 「프리셋은 표준 레이어(`standard.json`)의 키를 다시 정의할 수 없다」로 좁혔다. default 프리셋은 표준 밖의 `Cmd` 조합(`cmd-j` 합치기, `cmd-i` 인스펙터, `cmd-←` 등, 5.7·5.8)을 REQ대로 쓴다.
  - vim 프리셋은 별도 레이어 `PresetOverlay`다. default 위의 덮어쓰기가 충돌로 잡히지 않게 한다.
  - 카운트 접두는 계획의 `count::Digit` 대신 카탈로그의 `operator::Count(n)`를 쓴다.
  - 키맵 설정(`keymap.preset`, `keymap.swap_alt_cmd`)은 `settings.json`의 `keymap` 객체에 두고 필드 단위로 읽는다(모르는 키 보존).
  - 의존: `schemars = "1"`(GPUI가 이미 쓰는 판), 개발 의존 `insta`

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `src/actions.rs` | 네임스페이스 모듈 26개, Action 287개, 인자 타입(`LengthCode`, `ThemeKind`, `Int` 등), `ALL_ACTION_NAMES` |
| `src/keymap/source.rs` | 위치를 지키는 JSONC 읽기(주석, 끝 쉼표, BOM, 깊이 상한 64). 바인딩·`ActionRef`(`ns::Name`, `ns::Name(arg)`, `["ns::Name", 인자]`, `null`)·오류(줄·열) |
| `src/keymap/conflict.rs` | 같은 레이어·같은 컨텍스트의 같은 키(다른 Action)와 접두 충돌 |
| `src/keymap/mod.rs` | 레이어(표준 → 기본 프리셋 → vim → 사용자), 표준 키 보호, 수식키 교환(KM-11), `build`(GPUI `KeyBinding::load`, 컨텍스트 술어, `NoAction`), 설치, 1 s 폴링 핫리로드, 다시 걸 때 메뉴 다시 걸기, 프리셋 전환, 카운트 |
| `assets/keymaps/standard.json` | 표준 바인딩 24개 |
| `assets/keymaps/default.json` | 기본 프리셋 362개 |
| `assets/keymaps/vim.json` | vim 157개와 `null` 18개 |
| `docs/decisions/2026-10-17-keymap-conflicts.md` | REQ 안의 키 충돌 13건(A1–A13)과 키 표기 모호(B24·B25) 결정 요청. 충돌 키는 프리셋에서 비워 둠 |

## 테스트 결과

~~~
(게이트 실행 결과를 아래 「자동 게이트」에 적는다)
~~~

| 계획 # | 실제 이름 | 상태 |
| --- | --- | --- |
| 1 | `req_example_keymap_snapshot` | 통과(REQ 7.3 예시 21개 바인딩, 오류 없음) |
| 2 | `parse_errors_report_line_and_column` | 통과 |
| 추가 | `action_strings_accept_both_argument_forms` | 통과 |
| 3 | `same_physical_key_fires_same_action_in_korean_input` | 통과(`key: h`, `key_char: ㅗ`) |
| 4 | `conflicts_and_prefix_conflicts_are_detected` | 통과 |
| 5 | `more_specific_context_wins` | 통과 |
| 6 | `preset_switch_keeps_user_overrides` | 통과 |
| 7 | `cmd_shortcuts_are_preset_invariant` | 통과 |
| 8 | `multi_key_sequence_and_count_prefix` | 통과 |
| 9 | `user_keymap_hot_reloads_and_reports_errors` | 통과 |
| 10 | `swap_alt_cmd_option_rewrites_preset_bindings` | 통과 |
| 11 | `presets_have_no_conflicts_and_only_known_actions` | 통과(vim의 `y`/`y y`/`y y p` 접두만 허용) |
| 12 | `every_catalog_action_is_registered` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (874 passed)
- [x] deny
- [x] check-deps

## 알려진 한계

- 시퀀스 대기는 1 s 고정이다(결정 `2026-10-17-key-sequence-timeout.md` A).
- KM-7은 GPUI의 키 변환에 기댄다. 실제 한글 입력기에서의 확인은 S2 출구 수동 항목이다. 테스트는 GPUI가 만드는 키 입력 모양(`key`는 자판 글자, `key_char`는 입력 글자)을 흉내 낸다.
- 키 충돌 13건은 결정 대기라 그 키는 프리셋에서 비어 있다.
- `play::Sustain`(`Shift` 단독, 5.4)은 GPUI가 수식키 단독 바인딩을 받지 않아 바인딩하지 않았다(B24). Play 모드(S5)에서 키 이벤트로 직접 다룬다.
- 키맵 오류의 상태줄 표시는 S2-T5다. 지금은 `keymap::errors`로만 읽는다.
- 대부분의 Action은 처리기가 없어 아무 일도 하지 않는다. 각 단계가 처리기를 단다.
- 사용자 파일은 1초마다 수정 시각·크기로 살핀다. 1초 안에 같은 크기로 두 번 바뀌면 놓칠 수 있다.

## 리뷰어에게

스스로 불안한 곳:

1. 레이어 순서와 GPUI의 「나중 바인딩 우선」이 실제로 맞는지. 같은 깊이의 컨텍스트에서 사용자 → vim → 기본 → 표준 순으로 이기는지.
2. `build`가 `NoAction`으로 지우는 방식. GPUI는 같은 (컨텍스트, 키)의 앞 바인딩만 가린다. 컨텍스트 문자열이 다르면 지우지 못한다.
3. 프리셋 JSON의 바인딩 543개를 사람이 다 읽지 않았다. 카탈로그 대조는 생성 에이전트가 했다. 테스트는 「오류 없음, 같은 키 충돌 없음, 모든 Action 등록」까지만 본다.
4. JSONC 읽기의 `\u` 대리 쌍은 대체 문자로 바꾼다.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

커밋 `app: Fix keymap shadowing, shifted digits, counts and layer details`.

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | vim의 깊은 컨텍스트 `null`이 GPUI `NoAction` 규칙으로 vim 자신의 키를 가림 | 그 16개를 vim의 `Normal` 바인딩으로 직접 건다. 남은 `null`은 같은 깊이의 `m`·`z`뿐이다. `build`의 주석을 GPUI 동작대로 고쳤다 |
| 2 | macOS의 Shift+숫자는 `key: "#"`라 `shift-N` 점음표가 안 맞음 | `shift-N`과 기호 키 양쪽에 건다. 결정 요청 B25에 확인된 사실과 a′를 덧붙였다 |
| 3 | 표준 키 덮어쓰기 경고 없음(KM-12) | 사용자 레이어가 표준 키를 덮으면 경고를 오류 목록에 남긴다(덮어쓰기는 허용) |
| 4 | 다른 Action 뒤에도 카운트가 남음 | `observe_keystrokes`로 숫자 아닌 Action이 키로 실행되면 비운다 |
| 5 | 컨텍스트 공백 차이로 충돌을 놓침 | 비교할 때 공백을 지운다. 키도 GPUI 키 입력으로 정규화해 비교한다 |
| 6 | 프리셋·수식키 교환이 저장되지 않음 | 설정에 저장한다 |
| 7 | 잘못된 context 절이 전역이 됨 | 그 절을 건너뛴다 |
| 8 | 수식키 별칭(`super`, `CMD`)을 바꾸지 않음 | `Keystroke::parse`로 파싱해 바꾼다. 표준 키 비교도 정규화한다 |
| 9 | 앱에 `key_context`가 없어 컨텍스트 바인딩이 닿지 않음 | 프로젝트 창 루트에 `Normal`을 둔다. 모드·뷰 컨텍스트는 S2-T3 |
| 10 | 우선순위 예외 문서화 (제안) | 반영(`keymap/mod.rs` 머리 주석) |
| 11 | 중복 필드, 읽기 실패를 「없음」으로 처리 (제안) | 반영 |
| 12 | 카운트 `0`, 창별 카운트 (제안) | 결정 요청 A9(vim 숫자 키)와 함께 다룬다. 지금은 전역 카운트, `1`–`9` |
| 13 | 문법 오류 하나로 사용자 레이어 전체가 빠짐 (제안) | 반영. 마지막으로 읽힌 사용자 레이어를 유지한다 |

테스트를 둘 더했다: `syntax_error_in_user_keymap_keeps_last_good_layer`, `duplicate_fields_in_a_section_are_reported`. 리뷰어 테스트 26개를 포함해 앱 테스트가 모두 통과한다. 워크스페이스는 901개 통과, 1개 실패였다. 실패한 것은 S1-T4의 20초 지터 측정(`playback_jitter_p99_within_half_ms_short`, p99 0.517 ms)이고, 다시 돌리면 통과한다. 이 브랜치는 MIDI 코드에 닿지 않는다. 같은 바이너리 안의 루프백 테스트가 병렬로 도는 부하로 보이며, 직렬화는 후속 과제로 남긴다.

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

커밋 `app: Order sequences after prefixes and tie counts to key dispatch`.

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | vim `y`가 파일에서 `y y`·`y y p`보다 뒤라 시퀀스에 닿지 않음 | `build`가 같은 레이어 안에서 짧은 시퀀스를 먼저 건다. 파일 순서에 기대지 않는다 |
| 2 | 메뉴로 실행된 Action 뒤에도 카운트가 남음 | GPUI에 모든 디스패치를 보는 훅이 없다. `take_count`는 키 입력으로 시작된 디스패치 안에서만 카운트를 준다. 메뉴·팔레트 처리기는 받지 않는다 |
| 3 | 바인딩 없는 키(`escape`) 뒤에 카운트가 남음 | 키 입력이 끝났는데 카운트 숫자도, 대기 중인 시퀀스도 아니면 비운다 |
| 4 | 충돌 비교가 키를 정규화하지 않음(지난 수정이 적용되지 않았다) | `canonical_keys`로 묶는다. 이후 편집 스크립트는 모든 치환을 검사한다 |
| 5 | 수식키 교환 뒤 표준 키 덮어쓰기 경고 없음 | `build`가 교환한 키로도 본다 |
| 6 | 자기 바인딩 없는 접두는 GPUI가 끝없이 기다림 (제안) | 한계로 남긴다. GPUI는 대기할 짧은 바인딩이 없으면 타이머를 걸지 않는다. 결정 A의 1 s 대기는 짧은 바인딩이 있을 때만 적용된다 |

남는 한계: 「숫자 → 마우스로 메뉴 막대 실행 → 키 Action」 순서면 카운트가 그 키 Action에 붙는다. 메뉴 막대의 디스패치를 잡을 수 없기 때문이다.

리뷰어 테스트 변경: `review_s2_t2.rs`의 카운트 테스트 다섯 곳은 처리기 밖에서 `take_count`로 쌓인 값을 읽었다. 새 의미에서는 `None`이라, 같은 기대값을 `drain_count`(`pending_count` 뒤 `clear_count`)로 읽게 바꿨다. 내 테스트도 같다. 앱 테스트 3회 통과, 워크스페이스 925개 통과.
