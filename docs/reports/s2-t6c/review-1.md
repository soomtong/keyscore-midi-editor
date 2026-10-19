# 적대적 리뷰: S2-T6c 설정 > 입력·메트로놈·AI (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: 34eeee1 (`s2/t6c-settings-tabs`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t6c.rs` (`cargo test -p keyscore-app --test review_s2_t6c`, 13개 중 12 통과·1 실패)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일(`tests/settings_tabs.rs`)을 열기 전에 `src` 변경만 읽고 적었다.

1. 재생 중 메트로놈 포트를 이름 → 가상으로 바꾸면 클릭이 가상 포트로 옮기고 이름 포트에는 더 가지 않는가.
2. 재생 중 목록에는 있지만 이미 사라진 이름을 고르면 클릭이 멈추고(다른 포트로 새지 않고) 알리는가. 다시 있는 포트로 고르면 돌아오는가(`forget_missing`, `known_missing`).
3. 메트로놈 포트가 트랙 포트와 번호를 같이 쓰다가 다른 이름으로 바뀔 때, 스냅샷마다 다시 매겨지는 번호 때문에 옛 번호(트랙 포트)로 클릭이 새지 않는가.
4. 재생 중 메트로놈 이름 포트가 사라졌다 돌아올 때(포트 감시 `wanted_names`·`windows_using`) 사라진 동안 새지 않고 돌아오면 다시 울리는가.
5. 설정 창이 열린 채 새 목적지가 생기면(`ports_changed`) 포트 목록에 들어오는가.
6. 설정 창을 닫았다 다시 열 때 선택 상자가 지금 설정과 맞고, 첫 컨트롤 포커스가 맞는가.
7. 검색으로 거른 목록에서 고를 때, 걸러진 목록의 자리(row)가 아니라 원래 값이 들어가는가(`selected_index`를 읽는 구독).
8. 파일의 목록 밖 길이(75 ms)가 목록에 덧붙고, 다른 값으로 바꿀 수 있는가.
9. 엔진 없이 메트로놈 값을 바꾸고 재생·버튼을 눌러도 무너지지 않는가.
10. 키보드: 선택 상자를 `space`로 여는가(계획 끝의 정의 5), 보정 마법사 칸을 `Tab`으로 닿고 `space`로 여는가, 팔레트로 바꾼 레이아웃이 열린 창의 선택 상자에 맞는가.
11. 설정 파일 경계: 빈 이름, 여분 키가 붙은 포트 객체, 소수 채널, 문자열 숫자, `u32` 밖 길이, 대문자 레이아웃 이름, 모르는 키 보존.
12. 스케줄러에 `Swap`·`Metronome`·`MetronomeEnabled`가 따로 들어가는 순서에서 옛 메트로놈 설정(옛 포트 번호)이 새 스냅샷에 쓰이는 틈.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 이름 → 가상(재생 중) | 없음 | 6번 테스트는 정지 중 이름 포트만 본다 |
| 2 재생 중 사라진 이름 고르기 | 없음 | 6번은 정지 중 `AppSettings::update`로 넣고 재생 시작 |
| 3 트랙과 같이 쓰던 번호에서 떠나기 | 없음 | |
| 4 메트로놈 포트 소멸·복귀(감시) | 없음 | 계획 끝의 정의 3의 마지막 항목인데 테스트 목록에 없다 |
| 5 핫플러그 뒤 목록 갱신 | 없음 | |
| 6 닫았다 다시 열기 | 없음 | |
| 7 검색으로 고르기 | 부분 | `choose`가 번호로 검색한다. GM 이름 검색은 없음 |
| 8 목록 밖 길이 | 없음 | |
| 9 엔진 없음 | 없음 | |
| 10 space로 열기, 보정 칸 키보드 | 없음 | 7번은 `enter`만 쓴다 |
| 11 파일 경계 | 부분 | 1번이 범위 밖·형식 오류는 본다. 빈 이름·소수·문자열 숫자는 없음 |
| 12 명령 순서의 틈 | 없음 | |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 재생 중 이름 포트 → 가상 포트로 바꾸고 700 ms 동안 이름 포트 수신 | 못 깨뜨림 | `brk_named_to_virtual_while_playing_moves_clicks` |
| 2 | 재생 중 사라진 이름 Y를 고름(감시 없음): 클릭 멈춤·표시·알림, X로 되돌리면 복귀 | 못 깨뜨림 | `brk_switch_to_vanished_name_while_playing_silences_then_recovers` |
| 3 | 메트로놈을 트랙 포트 C(번호 공유)에서 A로 바꾸고 C 수신 | 못 깨뜨림 | `brk_metronome_leaves_shared_track_port` |
| 4 | 재생 중 메트로놈 이름 포트 소멸(감시) → 가상 포트로 새는지, 복귀 뒤 다시 울리는지 | 못 깨뜨림 | `brk_metronome_port_vanish_and_return_while_playing` |
| 5 | 설정 창이 열린 채 새 목적지 생성 → 검색해 고르기 | 못 깨뜨림 | `brk_port_list_refreshes_on_hotplug` |
| 6 | 설정 창 닫기 → 밖에서 강박 56 → 다시 열기: 라벨·포커스·고르기 | 못 깨뜨림 | `brk_reopen_settings_keeps_selection_in_sync` |
| 7 | GM 이름(`Cowbell`, `Claves`)과 「1」 검색으로 고르기 | 못 깨뜨림(56, 75, 1) | `brk_search_by_gm_name_picks_right_note` |
| 8 | 파일 길이 75 ms → 라벨, 다시 75 고르기, 500으로 바꾸고 저장 | 못 깨뜨림 | `brk_length_from_file_shown_and_replaceable` |
| 9 | 엔진 없이 채널·약박 바꾸고 메트로놈·재생 | 못 깨뜨림 | `brk_metronome_change_without_engine` |
| 10 | 입력 구역에서 `Tab` → 보정 칸, `space` | 못 깨뜨림 | `brk_calibrate_by_space` |
| 11 | 첫 컨트롤(키 레이아웃)에서 `space` → `down` → `enter` | 깨짐: 값이 Piano 그대로(`left: Piano, right: Solfege`). 트랜스포트는 `Stopped`라 space가 재생으로 새지는 않았다 | `brk_select_opens_with_space` |
| 12 | 팔레트 `SetKeyLayout(Piano)` 뒤 열린 창의 라벨과 선택 커서 | 못 깨뜨림 | `brk_key_layout_external_sync` |
| 13 | 파일 경계: `{"named": ""}`, 채널 `9.0`, 강박 `"37"`, 길이 `4294967296`, 레이아웃 `"Piano"`, 모르는 키 `x`·`y`·`z` | 못 깨뜨림(경고 5, 기본값, 모르는 키 보존) | `brk_settings_file_edge_values` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 모르는 키(최상위·`input`·`metronome`)는 쓸 때 남는다 | 아니오 | 시도 13 |
| 엔진에 넘기는 메트로놈 값은 범위 안 | 아니오. `MetronomeSettings`의 `pub` 필드로 범위 밖 값을 넣을 수 있지만 `config()`가 채널·노트는 기본값으로, 길이는 `clamp_length`로 고친다 | `settings.rs` `config` |
| 클릭은 메트로놈 포트가 이 스냅샷에 있을 때만 켠다 | 정상 경로에서는 아니오(시도 2·4). 재생 중 교체의 명령 순서에 짧은 틈이 있다(지적 2, 재현 못 함) | `transport.rs:228–240` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| MET-1 (지정 포트·채널·노트·길이, 기본 10/37/42, 버튼에 따름) | 충족(녹음·카운트인 강제 클릭은 계획대로 S5) | `settings.rs` `MetronomeSettings`, `transport.rs:228–240, 316–321`, 시도 1–4 |
| MET-1 「포트·채널·노트·길이는 설정 > 메트로놈」 | 충족 | `settings_tabs.rs` `render_metronome` |
| P3 (설정에서 레이아웃 하나를 고름) | 충족(저장까지. 해석은 S3) | `settings_window.rs` `SetKeyLayout` 처리기, 시도 12 |
| 7.6 메트로놈 포트·채널·노트 | 충족 | 위와 같음 |
| 7.6 레이턴시 보정 「설정 > 입력 > 보정 마법사」, 팔레트 「보정 마법사」 | 충족(자리. 마법사는 S5-T2) | `settings_tabs.rs` `render_input`, `commands/table.rs:1356` |
| 7.6 키 레이아웃 「설정 > 입력」 | 충족 | `settings_tabs.rs` `render_input` |
| 7.6 AI 설정 항목 다섯 자리 | 충족(비활성 자리, 내용은 S7) | `settings_tabs.rs` `render_ai` |
| 5.11 「키맵 편집 · 설정 · 대화상자: `Tab` 다음 필드 / `Shift+Tab` 이전 필드」 | 충족 | `settings_sections_keyboard_reachable`, 시도 10 |

REQ 미충족은 찾지 못했다. 지적 1은 REQ가 아니라 계획 끝의 정의 5와의 불일치다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 이 Task는 스케줄러(`keyscore-midi`)를 고치지 않았다. 새 명령 없이 기존 `set_metronome`·`set_metronome_enabled`를 부른다. 없음.
- 제품 코드의 `unwrap`/`expect`: 새로 더한 것 없음(`u16::try_from(..).unwrap_or(u16::MAX)`은 기존 트랙 경로와 같은 모양이고 포트 수가 32 이하라 닿지 않는다).
- 에러가 삼켜지는 곳: 재생 중 교체의 `set_metronome`·`set_metronome_enabled` 실패는 `send_error`로 모아 「바뀐 뱅크·프로그램을 보내지 못했다」 문구로 알린다. 메트로놈 실패인데 문구가 뱅크·프로그램을 말한다(제안 2).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 선택 상자가 `space`로 열리지 않는다. 계획 끝의 정의 5는 「선택 상자는 `Enter`/`space`로 열고 화살표로 고른다」인데 `Enter`만 된다(gpui-kit `Select` 트리거에 space 처리가 없다). 보정 칸은 직접 `on_key_down`으로 space를 받아 되는데 선택 상자는 같은 처리가 없다. 보고서의 「계획 대비 달라진 점」에도 없다. 재현: `brk_select_opens_with_space` 실패(`left: Piano, right: Solfege`). 고치거나 계획을 고쳐 「Enter만」으로 적고 그 이유를 보고서에 남긴다 | `settings_tabs.rs:461–472`(`row`) |
| 2 | 수정 | 재생 중 교체에서 `swap(snapshot)` → `set_metronome(config(port))` → `set_metronome_enabled`가 따로 큐에 들어간다. 실시간 스레드가 그 사이에 깨면(명령을 넣는 동안 큐가 비어 있는 순간) 새 스냅샷에 옛 메트로놈 설정(옛 포트 번호)으로 클릭을 하나 예약할 수 있다. 포트 번호는 스냅샷마다 다시 매겨지므로(보고서 「포트 번호는 스냅샷마다 다시 매겨진다」), 예를 들어 메트로놈이 이름 포트 A(번호 2)이고 재생 중 편집으로 이름 포트 D 트랙이 앞에 끼면 옛 번호 2가 D가 되어 클릭이 D로 갈 수 있다. 계획의 불변식 「클릭은 메트로놈 포트가 이 스냅샷에 있을 때만 켠다」를 짧은 틈 동안 어긴다. 타이밍 경쟁이라 재현 테스트로 잡지 못했다(시도 1–3은 통과). 교체 전에 `set_metronome_enabled(false)`를 먼저 넣거나, 메트로놈 포트 번호를 스냅샷에 싣는 식으로 순서를 닫는다. 재생 시작 경로(`transport.rs:316–321`)는 `stop` 뒤라 해당 없음 | `transport.rs:228–240` |
| 3 | 제안 | 테스트 목록에 메트로놈 포트 소멸·복귀(계획 끝의 정의 3 마지막 항목)와 재생 중 포트 바꾸기 테스트가 없다. 리뷰 테스트 `brk_metronome_port_vanish_and_return_while_playing`, `brk_switch_to_vanished_name_while_playing_silences_then_recovers`, `brk_named_to_virtual_while_playing_moves_clicks`를 PR에 남길 것을 권한다 | `tests/settings_tabs.rs` |
| 4 | 제안 | 재생 중 교체에서 메트로놈 명령 실패도 `send_error`로 묶여 「바뀐 뱅크·프로그램을 보내지 못했다」로 알린다. 원인과 다른 문구다 | `transport.rs:231–240, 268–273` |
| 5 | 제안 | `Engine::owner`를 끼워 넣으면서 `has_owner`의 doc 주석(「재생 중인 창이 있는가.」)이 `owner` 위로 밀렸다. `owner`에는 주석이 둘, `has_owner`에는 없다 | `engine.rs:310–324` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(실행해 깨진 계획 불일치)과 지적 2(클릭이 잘못된 포트로 갈 수 있는 순서 틈)를 고친 뒤 다시 본다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`commands/table.rs` 자리 설명 한 줄은 보고서에 적혀 있다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
