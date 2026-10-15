# 적대적 리뷰: S1-T2 OS 포트 열거·핫플러그·재연결 (리뷰 1회차)

- 리뷰어 세션: 구현 맥락을 물려받지 않은 별도 리뷰 에이전트. 구현자 세션과 다르다.
- 대상 커밋: `191babf` (브랜치 `s1/t2-port-enumeration`, `git diff main...HEAD`)
- 리뷰 테스트: `crates/keyscore-midi/tests/review_s1_t2.rs` (실패 4, 통과 6)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`src/` 변경과 계획·보고서를 읽고 적었다. 같은 시점에 `tests/os_ports.rs`도 함께 열었으므로 완전히 독립적인 목록은 아니다.

1. `diff_ports`는 (이름, 고유 ID)만 비교한다. 같은 이름·같은 ID의 목적지가 알림 사이에 사라졌다 다시 생기면(엔드포인트 참조만 바뀜) 이벤트가 없다. 그러면 `OsOutput`은 옛 엔드포인트로 보낸다.
2. 알림은 「프로세스에서 처음 `MIDIClientCreate`를 부른 스레드의 run loop」으로 온다. 이 가정이 깨지면(다른 코드가 먼저 클라이언트를 만듦, 이름이 같은 스레드) 감시가 조용히 죽는지, 그리고 drop 뒤 refcon `Arc::from_raw`의 안전 주장이 유지되는지.
3. `midi_thread::run`은 run loop에 소스가 없으면 `CFRunLoopRunInMode`가 곧바로 끝나 바쁜 대기가 될 수 있다(프로세스 클라이언트는 알림 함수가 없다).
4. 감시자는 `kMIDIMsgSetupChanged`만 본다. 이름 변경(`kMIDIMsgPropertyChanged`)을 놓치는지.
5. `Reconnector`는 사라짐을 먼저 처리하므로, 같은 최종 목록이라도 알림 묶음에 따라 연결 대상이 다를 수 있다(「가장 작은 ID」 규칙의 결정성).
6. `OsOutput::send`의 오류 경로가 `String`을 할당한다(스케줄러 스레드 할당 금지).
7. `is_server_gone`이 MIG -300..=-308을 모두 「서버 소멸」로 본다. -304는 `MIG_BAD_ARGUMENTS`다. 반대로 서버 재시작 뒤 낡은 클라이언트 핸들이 받을 법한 `kMIDIInvalidClient`(-10830), `kMIDIMessageSendErr`(-10838)는 빠졌다.
8. S1-T1 이월(서버 재시작 때 캐시 재생성과 가상 포트 재등록)을 이 Task가 실제로 처리했는지, 처리 테스트가 있는지.
9. `find_destination`의 `kind == 3` 검사로 소스 고유 ID를 거르는지.
10. 감시자 여러 개, 감시자 시작·해제를 포트 변화와 섞어 반복할 때 죽거나 채널이 남는지.
11. `midi-soak.sh`가 실패 로그를 정말 남기는지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 같은 이름·ID 빠른 재생성 | 없음 | 리뷰 테스트로 깨뜨림 |
| 2 첫 클라이언트 가정 | 없음 | 보고서 「알려진 한계」에만 있음. 리뷰 테스트로 실측 |
| 3 바쁜 대기 | 없음 | 리뷰 테스트로 확인, 못 깨뜨림 |
| 4 이름 변경 이벤트 | 단위(`diff_ports`)만 | 실제 CoreMIDI 경로는 리뷰 테스트로 확인, 못 깨뜨림 |
| 5 묶음에 따른 재연결 결과 | 없음 | 리뷰 테스트로 깨뜨림 |
| 6 송신 오류 경로 할당 | 없음 | CoreMIDI가 그 상태를 돌려주게 만들 방법이 없어 코드로만 확인 |
| 7 서버 소멸 분류 | `server_gone_status_clears_cached_client` | 이름과 달리 분류 함수만 본다. 캐시를 지우는지는 검사하지 않는다 |
| 8 서버 재시작 재생성·재등록 | 없음 | 재등록은 S2로 미뤘다 |
| 9 소스 ID 거르기 | 없음 | 리뷰 테스트로 확인, 통과 |
| 10 여러 감시자, 반복 시작·해제 | 부분(`watcher_drop_closes_event_channel_promptly`) | 리뷰 테스트로 확인, 통과 |
| 11 soak 로그 | 스크립트만 | 실행해 확인 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 다른 클라이언트가 고유 ID를 고정한 가상 목적지를 지우고 같은 이름·ID로 곧바로 다시 만든 뒤, 옛 `OsOutput`으로 송신. 3회·5회 반복 모두 이벤트 0개, `is_present=false`, 송신 미도착 | 깨짐 | `watcher_quick_recreate_with_same_unique_id_keeps_output_delivering_or_reports_change` |
| 2 | `coremidi` crate 클라이언트를 먼저 만들고(자식 프로세스) `PortWatcher::start`. 시작은 `Ok`, 목적지를 만들어도 3초간 이벤트 0개 | 깨짐 | `watcher_after_foreign_first_client_reports_appeared_or_fails_start` |
| 3 | 이름이 `keyscore-coremidi`인 사용자 스레드에서 첫 `MidiClient::new`(자식 프로세스). `on_midi_thread`가 그 스레드를 CoreMIDI 스레드로 알고 그 자리에서 클라이언트를 만든다. 이벤트 0개 | 깨짐 | `watcher_after_client_from_thread_named_like_midi_thread_reports_appeared` |
| 4 | 같은 최종 목록 [Dup 5, Dup 30]에 한 번의 비교(사라짐 20 → 나타남 5)로 갈 때와 두 알림(나타남 5 → 사라짐 20)으로 갈 때의 `Reconnector` 연결 대상 비교. 전자는 Dup 30, 후자는 Dup 5 | 깨짐 | `reconnector_same_final_ports_reach_same_link_regardless_of_batching` |
| 5 | 클라이언트만 있을 때와 감시자를 만든 뒤, 2초 쉬는 동안 프로세스 CPU 시간을 `ps`로 잼(자식 프로세스). 둘 다 20 ms 이하 | 못 깨뜨림 | `midi_thread_idle_after_client_create_uses_little_cpu` |
| 6 | 감시 중인 목적지의 `name` 속성을 다른 클라이언트에서 바꿈. `Disappeared(옛 이름, id)` 뒤 `Appeared(새 이름, id)`가 옴 | 못 깨뜨림 | `watcher_reports_rename_of_destination_as_disappear_then_appear` |
| 7 | 감시자 둘을 동시에 두고, 하나를 놓은 뒤에도 나머지가 받는지 | 못 깨뜨림 | `two_watchers_both_receive_appeared_and_disappeared` |
| 8 | 3초 동안 다른 스레드가 목적지를 3 ms 간격으로 만들고 지우는 사이, 감시자를 약 60회 시작·해제. 해제 뒤 채널이 닫히는지, 프로세스가 죽지 않는지, 마지막 감시자가 받는지 | 못 깨뜨림 | `watcher_start_drop_churn_while_destinations_change_keeps_working` |
| 9 | 우리 가상 소스의 고유 ID로 `OsOutput::open` | 못 깨뜨림(`PortGone`) | `os_output_open_with_unique_id_of_source_fails_with_port_gone` |
| 10 | 이름 변경 후 원래 이름으로 돌아올 때 `Reconnector` | 못 깨뜨림 | `reconnector_rename_away_and_back_loses_then_reconnects` |
| 11 | `scripts/midi-soak.sh 10`(리뷰 테스트를 잠시 빼고 구현만) | 10회 중 0회 실패, 2분 42초 | - |
| 12 | `scripts/midi-soak.sh 4 6`(회차 사이 6초, 서버 유휴 종료 경로) | 4회 중 0회 실패 | - |
| 13 | 리뷰 테스트 파일을 3회 반복 실행 | 매회 같은 4개 실패, 6개 통과(흔들림 없음) | - |

참고: 처음 soak를 돌리는 도중 리뷰 테스트 파일을 추가해서 6–10회차에 그 파일이 섞여 실패했다. 이 결과는 위 수치에서 뺐다(1–5회차는 구현만으로 통과).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 이름의 포트가 돌아오면 자동 재연결(OUT-3) | 예 | 같은 이름·ID의 빠른 재생성은 이벤트가 없다(시도 1) |
| 감시자는 시작에 성공하면 변화를 알린다 | 예 | 다른 코드의 첫 클라이언트, 이름이 같은 스레드(시도 2, 3) |
| 같은 이름 여럿이면 가장 작은 ID(결정적) | 예 | 알림 묶음에 따라 결과가 다르다(시도 4) |
| refcon은 콜백이 쓰는 동안 살아 있다 | 이번 실행으로는 못 깨뜨림 | 다만 안전 주장은 「콜백이 CoreMIDI 스레드에서만 온다」에 기대고, 시도 2·3에서 그 전제가 공개 API로 깨진다. 그때 콜백이 run loop이 도는 다른 스레드(예: GPUI 메인 스레드)로 가면 drop의 `MIDIClientDispose`와 콜백이 같은 스레드에서 직렬화되지 않는다 |
| drop 뒤 채널이 닫힌다 | 아니오 | 시도 8 |
| 송신 경로에 할당이 없다 | 오류 경로에서 예 | `os.rs:137` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-3 열거·선택 | 충족 | `os.rs:24` `destinations`, `os.rs:83` `OsOutput::open`, 테스트 `destinations_lists_virtual_destination_created_by_other_client` |
| OUT-3 핫플러그·소멸 감지 | 부분 충족 | `watcher.rs:41`. 같은 이름·ID 빠른 재생성을 놓친다(`ports.rs:43`, 시도 1). 첫 클라이언트 가정이 깨지면 조용히 아무것도 오지 않는다(시도 2) |
| OUT-3 상태줄 표시 | 범위 밖(S2) | `Action::Lost`만 낸다 |
| OUT-3 같은 이름이 돌아오면 자동 재연결 | 부분 충족 | `ports.rs:122`. 시도 1에서는 재연결 동작이 없고 송신이 버려진다 |
| OUT-8 대체 매핑 제안(데이터 구조) | 충족 | `ports.rs:174`. 지적 9의 제안 참고 |
| 9.2 포트 열거·핫플러그 감시는 백그라운드 | 충족 | `midi_thread.rs`, 콜백은 CoreMIDI 전용 스레드 |
| 9.2 스케줄러: 할당 금지 | 부분 충족 | `os.rs:137` 오류 경로의 `String` 할당 |
| S1-T1 이월: 서버 재시작 때 캐시 재생성과 가상 포트 재등록을 알림 처리에서 | 부분 충족 | 오류 상태로 캐시만 버린다(`client.rs:218`). 알림으로 감지하지 않고, 재등록은 결정 문서 없이 S2로 미뤘다 |
| S1-T1 이월: 재시도가 잠금을 쥔 채 잠들지 않음 | 충족 | `client.rs:177`, 테스트 `process_client_retry_does_not_hold_lock_while_sleeping` |
| S1-T1 이월: 반복 실행 스크립트 | 충족 | `scripts/midi-soak.sh`. 지적 10의 제안 참고 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: `OsOutput::send`가 `kMIDIUnknownEndpoint`·`kMIDIObjectNotFound`·`kMIDINoConnection`을 받으면 `MidiError::PortGone(self.name.clone())`로 할당한다(`os.rs:137`). 주석이 스스로 「이 오류를 돌려주는 경우에만 만든다」고 인정한다. S1-T4의 `assert_no_alloc`는 디버그 빌드에서 이 경로를 중단시킨다. 성공 경로(`with_event_list`)에는 할당·락이 없다.
- 제품 코드의 `unwrap`/`expect`: 없음(테스트 모듈에만 있다).
- 에러가 삼켜지는 곳
  - `watcher.rs:31` 받는 쪽이 사라진 뒤의 `send` 결과. 정당하다.
  - `watcher.rs:127–131` drop의 `on_midi_thread`와 `MIDIClientDispose` 결과. `on_midi_thread`가 실패하면 클라이언트를 해제하지 못한 채 다음 줄에서 refcon을 놓는다. 지금은 CoreMIDI 스레드가 끝나는 경로가 없어 도달하지 않지만, 안전 주장이 그 사실에 기댄다는 주석이 없다.
  - `os.rs:118` `MIDIPortDispose` 결과. 정당하다.
  - `midi_thread.rs:52` 작업 패닉. 호출자는 `Os { status: -1 }`을 받아 원인이 패닉인지 알 수 없다.
  - `PortWatcher::start`가 첫 클라이언트 가정이 깨진 상태를 알아채지 못하고 `Ok`를 돌려준다(시도 2).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 같은 이름·같은 고유 ID의 목적지가 빠르게 다시 만들어지면 콜백이 읽는 앞뒤 목록이 같아 이벤트가 없다. `Reconnector`는 연결된 줄 알고, 옛 `OsOutput`은 사라진 엔드포인트로 보내 CoreMIDI가 소리 없이 버린다. 다른 앱이 저장한 고유 ID로 가상 포트를 다시 등록하는 것은 흔한 관행이라, OUT-3의 「같은 이름의 포트가 돌아오면 자동 재연결」이 실패한다. 콜백의 비교 키에 엔드포인트 참조(`MIDIEndpointRef`)를 넣거나 `kMIDIMsgObjectRemoved`/`ObjectAdded`를 직접 처리해 사라짐·나타남을 놓치지 않게 해야 한다. 재현: 시도 1 | `ports.rs:43`, `watcher.rs:27`, `os.rs:109` |
| 2 | 차단 | 스케줄러가 부르는 `OsOutput::send`의 오류 경로가 `String`을 할당한다(REQ 9.2, `00-conventions.md` 실시간 규칙). 할당 없는 오류 값(예: `Os { call, status }` 또는 이름 없는 변형)을 돌려줘야 한다 | `os.rs:137` |
| 3 | 수정 | 첫 클라이언트 가정이 깨지면 `PortWatcher::start`가 `Ok`인데 이벤트가 영영 오지 않는다. 앱은 감시가 죽었음을 알 수 없다. 또 그때 알림이 run loop이 도는 다른 스레드로 가면, drop의 「해제가 끝나면 콜백이 더 오지 않는다」(`watcher.rs:126`, `:133`)와 콜백의 refcon 안전 주장(`watcher.rs:54`)의 전제(같은 스레드에서 직렬화)가 무너진다. 최소한 (a) refcon이 가리키는 상태를 프로세스 수명 동안 놓지 않고 drop에서는 `Sender`만 꺼내 채널을 닫아, 어느 스레드에서 콜백이 와도 해제 뒤 사용이 없게 하고, (b) `start`의 문서에 전제를 `# Panics`/`# Errors`처럼 드러내거나, 시작 직후 자기 확인(임시 가상 목적지를 만들어 알림이 오는지)으로 실패를 오류로 돌려줘야 한다. 재현: 시도 2 | `watcher.rs:54`, `watcher.rs:82`, `watcher.rs:133` |
| 4 | 수정 | 같은 최종 목록에서 알림 묶음에 따라 연결 대상이 다르다(Dup 30 대 Dup 5). 계획의 「가장 작은 ID(결정적)」와 어긋나고, 구현자가 「리뷰어에게」 3에서 물은 바로 그 경우다. 한 비교의 이벤트를 묶음으로 반영한 뒤 대상을 고르는 API(`apply_all(&[PortEvent])` 등)를 두거나, 규칙을 「연결 시점에 있는 것 가운데 최소 ID, 연결 뒤에는 바꾸지 않음」으로 계획·문서에 명시하고 테스트로 고정해야 한다. 재현: 시도 4 | `ports.rs:122–158` |
| 5 | 수정 | 서버 소멸 분류의 근거가 약하다. `mach/mig_errors.h`에서 -304는 `MIG_BAD_ARGUMENTS`, -300은 `MIG_TYPE_ERROR`로 서버 소멸이 아니다. 이런 상태에서 캐시를 버리면 옛 클라이언트를 해제하지 않은 채 두 번째 프로세스 클라이언트를 만든다(「클라이언트 하나」 전제가 깨지고, 그 뒤 가상 포트는 다른 클라이언트에 붙는다). 반대로 낡은 클라이언트 핸들이 받을 법한 `kMIDIInvalidClient`(-10830), 서버 통신 실패 `kMIDIMessageSendErr`(-10838), `MIG_SERVER_DIED`와 같은 뜻의 Mach 송신 오류(`MACH_SEND_INVALID_DEST` 등)는 빠졌다. S1-T1 리뷰 4가 본 -304는 서버가 뜨는 중의 `MIDIClientCreate` 응답이라 재시도 경로의 근거이지 캐시 무효화의 근거가 아니다. 목록을 줄이고 늘린 근거를 주석에 적어야 한다 | `client.rs:211–213` |
| 6 | 수정 | `server_gone_status_clears_cached_client`는 이름과 달리 분류 함수만 보고 캐시를 지우는지 검사하지 않는다. `forget_client_if_server_gone`도 `create_with_retry`처럼 슬롯을 인자로 받게 해 「다른 클라이언트면 지우지 않음」「같으면 지움」「서버 소멸이 아니면 지우지 않음」을 시험해야 한다 | `client.rs:218`, `client.rs:635` |
| 7 | 수정 | 단계 파일의 이월은 「서버 재시작 때 캐시된 핸들을 다시 만들고 가상 포트를 재등록하는 경로를 이 Task의 알림 처리에서 다룬다」이다. 구현은 오류 상태로만 캐시를 버리고, 재등록은 S2로 미뤘다. 범위를 옮기려면 결정 요청(`docs/decisions/`)이나 단계 파일의 이월 줄 갱신이 먼저 있어야 한다. 또 캐시를 버린 뒤에도 `port_names`에는 옛 서버의 이름이 남아, 같은 이름의 재등록은 옛 `VirtualOutput`을 놓기 전까지 `DuplicateName`이다. S2가 알아야 할 제약으로 보고서에 적어야 한다 | `client.rs:218`, `docs/reports/s1-t2/report.md` 「알려진 한계」 |
| 8 | 제안 | `on_midi_thread`가 스레드 이름으로 CoreMIDI 스레드를 알아본다. 같은 이름의 사용자 스레드가 첫 클라이언트를 만들면 감시가 죽는다(시도 3). `thread_local!` 표시나 `ThreadId` 비교가 더 정확하다 | `midi_thread.rs:68` |
| 9 | 제안 | `suggest_mapping`의 `Virtual { name }`은 만들 수 없는 제안을 낼 수 있다: `normalized_name`이 거부하는 이름, 서로 같은 중복 키가 되는 이름 둘(NFC/NFD 등), 16개(`MAX_VIRTUAL_OUTPUTS`)를 넘는 수. S2가 다룰 수 있지만 데이터 구조에서 표시하면(예: `Virtual`의 실현 가능 여부) UI가 단순해진다 | `ports.rs:174` |
| 10 | 제안 | `midi-soak.sh`는 성공한 회차의 `run-<i>.log`를 지우므로, 같은 디렉터리에서 다시 돌리면 이전 세션의 실패 로그가 사라진다. 세션마다 하위 디렉터리(날짜)를 쓰거나 지우지 않는 편이 증거를 남긴다. 저장소 루트가 아닌 곳에서 실행하면 상대 경로 `target/midi-soak`가 엉뚱한 곳에 생긴다 | `scripts/midi-soak.sh:10`, `:17` |
| 11 | 제안 | 작업이 run loop의 20 ms 조각 경계에서만 처리된다. `CFRunLoopWakeUp`이나 run loop 소스로 깨우면 감시자 시작·해제와 첫 클라이언트 생성 지연이 사라진다. 지금 동작에는 문제가 없다(시도 5, 8) | `midi_thread.rs:20`, `:56` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 2건(지적 1, 2)과 수정 5건(지적 3–7)을 고친 뒤 다시 리뷰한다. `review_s1_t2.rs`의 실패 테스트 4개 가운데 지적 1·3·4에 해당하는 3개는 수정 뒤 통과해야 한다. 지적 3을 「시작이 오류를 돌려준다」로 고치면 `watcher_after_foreign_first_client_reports_appeared_or_fails_start`도 통과한다. 지적 8을 거절하면 `watcher_after_client_from_thread_named_like_midi_thread_reports_appeared`의 처리(기대를 바꿀지)를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [ ] 테스트를 읽기 전에 1번을 썼다 (`src/`와 같은 시점에 `os_ports.rs`를 열었다. 1번 목록은 그 뒤에 적었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음. `coremidi-sys`, `core-foundation`은 S1-T1부터 있었다)
- [x] 스타일 지적을 적지 않았다
