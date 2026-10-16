# 적대적 리뷰: S2-T1 GPUI 앱과 창 모델 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `672e137` (브랜치 `s2/t1-app-window`, `main..HEAD` 7커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`src/*.rs`, `ksq.rs` 변경, gpui-base `resizable` 소스를 읽고, `tests/window.rs`를 열기 전에 적었다.

1. `ProjectDocument`가 `&mut Project`를 내주는 경로가 있는가. `Entity<ProjectDocument>`를 `update`로 얻은 `&mut ProjectDocument`에서 undo를 거치지 않고 모델을 바꿀 수 있는가.
2. `Settings::load`는 `layout`을 `LayoutSettings`로 통째로 역직렬화한다. `layout` 안의 모르는 키는 보존되지 않을 것이다(최상위 `extra`만 있다). 필드 하나의 타입이 틀리면 레이아웃 전체가 기본값이 된다.
3. `Settings::save`의 임시 파일(`settings.json.tmp`)이 실패 경로(부모가 파일, 대상이 디렉터리)에서 남는가.
4. 디바운스: `pending`을 덮어쓰면 앞 `Task`가 drop되어 취소되는가. 1초 전에는 쓰지 않는가.
5. `record_sizes`의 인덱스: gpui-base는 `visible(false)` 패널도 `sizes`에 남긴다(`sync_panels_count`가 자식 수를 센다). 숨은 패널은 prepaint되지 않아 `bounds`가 0이다. 다른 패널을 조절하면 `sync_real_panel_sizes`가 숨은 패널 크기를 0으로 덮어써, 다시 펼 때 저장된 너비가 아니라 최소값이 될 것이다.
6. 패널 토글 Action 처리기가 `ProjectWindow` 안쪽 div에만 있다. 창에 포커스가 없으면 GPUI는 루트 노드(gpui-kit `Root`)에서 디스패치하므로 처리기에 닿지 않을 것이다.
7. 여러 창: 각 창이 레이아웃 사본을 가지고, 바꿀 때 사본 전체를 설정에 쓴다. 다른 창의 변경을 되돌릴 것이다.
8. `.ksq` 정렬 수정: `push_extra`, `inline`, `key_text`는 `by_name`을 거치지만 포트(`t.port_extra().iter()`)와 노트 속성(`extra.iter()`)은 거치지 않는다. `preserve_order`에서는 `Map::remove`가 `swap_remove`라 읽기 뒤 순서가 섞인다.
9. 팔레트 12색이 두 테마에서 값이 다를 뿐 아니라 눈으로 구별되는가, 배경과 구별되는가.
10. `NOTICE`가 실제로 잠긴 버전·자산(Lucide 아이콘이 바이너리에 들어간다)과 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. undo 우회 | 부분(`document_changes_only_through_edits`) | 타입 수준 검사. 우회 경로는 없다(4절) |
| 2. 중첩 모르는 키 | 없음 | `settings_round_trip_keeps_unknown_keys`가 `layout.later`를 넣지만 보존을 확인하지 않는다 |
| 3. 실패 경로 임시 파일 | 없음 | |
| 4. 디바운스 | 없음 | 모든 테스트가 `flush`를 직접 부른다 |
| 5. 접힌 채 연 패널의 복원 | 없음 | `panel_layout_is_restored_from_settings`는 접힌 패널을 펴지 않는다 |
| 6. 포커스 없는 디스패치 | 없음 | |
| 7. 여러 창 레이아웃 | 없음 | 문서 독립만 검사 |
| 8. 포트·노트 모르는 키 정렬 | 없음 | 기존 퍼징 `ksq_fuzz_near_valid_files_never_panic_and_are_stable`이 잡는다(아래 지적 1) |
| 9. 팔레트 구별 | 부분 | `!=`만 검사 |
| 10. NOTICE | 없음 | 문서 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 포트 객체에 모르는 키 `zz_port`, `mm_port`, `aa_port`를 역순으로 두고 읽기 → 쓰기를 세 번 반복. `cargo test --workspace`(gpui-kit 때문에 `preserve_order` 켜짐)와 `cargo test -p keyscore-core -F serde_json/preserve_order` | 깨짐. 키 둘로 먼저 시험했을 때 첫 쓰기 `aa, zz`, 두 번째 쓰기 `zz, aa`. 셋에서도 이름 순이 깨진다. 쓰기 → 읽기 → 쓰기 바이트가 다르다. `-p keyscore-core`(기본 feature)에서는 통과 | `unknown_port_keys_are_written_by_name`, `reversed_unknown_keys_round_trip_byte_stable`(core) |
| 2 | 기존 `review_s0_t6`의 퍼징을 워크스페이스에서 실행(작업 트리에 추적되지 않은 `review_s0_t6.proptest-regressions`가 있다) | 깨짐. 3회 모두 실패. 최소 입력 `{"tracks":[{"clips":[],"port":{"mute":null,"named":"","channel":null}}]}`. 1과 같은 원인(포트 모르는 키) | 기존 테스트 `ksq_fuzz_near_valid_files_never_panic_and_are_stable` |
| 3 | 최상위·조성·트랙·클립·중첩 객체·노트 속성의 모르는 키 역순 | 못 깨뜨림. 이름 순 | `unknown_keys_are_written_by_name_in_fixed_places`, `unknown_note_keys_are_written_by_name` |
| 4 | 설정 `track_list_open: false, track_list_width: 300`으로 창을 열고 인스펙터를 `resize_panel(2, 320)`로 조절한 뒤 `ToggleTrackList` | 깨짐. 트랙 목록 너비 120(최소값). 숨은 패널의 상태 크기가 0이 되어 `size_range` 하한으로 그려진다. 다음 끌기에서 120이 저장될 수 있다 | `collapsed_panel_reopens_at_saved_width_after_sibling_resize` |
| 5 | 연 뒤에 인스펙터(310)를 접었다 펴기 | 못 깨뜨림. 310 | `panel_collapsed_after_open_reopens_at_previous_width` |
| 6 | `window.blur()` 뒤 `ToggleInspector` 디스패치 | 깨짐. 레이아웃이 그대로다. 처리기가 `ProjectWindow` 안쪽 div에만 있어 루트 노드 경로에 없다. gpui-component `Input`은 `window.blur()`를 부른다(input.rs:1128) | `panel_toggle_works_when_window_has_no_focus` |
| 7 | `{"layout": {"inspector_open": false, "piano_roll_height": 321}}`을 읽고 저장 | 깨짐. `piano_roll_height`가 사라진다 | `unknown_layout_keys_survive_save` |
| 8 | 부모가 파일인 경로, 디렉터리인 대상에 저장 | 못 깨뜨림. 둘 다 `Err`, `.tmp` 없음 | `save_to_unwritable_place_fails_without_leftovers` |
| 9 | 모르는 최상위 키·중첩 객체가 든 설정을 두 번 읽고 쓰기 | 못 깨뜨림. 바이트 같음 | `save_is_byte_stable_across_round_trips` |
| 10 | 토글 → 0.6 s → 토글 → 0.6 s → 0.5 s (가짜 시계) | 못 깨뜨림. 1초 전에는 파일이 없고, 앞 예약이 취소되며, 마지막 상태가 쓰인다 | `debounce_coalesces_and_writes_final_state` |
| 11 | 라이트·다크에서 12색 쌍별 RGB 거리, 배경과의 거리 | 못 깨뜨림. 최소 쌍 거리 라이트 0.31, 다크 0.21. 배경 거리 0.15 초과 | `palette_colors_are_visibly_distinct_in_both_themes` |
| 12 | 너비 −5, 1e30, 높이 0인 설정으로 창 열기 | 못 깨뜨림. 기본 레이아웃 | `out_of_range_sizes_open_with_defaults` |
| 13 | 창 A에서 트랙 목록을 300으로 조절한 뒤 창 B에서 하단 영역만 토글(임시 테스트로 실행, 제안이라 커밋하지 않음) | 깨짐. 설정의 `track_list_width`가 220으로 돌아간다 | 없음(지적 5) |
| 14 | `ProjectDocument::apply(vec![])`(임시 테스트로 실행, 제안이라 커밋하지 않음) | 깨짐. `undo_len` 0인데 `is_dirty()`가 참 | 없음(지적 6) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 모델은 undo 엔진을 거쳐서만 바뀐다 | 아니오 | `ProjectDocument`는 `project() -> &Project`, `apply`·`undo`·`redo`만 낸다. `Project`의 가변 메서드(`tempo_map_mut` 등)는 `&mut Project`가 필요하다. `*doc = ProjectDocument::new(p)`로 문서 전체를 바꿀 수는 있으나 히스토리째 새 문서이고 모델 불변식은 `core`가 지킨다(파일 열기 용도로 정당) |
| 창 하나 = 문서 하나 | 아니오 | `open_project_window`가 창마다 새 `Entity<ProjectDocument>` |
| 설정 쓰기 원자성 | 아니오 | 임시 파일 + rename, 실패 시 임시 파일 삭제(시도 8) |
| 설정 모르는 키 보존 | 예 | `layout` 안의 키(시도 7) |
| `.ksq` 쓰기 결정성 | 예 | 포트 모르는 키(시도 1, 2) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.1 화면 구성 | 충족(골격) | `window.rs:142-210`, 자리표시자 6영역. 메뉴 7개는 S2-T4 |
| UI-7 다크·라이트 | 충족 | `theme.rs:16-23`. 사용자 테마 JSON(C)은 범위 밖 |
| UI-12 인스펙터 접기, `Cmd+I` | 부분 충족 | `lib.rs:56`, `window.rs:86`. 포커스가 없으면 `Cmd+I`가 무시된다(시도 6) |
| SEQ-7 트랙 색 | 충족 | `palette.rs:28-46`, 12색 구별(시도 11) |
| 9.1 GPUI 스냅샷 | 충족 | `Cargo.toml` `gpui-kit = "=0.7.1"`, `gpui-pre 0.3.8`. 다만 하위 crate는 0.7.2로 잠겼다(지적 7) |
| 9.4 gpui-kit 컴포넌트 | 충족 | `Root`·`Resizable`·`StatusBar`·`Theme` |

「부분 충족」 하나(UI-12)는 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(앱은 아직 MIDI를 쓰지 않는다).
- 제품 코드의 `unwrap`/`expect`: 없음.
- 에러가 삼켜지는 곳
  - `lib.rs:37`, `lib.rs:133`: `open_project_window`의 `Err`를 `let _`로 버린다. 새 창이 안 열려도 알림이 없다(S2-T5 상태줄 전까지).
  - `app_state.rs:64-66`: 설정 저장 실패는 `eprintln!`만. 보고서의 알려진 한계에 적혀 있다.
  - `settings.rs:116-118`: 깨진 설정 파일은 기본값으로 읽고, 다음 변경 때 덮어쓴다. 원본을 남기지 않는다(지적 8).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `.ksq` 정렬 수정이 포트 객체의 모르는 키를 빠뜨렸다. 워크스페이스 빌드(앱 바이너리 포함)는 `preserve_order`라 `Map::remove`(`swap_remove`)가 순서를 섞고, 쓰기 → 읽기 → 쓰기 바이트가 달라진다. 기존 퍼징 테스트 `ksq_fuzz_near_valid_files_never_panic_and_are_stable`이 작업 트리의 회귀 시드로 `cargo test --workspace`에서 매번 실패한다(보고서의 827 통과와 다르다). `t.port_extra().iter()`도 `by_name`으로. 노트 속성(`ksq.rs:220-223`)도 지금은 읽을 때 정규화되어 통과하지만 같은 방식으로 맞추면 안전하다. 실패 테스트: `unknown_port_keys_are_written_by_name`, `reversed_unknown_keys_round_trip_byte_stable`(core) | `crates/keyscore-core/src/ksq.rs:129-134` |
| 2 | 수정 | 접힌 채로 연 패널은 다른 패널을 조절한 뒤 펴면 저장된 너비가 아니라 최소값(120)으로 그려진다. gpui-base가 숨은 패널의 `bounds`(0)로 `sizes`를 덮어쓰고 `panels[i].size = Some(..)`로 고정하기 때문이다. 펼 때 저장된 너비를 `resize_panel` 등으로 다시 넣어야 한다. 실패 테스트: `collapsed_panel_reopens_at_saved_width_after_sibling_resize` | `crates/keyscore-app/src/window.rs:82-100, 174-191` |
| 3 | 수정 | 패널 토글 처리기가 `ProjectWindow` 안쪽 div에만 있어, 창에 포커스가 없으면 `Cmd+I`와 보기 메뉴의 토글이 아무 일도 하지 않는다(UI-12). gpui-component `Input`이 `window.blur()`를 부르므로 S2-T5 인스펙터 필드가 생기면 실제로 일어난다. 앱 수준 처리기(`cx.on_action`에서 활성 창의 `ProjectWindow`로 전달) 등으로 포커스와 무관하게. 실패 테스트: `panel_toggle_works_when_window_has_no_focus` | `crates/keyscore-app/src/window.rs:148-151` |
| 4 | 수정 | 설정의 `layout` 안 모르는 키가 저장 때 사라진다. 모듈 문서와 계획은 「모르는 키는 보존」이라 한다. 뒤 버전이 레이아웃 키를 더하면 앞 버전이 지운다. `LayoutSettings`에 `#[serde(flatten)] extra` 등. 실패 테스트: `unknown_layout_keys_survive_save` | `crates/keyscore-app/src/settings.rs:29-38, 121-126` |
| 5 | 제안 | 창마다 레이아웃 사본을 가지고 바꿀 때 사본 전체를 설정에 써서, 창 B의 토글이 창 A에서 바꾼 너비를 되돌린다. 바뀐 필드만 설정에 반영하는 편이 「마지막으로 바꾼 레이아웃」 의도에 맞다 | `crates/keyscore-app/src/window.rs:95-100` |
| 6 | 제안 | `apply(vec![])`가 아무것도 바꾸지 않았는데 `dirty`를 세운다. S2-T7 닫기 시트가 헛되이 뜰 수 있다. undo 길이 변화 등으로 판단 | `crates/keyscore-app/src/document.rs:39-43` |
| 7 | 제안 | `NOTICE`는 「gpui-kit 0.7.1(gpui-kit, gpui-base, gpui-component, gpui-kit-assets)」라 하지만 `Cargo.lock`은 gpui-base·gpui-component·gpui-kit-assets 0.7.2, gpui-component-macros 0.7.3이다. 또 `run()`이 `gpui_kit::assets::Assets`로 Lucide 아이콘을 바이너리에 넣는데, Lucide ISC와 Feather MIT는 「이 고지가 모든 사본에 나타나야」 한다. 「옮길 고지문이 없다」는 Apache NOTICE에만 맞다. 배포 패키지 Task 전에 `LICENSE-LUCIDE` 전문을 옮겨 둘 것 | `NOTICE:12-18` |
| 8 | 제안 | 깨진 설정 파일(JSON이 아님, 숫자 범위 초과 등)은 기본값으로 읽고, 첫 변경 1초 뒤 통째로 덮어쓴다. `settings.json.bak` 등으로 원본을 남기거나, 필드 하나가 틀리면 레이아웃 전체 대신 그 필드만 기본값으로 | `crates/keyscore-app/src/settings.rs:116-126` |
| 9 | 제안 | 설정의 즉시 저장이 `Quit` Action에만 있다. Dock의 종료·로그아웃처럼 Action을 거치지 않는 종료에서 1초 안의 변경을 잃는다. `cx.on_app_quit`에서 `flush`. 보고서 알려진 한계와 같은 내용이며 비용이 작다 | `crates/keyscore-app/src/lib.rs:44-47` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

수정 4건. 특히 지적 1은 이 Task가 들인 core 회귀를 덜 고친 것이고, 자동 게이트 `cargo test --workspace`가 기존 테스트로 실패한다. 실패하는 리뷰 테스트(앱 3개, core 2개)는 저장소에 남긴다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(core 변경은 이 Task가 들인 회귀 수정으로 보고서에 적혀 있다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`cargo deny check` 통과, 결정 A와 `deny.toml` 일치)
- [x] 스타일 지적을 적지 않았다
