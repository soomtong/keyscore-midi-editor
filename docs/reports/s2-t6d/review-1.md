# 적대적 리뷰: S2-T6d 설정 > MIDI 출력의 가상 포트 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 700f138 (`s2/t6d-midi-output`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t6d.rs`(GUI 테스트는 모두 자식 프로세스에서 돈다), `crates/keyscore-midi/tests/review_s2_t6d.rs`

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `set_name`의 잠금 순서(이름 목록 → 포트 이름)가 만들기·drop·`reregister`·`MidiPort::name`과 엇갈리지 않는가. 동시에 이름 바꾸기·만들기·버리기·송신을 돌려도 교착하지 않는가.
2. 실시간 스레드의 `send`·스케줄러가 `name()`(잠금)을 부르지 않는가.
3. 재생 중 추가·이름 변경이 이미 그 이름을 기다리던 트랙을 다시 잇는가(지우기는 `transport_refresh`를 부르는데 추가·이름 변경은 트랙 편집이 없으면 부르지 않는다). 상태줄 「포트 없음」과 실제 소리가 맞는가.
4. 지운 뒤 곧바로 추가: `retired`가 이름 목록 자리를 쥔 채면 같은 번호를 만들 수 없다.
5. 설정 파일 `virtual_ports`가 지금 보이는 목적지 이름과 같으면? UI 경로(`check_new_name`)는 거부하는데 설정 파일 검사는 이 규칙을 보지 않는다. 그 목적지를 쓰던 트랙이 앱 포트로 가로채인다.
6. 기본 포트 이름 변경: `Named(옛 기본 이름)` 트랙만 고치고 `Virtual`은 그대로, undo·redo가 창마다 한 단위.
7. 보이는 모양 중복(NBSP, ZWJ) 이름으로 바꾸기 거부.
8. 삭제 Action: 설정 창이 없을 때, 기본 포트가 골라진 채 키보드로 꺼진 버튼을 누를 때.
9. 서버 재시작 복구(`reregister_main`)가 추가 포트도 다시 등록하는가. 추가 포트 하나가 실패하면 감시자 재시작·재시도는?
10. 메트로놈 구역의 포트 목록이 추가·이름 변경·삭제를 따라가는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 잠금 순서·동시 실행 | 없음 | `virtual_output_renames_while_shared`는 한 스레드 |
| 2 실시간 경로 잠금 | 없음(코드 확인 대상) | `scheduler.rs`·`watcher.rs`에 `name()` 호출 없음 |
| 3 재생 중 추가·이름 변경의 재연결 | 없음 | 구현자 테스트는 이미 그 포트를 쓰던 트랙만 본다 |
| 4 지운 뒤 추가 | 없음 | |
| 5 설정 파일의 목적지 이름 | 없음 | `midi_output_settings_round_trip`은 규칙·중복·16개만 |
| 6 기본 포트 이름 변경 | 없음 | 구현자 테스트는 추가 포트만 바꾼다 |
| 7 보이는 모양 중복 | 일부 | `rename_rejects_bad_names`는 같은 문자열만 |
| 8 삭제 Action 경계 | 일부 | 클릭만, 설정 창 없음·키보드 경로 없음 |
| 9 복구 재등록 | 없음 | |
| 10 메트로놈 목록 | 없음 | |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 재생 중인 트랙이 「기본 이름 2」(아직 없음)를 가리킬 때 `add_virtual_port`로 그 이름을 만든다. 감시 5주기 뒤 상태줄과 소리 | 깨짐. 상태줄 「포트 없음」은 사라지는데 노트가 2 s 안에 새 소스로 오지 않는다 | `add_virtual_port_during_playback_reconnects_waiting_track` |
| 2 | 재생 중 추가 포트 이름을 재생 중인 트랙이 기다리던 이름으로 바꾼다 | 깨짐. 노트가 2 s 안에 오지 않는다 | `rename_during_playback_reconnects_waiting_track` |
| 3 | 가상 목적지 D를 만들고 설정 파일 `virtual_ports: [D]`로 시작, 트랙 `Named(D)` 재생 | 깨짐. 설정 파일 검사가 받아들여 앱 포트 D가 생기고(`names = [main, D]`), D 목적지로 노트가 가지 않는다 | `settings_virtual_port_named_like_destination_is_rejected` |
| 4 | 이름 바꾸기 400회 + 같은 이름 노린 만들기·버리기 400회 + `name()`·`send` 400회를 세 스레드로 | 못 깨뜨림(0.12 s, 교착 없음, 끝난 뒤 현재 이름으로 새로 만들면 거부) | `set_name_races_with_create_drop_and_send_without_deadlock`(midi) |
| 5 | 재생 중 지우고 곧바로 추가, 정지 뒤 다시 지우고 추가 | 못 깨뜨림. 두 번 다 「기본 이름 2」를 다시 쓴다(200 ms 대기 안에 스냅샷이 놓는다) | `remove_then_add_reuses_name` |
| 6 | 프로젝트 창에서 `RenameVirtualPort`(설정 창 없음)로 기본 포트 이름 변경. `Virtual`·`Named(기본)` 두 트랙, undo·redo, 재생 | 못 깨뜨림 | `rename_main_from_project_window_updates_named_main_tracks` |
| 7 | 서버를 끄지 않고 `recover_virtual_port`를 불러 추가 포트 재등록 뒤 재생 | 못 깨뜨림 | `recover_reregisters_extra_ports` |
| 8 | 설정 창 없이 `RemoveVirtualPort` → 설정 창이 열리고 아무것도 지우지 않음. 기본 포트를 고르고 Tab×3 → 삭제 칸에서 space·enter | 못 깨뜨림(라벨 「삭제(꺼짐)」, 포트 그대로) | `remove_action_without_settings_window_and_main_by_keyboard` |
| 9 | 다른 가상 포트 이름에 NBSP·ZWJ를 끼운 이름으로 바꾸기 | 못 깨뜨림(둘 다 거부) | `rename_rejects_visual_duplicate_of_other_port` |

실행: `cargo test -p keyscore-app --test review_s2_t6d` → 5 passed, 3 failed. `cargo test -p keyscore-midi --test review_s2_t6d` → 1 passed. 두 파일 모두 `cargo clippy -p keyscore-midi -p keyscore-app --all-targets -- -D warnings`, `cargo fmt --all -- --check` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 앱 가상 포트 합쳐 16개 이하 | 아니오 | 프로세스 전역 `PORT_NAMES`(구현자 `add_virtual_port_stops_at_sixteen`), 설정 파일은 15개에서 자른다(`settings.rs:336`) |
| 보이는 모양 기준 이름 중복 없음 | 아니오 | 시도 4·9 |
| 기본 포트는 스냅샷 포트 0이고 지울 수 없다 | 아니오 | 시도 8, `engine.rs:310` |
| 설정 `virtual_ports` = 엔진의 추가 포트(추가·이름 변경·삭제가 함께 바꾼다) | 아니오(설치 실패 항목은 설정에 남는다는 보고서의 좁힌 정의 기준) | |
| 실시간 송신 경로에 잠금 없음 | 아니오 | `client.rs`의 `send`는 `endpoint`만 쓴다. `scheduler.rs`에 `name()` 호출 없음 |
| 「앱 포트 이름은 지금 보이는 목적지 이름과 다르다」(계획 3의 이름 규칙) | 예 | 설정 파일 `virtual_ports`/`main_name`(시도 3) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 | 충족 | `lib.rs:155-160`(설정의 기본 이름 또는 "KeyScore Out"), `engine.rs:356-397` |
| OUT-2 | 충족 | 추가·이름 변경·삭제 `engine.rs:205-334`, 16개 상한, 설정 > MIDI 출력 `settings_ports.rs` |
| OUT-3 | 부분 충족(미충족으로 센다) | 재생 중 앱 포트가 그 이름으로 「나타나도」 기다리던 트랙을 잇지 않고 상태줄만 「있음」이 된다(지적 1). 설정 파일로 목적지와 같은 이름의 앱 포트가 생기면 그 목적지가 포트 메뉴(`engine.rs:177-182`)에서 사라지고 라우팅도 가로채인다(지적 2). 서버 재시작 재등록은 충족(`engine.rs:527-543`) |
| OUT-8(이 Task 몫: 이름으로 가리킴, 이름 변경 때 열린 트랙 고침) | 충족 | `engine.rs:275-294`, 결정 2026-10-19 A |
| 7.6 「가상 포트 추가·이름 변경 = 설정 > MIDI 출력」 | 충족 | `settings_ports.rs`, `commands/table.rs` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `VirtualOutput::send`는 이름 잠금을 쓰지 않고, 스케줄러는 `name()`을 부르지 않는다.
- 제품 코드의 `unwrap`/`expect`: 새로 든 것 없음. 잠금 독(poison)은 `unwrap_or_else(PoisonError::into_inner)`로 받는다.
- 에러가 삼켜지는 곳
  - `engine.rs:327-328` 정지 중 삭제의 빈 스냅샷 교체 실패(`let _`). 실패하면 지운 포트를 옛 스냅샷이 계속 쥐어 소스가 다음 감시 주기 이후까지 남는다. 알림 없음.
  - `engine.rs:540-545` 추가 포트 재등록 실패는 `recover`의 오류로 합쳐지는데, 아래 제안 1처럼 그 뒤 처리가 기본 포트 실패와 같아 재시도되지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 재생 중 `add_virtual_port`·`rename_virtual_port`가 스냅샷을 다시 만들지 않는다. 재생 중인 트랙이 그 이름을 기다리고 있었으면(옛 프로젝트의 포트 이름으로 새 포트를 만들거나 이름을 맞춘 경우) 소리는 다음 편집·재생 시작까지 나가지 않는데, `port_present`는 앱 포트를 「있음」으로 보고 감시(`wanted_names`)는 앱 포트를 빼므로 상태줄은 「포트 없음」을 지우고 재연결도 일어나지 않는다. 목적지가 돌아올 때의 자동 재연결(OUT-3)과 다르게 동작하고 상태줄이 실제 소리와 어긋난다. 재현: `add_virtual_port_during_playback_reconnects_waiting_track`(실패: 「상태줄은 포트가 있다는데 재생 중인 트랙의 소리가 새 가상 포트로 나가지 않는다」), `rename_during_playback_reconnects_waiting_track`(실패: 「재생 중인 트랙이 기다리던 이름으로 바꿨는데 소리가 나가지 않는다」) | `engine.rs:205-235`, `engine.rs:243-297`, `ports.rs:77-79`, `ports.rs:146-148` |
| 2 | 수정 | 설정 파일 검사가 계획 3의 이름 규칙 가운데 「지금 보이는 목적지 이름과 같으면 거부」를 보지 않는다. 손 편집한 `virtual_ports`(또는 `main_name`)가 목적지 이름이면 설치가 그 이름의 앱 포트를 만들고, 그 목적지를 쓰던 프로젝트의 트랙은 앱 포트로 간다. 그 목적지는 포트 메뉴에서도 빠져 고를 수 없다. 보고서의 알려진 한계(「나중에 생긴 목적지」)와 달리 설치 시점에 이미 보이는 목적지다. 재현: `settings_virtual_port_named_like_destination_is_rejected`(실패: 「목적지 「KS rv6d … hijack-dest」을 쓰던 트랙의 소리가 목적지로 가지 않는다(가상 포트: [… hijack, … hijack-dest])」) | `settings.rs:297-349`, `engine.rs:363-379`, `lib.rs:155-160` |
| 3 | 제안 | 서버 재시작 복구에서 추가 포트 하나라도 재등록에 실패하면 `recover`가 `Err`를 돌려 `restart_watcher`를 부르지 않는다. 그런데 기본 포트는 이미 등록돼 `tick`의 `!main_registered()` 조건이 거짓이라 다시 시도하지 않는다. 그 뒤로 포트 감시(OUT-3)가 옛 서버의 감시자에 머문다. 실패 메시지도 기본 포트 이름만 말한다. MIDI 서버 재시작·실패 주입을 할 수 없어 재현 테스트는 없다(코드 읽기) | `engine.rs:451-470`, `engine.rs:540-545`, `ports.rs:275-279` |
| 4 | 제안 | `check_new_name`은 「지금 보이는」 목적지만 본다. 잠시 꺼진 장치의 이름으로 가상 포트 이름을 바꾸면 그 장치를 쓰던 열린 트랙이 앱 포트로 가고, 장치가 돌아와도 감시는 그 이름을 보지 않는다. 계획대로이지만 이름 변경 때 「이 이름을 쓰는 트랙이 있다」 정도는 알리는 편이 낫다 | `engine.rs:188-197` |
| 5 | 제안 | `port_choices`는 앱 포트와 목적지를 문자열 그대로 비교한다. 이름 규칙은 보이는 모양 기준이라, 보이는 모양만 같은 목적지가 메뉴에 앱 포트와 나란히 두 줄로 보일 수 있다 | `engine.rs:177-182` |
| 6 | 제안 | 추가·이름 변경·삭제가 설정 창의 메트로놈 포트 목록을 「다시 읽기」로 표시하지 않는다(`ports_changed`는 핫플러그에서만). 메트로놈 구역을 보는 동안 프로젝트 창의 키맵으로 이름을 바꾸면 선택 상자 목록이 옛 이름을 보인다(구역을 다시 열면 맞는다). 코드 읽기 | `settings_tabs.rs:217-219`, `engine.rs:232-234`, `engine.rs:295` |
| 7 | 제안 | 정지 중 삭제의 빈 스냅샷 교체 실패를 삼킨다. 실패하면 알린다 | `engine.rs:327-328` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2(수정)를 고치고 위 두 리뷰 테스트 파일을 PR에 더한다. 지적 2에서 설정 파일의 목적지 이름 항목을 빼는지(경고) 아니면 만들되 알리는지는 계획 3의 이름 규칙에 맞춰 구현자가 정하고, 바꾸면 리뷰 테스트의 기대(`names == [main]`)를 그 결정에 맞춘다(입력은 그대로).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(기존 midi 테스트 11곳은 `MidiPort::name` 반환형 변경에 따른 시그니처만, 계획 파일 변경은 Task 나눔 커밋)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음, Cargo.toml·Cargo.lock 변경 없음)
- [x] 스타일 지적을 적지 않았다
