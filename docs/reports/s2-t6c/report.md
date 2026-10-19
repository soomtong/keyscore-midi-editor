# Task 보고: S2-T6c 설정 > 입력·메트로놈·AI

- PR: 브랜치 `s2/t6c-settings-tabs`
- 계획 파일: docs/reports/s2-t6c/plan.md
- 계획 대비 달라진 점
  - 시험 API `focused_control`은 포커스를 묻느라 `Window`를 받는다(`focused_control(window, cx)`).
  - 명령 표의 `settings::SetKeyLayout` 자리 설명을 「설정 > 입력 > 레이아웃 선택 상자」으로 고쳤다.
  - 구역 전에 정한 결정: undo 상한은 사람 결정 A(`docs/decisions/2026-10-19-undo-history-memory-limit.md`). 구현은 S2-T6e다.

## 구현 요약

- `src/settings.rs`
  - `InputSettings { key_layout }`(기본 건반형)와 `MetronomeSettings { port, channel, accent, beat, length_ms }`(기본 가상 포트·10·37·42·50 ms)를 더했다.
  - 읽기는 필드마다 검사한다. 잘못된 필드는 그 필드만 기본값으로 두고 경고한다. 객체가 아니면 통째로 기본값이다. 두 객체 안과 최상위의 모르는 키는 보존한다.
  - 포트 모양은 `.ksq` 트랙 포트와 같다(`"virtual"` 또는 `{"named": 이름}`).
  - `MetronomeSettings::config(port)`가 스케줄러 설정(`MetronomeConfig`)을 만든다.
- `src/engine.rs`
  - `prepare(project, live, metronome_port)`: 메트로놈 포트를 트랙 포트와 같은 규칙으로 연다(같은 이름은 고유 ID 최소, 트랙이 이미 연 포트면 그 번호, 상한·없음은 이유와 함께).
  - 결과는 `Prepared.metronome`(번호)과 `metronome_problem`이다. 쓰지 않는 포트 정리(`opened.retain`)는 메트로놈 포트를 놓지 않는다.
  - `metronome_changed(cx)`: 설정이 바뀌면 재생 중인 창이 스냅샷을 다시 만든다. 새로 고른 이름은 「지난 재생 시작 때 못 찾음」에서 뺀다.
- `src/transport.rs`
  - 재생 시작과 재생 중 교체마다 메트로놈 설정과 그 스냅샷의 포트 번호를 넘긴다. 포트 번호는 스냅샷마다 다시 매겨진다.
  - 메트로놈 포트가 없으면 버튼 상태는 그대로 두고 스케줄러의 클릭만 끈다. 상태 메시지는 「메트로놈 포트 「X」이 없어 클릭이 나지 않는다」다.
  - 창 API `metronome_port_missing()`.
- `src/ports.rs`
  - 메트로놈 이름 포트도 감시 대상이다. 그 포트가 사라지거나 돌아오면 모든 창이 트랙 포트와 같이 처리한다.
  - 포트가 오가면 설정 창의 포트 목록을 다시 읽는다.
- `src/settings_tabs.rs`(새 파일)
  - 입력 구역
    - 키 레이아웃 선택 상자. 고르면 `settings::SetKeyLayout`을 낸다.
    - 「보정 마법사」 버튼. `settings::Calibrate`를 낸다.
  - 메트로놈 구역: 포트·채널·강박·약박·길이 선택 상자.
    - 노트는 번호와 GM 타악기 이름(35–81)으로 보인다.
    - 길이는 10–500 ms 목록이다. 파일의 다른 값은 목록에 덧붙인다.
    - 포트 목록은 가상 포트, 지금 목적지, 목록에 없는 지금 값이다.
  - AI 구역: 다섯 항목 이름을 비활성으로 보인다(「S7에서 켠다」).
  - 모든 선택 상자는 검색할 수 있고 Tab 정지점이다. 구역을 열면 첫 컨트롤이 포커스를 받는다.
  - 설정이 밖에서 바뀌면(팔레트, 파일) 그릴 때 선택을 맞춘다.
- `src/settings_window.rs`
  - 앱 전역 처리기 `settings::SetKeyLayout`(설정을 바꾸고 저장)과 `settings::Calibrate`(입력 구역을 열고 「보정 마법사는 녹음 단계(S5)에서 들어온다」를 알림).
  - 구역별 그리기를 연결했다. MIDI 출력 구역은 S2-T6d 자리표시자다.
- `src/gm.rs`: `GM_PERCUSSION`(35–81), `percussion_label`.

## 테스트 결과

~~~
cargo test --workspace: 1461 passed, 0 failed, 0 ignored(리뷰 1 대응 뒤 전체 게이트. 리뷰 2 테스트 6개는 그 파일만 실행해 통과)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `settings_input_and_metronome_round_trip` | 통과 |
| 2 | `set_key_layout_updates_settings_and_input_tab` | 통과 |
| 3 | `calibrate_opens_input_section_with_notice` | 통과 |
| 4 | `metronome_settings_change_clicks` | 통과 |
| 5 | `metronome_change_applies_while_playing` | 통과 |
| 6 | `metronome_named_port_routes_and_missing_is_reported` | 통과 |
| 7 | `settings_sections_keyboard_reachable` | 통과 |
| 8 | `ai_section_is_disabled_placeholder` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(첫 실행에서 `commands` 검사 하나가 자리 설명의 「키」 낱말을 잡아 문구를 고친 뒤 그 파일 통과)
- [x] deny
- [x] 단계별 추가 게이트: `scripts/check-deps.sh`

## 알려진 한계

- 키 레이아웃 값은 저장만 한다. 음 키 해석은 S3 Step 입력이 읽는다.
- 보정 마법사는 S5-T2다. 지금 버튼과 Action은 입력 구역을 열고 알리기만 한다.
- 클릭 벨로시티(127/100)는 REQ에 설정 항목이 없어 고정이다.
- Sequence 녹음·카운트인의 강제 클릭(MET-1 뒤 절)은 S5다.
- 메트로놈 포트가 사라졌다 돌아오는 경우의 메시지는 트랙 포트와 같은 문구(「출력 포트 「X」…」)다. 모든 창에 뜬다(메트로놈은 모든 창이 쓴다).
- 설정 창 왼쪽 구역 버튼도 Tab 정지점이다. 구역 안 마지막 컨트롤에서 `Tab`을 누르면 구역 목록으로 간다.

## 리뷰어에게

- 재생 중 메트로놈 포트를 바꿀 때(가상 ↔ 이름, 이름 ↔ 없는 이름), 스냅샷 교체와 `set_metronome`·`set_metronome_enabled` 순서에서 옛 포트 번호로 클릭이 새는 틈이 있는지.
- 메트로놈 포트를 트랙 포트와 함께 셀 때 포트 상한(`MAX_SCHEDULER_PORTS`)과 `known_missing` 처리.
- 선택 상자 동기화(`Tabs::sync`)가 밖의 변경(팔레트, 파일 손 편집 뒤 재시작)과 고르는 중의 변경에서 어긋나지 않는지.

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1: 선택 상자가 `space`로 열리지 않는다(계획 5) | 감싼 칸이 닫힌 선택 상자에 포커스가 있을 때 `space`를 받아 선택 상자의 `enter`와 같은 Action(`Confirm`)을 낸다. 열린 동안은 검색란이 포커스라 공백은 글자다 |
| 지적 2: 재생 중 교체에서 `swap`과 `set_metronome` 사이에 옛 포트 번호로 클릭이 샐 수 있다 | 교체 전에 클릭을 끄고(`set_metronome_enabled(false)`), 교체, 새 포트 번호 설정, 다시 켬 순서로 보낸다. 큐가 그 사이에서 나뉘면 박 하나가 울리지 않을 수 있다(새는 것보다 낫다) |
| 제안 3: 소멸·복귀·재생 중 포트 바꾸기 테스트를 남긴다 | `tests/review_s2_t6c.rs`를 그대로 남긴다(13개 통과) |
| 제안 4: 메트로놈 명령 실패가 뱅크·프로그램 문구로 알려진다 | 「바뀐 메트로놈 설정을 보내지 못했다」로 따로 알린다 |
| 제안 5: `has_owner` 문서 주석 자리 | 되돌렸다 |

## 리뷰 2 대응

판정 「병합 가능」.
- 제안 1(명령 큐가 가득 찬 경우에만 닿는 오류 경로): 알려진 한계로 둔다.
  - 끄기 뒤 교체가 실패하면 다음 편집·토글까지 클릭이 나지 않는다.
  - `set_metronome` 실패 뒤 토글하면 옛 포트 번호로 클릭이 날 수 있다.
  - S2-T6d(MIDI 출력 탭에서 포트별 클럭 명령을 같은 경로에 더한다)에 이월해 교체 묶음의 실패 처리를 함께 본다.
- 제안 2: 모듈 문서에 `space`를 적었다.
- 제안 3: `tests/review_s2_t6c_r2.rs`를 남긴다.
