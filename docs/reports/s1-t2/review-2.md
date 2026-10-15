# 적대적 리뷰: S1-T2 OS 포트 열거·핫플러그·재연결 (리뷰 2회차)

- 리뷰어 세션: 구현 맥락을 물려받지 않은 별도 리뷰 에이전트. 구현자 세션과도, 리뷰 1회차 세션과도 다르다.
- 대상 커밋: `779ecf1` (브랜치 `s1/t2-port-enumeration`). 범위는 리뷰 1 뒤 변경 `git diff 18f07e0 779ecf1`
- 리뷰 테스트: `crates/keyscore-midi/tests/review_s1_t2_r2.rs` (실패 0, 통과 9)
- 판정 기준: 리뷰 1 수정의 미해결·회귀, REQ OUT-3·OUT-8·9.2 위반(특히 송신 경로의 할당·락·로그), unsafe SAFETY 주장의 오류, 패닉·교착·무한 대기만 「수정」 이상으로 셌다. 나머지는 「제안」이다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 18f07e0 HEAD -- crates/keyscore-midi/src scripts`와 보고서 「리뷰 대응」을 읽고 적었다. 리뷰 1 테스트 파일은 그 전에 열었지만, 구현자가 새로 더한 테스트(`virtual_port.rs`의 `reregister_keeps_name_listing_and_delivery`, `ports.rs`·`client.rs` 단위 테스트)는 목록을 적은 뒤에 봤다.

1. `diff_ports`가 객체 참조로 비교한다. `CoreMIDI`가 지운 객체의 참조 값을 다시 쓰면 같은 이름·ID의 빠른 재생성을 다시 놓친다.
2. 이름·ID가 같은 목적지 둘이 동시에 있을 수 있으면, `Reconnector`의 `retain(|p| p != port)`(같음은 이름·ID만 본다)가 하나의 사라짐으로 둘을 지운다.
3. 같은 목적지를 두 번 열거할 때 참조가 달라지면 가짜 사라짐·나타남이 쏟아진다.
4. 「최소 ID」 규칙이 묶음 방식 하나가 아니라 모든 이벤트 순서에서 매 단계 지켜지는가.
5. 탐침이 가짜 성공할 수 있는가(탐침과 무관한 알림이 `seen_on_midi_thread`를 켬). 가짜 실패할 수 있는가(부하 때 2초 초과). 실패 시 감시자 클라이언트를 해제하는데, 그 뒤 다른 스레드의 콜백이 refcon을 쓰는가.
6. 탐침 소스가 남는 경로, 앱의 같은 이름 포트와의 간섭, 다른 감시자에 주는 영향, 16개 상한과의 관계.
7. 알림이 run loop이 도는 다른 스레드로 가는 경우(GPUI 메인 스레드와 같은 경우) `start`가 곧바로 실패하는지, 2초를 다 기다리는지.
8. `start`가 이제 탐침 때문에 얼마나 블로킹하는가(REQ 9.2 메인 스레드 블로킹 금지).
9. `OsOutput::send`의 새 오류 경로(`DestinationGone`)에 할당·락이 정말 없는가. `forget_client_if_server_gone`(잠금)이 송신 경로에 들어갔는가.
10. 서버 소멸 상태 값(-10830, -10839, -308)이 헤더 값과 맞는가.
11. `reregister`: 옛 엔드포인트가 살아 있을 때 부르면 같은 이름 소스가 잠깐 둘. 실패하면 옛 것 유지. 그 뒤 `rename`·drop이 새 엔드포인트에 작동하는가, 이름 자리가 풀리는가.
12. `is_registered`가 옛 참조로 이름을 읽는데, 서버 재시작 뒤 새 서버가 같은 참조 값을 다른 객체에 주면 거짓 참이 된다.
13. `Suggestion::Virtual`이 여전히 만들 수 없는 이름을 낼 수 있는가(정규화 뒤 겹치는 두 이름, 16개 초과, 앱 자신의 포트 이름).
14. soak 스크립트의 새 디렉터리 규칙과 상대 경로.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 참조 재사용 | 단위(`diff_reports_recreated_port_with_same_name_and_id`)와 리뷰 1 테스트 1회 재생성 | 10회 연속 재생성 리뷰 테스트로 확인, 못 깨뜨림. 실측 참조는 단조 증가했다 |
| 2 같은 이름·ID 둘 | 없음 | `CoreMIDI`가 `kMIDIIDNotUnique`(-10843)로 거부함을 테스트로 고정 |
| 3 가짜 이벤트 | 없음 | 리뷰 테스트로 확인, 못 깨뜨림 |
| 4 최소 ID 결정성 | 단위 1개, 리뷰 1 테스트 1개 | 모든 부분집합 쌍과 모든 순서로 확인, 못 깨뜨림 |
| 5 탐침 가짜 성공·실패 | 리뷰 1 테스트(첫 클라이언트가 run loop 없는 스레드) | 부하·동시 시작·run loop이 도는 다른 스레드로 확인, 못 깨뜨림 |
| 6 탐침 소스 잔존·간섭 | 없음 | 리뷰 테스트로 확인, 못 깨뜨림 |
| 7 다른 스레드로 가는 알림 | 없음 | 리뷰 테스트로 확인. 약 0.1–0.25초에 실패한다 |
| 8 `start` 블로킹 | 없음 | 실측 약 0.2–0.25초. 제안 3 |
| 9 송신 경로 할당·락 | 없음(오류 상태를 일으킬 수 없음) | 코드로 확인. 송신 결과가 `Ok` 또는 `DestinationGone`뿐임은 리뷰 테스트로 확인 |
| 10 서버 소멸 값 | 단위(`server_gone_status_clears_cached_client`) | `MIDIServices.h`·`mig_errors.h` 값과 맞다 |
| 11 `reregister` | `reregister_keeps_name_listing_and_delivery` | `rename`·drop·이름 자리는 리뷰 테스트로 확인. 같은 이름 소스 둘은 실험으로 관찰(제안 4) |
| 12 `is_registered` 거짓 참 | 없음 | 서버 재시작은 시스템 프로세스 종료가 필요해 실행하지 않았다(제안 5) |
| 13 `Virtual` 실현 가능성 | 단위(`Bad\0Name` → `Unavailable`) | 코드로 확인(제안 6) |
| 14 soak | 스크립트만 | 실행해 확인 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 리뷰 1 테스트 파일이 바뀌지 않았는지 `git diff 18f07e0 HEAD`로 확인. HEAD에서 10개 통과. `src/`만 `18f07e0`으로 되돌린 `/tmp` 사본에서 같은 4개 실패(`reconnector_same_final_ports_...`, `watcher_after_client_from_thread_named_...`, `watcher_after_foreign_first_client_...`, `watcher_quick_recreate_...`) | 수정이 실제로 4개를 고쳤다 | - |
| 2 | 다른 클라이언트가 고유 ID를 고정한 목적지를 10회 연속 지우고 같은 이름·ID로 다시 만듦. 매 회차 옛 ID의 사라짐과 새 나타남, `Reconnector` 연결 유지, 다시 연 `OsOutput`으로 송신 도착. 별도 실험에서 30회 재생성한 참조는 `0x14e8d846`부터 1씩 늘었다 | 못 깨뜨림 | `watcher_repeated_quick_recreate_with_same_unique_id_reports_every_round` |
| 3 | 같은 이름의 두 번째 목적지에 이미 쓰인 고유 ID를 붙임 | 못 깨뜨림(-10843으로 거부. 이름·ID가 같은 둘은 동시에 없다) | `second_destination_with_taken_unique_id_is_refused` |
| 4 | 살아 있는 목적지를 두 번 열거해 `diff_ports` 20회 | 못 깨뜨림(이벤트 0) | `diff_ports_of_two_snapshots_without_change_reports_nothing_for_live_destination` |
| 5 | 포트 넷(같은 이름 셋, 다른 이름 하나)의 시작·끝 부분집합 256쌍과 그 사이 이벤트의 모든 순열에서 매 단계 연결 대상 비교 | 못 깨뜨림 | `reconnector_any_event_order_links_lowest_present_id_at_every_step` |
| 6 | 감시자 40개를 살려 두고, 다른 스레드가 목적지를 1 ms 간격으로 만들고 지우는 동안 16개 스레드가 함께 `start` | 못 깨뜨림(모두 `Ok`, 각각 약 0.20초) | `watcher_concurrent_starts_under_churn_all_succeed_well_before_probe_timeout` |
| 7 | (자식 프로세스) `coremidi` crate 클라이언트를 run loop이 도는 스레드에서 먼저 만든 뒤 `start` 3회, 이어 다른 스레드 알림을 일으키는 목적지 30회 생성·삭제 | 못 깨뜨림(매번 `NotificationsUnavailable`, 0.09–0.26초. 프로세스가 살아 있고 탐침 소스가 남지 않으며 가상 포트를 그대로 만든다) | `watcher_start_after_foreign_first_client_on_run_loop_thread_fails_fast_and_stays_usable` |
| 8 | (자식 프로세스) 앱 포트 이름을 탐침과 같은 `KeyScore watcher probe`로 만들고 `start` 3회, 포트를 놓은 뒤 다시 `start` | 못 깨뜨림(앱 포트만 남고 탐침은 사라진다) | `watcher_start_with_own_port_named_like_probe_removes_only_probe` |
| 9 | `reregister` 3회 뒤 `rename`, 옛 이름 재사용, 다시 `reregister` 뒤 drop | 못 깨뜨림(새 엔드포인트에 작동, 이름 자리 해제) | `reregister_then_rename_and_drop_act_on_new_endpoint_and_free_name` |
| 10 | 등록된 포트에 `reregister`를 50회 부르는 동안 다른 스레드가 `source_names`를 계속 셈 | 같은 이름 소스가 최대 2개 보였다(제안 4). 끝난 뒤 1개, drop 뒤 0개 | 테스트 없음(제안이라 지금 동작을 고정하지 않음) |
| 11 | 목적지를 지운 뒤 옛 `OsOutput`으로 50회 송신 | 못 깨뜨림(`Ok` 또는 `DestinationGone`뿐) | `os_output_send_after_destination_removed_returns_ok_or_destination_gone` |
| 12 | `scripts/midi-soak.sh 10` (리뷰 2 테스트 파일을 더하기 전) | 10회 중 0회 실패, 3분 34초 | - |
| 13 | `scripts/midi-soak.sh 3 6` (리뷰 2 테스트 파일을 더하기 전) | 3회 중 0회 실패, 1분 23초 | - |
| 14 | 리뷰 2 테스트 파일을 3회 반복 실행(`/tmp` 사본) | 매회 9개 통과(약 17초) | - |

참고: 시도 8의 첫 판은 같은 바이너리의 다른 테스트가 동시에 `start`를 불러 그 탐침 소스까지 세어(시스템 전체에 보인다) 실패했다. 감시자를 시작하는 테스트를 정적 잠금으로 직렬화했다. 이것은 구현 결함이 아니라 탐침이 시스템 전체에 보인다는 사실의 확인이다(제안 2).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 이름의 포트가 돌아오면 자동 재연결(OUT-3), 같은 이름·ID의 빠른 재생성 포함 | 아니오 | 시도 2. 참조는 재사용되지 않고, 이름·ID가 같은 둘은 공존하지 않는다(시도 3) |
| 감시자는 시작에 성공하면 변화를 알린다 | 아니오 | 시도 6, 7과 리뷰 1 테스트 |
| 같은 이름 여럿이면 지금 있는 것 가운데 최소 ID(결정적) | 아니오 | 시도 5 |
| refcon은 콜백이 쓰는 동안 살아 있다 | 아니오 | 정상 경로는 탐침이 「알림은 CoreMIDI 스레드로만」을 확인한 뒤에만 성공하고, 해제와 콜백이 그 스레드에서 직렬화된다. 오류 경로는 참조를 놓지 않는다(시도 7) |
| 송신 경로에 할당·락이 없다 | 아니오 | `os.rs:128–140`. `with_event_list`는 스택 버퍼, 오류는 `DestinationGone`·`Os { call: &'static str, .. }`. `forget_client_if_server_gone`은 `open`·생성·`reregister`에만 있다 |
| 가상 포트 이름은 프로세스에서 하나 | `reregister` 중에만 시스템 목록에 잠깐 둘 | 시도 10, 제안 4 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-3 열거·선택 | 충족 | `os.rs:24` `destinations`, `os.rs:83` `OsOutput::open` |
| OUT-3 핫플러그·소멸 감지 | 충족 | `ports.rs:77` 참조 비교, `watcher.rs:128` 탐침. 시도 2, 4, 6, 7 |
| OUT-3 상태줄 표시 | 범위 밖(S2) | `Action::Lost`, `12-S2-app-shell.md:37` 이월 |
| OUT-3 같은 이름이 돌아오면 자동 재연결 | 충족 | `ports.rs:180–200`. 시도 2, 5 |
| OUT-8 대체 매핑 제안(데이터 구조) | 충족 | `ports.rs:220`. 제안 6 참고 |
| 9.2 포트 열거·핫플러그 감시는 백그라운드 | 충족(호출 위치는 S2 책임) | 콜백은 CoreMIDI 전용 스레드. `start`는 약 0.25초 블로킹하므로 메인 스레드에서 부르면 안 된다(제안 3) |
| 9.2 스케줄러: 할당·락 금지 | 충족 | `os.rs:137` `DestinationGone`, `error.rs:27` |
| S1-T1 이월: 서버 재시작 때 캐시 재생성과 가상 포트 재등록 | 충족(알아차려 부르는 일은 S2 이월로 단계 파일에 기록) | `client.rs:217–236`, `client.rs:402–423`, `11-S1-midi-engine.md:21`, `12-S2-app-shell.md:37` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `OsOutput::send`(`os.rs:128`)와 `VirtualOutput::send`(`client.rs:447`) 모두 스택 버퍼와 `&'static str`·정수만 쓴다.
- 제품 코드의 `unwrap`/`expect`: 없음(테스트 모듈에만 있다).
- 에러가 삼켜지는 곳
  - `watcher.rs:129–133` 탐침 실패 경로의 `MIDIClientDispose` 결과. 할 일이 없어 정당하다.
  - `watcher.rs:186` 탐침 소스 `MIDIEndpointDispose` 결과. 정당하다.
  - `watcher.rs:152–154`, `:166–168` 탐침 중 `MidiClient::new` 실패와 `MIDISourceCreateWithProtocol` 실패가 모두 `false`가 되어 `NotificationsUnavailable`로 보고된다. 원인이 서버 소멸(-10830 등)이어도 앱은 「첫 클라이언트를 다른 코드가 만들었다」는 안내를 받고, 탐침 경로에서는 `forget_client_if_server_gone`도 부르지 않는다(제안 1).
  - `client.rs:421` `reregister`의 옛 엔드포인트 해제 결과. 서버가 바뀌었으면 이미 없으므로 정당하다.

## 7. 지적 사항

판정 기준(1회차와 달리 좁혔다) 아래 「차단」「수정」은 없다. 아래는 모두 구현자가 거절할 수 있는 제안이다. 제안을 고정하는 실패 테스트는 두지 않았다.

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 탐침이 CoreMIDI 호출 실패(프로세스 클라이언트 생성, 탐침 소스 생성)를 「알림이 다른 스레드로 감」과 구별하지 않는다. 서버가 다시 뜨는 중이면 앱은 엉뚱한 `NotificationsUnavailable`을 받고, 오류 경로의 의도된 누수(`State` 하나)도 생긴다. `probe_notifications`가 `Result<bool, MidiError>`를 돌려 CoreMIDI 오류는 `Os`로 내고(이때는 알림 스레드가 확인되지 않았으니 누수는 그대로 두되), 소스 생성 실패에는 `forget_client_if_server_gone`을 부르면 된다 | `watcher.rs:149–168` |
| 2 | 제안 | 탐침 소스는 시스템 전체에 보인다. `start`마다 다른 앱(DAW)의 입력 목록에 `KeyScore watcher probe`가 잠깐 나타났다 사라지고, 같은 시각의 다른 KeyScore 프로세스나 테스트도 그것을 센다(시도 8의 첫 실패). 보고서 「알려진 한계」나 `start` 문서에 적어 두면 S2가 시작 시 한 번만 부르도록 설계할 수 있다 | `watcher.rs:155` |
| 3 | 제안 | `start`가 이제 약 0.2–0.25초(알림이 run loop 없는 스레드로 가면 2초) 블로킹한다. REQ 9.2는 메인 스레드 블로킹을 금하므로 `start` 문서에 「제어·백그라운드 스레드에서 부른다, 대략의 소요 시간」을 적고 S2 이월에 더하기를 권한다. 다른 코드가 메인 run loop에서 첫 클라이언트를 만든 상태에서 메인 스레드가 `start`를 부르면 콜백이 그 스레드로 오지 못해 늘 2초를 다 기다린다 | `watcher.rs:90`, `:147` |
| 4 | 제안 | `reregister`는 새 소스를 만든 뒤 옛 것을 지우므로, 옛 엔드포인트가 살아 있을 때 부르면 다른 앱 목록에 같은 이름 소스가 잠깐 둘 보인다(시도 10, 최대 2). drop이 이름 목록 잠금을 쥐고 해제하는 이유(같은 이름이 둘 보이지 않게)와 어긋난다. 문서대로 `is_registered`가 거짓일 때만 쓰면 생기지 않으므로, 참이면 아무것도 하지 않거나 문서에 「살아 있을 때 부르면 잠깐 둘」을 적으면 충분하다 | `client.rs:402–423` |
| 5 | 제안 | `is_registered`는 옛 참조로 이름을 읽기만 한다. 새 서버가 같은 참조 값을 다른 객체에 주면 거짓 참이 되어 재등록을 건너뛴다. 실측 참조는 한 서버 안에서 단조 증가했지만 서버 재시작을 넘어선 동작은 확인하지 못했다. 읽은 이름이 `self.name`과 같은지, 소스 종류인지까지 보면 위험이 줄어든다 | `client.rs:390` |
| 6 | 제안 | `Suggestion::Virtual`은 여전히 함께 만들 수 없는 제안을 낼 수 있다. 정규화 뒤 같은 중복 키가 되는 저장 이름 둘(예: `"Pad Synth"`와 `"Pad  Synth"`), 16개 초과, 앱 자신의 기존 가상 포트와 같은 이름. 커밋 제목(「creatable」)보다 범위가 좁으니, S2가 다룰 일로 문서에 적거나 키 중복은 첫 것만 `Virtual`로 내면 된다 | `ports.rs:220–235` |
| 7 | 제안 | `diff_ports`는 공개 함수인데, `PortInfo::new`로 만든 값(참조 0)과 `destinations()`를 비교하면 `before == after`여도 사라짐·나타남이 나온다. 지금 호출자는 감시자뿐이라 영향이 없다. 문서에 「`destinations()`가 돌려준 목록끼리 비교한다」를 적거나 한쪽 참조가 0이면 참조를 무시하면 된다 | `ports.rs:77–90` |
| 8 | 제안 | MIDI 서버가 다시 뜨면 감시자 자신의 알림 클라이언트도 옛 서버의 것이다. 감시가 계속 되는지 확인되지 않았고, S2 이월(`12-S2-app-shell.md:37`)은 가상 포트 재등록만 적는다. 「서버 재시작을 알아차리면 감시자도 다시 시작한다」를 함께 적기를 권한다 | `watcher.rs:90`, `plan/stages/12-S2-app-shell.md:37` |
| 9 | 제안 | soak 스크립트는 성공한 실행마다 빈 디렉터리를 남기고, 리뷰 1 제안 10의 뒷부분(저장소 루트가 아닌 곳에서 실행하면 `target/midi-soak`가 엉뚱한 곳에 생김)은 그대로다. `cd "$(dirname "$0")/.."` 한 줄과 끝에 `rmdir "$out_dir" 2>/dev/null`이면 된다 | `scripts/midi-soak.sh:11–12` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1의 차단 2건과 수정 5건이 모두 해소됐다. 리뷰 1 테스트 4개는 수정 뒤 통과하고 수정을 되돌리면 다시 실패한다. 이번 회차의 깨뜨리기 시도 11건은 모두 실패했고, 판정 기준의 (a)–(d)에 드는 결함을 찾지 못했다. 위 제안 9건은 거절할 수 있으며, 거절하면 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [ ] 테스트를 읽기 전에 1번을 썼다 (리뷰 1 테스트 파일은 먼저 열었다. 구현자의 새 테스트는 1번을 적은 뒤에 봤다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다 (단계 파일 변경은 리뷰 1 지적 7의 요구다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다 (`#[ignore]` 2개는 S1-T1부터 있던 자식 프로세스용)
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음. 리뷰 테스트는 기존 `core-foundation`의 안전한 `CFRunLoop::run_current`만 쓴다)
- [x] 스타일 지적을 적지 않았다
