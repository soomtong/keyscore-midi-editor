# 적대적 리뷰: S2-T5d 포트 감시와 재연결 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 269b6f5 (`s2/t5d-port-watch`, `git diff main...HEAD`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t5d.rs`(깨뜨리지 못한 시도 10개, 3회 연속 통과). 깨진 시도 3개는 부록.

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트와 보고서를 열기 전에 `src/ports.rs`, `src/engine.rs`, `src/transport.rs`, `src/window.rs`의 diff와 `keyscore-midi`의 `watcher.rs`·`ports.rs`·`os.rs`·`client.rs`·`scheduler.rs`만 읽고 적었다.

1. 같은 이름 포트가 여럿일 때 엔진과 `Reconnector`가 같은 포트를 고르는가. `Engine::open_named`는 열거 순서의 첫 포트(`list.iter().find`), `Reconnector`는 고유 ID가 가장 작은 포트(`best_match`)를 고른다. 엔진이 쓰는 포트가 ID가 작은 쪽이 아니면, 그것이 사라져도 `Reconnector`는 동작을 내지 않아 재생이 사라진 목적지로 계속 간다고 예상한다.
2. 감시 한 바퀴(100 ms) 안에 사라졌다 돌아오면 `Lost`와 `Reconnect`가 한 번에 나와 재생이 새 목적지로 옮겨 가는가.
3. 이름이 바뀐 포트. `PortWatcher`는 `SetupChanged`만 보는데, 이름 속성 변경에도 그 알림이 오는가. 또 `OsOutput::is_present`는 고유 ID와 엔드포인트만 보므로 이름이 바뀐 목적지를 「있음」으로 볼 것이다.
4. 한 포트를 여러 창이 쓰고, 재생하지 않는 창만 그 포트를 쓰는 경우: 메시지가 그 창에만 가고, 재생 중인 다른 창은 멈추거나 메시지를 받지 않는가.
5. 재생 중 포트가 사라진 동안 편집(스냅샷 교체)하고 돌아오면, 새 목적지로 프로그램과 노트가 닿는가. 이때 `transport_document_changed`의 차이 송신과 `transport_port_returned`의 재송신이 겹쳐 프로그램이 두 번 가는가.
6. 재생 중 트랙 포트를 없는 이름으로 바꾸면 「포트 없음」이 곧바로 보이고, 그 이름이 나중에 나타나면 그쪽으로 소리가 가는가(`wanted_names`가 다음 바퀴에 그 이름을 등록).
7. undo가 사라진 포트 이름을 되돌릴 때 상태줄이 갱신되는가.
8. `recover_virtual_port`: 엔진·감시 없이, 두 번 연달아, 정지 중 이름 포트 테스트 노트가 스케줄러에 스냅샷을 넣은 상태, 일시정지 창이 있는 상태. `Arc::get_mut` 대기로 메인 스레드가 얼마나 막히는가. 복구 뒤 테스트 노트(이름·가상)가 닿는가. 감시자가 다시 시작되는가.
9. 서버 확인 실패 경로: `recover_virtual_port`가 실패 메시지를 스스로 남기고 `Err`도 돌려주는데, `tick`이 또 남긴다(중복). 서버가 내려간 동안 1 s마다 반복된다.
10. `reregister_main`의 두 번째 `swap`이 실패하면 스케줄러에 포트 0이 없는 빈 스냅샷이 남는다.
11. 닫힌 창: `OpenWindows`의 약한 참조, 닫힌 창이 쓰던 포트의 변화가 패닉 없이 지나가는가.
12. 실시간 경로: 스케줄러 스레드 코드에 변경이 있는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 같은 이름 포트 여럿 | 없음 | 리뷰가 깨뜨림(부록 A) |
| 2 한 바퀴 안의 소멸·복귀 | 없음 | 리뷰 테스트로 확인, 통과 |
| 3 이름 바뀜 | 없음 | 감시는 감지함(통과). `is_present`는 이름을 보지 않음(부록 B) |
| 4 여러 창 | 없음 | 리뷰 테스트로 확인, 통과 |
| 5 사라진 동안 편집 후 복귀 | 없음 | 소리는 닿음(통과). 프로그램 두 번(부록 C, 보고서가 이미 인정) |
| 6 없는 이름으로 바꾸기 | 없음 | 리뷰 테스트로 확인, 통과 |
| 7 undo | 없음 | 리뷰 테스트로 확인, 통과 |
| 8 복구 경계 | 일부(`recover_virtual_port_stops_playback_and_reregisters`: 재생 중 1회) | 두 번·감시 없음·엔진 없음·이름 포트 스냅샷·일시정지는 리뷰 테스트로 확인, 통과 |
| 9 서버 확인 실패 반복 | 없음 | 서버 재시작을 일으킬 수 없어 코드로만 확인 |
| 10 두 번째 swap 실패 | 없음 | 코드로만 확인 |
| 11 닫힌 창 | 없음 | 리뷰 테스트로 확인, 통과 |
| 12 실시간 경로 | 해당 없음 | `keyscore-midi` 변경 없음 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 같은 이름 포트 둘, 먼저 열거되는 쪽(엔진이 연 쪽)의 고유 ID가 더 큼. 재생 중 그것을 지움 | 깨짐: 남은 같은 이름 포트로 노트가 가지 않음. 메시지 없음, 「포트 없음」 없음 | 부록 A `break_duplicate_names_engine_port_vanishes_while_lower_id_remains` |
| 2 | 같은 조건에서 먼저 열거되는 쪽의 ID가 더 작음(엔진·`Reconnector` 일치) | 못 깨뜨림 | `duplicate_names_consistent_choice_reconnects` |
| 3 | 감시 없이: 연 포트의 이름을 바꾼 뒤 다시 재생 | 깨짐: 옛 이름 트랙의 노트가 이름이 바뀐 포트로 감. 「없어」 메시지 없음 | 부록 B `break_is_present_ignores_rename` |
| 4 | 감시 중: 쓰는 포트의 이름을 바꿈 | 못 깨뜨림(「사라졌다」, 「포트 없음」) | `renamed_in_use_port_is_reported_lost` |
| 5 | 재생 중 사라진 동안 편집(뮤트) 후 복귀: 프로그램 횟수 | 깨짐(경미): `C1 05`가 두 번 감 | 부록 C `break_program_sent_once_on_return_after_edit` |
| 6 | 같은 조건에서 노트가 새 목적지로 닿는가 | 못 깨뜨림 | `edit_during_missing_then_return_reaches_new_destination` |
| 7 | 감시 한 바퀴 안에 지우고 다시 만듦 | 못 깨뜨림 | `rapid_vanish_and_return_within_one_tick_reconnects_playback` |
| 8 | 가상 포트로 재생하는 창 A, 그 포트를 쓰는 정지 창 B. 소멸·복귀 | 못 깨뜨림(메시지는 B에만, A는 계속 재생) | `vanish_and_return_messages_only_user_window_and_other_keeps_playing` |
| 9 | 재생 중 `track::SetPort`로 없는 이름, 그 뒤 그 이름 목적지 생성 | 못 깨뜨림 | `port_changed_to_missing_name_while_playing_then_appears` |
| 10 | 포트를 가상으로 바꾸고 원래 포트를 지운 뒤 undo, 그다음 복귀 | 못 깨뜨림 | `undo_restoring_missing_port_shows_and_clears_status` |
| 11 | 재생 중 창을 닫고 그 포트를 지웠다 다시 만듦 | 못 깨뜨림 | `closed_window_port_changes_are_harmless` |
| 12 | 엔진 없이 복구, 감시 없이 복구 | 못 깨뜨림 | `recover_without_engine_or_monitor` |
| 13 | 이름 포트 테스트 노트 뒤 복구 두 번(500 ms 안), 일시정지 창 유지, 뒤 테스트 노트(이름·가상) | 못 깨뜨림 | `recover_twice_then_test_notes_still_reach` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 스케줄러 스레드는 바뀌지 않는다 | 아니오 | `keyscore-midi`에 변경 없음. 앱은 제어 API(`stop`, `swap`, `collect_garbage`, `send_now`)만 부른다 |
| 스냅샷이 쓰는 포트는 스냅샷이 놓을 때까지 산다 | 아니오 | `port_lost`·`port_returned`·`prepare`의 정리는 `opened`의 `Arc`만 놓는다. 시도 8·11에서 재생이 끊기지 않음 |
| 포트 확인·열기·해제는 메인 스레드에서만 | 아니오 | `PortWatcher::start`와 그 직후 `destinations()`만 백그라운드(`ports.rs:91-93`) |
| 이름 포트는 「그 이름의」 목적지로만 보낸다(OUT-3, OUT-4 라우팅) | 예(감시 없음 또는 감시 한 바퀴 사이) | 부록 B. `is_present`가 이름을 보지 않는다 |
| 사라진 포트는 감시가 알아차려 상태줄에 보이고, 같은 이름이 있으면 그쪽으로 보낸다 | 예 | 부록 A |

모델 불변식(겹침, 노트는 클립 안, 채널 1–16)은 이 Task가 건드리지 않는다.

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 | 충족(복구 절차). 감지는 수동 확인 항목 | `engine.rs:186-207`, `ports.rs:218-224`. 시도 12·13 |
| OUT-3 소멸 감지·상태줄 | 부분 충족(미충족으로 셈) | 같은 이름 포트가 둘 이상이면 엔진이 쓰는 포트의 소멸을 놓친다(부록 A, `engine.rs:413` vs `Reconnector::best_match`). 새 장치 핫플러그(쓰지 않는 포트의 나타남)는 상태줄에 아무것도 보이지 않는다(계획이 그렇게 정함, 제안 6) |
| OUT-3 같은 이름 자동 재연결 | 부분 충족(미충족으로 셈) | 한 개 이름은 충족(시도 7·9·`playback_reconnects_to_returned_port`). 같은 이름 둘 이상은 부록 A |
| OUT-4 재생 시작 시 프로그램·뱅크 | 충족 | 복귀 때 다시 보냄(`transport.rs:348-379`). 사라진 동안 편집했으면 두 번(부록 C) |
| OUT-8 | 범위 밖(S2-T7) | 계획 「범위 밖」 |
| UI-5 출력 포트 상시 표시 | 충족 | `window.rs:676-690, 734-741`. 감시 재시작 동안(`ports.rs:88`)은 「포트 없음」이 잠깐 숨는다 |
| 7.1 상태줄 | 충족 | 위와 같음 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `keyscore-midi`에 변경이 없다.
- 메인 스레드 블로킹: `reregister_main`이 `Arc::get_mut`을 기다리며 1 ms씩 최대 2 s 잔다(`engine.rs:251-262`). 시도 13에서 두 번 연달아 부른 시간은 500 ms 안이었다. 정상 경로에서는 짧지만 최악 2 s 동안 UI가 멈춘다. 계획이 받아들인 방식이라 지적하지 않는다.
- 제품 코드의 `unwrap`/`expect`: 이 diff에 새로 들어온 것 없음. `u16::try_from(..).unwrap_or(u16::MAX)`는 기존 코드.
- 에러가 삼켜지는 곳
  - `reregister_main`의 두 번째 `swap`이 실패하면(`engine.rs:265`) `?`로 빠져나가 `reregister` 결과가 사라지고, 스케줄러에는 포트 0이 없는 빈 스냅샷이 남는다. 그 뒤 가상 포트 테스트 노트·미리듣기는 소리 없이 버려진다(제안 4).
  - `recover_virtual_port`의 `owner.update` 결과를 `let _`로 버린다(`engine.rs:192`). 창이 이미 해제됐으면 할 일이 없어 정당하다.
  - `recover_virtual_port`는 실패를 스스로 알리고 `Err`도 돌려주며, `tick`이 그 `Err`를 다시 알린다(`ports.rs:221-223`, `engine.rs:198-204`). 삼키는 것은 아니지만 같은 실패가 두 번 남는다(지적 3).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 같은 이름 포트가 둘 이상일 때 엔진은 열거 순서의 첫 포트를 열고(`list.iter().find`), `Reconnector`는 고유 ID가 가장 작은 포트에 연결됐다고 본다. 둘이 다르면 엔진이 쓰는 포트가 사라져도 `Reconnector`는 동작을 내지 않는다. 재생은 다음 편집이나 다음 재생 시작까지 사라진 목적지로 소리 없이 가고, 메시지도 「포트 없음」도 없다(부록 A). 반대 경우에는 쓰지 않는 쪽이 사라질 때 「사라졌다」「다시 연결했다」가 거짓으로 남는다. 엔진이 고르는 규칙을 `Reconnector`와 같게(고유 ID 최소) 하거나, 엔진이 연 포트의 정체(`PortInfo`)로 소멸을 판정해야 한다. 같은 이름 장치 두 대(같은 모델 USB 신디 두 대 등)는 실제로 있다 | `crates/keyscore-app/src/engine.rs:413`, `crates/keyscore-app/src/ports.rs:157-216` |
| 2 | 수정 | `is_present`가 이름을 보지 않는다. 연 목적지의 이름이 바뀌면(다른 앱이 가상 포트 이름을 바꿈) `opened`의 핸들을 그대로 다시 써서, 옛 이름 트랙의 소리가 이름이 다른 포트로 간다(부록 B). 감시가 돌면 `Lost`가 핸들을 놓아 막히지만, 감시가 없거나(`NotificationsUnavailable`, 시작 중, 재시작 중) 이름이 바뀐 뒤 감시 한 바퀴 안의 편집·재생에서는 그대로다. 이 Task가 이월받은 「다시 쓰기 전에 확인」의 목적이 감시 없이도 맞는 포트로 보내는 것이므로, 다시 쓰기 전 확인에 이름 비교를 더해야 한다(`OsOutput::name()`과 지금 엔드포인트 이름) | `crates/keyscore-app/src/engine.rs:401-408`, `crates/keyscore-midi/src/os.rs:109-111` |
| 3 | 수정 | 서버 확인에서 복구가 실패하면 같은 실패를 두 번 알린다: `recover_virtual_port`가 「…다시 등록하지 못했다: e」를 열린 창마다 남기고, `tick`이 그 `Err`로 「가상 포트를 다시 등록하지 못했다: e」를 또 남긴다. 서버가 내려간 동안 `is_registered`가 계속 거짓이므로 1 s마다 반복되어 메시지 목록(32개)이 몇 초 만에 이것으로 찬다. 감시자 재시작(`restart_watcher`)도 1 s마다 반복된다. 한 번만 알리고, 같은 실패의 반복은 억제해야 한다. 서버 재시작은 자동 테스트로 일으킬 수 없어 코드로만 확인했다 | `crates/keyscore-app/src/ports.rs:218-224`, `crates/keyscore-app/src/engine.rs:196-206` |
| 4 | 제안 | `reregister_main`의 마지막 `swap`이 실패하면(`QueueFull`) `?`로 빠져 `reregister` 결과를 잃고, 스케줄러에 포트가 없는 빈 스냅샷이 남는다. 가상 포트 테스트 노트·미리듣기가 다음 재생 시작까지 조용히 버려진다. 실패해도 포트 0 스냅샷을 다시 넣도록 재시도하거나 오류를 알린다 | `crates/keyscore-app/src/engine.rs:263-270` |
| 5 | 제안 | 사라진 동안 편집했다가 돌아오면 `transport_document_changed`의 차이 송신과 `transport_port_returned`의 재송신이 겹쳐 같은 뱅크·프로그램을 두 번 보낸다(부록 C). 보고서가 이미 인정한 것이다. 소리는 맞지만 뱅크 셀렉트를 받으면 음색을 다시 불러오는 장치에서는 끊김이 두 번 난다. 재송신 대상에서 방금 보낸 것을 빼면 된다 | `crates/keyscore-app/src/transport.rs:356-372` |
| 6 | 제안 | OUT-3의 「핫플러그 … 감지해 상태줄에 표시」 가운데 새 장치가 꽂힌 것(쓰는 트랙이 없는 포트의 나타남)은 어디에도 보이지 않는다. 계획이 「쓰지 않는 포트의 변화는 메시지를 남기지 않는다」로 정했으나 REQ 문구와 어긋날 수 있다. 결정 요청으로 확인하기를 권한다 | `crates/keyscore-app/src/ports.rs:157-216` |

차단 지적은 없다. 지적 1은 OUT-3의 「같은 이름의 포트가 돌아오면 자동 재연결」과 소멸 표시를 같은 이름 포트가 여럿인 경우에 깨므로 REQ 부분 충족이다. 다만 흔한 단일 이름 경로는 맞고 데이터 손실이 아니며 고칠 범위가 작아 「수정」으로 둔다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1–3을 고치고, 부록 A·B를 테스트로 들인 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 깨진 테스트

아래 테스트는 `crates/keyscore-app/tests/review_s2_t5d.rs`의 도우미(`start`, `start_monitor`, `open`, `run`, `label`, `messages`, `idle`, `pump`, `destination`, `destination_with`, `wait_count`, `wait_seen`, `hears`, `drain`, `project_on`)를 쓴다. 그 파일 끝에 붙이면 돈다.

### A. 같은 이름 포트: 엔진이 쓰는 쪽이 사라지면 재생이 옮겨 가지 않는다(지적 1)

~~~rust
// ===== BREAK CANDIDATES =====

/// 같은 이름 포트 둘. 먼저 열거되는 쪽(엔진이 여는 쪽)의 고유 ID가 더 크다. `Reconnector`는 ID가 작은
/// 쪽에 연결됐다고 본다. 엔진이 쓰는 쪽이 사라지면 남은 같은 이름 포트로 재생이 이어져야 한다.
#[gpui_kit::test]
fn break_duplicate_names_engine_port_vanishes_while_lower_id_remains(cx: &mut TestAppContext) {
    start(cx, "dupbad", false);
    let name = unique("dup bad");
    let base = 0x2200_0000 + i32::try_from(std::process::id() % 0xFFFF).unwrap() * 16;
    let high = destination_with(&name, Some(base + 2));
    let low = destination_with(&name, Some(base + 1));
    let order: Vec<i32> = keyscore_midi::destinations()
        .iter()
        .filter(|d| d.name() == name)
        .map(keyscore_midi::PortInfo::unique_id)
        .collect();
    assert_eq!(order, vec![base + 2, base + 1], "열거 순서 전제");
    start_monitor(cx);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(
        hears(&high.rx, 0x91, Duration::from_secs(3)),
        "엔진은 먼저 열거된(ID가 큰) 목적지로 보낸다는 전제"
    );
    drop(high);
    wait_count(&name, 1);
    idle(cx, Some(h), 10);
    let heard = hears(&low.rx, 0x91, Duration::from_secs(3));
    let m = messages(cx, &v);
    let missing = label(cx, h, "status-port-missing");
    run(cx, h, transport::Stop);
    assert!(
        heard,
        "엔진이 쓰던 같은 이름 포트가 사라졌는데 남은 포트로 재생이 옮겨가지 않았다(상태줄 포트 없음: {missing:?}, 메시지: {m:?})"
    );
}
~~~

실패 출력:

~~~
---- break_duplicate_names_engine_port_vanishes_while_lower_id_remains stdout ----
panicked at crates/keyscore-app/tests/review_s2_t5d.rs:625:5:
엔진이 쓰던 같은 이름 포트가 사라졌는데 남은 포트로 재생이 옮겨가지 않았다(상태줄 포트 없음: None, 메시지: [])
~~~

### B. 감시 없이 이름이 바뀐 포트로 소리가 간다(지적 2)

~~~rust
/// 감시 없이: 연 포트의 이름이 바뀐 뒤 다시 재생하면, 트랙의 이름과 다른 포트로 소리가 가면 안 된다.
#[gpui_kit::test]
fn break_is_present_ignores_rename(cx: &mut TestAppContext) {
    start(cx, "renamenomon", false);
    let name = unique("synth rn");
    let dest = destination(&name);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    run(cx, h, transport::Stop);
    let renamed = format!("{name} 2");
    dest.dest.set_property_string("name", &renamed).unwrap();
    wait_count(&name, 0);
    drain(&dest.rx);
    run(cx, h, transport::Play);
    let heard = hears(&dest.rx, 0x91, Duration::from_secs(1));
    run(cx, h, transport::Stop);
    assert!(
        !heard,
        "「{name}」 트랙의 소리가 이름이 「{renamed}」로 바뀐 포트로 갔다: {:?}",
        messages(cx, &v)
    );
}

~~~

실패 출력:

~~~
---- break_is_present_ignores_rename stdout ----
panicked at crates/keyscore-app/tests/review_s2_t5d.rs:668:5:
「KS rv5d 25095 synth rn」 트랙의 소리가 이름이 「KS rv5d 25095 synth rn 2」로 바뀐 포트로 갔다: []
~~~

### C. 사라진 동안 편집한 뒤 복귀하면 프로그램을 두 번 보낸다(제안 5)

~~~rust
/// 사라진 동안 편집한 뒤 돌아오면 프로그램 체인지는 한 번만 보낸다.
#[gpui_kit::test]
fn break_program_sent_once_on_return_after_edit(cx: &mut TestAppContext) {
    start(cx, "dupprog", true);
    let name = unique("synth dupprog");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    drop(dest);
    wait_count(&name, 0);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("사라졌다")),
        "사라짐 메시지가 없다",
    );
    run(cx, h, track::Next);
    run(cx, h, track::ToggleMute);
    let back = destination(&name);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("다시 연결했다")),
        "다시 연결 메시지가 없다",
    );
    let deadline = Instant::now() + Duration::from_millis(800);
    let mut got = Vec::new();
    while Instant::now() < deadline {
        if let Ok(m) = back.rx.recv_timeout(Duration::from_millis(20)) {
            got.push(m);
        }
    }
    run(cx, h, transport::Stop);
    let programs = got.iter().filter(|m| *m == &vec![0xC1, 5]).count();
    assert_eq!(programs, 1, "{got:?}");
}
~~~

실패 출력:

~~~
---- break_program_sent_once_on_return_after_edit stdout ----
assertion `left == right` failed: [[193, 5], [193, 5], [129, 60, 0], [145, 60, 100], [129, 60, 0]]
  left: 2
 right: 1
~~~
