# Task 보고: S2-T1 GPUI 앱과 창 모델

- PR: 브랜치 `s2/t1-app-window`
- 계획 파일: `docs/reports/s2-t1/plan.md`
- 구현: 주 세션
- 계획 대비 달라진 점
  - 의존(결정 `docs/decisions/2026-10-16-gpui-kit-licenses-and-advisories.md` A)
    - gpui-kit이 끌어오는 라이선스 0BSD·CC0-1.0을 `deny.toml` 허용 목록에 더했다.
    - 유지보수 중단 권고 넷을 ID·이유와 함께 무시한다.
    - `NOTICE`를 갱신했다.
  - core 수정 하나(`core: Sort unknown .ksq keys explicitly when writing`)
    - gpui-kit이 들어오자 워크스페이스 feature 통합으로 `serde_json`의 `preserve_order`가 켜졌다.
    - `.ksq` 쓰기가 「`Map`은 이름 순」이라고 가정한 곳이 흔들려, core 퍼징 리뷰 테스트 3개가 실패했다.
    - 모르는 키를 쓸 때 명시적으로 정렬한다. 이 Task가 들인 회귀라 함께 고쳤다.
  - 테스트 먼저 규칙에서 벗어난 점: 설정 읽기·쓰기(`settings.rs`)는 뼈대 대신 구현을 먼저 썼다. 실패 테스트 커밋에서 팔레트·패널 토글만 `todo!()`였다.
  - 크기 저장은 그룹의 `on_resize`(끌기에서만 불림) 대신, 창이 가진 `ResizableState`의 `Resized` 이벤트를 구독한다. 끌기와 `resize_panel` 모두 이 이벤트를 낸다.
  - 테스트 9를 더했다: `resizing_panels_is_saved_for_next_window`.

## 구현 요약

| 파일 | 내용 |
| --- | --- |
| `src/lib.rs` | `init`(gpui-kit 초기화, 설정 설치, 테마 적용, Action 처리, 임시 키 바인딩, 메뉴 셋), `open_project_window`, `run`, `OpenError` |
| `src/window.rs` | `ProjectWindow` |
| | 7.1 영역 자리표시자: `transport`, `track-list`, `main-view`, `inspector`, `bottom-panel`, `status-bar` |
| | 가로·세로 `Resizable`, 접기 Action, 크기 이벤트 구독 |
| `src/document.rs` | `ProjectDocument`: `core::Document`를 감싸고 읽기·`apply`·`undo`·`redo`만 내준다. `dirty` 표시 |
| `src/settings.rs` | `Settings`, `LayoutSettings`, `ThemeChoice`. 모르는 키 보존, 잘못된 값은 기본값 + 경고, 임시 파일 + rename |
| `src/app_state.rs` | 전역 `AppSettings`: 바뀌면 1초 디바운스 저장, `flush` |
| `src/theme.rs` | 시스템·라이트·다크 적용 |
| `src/palette.rs` | `track_color`: 12색을 gpui-kit 테마 토큰 12개에 대응 |
| `src/actions.rs` | 새 창, 창 닫기, 종료, 패널 토글 셋, 테마 셋 |

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 827 passed, 0 failed(정정: 이 실행은 퍼징 테스트가 실패 입력을 우연히 만나지 않은 것이다. 리뷰 1 지적 1 참고)
keyscore-app 테스트 9개 3회 반복 통과
수동: 앱 바이너리를 임시 HOME으로 6초 실행 — 패닉 없이 창이 뜬 채로 있음
~~~

| 계획 # | 실제 이름 | 상태 |
| --- | --- | --- |
| 1 | `settings_round_trip_keeps_unknown_keys` | 통과 |
| 2 | `settings_missing_or_broken_file_falls_back_to_defaults` | 통과 |
| 3 | `opening_window_shows_all_layout_regions` | 통과 |
| 4 | `toggle_actions_collapse_and_restore_panels` | 통과 |
| 5 | `panel_layout_is_restored_from_settings` | 통과(인스펙터 너비 333 px ± 2) |
| 6 | `theme_choice_switches_mode_and_palette` | 통과 |
| 7 | `each_window_owns_its_own_document` | 통과 |
| 8 | `document_changes_only_through_edits` | 통과 |
| 추가 | `resizing_panels_is_saved_for_next_window` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (827 passed)
- [x] deny(결정 A 반영 뒤)
- [x] 단계별 추가 게이트: `check-deps`(앱 내부 의존 변화 없음)

## 알려진 한계

- 키 바인딩은 임시로 `init`에 직접 건다(`Cmd+N/W/Q/I`). 키맵 엔진은 S2-T2, 메뉴 7개는 S2-T4다.
- 마지막 창을 닫아도 앱은 끝나지 않는다(macOS 관례). `Quit`만 앱을 끝낸다.
- 종료는 `Quit` Action으로 할 때만 설정을 바로 저장한다. 다른 경로(강제 종료)는 디바운스 1초 안의 변경을 잃을 수 있다.
- 테마 「시스템」은 시작할 때와 고를 때 외관을 읽는다. macOS 외관이 실행 중에 바뀌는 것은 따라가지 않는다.
- 창 크기·위치는 저장하지 않는다(계획 범위 밖).
- 패널이 접힌 상태에서 크기 목록 길이가 달라지면 그때의 크기는 저장하지 않는다.

## 리뷰어에게

스스로 불안한 곳:

1. `record_sizes`가 `ResizableState::sizes()`의 순서를 자식 순서로 가정한다. 접힌 패널(`visible(false)`)이 목록에 남는지, 빠지는지에 따라 인덱스가 어긋날 수 있다.
2. `ProjectWindow::new`가 창을 열 때의 설정 레이아웃을 복사한다. 창 둘이 각자 토글하면 마지막 변경이 설정에 남는다. 「다음 창은 마지막으로 바꾼 레이아웃」이 의도다.
3. `AppSettings::update`의 디바운스 작업이 `pending`을 덮어써 앞 작업을 취소하는지(GPUI `Task` drop은 취소다).
4. `core` 수정이 `.ksq` 출력 바이트를 바꾸지 않는지. 기본 feature에서는 `Map`이 이미 이름 순이라 같은 바이트다.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

정정: 위 「827 passed」는 core 퍼징 테스트(`ksq_fuzz_near_valid_files_never_panic_and_are_stable`)가 그 실행에서 실패 입력을 만나지 않았을 뿐이다. 첫 core 수정이 포트·노트 객체의 모르는 키를 빠뜨려, 워크스페이스 빌드(`preserve_order` 켜짐)에서 쓰기가 결정적이지 않았다.

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | `.ksq` 포트 객체의 모르는 키가 정렬되지 않음 | 포트와 노트 속성 객체도 `by_name`을 거친다. 쓰기의 모든 `Map` 순회를 확인했다. 실패를 찾은 proptest 시드를 커밋했다. 커밋 `core: Sort unknown port and note keys too when writing .ksq` |
| 2 | 접힌 채 연 패널을 다른 패널 조절 뒤 펴면 최소 너비 | 접힌 패널을 펼 때 그 그룹의 `ResizableState`를 새로 만든다. 그래서 저장된 크기로 다시 그린다 |
| 3 | 창에 포커스가 없으면 패널 토글이 무시됨 | `on_focus_lost`에서 창의 루트 핸들로 포커스를 되돌린다(Zed Workspace 방식) |
| 4 | `layout` 안의 모르는 키가 저장 때 사라짐 | `layout`을 필드마다 읽고 나머지 키를 `layout_extra`로 보존한다 |
| 5 | 창마다 레이아웃 전체를 써서 다른 창의 변경을 되돌림 (제안) | 반영. 바뀐 항목만 설정에 옮긴다(`merge_changes`) |
| 6 | 빈 편집이 dirty를 남김 (제안) | 반영 |
| 7 | `NOTICE` 버전, Lucide·Feather 고지 (제안) | 반영. `Cargo.lock` 버전, `LICENSE-LUCIDE` 전문 |
| 8 | 깨진 설정을 백업 없이 덮어씀, 필드 하나로 레이아웃 전체가 기본값 (제안) | 반영. JSON 객체가 아니면 `settings.json.bak`으로 복사하고, 필드는 하나씩 기본값으로 둔다 |
| 9 | `Quit` 밖의 종료에서 디바운스 중인 설정을 잃음 (제안) | 반영. `on_app_quit`에서 저장한다 |

테스트를 셋 더했다: `broken_settings_are_backed_up_and_bad_fields_fall_back_alone`, `toggling_in_one_window_keeps_other_windows_changes`, `empty_edit_does_not_mark_document_dirty`. 리뷰어 테스트는 바뀌지 않았다. 워크스페이스 843개 통과.

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 리뷰 1 지적 2가 증상만 덮음. 접힌 패널이 gpui-base 크기 상태에 남아 형제 패널 조절이 틀어짐 | 리뷰의 첫째 방법. 펴진 패널만 그룹의 자식으로 두고, 접거나 펼 때마다 크기 상태를 새로 만들며, 크기를 펴진 패널 순서로 매핑한다. 커밋 `app: Keep collapsed panels out of the resizable groups` |
| 2 | UTF-8이 아닌 설정 파일은 `.bak` 없이 덮어씀 (제안) | 반영 |

리뷰어 테스트 변경: `track_list_collapsed_after_open_reopens_after_inspector_resize`는 트랙 목록이 접힌 동안 인스펙터를 자식 2로 가리켰다(옛 구조). 수정 뒤 자식은 [메인, 인스펙터]라 1로 바꿨다. 검사 의도는 같다. 워크스페이스 857개 통과.
