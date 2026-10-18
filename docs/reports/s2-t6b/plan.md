# Task 계획: S2-T6b `?` 키맵 오버레이

- 브랜치: `s2/t6b-keymap-overlay`(S2-T6을 나눈 둘째 Task)
- 관련 REQ: UI-6, KM-10, 5.8(`?`), 7.6(도움말 > 키맵 보기), 11절 컴포넌트 표(`Sheet`/`Popover` + `Kbd`)
- 선행 Task: S2-T6a(키맵 편집기), S2-T3(모드 키 컨텍스트 경로), S2-T4(명령 표)
- 결정: A13(Esc = 통상의 취소, `help::CloseOverlay` 신규, 컨텍스트 `Overlay`)

## 끝의 정의 (내 말로)

1. `help::KeymapOverlay`(Normal의 `?`, 메뉴 도움말 > 키맵 보기, 팔레트)가 프로젝트 창 위에 오버레이를 연다. 이미 열려 있으면 닫는다.
2. 오버레이는 지금 모드의 키 컨텍스트 경로에서 실제로 동작하는 키를 보인다. 엔진 바인딩마다 그 경로에서 맞고, 그 키를 눌렀을 때 GPUI가 고르는 첫 Action이 그 바인딩인 것만 넣는다(KM-10 「항상 현재 키맵」: 사용자 키맵·프리셋·수식키 교환을 그대로 반영한다). 줄은 「키 · 명령 제목(이름)」이다. 같은 Action의 여러 키는 한 줄에 모은다.
3. 한 쪽에 정해진 줄 수만 보이고 `Tab`/`Shift+Tab`(`help::NextPage`/`PrevPage`)으로 넘긴다. 끝에서 멈춘다.
4. 수식키 없는 `Esc`(`help::CloseOverlay`, A13)나 바깥 클릭이 닫는다. 닫으면 포커스가 문서로 돌아간다. 오버레이가 열린 동안 문서의 모드 키는 먹지 않는다(오버레이가 포커스를 가진다).
5. 키맵이 바뀌면(핫리로드, 편집기) 열린 오버레이도 다음 그리기에 새 키를 보인다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `question_mark_opens_overlay_with_mode_keys` | UI | Normal의 `?`가 연다. `m`의 뮤트 줄이 있고 Step 전용 줄은 없다 |
| 2 | `overlay_pages_with_tab_and_closes_with_escape` | UI | `Tab`/`Shift+Tab` 쪽 넘김(끝에서 멈춤), `Esc`로 닫힘, 닫은 뒤 모드 키가 산다, 열린 동안 `m`이 뮤트하지 않는다 |
| 3 | `overlay_reflects_user_keymap` | UI | 사용자 키맵이 `m`을 지우고 다른 키를 걸면 오버레이가 그 키를 보인다(KM-10) |
| 4 | `overlay_shows_current_mode` | UI | Step 모드에서 메뉴 Action으로 열면 Step 키(`space` 쉼표)를 보인다 |

## 범위 밖

- 건반 뷰의 키 라벨(5.4 건반 뷰, S5)
- 설정 창 안의 오버레이(설정 창에는 `?` 바인딩이 없다)

## 리스크·결정 요청

- 한 쪽의 줄 수(24)와 같은 Action 키 모으기는 표시 세부다.
