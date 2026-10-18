# Task 보고: S2-T6b `?` 키맵 오버레이

- PR: 브랜치 `s2/t6b-keymap-overlay`
- 계획 파일: docs/reports/s2-t6b/plan.md
- 계획 대비 달라진 점: 없음. 새 Action `help::CloseOverlay`(A13)와 기본 프리셋 `Overlay` 컨텍스트의 `escape` 바인딩을 더했다.

## 구현 요약

- `src/overlay.rs`(새 파일)
  - `lines_for(경로)`: 엔진에 걸린 바인딩을 하나씩 본다. 다음 셋을 모두 만족하는 것만 넣는다(KM-10).
    - 지금 모드 키 컨텍스트 경로에서 맞는다(`depth_of`).
    - GPUI가 그 키에 고르는 첫 바인딩(`bindings_for_input`)이 그것이다. 사용자 키맵·지정 해제·수식키 교환이 그대로 반영된다.
    - 명령 표에 있다(gpui-component 바탕 레이어는 뺀다).
  - 같은 Action의 키는 한 줄에 모으고, 줄은 명령 표 순서로 둔다.
  - `help_keymap_overlay`: 열기/닫기를 오간다. 열 때 팔레트·팝업을 닫고 오버레이로 포커스를 옮긴다.
  - `render_overlay`: 창 위 반투명 배경 + 가운데 패널(컨텍스트 `Overlay`, 포커스 보유).
    - 한 쪽 24줄이고 `help::NextPage`/`PrevPage`로 넘기며 끝에서 멈춘다.
    - `help::CloseOverlay`(`Esc`)나 바깥 클릭으로 닫으면 문서로 포커스가 돌아간다.
    - 그릴 때마다 줄을 다시 구하므로 열린 동안 키맵이 바뀌어도 따라간다.
- `src/window.rs`: `overlay` 필드, 처리기 연결, 루트의 자식으로 그리기(모드 키 컨텍스트 겹 바깥). 팝업을 열면 오버레이를 닫는다.

## 테스트 결과

~~~
cargo test --workspace: 1405 passed, 0 failed, 0 ignored
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `question_mark_opens_overlay_with_mode_keys` | 통과 |
| 2 | `overlay_pages_with_tab_and_closes_with_escape` | 통과 |
| 3 | `overlay_reflects_user_keymap` | 통과 |
| 4 | `overlay_shows_current_mode` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- 오버레이는 연 때의 모드 경로를 쓴다. 열린 동안에는 모드가 바뀌지 않는다(오버레이가 포커스를 가진다).
- 키 표기는 GPUI 표기(`ctrl-alt-u`)다. macOS 기호(⌃⌥) 표기와 `Kbd` 컴포넌트 모양은 S8 다듬기에서 본다.
- 명령 표에 없는 바탕 레이어 키(gpui-component의 입력란 키 등)는 보이지 않는다.

## 리뷰어에게

- 「실제로 동작하는 키」 판정(`bindings_for_input`의 첫 결과와 같은가)이 접두 시퀀스(`g g`)·대기 중 바인딩에서 맞는지.
- 오버레이가 열린 동안 키가 문서로 새지 않는지, 닫은 뒤 포커스.
