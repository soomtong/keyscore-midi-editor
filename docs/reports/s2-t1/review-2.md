# 적대적 리뷰: S2-T1 GPUI 앱과 창 모델 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다. 1회차 리뷰어와도 다른 세션이다.
- 대상 커밋: `698ba85` (리뷰 1 이후 `f198588..698ba85`)
- 범위: 확인 리뷰. A) 리뷰 1 지적 1–4가 근본에서 고쳐졌는가, B) 받아들인 제안 5–9가 보고서대로 동작하는가, C) 회귀와 fmt·clippy. 그 밖은 제안까지만.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff f198588..HEAD`의 `src/`와 gpui-base `resizable`, gpui-pre `on_focus_lost`·`shutdown` 소스를 읽고, 리뷰 테스트 본문을 열기 전에 적었다(구현자가 더한 테스트 셋은 diff에서 보았다).

1. `ksq.rs` 쓰기 경로의 `Map` 순회가 모두 `by_name`을 거치는가. 노트 `extra` 문자열은 읽을 때 `inline`으로 만들어지므로 그 안의 중첩 객체도 정렬되는가.
2. 지적 2의 수정은 「다시 펼 때 크기 상태를 새로 만든다」뿐이다. 원인(숨은 패널이 gpui-base `sizes`에 남는다)은 그대로다. 접혀 있는 동안 형제 패널을 조절하면 `resize_panel_at_handle`이 동기화 전 `old_sizes`(숨은 패널의 옛 크기 포함)로 합을 계산해, 넘친 만큼 조절한 패널에서 깎을 것이다. 숨은 패널이 빈 `div`가 되면 그 몫이 flex로 형제에게 나뉠 것이다.
3. `on_focus_lost`는 그리기 단계에서 「앞 포커스 경로가 있었고 지금 경로가 빈」 때만 부른다. 포커스 요소가 트리에서 사라진 경우도 경로가 비므로 잡힐 것이다. 연달아 blur해도 매번 되돌리는가.
4. `layout_extra`는 `extend` 순서(모르는 키 → 알려진 키)로 쓴다. `preserve_order`에서 `Map::remove`가 `swap_remove`라 첫 저장에서 사용자 순서가 바뀔 수 있으나 두 번째부터는 바이트가 같아야 한다. null 필드, `layout: null`.
5. `merge_changes`: 창 B의 끌기가 B의 다른 필드(바뀌지 않음)를 쓰지 않는가. 다시 펴서 새 크기 상태가 생겨도 되돌리지 않는가.
6. `.bak`: `read_to_string` 실패(UTF-8이 아닌 바이트)는 앞 분기에서 돌아가 백업이 없을 것이다.
7. `on_app_quit`: `App::shutdown`이 디바운스 중인 설정을 바로 쓰는가.
8. `NOTICE`의 Lucide 전문이 gpui-kit-assets 0.7.2 `LICENSE-LUCIDE`와 같은가, 버전이 `Cargo.lock`과 같은가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 모든 깊이의 정렬 | 부분 | 리뷰 1 core 테스트는 포트·노트의 중첩 객체를 다루지 않는다 |
| 2. 접힌 동안 형제 조절 | 없음 | 리뷰 1 테스트는 「다시 편 패널」의 너비만 본다 |
| 3. 포커스 요소 제거 | 없음 | `window.blur()` 한 번만 |
| 4. 중첩 모르는 값, null | 부분 | `unknown_layout_keys_survive_save`는 숫자 하나 |
| 5. 여러 창 끌기 | 부분 | `toggling_in_one_window_keeps_other_windows_changes`는 토글만 |
| 6. UTF-8 아닌 파일 백업 | 없음 | `{ not json`만 |
| 7. 종료 때 저장 | 없음 | |
| 8. NOTICE | 없음 | 문서 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `cargo test --workspace --no-fail-fast` 3회(`preserve_order` 켜짐) | 못 깨뜨림. 3회 모두 `ksq_fuzz_near_valid_files_never_panic_and_are_stable`·리뷰 1 core 테스트 통과. 1회차(리뷰 2 테스트 추가 전) 843 통과, 실패 0. 2·3회차 실패는 이 리뷰의 앱 테스트뿐 | 없음(기존 테스트) |
| 2 | 조성·포트·노트·최상위의 모르는 값 안에 중첩 객체·배열 속 객체를 역순으로 두고 읽기 → 쓰기 4회. `-p keyscore-core`(기본 feature)와 워크스페이스 둘 다 | 못 깨뜨림. 모든 깊이 이름 순, 바이트 같음. `ksq.rs`의 쓰기 쪽 `Map::iter`는 `by_name` 안 하나뿐이다 | `nested_unknown_objects_everywhere_are_sorted_and_byte_stable`(core) |
| 3 | 인스펙터를 접기만 한다(기본 레이아웃) | 깨짐. 트랙 목록이 220에서 350으로 그려진다. 레이아웃·설정 값은 220 그대로다. 숨은 패널이 빈 `div`가 되어 그 260이 트랙 목록과 메인에 flex로 나뉜다 | `collapsing_inspector_keeps_track_list_width` |
| 4 | 인스펙터가 접힌 채 열고 트랙 목록을 260으로 조절 | 깨짐. 그린 너비 210, 저장 160. gpui-base `sizes`가 `[220, 1316, 100]`(숨은 패널의 자리 크기 100이 합에 든다)이라 합이 컨테이너(1536)를 100 넘고, 넘친 100을 조절한 패널에서 깎는다. 끌기도 같은 `resize_panel_at_handle`을 거친다 | `sibling_resize_while_panel_collapsed_at_open_is_drawn_and_saved_as_requested` |
| 5 | 연 뒤 인스펙터를 접고 트랙 목록을 260으로 조절 | 깨짐. 저장 260, 그린 너비 390(시도 3의 어긋남 위에 더해진다) | `sibling_resize_after_collapsing_panel_is_drawn_and_saved_as_requested` |
| 6 | 시도 4 뒤 인스펙터를 편다 | 깨짐. 인스펙터는 300(지적 2 수정이 동작)이나 트랙 목록은 요청한 260이 아니라 잘못 저장된 160 | `inspector_collapsed_at_open_reopens_at_saved_width_after_track_list_resize` |
| 7 | 두 패널을 접은 채 열고 트랙 목록을 펴 270으로 조절한 뒤 인스펙터를 편다 | 깨짐. 트랙 목록 170(같은 원인) | `two_collapsed_panels_reopen_one_by_one_at_their_widths` |
| 8 | 연 뒤 트랙 목록을 접고 인스펙터(마지막 패널)를 330으로 조절한 뒤 트랙 목록을 편다 | 못 깨뜨림. 260·330. 마지막 패널 조절은 숨은 패널 쪽 합을 건드리지 않는다 | `track_list_collapsed_after_open_reopens_after_inspector_resize` |
| 9 | 하단 높이를 280으로 조절하고 접은 뒤 가로 그룹을 조절하고 편다 | 못 깨뜨림. 280·250 | `bottom_panel_reopens_at_resized_height_after_other_group_resize` |
| 10 | 트리에 없는 포커스 핸들로 포커스를 옮기고 그 핸들을 drop한 뒤 `ToggleInspector`, 이어 blur를 두 번 더 하며 토글 | 못 깨뜨림. 매번 닿는다 | `panel_toggle_works_after_focused_element_is_removed` |
| 11 | `layout` 안 중첩 객체·배열·null 모르는 값, 알려진 키 사이에 섞어 두고 저장 4회 | 못 깨뜨림. 값 같음, 첫 저장 뒤 바이트 같음 | `nested_unknown_layout_values_survive_and_are_byte_stable` |
| 12 | `inspector_width: null`, `bottom_open: null`, 그리고 `layout: null` | 못 깨뜨림. 그 필드만 기본값, 경고 2개·1개, 저장 뒤 다시 읽으면 같다 | `null_layout_fields_fall_back_alone` |
| 13 | 창 A에서 트랙 목록 310, 창 B에서 인스펙터 330을 끌고, B에서 하단을 접었다 편다(새 크기 상태) | 못 깨뜨림. 설정은 310·330 | `dragging_in_one_window_keeps_other_windows_untouched_widths` |
| 14 | 배열 `[1, 2]`, 빈 파일, UTF-8이 아닌 바이트의 설정 파일 | 깨짐(마지막만). 앞 둘은 `.bak`이 생긴다. UTF-8이 아니면 `read_to_string` 실패 분기로 돌아가 백업 없이 기본값이고, 첫 변경 1초 뒤 덮어쓴다 | `non_object_settings_files_are_backed_up` |
| 15 | 하단을 토글한 직후(디바운스 전) `App::shutdown` | 못 깨뜨림. 파일에 `bottom_open: false` | `app_shutdown_flushes_pending_settings` |
| 16 | `NOTICE`의 Lucide 절을 gpui-kit-assets 0.7.2 `LICENSE-LUCIDE`와 `diff`, 버전을 `Cargo.lock`과 대조 | 못 깨뜨림. 바이트 같음. gpui-kit 0.7.1, gpui-base·gpui-component·gpui-kit-assets 0.7.2, gpui-component-macros 0.7.3 일치 | 없음(문서) |
| 17 | `apply(vec![])` 뒤 `is_dirty` | 못 깨뜨림(구현자 테스트 `empty_edit_does_not_mark_document_dirty` 통과) | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `.ksq` 쓰기 결정성(feature 무관) | 아니오 | 시도 1, 2 |
| 설정 모르는 키 보존 | 아니오 | 시도 11, 12 |
| 그린 패널 크기 = 창 레이아웃 값 = 저장 값 | 예 | 시도 3–7. 패널을 하나 접으면 형제의 그린 너비가 레이아웃과 어긋나고, 접힌 동안의 조절은 잘못된 값으로 저장된다 |
| 포커스와 무관한 패널 토글 | 아니오 | 시도 10 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.1 화면 구성(접히는 패널, 마지막 레이아웃 복원) | 부분 충족 | `window.rs:121-159, 205-238`. 접기 자체와 다시 펴기는 맞으나 접힌 동안 형제 패널의 너비가 어긋나고 그 값이 저장된다(시도 3–7) |
| UI-12 인스펙터 접기, `Cmd+I` | 부분 충족 | 토글은 포커스와 무관하게 닿는다(시도 10). 인스펙터를 접으면 트랙 목록 너비가 바뀐다(시도 3) |
| IO-1 `.ksq` 결정적 쓰기 | 충족 | `ksq.rs:131, 221, 245-249` |

「부분 충족」은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(앱은 아직 MIDI를 쓰지 않는다).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음(`settings.rs`의 `unwrap_or_default`는 바로 앞 가드로 `Some`).
- 에러가 삼켜지는 곳: `settings.rs` `.bak` 복사 실패는 경고 문구에서만 빠진다(의도). `settings.rs:137-146` 읽기 실패(UTF-8 아님, 권한)는 백업 없이 기본값(지적 2).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 2가 근본에서 고쳐지지 않았다. 원인은 `visible(false)` 패널이 gpui-base `ResizableState`의 `sizes`·`panels`에 남는 것인데, 수정은 「다시 펼 때 상태를 새로 만든다」로 증상 하나만 덮었다. 남은 증상: (a) 인스펙터를 접기만 해도 트랙 목록이 220 → 350으로 그려지고 레이아웃 값(220)과 어긋난다. (b) 패널이 접힌 동안 형제를 끌면 숨은 패널의 옛 크기가 합에 들어 넘친 만큼 깎이고(260 요청 → 저장 160, 그림 210), 그 잘못된 값이 설정에 남아 다시 펴도 돌아오지 않는다. 숨은 패널을 그룹의 자식에서 빼고(펴진 패널만 자식으로, 인덱스는 `record_sizes`에서 매핑) 접기·펴기 때 상태를 새로 만들거나, gpui-base에서 숨은 패널을 합·flex에서 제외하도록 고쳐 스냅샷을 올리는 방법이 있다. 실패 테스트: `collapsing_inspector_keeps_track_list_width`, `sibling_resize_while_panel_collapsed_at_open_is_drawn_and_saved_as_requested`, `sibling_resize_after_collapsing_panel_is_drawn_and_saved_as_requested`, `inspector_collapsed_at_open_reopens_at_saved_width_after_track_list_resize`, `two_collapsed_panels_reopen_one_by_one_at_their_widths` | `crates/keyscore-app/src/window.rs:121-159, 205-238` |
| 2 | 제안 | 보고서는 「JSON 객체가 아니면 `settings.json.bak`으로 복사」라 하지만 UTF-8이 아닌 파일은 `read_to_string` 실패 분기로 먼저 돌아가 백업이 없고, 첫 변경 때 덮어쓴다. `fs::read`로 바이트를 읽어 UTF-8·JSON 실패를 같은 백업 분기로 보내거나, 보고서 문구를 좁힐 것. 실패 테스트: `non_object_settings_files_are_backed_up` | `crates/keyscore-app/src/settings.rs:137-158` |

리뷰 1 지적 1(`.ksq` 정렬), 3(포커스), 4(`layout` 모르는 키)은 근본에서 해결됐다. 제안 5·6·7·9는 보고서대로 동작한다. 제안 8은 지적 2를 빼면 보고서대로다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

수정 1건(리뷰 1 지적 2의 근본 원인). 실패하는 리뷰 테스트(앱 6개: 지적 1에 5개, 지적 2에 1개)는 저장소에 남긴다. 지적 2의 테스트는 제안이므로 구현자가 거절하면 보고서 문구를 고치고 그 테스트의 UTF-8 경우를 근거와 함께 조정할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(core 변경은 이 Task가 들인 회귀 수정, proptest 회귀 시드 파일은 리뷰 1이 찾은 실패 입력)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`f198588..HEAD`에서 리뷰 테스트 파일 변경 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음, `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다

자동 게이트: `cargo fmt --all -- --check` 통과, `cargo clippy --workspace --all-targets -- -D warnings` 통과(리뷰 테스트 포함), `cargo deny check` 통과. `cargo test --workspace`는 이 리뷰의 앱 테스트 6개만 실패한다.
