# Task 계획: S2-T6f 이월 줄 정리

- 브랜치: `s2/t6f-carry-overs`
- 관련 REQ: EDT-1(결정 `docs/decisions/2026-10-19-undo-history-memory-limit.md` A), UI-6·KM-10(오버레이), OUT-3(핫플러그 알림), 7.1(레이아웃 패널), UI-2(모드 메뉴)
- 선행 Task: S0-T4(히스토리), S2-T1(창·패널), S2-T6b(오버레이), S2-T6d·T6e(가상 포트·포트 감시)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

단계 파일 S2-T6 절과 S2-T5 절의 이월 줄 여섯을 끝낸다.

1. undo 상한 알림(결정 A)
   - core `Document`에 지금까지 상한 때문에 버린 undo 단위 수(`dropped_undo_units`)를 알리는 공개 API를 둔다. `History`가 버릴 때 센다.
   - 앱은 편집·되돌리기·다시 실행 뒤 그 수가 0에서 처음 늘면 그 창 상태줄에 한 번 알린다: 「되돌리기 기록이 메모리 상한(256 MiB)을 넘어 가장 오래된 단계부터 버렸다」. 두 번째부터는 알리지 않는다.
   - 시험을 위해 상한을 받는 창 열기 함수(`open_project_window_with_history_limit`)를 둔다. 앱의 실제 상한은 그대로 256 MiB다.
2. 바탕 레이어 `alt-cmd-i`
   - gpui-component의 디버그 전역 키(`debug_assertions`에서만)가 키맵 편집기 목록·`?` 오버레이·메뉴 키 표시·충돌 검사에 나오지 않음을 시험으로 고정한다. 지금 편집기·오버레이는 명령 표에 있는 Action만 보여 이미 빠져 있다. 시험이 통과하면 코드는 바꾸지 않는다.
3. 패널 너비(S2-T1 리뷰 3)
   - 창 크기를 바꾸면 사이드 패널(트랙 목록·인스펙터)과 아래 패널은 저장된 크기를 지키고 메인 영역이 늘고 준다. 이어 한 패널을 조절해도 다른 패널의 저장 크기가 바뀌지 않는다. 리뷰 부록 두 테스트를 저장소에 넣는다.
4. 창 최소 크기
   - 프로젝트 창의 최소 크기를 둔다(가로 720, 세로 480 논리 픽셀. 모드 메뉴가 상태줄 위에 떠도 모드 표시를 덮지 않고, 펴진 세 패널 기본 크기와 메인 영역이 들어가는 값). 설정 창은 최소 가로 560, 세로 400.
5. `?` 오버레이 인자 라벨(S2-T6b 리뷰 3)
   - 인자 라벨을 키·이름으로 찾지 않는다. 실효 항목마다 그 `args`로 Action을 만들어 엔진이 그 키에 고른 Action과 같은지(`partial_eq`) 보고 같은 항목의 라벨을 쓴다. 리뷰 부록 C1·C2를 저장소 테스트로 넣는다.
6. 앱 포트 이름의 트랙 창 핫플러그 알림(S2-T6e 리뷰 3)
   - 트랙이 앱 가상 포트 Y를 쓰는 창에서도 같은 이름의 목적지 Y가 오가면 핫플러그 알림이 보인다(`windows_using`의 트랙 필터가 앱 포트 이름을 뺀다). 대조군 `review3_control_track_on_own_port_hides_same_name_destination`의 그 창 기대값을 바꾼다.

## 공개 인터페이스 초안

~~~rust
// keyscore_core::Document
pub fn dropped_undo_units(&self) -> u64;
// keyscore_app
pub fn open_project_window_with_history_limit(project: Project, limit_bytes: usize, cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<ProjectWindow>)>;
pub const PROJECT_WINDOW_MIN_SIZE: Size<Pixels>; // 시험이 창 옵션을 본다
pub fn project_window_options(...) -> WindowOptions;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `document_counts_dropped_undo_units` | 단위(core) | 상한을 넘겨 버리면 수가 늘고, undo·redo로 버려도 센다. 버리지 않으면 0 |
| 2 | `undo_limit_notice_shows_once` | UI | 작은 상한 창에서 편집을 쌓으면 상태줄 알림이 한 번 뜨고, 더 버려도 같은 알림이 다시 쌓이지 않는다. 상한 안이면 알림이 없다 |
| 3 | `base_debug_key_is_hidden_from_editor_overlay_and_menus` | UI | `alt-cmd-i`가 키맵 편집기 목록·오버레이 줄·메뉴 키 표시에 없고, 그 키로 바인딩을 잡아도 바탕 레이어와 충돌로 보이지 않는다 |
| 4 | `window_resize_keeps_side_panels_at_stored_widths`, `inspector_resize_after_window_resize_keeps_track_list_width` | UI | S2-T1 리뷰 3 부록 그대로 |
| 5 | `project_window_has_min_size` | UI | 프로젝트 창·설정 창 옵션에 최소 크기가 있다 |
| 6 | `argument_label_with_global_override`, `argument_label_editor_rebind_shadowed_by_deeper_context` | UI | S2-T6b 리뷰 3 부록 C1·C2 그대로 |
| 7 | `destination_named_like_own_track_port_is_announced` | 루프백 | 트랙이 앱 포트 Y를 쓰는 창에서 목적지 Y가 생기고 없어지면 핫플러그 알림이 그 창에도 뜬다 |

## 불변식

- core `Document`의 공개 API로 히스토리를 바꿀 수 없다(읽기만 더한다).
- 레이아웃 설정 값은 사람이 조절한 패널만 바뀐다.

## 범위 밖

- S2-T7(저장·열기·복구)과 S2 출구.
- undo 상한을 설정에서 고르기(결정 A는 고정 256 MiB).

## 리스크·결정 요청

- 창 최소 크기 값은 UI 세부다. 패널 기본 크기(220 + 260, 아래 160)와 상태줄·트랜스포트 높이로 정했다.
- 3의 고치는 방법은 gpui-base 리사이즈 그룹의 동작에 달려 있다. 사이드 패널을 고정 너비로 두는 방법이 막히면 `record_sizes`가 조절된 패널만 쓰게 하는 방법으로 간다.
