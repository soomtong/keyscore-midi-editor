# 적대적 리뷰: S2-T7d 없는 출력 포트의 대체 매핑 제안(OUT-8) (리뷰 2회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음). 1회차 리뷰어와도 다른 세션이다
- 대상 커밋: `d069a47`(브랜치 `s2/t7d-port-mapping`, 1회차 뒤 수정 08f3fe9, 0f1eaad, 494e5aa, 5a7b153)
- 리뷰 시험: `crates/keyscore-app/tests/review_s2_t7d_r2.rs`(4개). 결함을 증명하는 3개는 지금 실패하고, 대조군 1개는 통과한다. 모두 CoreMIDI 목적지를 만들거나 포트 감시를 돌려 자식 프로세스에서 혼자 돈다. 실행: `cargo test -p keyscore-app --test review_s2_t7d_r2`
- 기존 시험: `--test port_mapping`(20 통과), `--test review_s2_t7d`(5 통과, 1회차 리뷰 시험 모두 통과)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`port_mapping.rs`, `engine.rs`의 `add_mapped_virtual_port`·`release_mapped`·`remove_virtual_port`·`rename_virtual_port`·`install`·`open_port`·`port_choices`, `ports.rs`의 `tick`·`release_returned_devices`·`wanted_names`, `settings.rs`의 `mapped_ports` 읽기·쓰기, `transport.rs`의 `transport_document_changed`·`transport_port_returned`를 읽고 적었다.

1. `release_mapped`가 지운 뒤의 다시 준비(`remove_virtual_port` → `transport_refresh`)가 일반 재연결(`transport_port_returned`)과 같은 일을 하는가. 특히 음색 기록(`transport.setup`, 목적지 이름이 열쇠)을 지우는가. 같은 이름 아래 앱 포트에서 장치로 바뀐다.
2. 「같은 이름」 판정이 모든 경로에서 맞는가. `release_mapped`와 `install`은 정확히 같은 이름을 보고, `port_choices`·`install`의 `collides`·`check_new_name`은 보이는 모양을 본다. 시트는 정규화하지 않은 이름을 정규화한 이름으로 만들고 트랙을 바꾼다. 그 장치가 원래 이름으로 돌아오면 어떻게 되는가.
3. `install`이 만들지 않은 포트(`collides`, 만들기 실패)의 표시가 `mapped`와 설정에 남는가. 남으면 나중에 다른 포트가 그 이름이 될 때 사용자 포트에 표시가 붙는가.
4. `open_port`가 장치를 먼저 여는 규칙: 장치가 없을 때 `open_named`가 `known_missing`에 넣는 부작용, 메트로놈·클럭 포트(`extra_port`)도 같은 길을 타는가.
5. 시트를 다시 묻는 반복: 사용자 입력 없이 도는 경우, 창이 닫힌 뒤.
6. 설정 `mapped_ports`: 손으로 고친 파일(없는 이름, 겹침, 정규화 전 이름, 글자 아님), 비었을 때 키를 쓰지 않는가.
7. 시작 때 설정에서 빼는 일의 알림이 사용자에게 닿는가(`install`은 창보다 먼저 돈다).
8. 이름 변경: 표시 지우기, 같은 이름으로 다시 바꾸기.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 재생 중 해제 뒤 음색 | 없음 | `returning_device_removes_mapped_port`는 정지 중만 본다. 깨짐(지적 1) |
| 2 정규화한 이름의 장치 | 없음 | 깨짐(지적 2) |
| 3 시작 때 남는 표시 | 없음 | 깨짐(지적 2의 시작 경로, 7절 #4) |
| 4 `open_port` 규칙 | 일부 | 리뷰 1 시험(정지 중 재생)이 통과. `extra_port`는 같은 `open_port`를 부른다(코드 대조). `known_missing` 삽입은 `release_mapped`의 `port_returned`가 지워 문제를 찾지 못했다 |
| 5 반복 | 있음 | `review1_stale_sheet_does_not_silently_fall_back_to_main`. 반복은 매번 `files::ask`를 기다리고, 창이 닫히면 `ask`가 `None`이라 끝난다(코드 대조) |
| 6 설정 읽기·쓰기 | 있음 | `settings_keep_only_mapped_names_that_are_virtual_ports`. 정규화 전 이름은 `virtual_ports`(정규화됨)와 달라 빠진다. 안전한 쪽이다 |
| 7 시작 때 알림 | 없음 | 7절 #3(제안) |
| 8 이름 변경 | 있음 | `returning_device_keeps_user_and_renamed_ports`. 같은 이름으로 바꾸면 `new == old`로 일찍 돌아와 표시가 남는다(괜찮다) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 프로그램 5인 트랙, 장치 없음 → [제안대로 매핑] → 재생 → 재생 중 장치가 돌아옴(포트 감시 돌림) | 깨짐: 대체 포트는 지워지고 노트는 장치로 가지만(2개) 프로그램 체인지는 0개. 장치가 기본 음색으로 소리 난다 | `review2_returning_device_during_playback_gets_program`(실패) |
| 2 | 1의 대조군: [그대로 두기] 뒤 재생 중 같은 장치가 돌아옴(일반 OUT-3 재연결) | 못 깨뜨림: 노트와 프로그램 체인지가 모두 간다. 1의 0개가 시험 장치 탓이 아니다 | `review2_control_plain_reconnect_during_playback_gets_program`(통과) |
| 3 | 저장된 이름이 `"KS  rv7d2 … wide synth"`(두 칸 공백)인 트랙 → [제안대로 매핑](정규화 이름의 포트를 만들고 트랙을 그 이름으로 바꿈) → 장치가 원래 이름 그대로 돌아옴 → 감시 여러 주기 → 재생 | 깨짐: 대체 포트가 남고, `port_choices`에 장치가 없고(보이는 모양이 같은 앱 포트가 이긴다), 장치로 노트 0개. 상태줄은 「새로 보인다」뿐 | `review2_unnormalized_device_name_returns`(실패) |
| 4 | 3의 시작 경로: 설정 `virtual_ports`·`mapped_ports`에 정규화 이름, 장치가 원래 이름으로 이미 있음 → `install` | 깨짐: 포트는 「목적지 이름이라 만들지 않았다」로 건너뛰지만 `virtual_ports`·`mapped_ports`에 남는다. 트랙(정규화 이름)은 「포트 없음」. 장치가 빠지면 다음 시작에 대체 포트가 다시 생긴다 | `review2_startup_unnormalized_device_keeps_stale_mapping`(실패) |
| 5 | 구현자 시험 `port_mapping`, 1회차 리뷰 시험 `review_s2_t7d` | 통과(20, 5). 1회차 지적 1–3, 제안 4–7의 수정이 그 시험으로 확인된다 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `mapped_ports`는 `virtual_ports`의 부분집합(설정 파일) | 아니오 | 읽기가 걸러내고, 지우기·이름 변경·시작이 함께 고친다 |
| 엔진의 `mapped`는 실제로 만든 추가 포트의 이름 | 예 | `install`이 `collides`·만들기 실패로 건너뛴 이름을 `mapped`에 남긴다(시도 4, 7절 #4) |
| 사용자가 만든 가상 포트는 장치가 돌아와도 지우지 않는다(결정 3 A) | 조건부 예 | 위 줄의 남은 표시가 있을 때 다른 사용자 포트를 그 이름으로 바꾸면 표시가 그 포트에 붙는다(`rename_virtual_port`는 옛 이름의 표시만 지운다). 코드 대조, 실행하지 않음(7절 #4) |
| 적용은 시트에 보인 제안과 같다(1회차 지적 2) | 아니오 | `ask_port_mapping`이 다시 계산한 줄이 다르면 다시 묻는다 |
| 시트 글자는 이름 규칙이 거부하는 문자를 보이게 적는다(1회차 지적 3) | 아니오 | `shown`이 `is_rejected_name_char`를 쓴다. `Create`의 대상 이름은 정규화를 통과한 이름이라 그 문자가 없다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-8 | 충족 | 시트·명령·다시 계산·적용(`port_mapping.rs`), 열기·복구(`files.rs:139`, `autosave.rs:589`) |
| OUT-3 | 부분 충족 | 정확히 같은 이름의 장치는 돌아오면 다시 잇는다(`ports.rs` `release_returned_devices`, `engine.rs:309`). 그러나 재생 중이면 음색을 다시 보내지 않고(지적 1), 시트가 이름을 정규화한 경우는 잇지 못한다(지적 2) |
| OUT-4 | 부분 충족 | 재생 중 해제 경로에서 돌아온 장치에 뱅크·프로그램이 가지 않는다(지적 1) |
| OUT-2 | 충족 | 이름 규칙·중복·16개 상한은 기존 검사(`add_named_virtual_port`) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(바뀐 코드는 메인 스레드다).
- 제품 코드의 `unwrap`/`expect`: 없음.
- 에러가 삼켜지는 곳: `install`이 시작 때 대체 포트를 설정에서 빼며 남기는 「장치 「…」이 있어 대체로 만든 가상 포트를 지웠다」는 `say_everywhere`로만 보낸다. `install`은 첫 창보다 먼저 돌아(`lib.rs:215`) 받을 창이 없고 `eprintln`도 없다(7절 #3).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 재생 중에 대체 포트의 장치가 돌아오면 노트는 장치로 가지만 뱅크·프로그램이 가지 않는다. `release_mapped`는 `remove_virtual_port`의 다시 준비(`transport_refresh` → `transport_document_changed`)에 맡기는데, 이 길은 창의 음색 기록(`transport.setup`)을 지우지 않는다. 기록의 열쇠가 목적지 이름이라, 대체 포트에 보냈던 프로그램이 「같은 목적지에 이미 보냄」으로 남고 `same_as` 비교가 장치에 보낼 것을 걸러낸다. 일반 재연결(`transport_port_returned`)은 바로 이 이유로 `setup.retain(|s| s.destination != name)`을 한다. 결과: 돌아온 신디가 정지·재생 전까지 틀린 음색으로 소리 난다(OUT-4, OUT-3 「자동 재연결」이 일반 경로보다 못하다). 고치는 방법 예: `release_mapped`가 재생 중 주인 창의 `transport_port_returned(destination)`을 부르거나, 지우기 전에 그 이름의 음색 기록을 지운다. 재현: `review2_returning_device_during_playback_gets_program`(대조군 `review2_control_plain_reconnect_during_playback_gets_program`) | `engine.rs:309-332`, `engine.rs:438-441`, `transport.rs:441-453` |
| 2 | 차단 | 시트가 이름을 정규화한 경우(저장된 이름에 두 칸 공백, 앞뒤 공백, NFD 등) 결정 3 A가 성립하지 않는다. 시트는 정규화 이름 `N`으로 포트를 만들고 트랙을 `N`으로 바꾼다(결정 「구현 세부」). 장치가 원래 이름 `S`로 돌아오면 `release_mapped`는 `N == S`만 보아 지우지 않고, `port_choices`는 `same_port_name(N, S)`라 장치를 메뉴에서 뺀다. 트랙은 계속 대체 포트로 가고, 사용자는 메뉴에서 장치를 고를 수도 없다(대체 포트를 손으로 지워도 트랙이 `N`이라 장치 `S`에 닿지 않는다). 리뷰 1 지적 1과 같은 갈래의 결함이 다른 경로로 남았다. 시작 경로도 같다: `install`은 정확히 같은 이름만 빼고, `collides`(보이는 모양)로 포트를 건너뛰며 설정·표시는 남겨 트랙이 「포트 없음」이 된다. 고치는 방법은 판정 기준(보이는 모양)과 트랙을 장치 이름 `S`로 되돌리는 일이 함께 필요하다. 트랙을 다시 고치는 것이 결정 3 A(「트랙은 장치로 다시 이어진다」)의 범위인지 애매하면 결정 요청을 쓴다. 재현: `review2_unnormalized_device_name_returns`, `review2_startup_unnormalized_device_keeps_stale_mapping` | `engine.rs:309-316`, `engine.rs:634-647`, `engine.rs:679`, `engine.rs:203-216`, `port_mapping.rs:104`, `port_mapping.rs:178-184` |
| 3 | 제안 | 시작할 때 대체 포트를 설정에서 뺀 알림이 어디에도 닿지 않는다. `install`은 첫 창보다 먼저 돌고(`lib.rs:215-220`) 이 메시지에는 `eprintln`도 없다. 사용자는 설정 > MIDI 출력에서 포트가 사라진 이유를 모른다. 첫 창을 연 뒤 보이거나 다른 시작 메시지처럼 `eprintln`을 더한다 | `engine.rs:648-653` |
| 4 | 제안 | `install`이 만들지 않은 대체 포트 이름(`collides`, `create_virtual_output` 실패)이 엔진 `mapped`와 설정 `mapped_ports`에 남는다. 그 상태에서 사용자가 자기 포트를 그 이름으로 바꾸면 `rename_virtual_port`는 옛 이름의 표시만 지워, 사용자 포트가 대체 포트로 표시되고 장치가 돌아오면 지워진다(결정 3 A 「사용자가 직접 만든 가상 포트의 규칙은 그대로」). `install`에서 `mapped`를 실제로 만든 `extras`로 좁히거나, 이름 변경이 새 이름의 표시도 지운다. 코드 대조만 했다(지적 2를 고치면 경로 대부분이 사라진다) | `engine.rs:675-707`, `engine.rs:362-365` |

1회차 항목 확인: 지적 1(정확히 같은 이름의 장치)은 정지 중 재생에서 고쳐졌다(`review1_mapped_port_does_not_shadow_returning_device`, `returning_device_removes_mapped_port` 통과). 다만 재생 중 경로(지적 1)와 정규화 경로(지적 2)가 남았다. 지적 2·3, 제안 4–7은 고쳐졌고 시험이 통과한다. 사용자가 직접 만든 포트와 이름을 바꾼 포트는 장치가 돌아와도 남는다(`returning_device_keeps_user_and_renamed_ports` 통과, S2-T6d 규칙 유지).

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1은 이 Task 안에서 고칠 수 있다. 지적 2는 판정 기준과 트랙 되돌리기를 함께 정해야 하며, 결정 3 A로 덮이지 않는다고 보면 결정 요청을 먼저 쓴다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(3번 반복 실행해 같은 결과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`keyscore-midi`의 `is_rejected_name_char` 공개는 1회차 지적 3의 수정이다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 없다
- [x] 스타일 지적을 적지 않았다
