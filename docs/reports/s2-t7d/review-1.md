# 적대적 리뷰: S2-T7d 없는 출력 포트의 대체 매핑 제안(OUT-8) (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `512f7d4`(브랜치 `s2/t7d-port-mapping`, `main...` diff 14개 파일)
- 리뷰 시험: `crates/keyscore-app/tests/review_s2_t7d.rs`(5개). 결함을 증명하는 4개는 지금 실패하고, 대조군 1개는 통과한다. 개수를 세거나 CoreMIDI 목적지를 만드는 시험은 자식 프로세스에서 혼자 돈다(결정 `2026-10-15-virtual-port-limit-scope.md`). 실행: `cargo test -p keyscore-app --test review_s2_t7d`

## 1. 코드만 읽고 적은 「내가 검사할 것」

제품 코드(`port_mapping.rs`, `engine.rs`의 `add_named_virtual_port`·`open_port`·`best_match`·`port_choices`, `files.rs`의 `open_path`·`open_paths`·`run_file_flow`, `autosave.rs`의 `recover`, keyscore-midi `normalized_name`·`duplicate_key`·`create_virtual_output`)를 읽고 적었다. 구현자 시험은 이 목록 뒤에 도움 함수를 빌리려고 열었다.

1. 「없는 포트」 판정이 엔진의 여는 규칙(기본 포트 이름 정확히 같음 → 0, 추가 가상 포트 정확히 같음, 목적지 `best_match` 정확히 같음)과 같은가.
2. `Create`로 만든 가상 포트가, 그 이름의 장치(목적지)가 나중에 돌아왔을 때 무엇을 하는가. 엔진은 앱 포트를 먼저 연다(`engine.rs:1084`). OUT-3 자동 재연결과 부딪히는가.
3. 적용 때 다시 계산하면 시트에 보인 것과 다른 일이 일어날 수 있다. 특히 `Create`가 `Main(TooMany)`로 바뀌어 트랙이 기본 포트로 가는 경우, 사용자가 그것을 알 수 있는가.
4. `shown`이 시트 글자를 지키는가. 이름 규칙이 거부하는 문자(양방향 제어, 폭 없는 문자)는 `is_control`이 아니다.
5. 정규화 이름이 같은 두 줄의 차례가 뒤집히면 둘째 줄이 `Use(자기 이름)`이 되어 같은 값 편집을 넣는가. 상태줄 셈과 시트 설명이 맞는가.
6. 시트가 `file_flow`를 잡은 새 창이 `open_paths`의 실패 시트 호스트 고르기, 손대지 않은 원래 창 닫기, 끝내기(`confirm_close_all`)와 겹치는가.
7. 16개 상한: `plan`은 `own.len()+created.len()`으로 세고, keyscore-midi는 프로세스의 `PORT_NAMES`(지운 뒤 아직 놓지 않은 포트 포함)로 센다.
8. 구현자 시험 14(`apply_recomputes_when_port_appeared_meanwhile`)가 다시 계산하지 않는 구현에서도 통과하는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 없는 포트 판정 | 있음 | `plan_skips_virtual_own_ports_and_destinations`(순수). 코드 대조로 엔진과 같음을 확인 |
| 2 돌아온 장치 | 없음 | 깨짐(지적 1) |
| 3 시트와 다른 적용 | 일부 | 시험 14는 「포트가 이미 생긴」 경우만 본다. 기본 포트로 바뀌는 경우 없음. 깨짐(지적 2) |
| 4 시트 글자 | 일부 | `describe_names_action_and_track_count`는 U+0007만 본다. 깨짐(지적 3) |
| 5 같은 값 편집 | 없음 | 깨짐(제안 1) |
| 6 다른 흐름과의 겹침 | 없음 | 코드 대조: `open_paths`의 `free`가 흐름이 도는 창을 피하고, 원래 창 닫기는 원래 창의 흐름만 본다. 문제를 찾지 못했다 |
| 7 16개 상한의 셈 | 일부 | `plan_over_limit_falls_back_to_main`(순수) |
| 8 시험 14의 강도 | 있음 | 약함(제안 2) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 장치 이름이 없는 프로젝트를 열어 [제안대로 매핑] → 같은 이름 목적지(다시 꽂은 장치)를 만들고 재생 | 깨짐: 장치로 노트 0개. `port_choices`에도 그 장치가 없다(`[Virtual, Named(장치)]`는 앱 포트). 시트 문구는 「같은 이름의 가상 포트를 만든다(트랙 1개)」뿐 | `review1_mapped_port_does_not_shadow_returning_device`(실패) |
| 2 | 1의 대조군: [그대로 두기] 뒤 같은 장치가 돌아오고 재생 | 못 깨뜨림(장치로 노트가 간다). 1의 0개가 시험 장치 탓이 아님을 보인다 | `review1_control_unmapped_track_reaches_returning_device`(통과) |
| 3 | 시트가 「같은 이름의 가상 포트를 만든다」를 보이는 동안 앱 포트를 16개로 채우고 [제안대로 매핑] | 깨짐: 트랙이 `Virtual`(기본 포트)로 바뀌고 상태줄은 「가상 포트 0개를 만들고 트랙 1개를 바꿨다」뿐 | `review1_stale_sheet_does_not_silently_fall_back_to_main`(실패) |
| 4 | 트랙 포트 이름에 U+202E(RLO), U+200B, U+2066, U+FEFF | 깨짐: `plan`은 `Main(InvalidName)`이지만 `describe`가 그 문자를 그대로 시트에 넣는다(`"「Bad\u{202e}Port」: … 기본 포트 「KeyScore Out」으로 바꾼다"`). RLO는 줄의 뒷부분을 뒤집어 보이고, U+200B는 「BadPort」처럼 멀쩡해 보이는 이름을 「쓸 수 없다」고 한다 | `review1_sheet_line_escapes_bidi_and_invisible_characters`(실패) |
| 5 | 트랙 순서 [`"A  B"`(공백 둘), `"A B"`], 둘 다 없음 | 깨짐: 시트 둘째 줄 「「A B」: 트랙 1개를 「A B」로 바꾼다」, 상태줄 「트랙 2개를 바꿨다」(실제로 바뀐 것은 1개). 결과 포트는 맞다 | `review1_already_normalized_second_name_is_not_a_change`(실패) |
| 6 | 구현자 시험 `port_mapping`(16), `commands`(12) | 통과. 새 Action의 명령 표·메뉴·UI-11 시험 포함 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 트랙 변경은 `edit_document` 한 번(undo 한 단위) | 아니오 | `port_mapping.rs:201-234`. 같은 값 편집이 섞일 수 있다(제안 1) |
| 보이는 모양이 같은 가상 포트가 둘 생기지 않는다 | 아니오 | `add_named_virtual_port` → `check_new_name` + `create_virtual_output`의 `duplicate_key` |
| 앱 가상 포트 16개 상한(프로세스 단위) | 아니오 | keyscore-midi `PORT_NAMES`. `plan`의 셈이 지운 뒤 아직 놓지 않은 포트를 빼고 세지만, 그때는 만들기가 실패하고 트랙은 그대로라 상한은 지켜진다 |
| 창마다 흐름 하나 | 아니오 | `run_file_flow` |
| 「적용 결과는 시트에 보인 제안이다」 | 예 | 지적 2 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-8 | 부분 충족 | 제안 시트·명령·적용은 있다(`port_mapping.rs`, `files.rs:139`, `autosave.rs:589`). 다만 적용이 보이지 않은 제안을 할 수 있다(지적 2), 시트 글자가 깨질 수 있다(지적 3) |
| OUT-3 | 미충족(이 Task가 회귀를 만든다) | 매핑으로 만든 가상 포트가 같은 이름의 돌아온 장치를 가린다(`engine.rs:1084` `open_port`, `engine.rs:200` `port_choices`). 지적 1 |
| OUT-2 | 충족 | 이름 규칙·중복·16개 상한을 기존 검사로 지킨다(`engine.rs:275`) |
| UI-11 | 충족 | `commands/table.rs:785`, `commands` 시험 통과 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 메인 스레드 코드만 바꾼다).
- 제품 코드의 `unwrap`/`expect`: 없음(`unwrap_or_default`는 기본 포트 이름이 없을 때 빈 문자열로 표시만 한다).
- 에러가 삼켜지는 곳: `ask_port_mapping`은 그 창의 흐름이 이미 돌면 아무 말 없이 시트를 띄우지 않는다(`run_file_flow`). 명령 `midi::MapMissingPorts`를 다른 시트가 떠 있는 창에서 메뉴로 부르면 반응이 없다(제안 4).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 매핑으로 만든 가상 포트가 같은 이름 장치의 재연결을 영구히 막는다. 같은 Mac에서 USB 신디를 꺼 둔 채 프로젝트를 열어도 시트가 뜬다(판정은 「다른 Mac」을 구별하지 못한다). 받아들이면 그 장치를 다시 켜도 엔진은 앱 포트를 먼저 열어 노트가 장치로 가지 않고, 포트 메뉴에서도 장치가 사라진다. 사용자가 설정 > MIDI 출력에서 그 포트를 지워야 풀린다. REQ OUT-3 「같은 이름의 포트가 돌아오면 자동 재연결」과 어긋나며, 결정 `2026-10-22-out8-port-mapping-sheet.md` 2 A의 선택지 설명에 이 대가가 없었다(S2-T6d 보고 「알려진 한계」의 앱 포트 우선 규칙이 이제 흔한 경로가 된다). 취향이 아니라 REQ 충돌이므로 임의로 고치지 말고 결정 요청을 쓴다. 선택지 예: (a) 시트 줄에 「장치가 돌아와도 이 가상 포트로 간다」를 알리고 보고서 한계에 적는다, (b) 매핑으로 만든 포트는 같은 이름 목적지가 보이면 목적지에 양보한다, (c) 2 B(트랙을 기존 포트로)로 바꾼다. 재현: `review1_mapped_port_does_not_shadow_returning_device`(대조군 `review1_control_unmapped_track_reaches_returning_device`) | `port_mapping.rs:81-104`, `engine.rs:1084-1094`, `engine.rs:200-214` |
| 2 | 차단 | 적용 때 다시 계산한 결과가 시트와 다르면 보이지 않은 변경을 알림 없이 넣는다. 시트는 「같은 이름의 가상 포트를 만든다」였는데, 그 사이 앱 포트가 16개가 되면(여러 파일을 한꺼번에 열고 다른 창에서 먼저 매핑한 경우) 트랙을 기본 포트로 바꾸고 상태줄은 「가상 포트 0개를 만들고 트랙 1개를 바꿨다」뿐이다. 어느 이름이 왜 기본 포트로 갔는지 알 수 없다. OUT-8은 「제안」이고 결정 세부는 「결과는 상태줄에 요약한다」인데 요약이 그 결과를 담지 않는다. 고치는 방법: 다시 계산한 줄이 시트와 다르면 시트를 새 내용으로 다시 띄운다(권장), 또는 적어도 요약에 기본 포트로 보낸 이름과 이유를 적는다. 재현: `review1_stale_sheet_does_not_silently_fall_back_to_main` | `port_mapping.rs:168-234` |
| 3 | 차단 | `shown`이 C0/C1 제어 문자와 U+2028/2029만 고친다. 이름 규칙이 바로 「다른 이름처럼 보이게 한다」는 이유로 거부하는 양방향 제어 문자(U+202A–202E, U+2066–2069)와 폭 없는 문자(U+200B, U+FEFF 등)는 그대로 시트에 들어간다. RLO는 그 줄의 「기본 포트 「…」으로 바꾼다」 부분을 뒤집어 보여 제안 내용을 잘못 읽게 하고, U+200B는 멀쩡해 보이는 이름을 「쓸 수 없다」고 한다. 함수 주석(「시트 글자를 깨뜨리지 않게」)이 스스로 내건 목적을 지키지 못한다. `Mapping::Use`의 대상 이름도 `shown`을 거치므로 같은 문제가 있다. 고치는 방법: 일반 범주 `Cc`·`Cf`·`Zl`·`Zp`(허용 결합자 ZWJ·ZWNJ 정도만 예외)를 `\u{..}`로 적는다. 재현: `review1_sheet_line_escapes_bidi_and_invisible_characters` | `port_mapping.rs:129-140` |
| 4 | 제안 | 정규화 이름이 같은 두 이름에서 정규화하지 않은 쪽이 트랙 순서상 먼저면, 둘째 줄이 `Use(자기 이름)`이 되어 시트에 「「X」: 트랙 1개를 「X」로 바꾼다」가 나오고 적용은 같은 값 `SetTrackProp`을 넣어 「트랙 2개를 바꿨다」고 센다(실제 1개). 결과 포트는 맞다. `mapping_for`에서 `Use(n)`의 `n == saved`면 줄을 「첫 줄이 만들 포트를 그대로 쓴다」로 보이고 편집에서 빼면 된다. 재현: `review1_already_normalized_second_name_is_not_a_change` | `port_mapping.rs:88-104`, `port_mapping.rs:197` |
| 5 | 제안 | 구현자 시험 14 `apply_recomputes_when_port_appeared_meanwhile`는 다시 계산하지 않는 구현에서도 통과한다. 다시 계산하지 않으면 `add_named_virtual_port`가 `DuplicateName`으로 실패하고, 포트 수(1개)·트랙(그대로)은 같으며, 요약은 「만들지 못한 포트: …」인데 시험은 「못했다」만 찾는다. 「만들지 못한」 또는 「가상 포트 0개」를 확인하도록 바꾼다 | `tests/port_mapping.rs:393` |
| 6 | 제안 | `Main(TooMany)` 설명은 「가상 포트가 이미 16개라」지만, 같은 시트의 앞 줄이 만들 포트로 16개에 이르는 경우(앱 포트 15개 + 없는 이름 둘)도 이 문구다. 「16개를 넘어」 정도로 바꾼다 | `port_mapping.rs:122-124` |
| 7 | 제안 | `midi::MapMissingPorts`를 이미 흐름(저장·닫기 시트 등)이 도는 창에서 부르면 아무 반응이 없다(`run_file_flow`가 조용히 무시). 메뉴에서 부를 수 있으므로 상태줄 한 줄이 있으면 좋다 | `port_mapping.rs:242-256`, `files.rs:306-321` |

판정에 쓴 구분: 지적 1은 REQ OUT-3과 결정 2 A 사이의 충돌(결정 요청 필요), 지적 2는 OUT-8 「제안」과 결정 세부 「요약」의 미충족, 지적 3은 구현이 스스로 내건 불변식과 결정 `2026-10-15-port-name-visual-duplicates.md`·keyscore-midi 이름 규칙의 취지 위반이다. 제안 4–7은 취향 또는 시험 강도다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 2·3은 이 Task 안에서 고칠 수 있다. 지적 1은 결정 요청(`docs/decisions/`)을 쓰고 사람의 답에 따라 고친 뒤 재리뷰한다. 결정이 (a)면 리뷰 시험 `review1_mapped_port_does_not_shadow_returning_device`의 기대(시트 문구)는 결정 문구에 맞춰 조정해도 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/12-S2-app-shell.md`의 나누기 줄과 카탈로그 갱신은 결정 문서의 「나누기」에 따른다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
