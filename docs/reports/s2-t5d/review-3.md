# 적대적 리뷰: S2-T5d 포트 감시와 재연결 (리뷰 3회차)

- 리뷰어 세션: 구현 세션·리뷰 1·리뷰 2 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: e089260 (`s2/t5d-port-watch`, `git diff main...HEAD`). 리뷰 2 대응 수정은 1c2c9b6
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t5d_r3.rs`(깨뜨리지 못한 시도 6개, 병렬 3회·단일 스레드 1회 연속 통과). 깨진 시도 1개는 부록.
- 자동 게이트: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check` 모두 통과(리뷰 테스트 포함).

## 1. 코드만 읽고 적은 「내가 검사할 것」

1c2c9b6의 diff와 `ports.rs`(`tick`, `announce_hotplug`, `hotplug_message`), `transport.rs`(`transport_port_lost`, `transport_document_changed`, `transport_port_returned`, `report_missing`), `engine.rs`(`port_lost`, `port_returned`, `prepare`, `open_named`), `keyscore-midi`의 `Reconnector::apply`만 읽고 적었다.

1. 재생 중 쓰는 포트의 이름이 바뀌면 `Lost` → `port_lost`(핸들 놓음, `known_missing`) → `transport_port_lost` → 라이브 준비가 그 이름을 `NotFound`로 빼고 교체한다. 바뀐 동안 소리가 가지 않는가. 이름을 되돌리면 「다시 연결했다」(「바꿨다」 아님)와 함께 프로그램이 한 번, 노트가 다시 가는가.
2. 같은 창의 다른 이름 포트 트랙은 교체 동안 노트가 끊기지 않고 프로그램을 다시 받지 않는가(`setup` 비교가 그 목적지를 「같은 것」으로 보는가).
3. `transport_port_lost`는 주인 창이 재생 중일 때만 움직인다. 재생하지 않는 창이 잃은 포트를 쓰고 다른 창이 다른 포트로 재생 중이면, 재생 중인 창은 건드리지 않는가. 재생하지 않는 창에 「소리 내지 않는다」가 붙지 않는가.
4. 같은 포트를 쓰는 두 창 중 재생 창만 교체하고, 각 창이 「사라졌다」를 한 번만 받는가. 그 뒤 감시를 더 돌려도 메시지가 늘지 않는가(`report_missing`의 중복 억제).
5. `Reconnector::apply`는 쓰던 포트가 빠지고 같은 이름 포트가 남으면 한 이벤트에서 `Lost`와 `Reconnect`를 함께 낸다. 그러면 같은 틱에 `transport_port_lost`(그 이름 없이 교체, 「없어 그 트랙은 소리 내지 않는다」)와 `transport_port_returned`(새 포트로 교체)가 잇따른다. 실제로는 소리가 계속 나는데 「소리 내지 않는다」가 남을 것이라 예상한다. 문구는 「다시 연결했다」여야 한다(`!applied.any(Lost)` 조건).
6. 끊긴 적 없이 더 작은 ID의 같은 이름 포트가 나타나면 `connected_before`가 참 → 「같은 이름의 다른 포트로 바꿨다」. `before`는 `wanted`만 보므로 이번 틱에 처음 원하게 된 이름은 거짓이다(그 이름은 이벤트 뒤에 등록되므로 동작 자체가 없다).
7. `announce_hotplug`는 창마다 이벤트를 나타남·사라짐으로 나눠 한 줄씩 낸다. 같은 틱에 쓰는 포트가 섞이면 쓰는 창의 묶음 줄에서 그 이름이 빠지는가. 이름은 모두 들어가는가.
8. 같은 틱에 잃는 쓰는 포트가 많으면 이름마다 교체가 한 번씩 들어간다. 명령 큐(256)와 포트 상한(32)으로 보아 큐가 차지 않는다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 이름 바뀜·되돌림 | 일부(`rename_while_playing_with_monitor_stops_sound_to_renamed_port`: 바뀐 동안 소리 없음만) | 리뷰 테스트로 확인, 통과 |
| 2 다른 포트 트랙 | 없음 | 리뷰 테스트로 확인, 통과 |
| 3 재생하지 않는 창의 포트 소멸 | 일부(`vanish_and_return_messages_only_user_window_and_other_keeps_playing`는 가상 포트로 재생) | 리뷰 테스트로 확인(다른 이름 포트로 재생), 통과 |
| 4 공유 포트 | 없음 | 리뷰 테스트로 확인, 통과 |
| 5 같은 이름 둘 중 쓰던 포트 소멸 | 일부(`duplicate_names_consistent_choice_reconnects`는 문구만) | 리뷰가 깨뜨림(부록 A, 메시지 목록에만 남음) |
| 6 「바꿨다」 문구 | 없음(구현 쪽 테스트에 「바꿨다」를 보는 것이 없다) | 리뷰 테스트로 확인, 통과 |
| 7 핫플러그 묶음 | 없음(기존 테스트는 이름 포함만 본다) | 리뷰 테스트로 확인, 통과 |
| 8 큐 | 없음 | 코드로 확인(`QUEUE = 256`, `MAX_SCHEDULER_PORTS = 32`, 교체마다 `collect_garbage`) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 두 이름 포트 트랙 재생 중 한 포트 이름을 바꿨다가 되돌림. 바뀐 동안 소리 없음, 「소리 내지 않는다」 한 번, 되돌리면 프로그램 한 번·노트·마지막 「다시 연결했다」, 「바꿨다」 없음, 다른 포트에 프로그램 재송신 없음 | 못 깨뜨림 | `rename_away_and_back_while_playing_restores_sound_and_keeps_other_port` |
| 2 | 위와 같은 구성에서 이름 바뀜 동안 다른 포트 노트 간격·프로그램 재송신 | 못 깨뜨림 | `other_port_sound_continuous_through_rename` |
| 3 | 창 B(정지)가 쓰는 포트 소멸, 창 A는 다른 이름 포트로 재생. A 재생 유지·프로그램 재송신 없음, A에 그 포트 문구 없음, B에 「소리 내지 않는다」 없음 | 못 깨뜨림 | `lost_port_of_idle_window_does_not_touch_playing_window` |
| 4 | 같은 포트를 쓰는 두 창(하나만 재생)에서 이름 바뀜 뒤 감시를 더 돌림. 재생 창 (사라졌다 1, 소리 내지 않는다 1), 다른 창 (1, 0), 소리 없음 | 못 깨뜨림 | `shared_port_lost_reprepares_only_owner_once` |
| 5 | 정지 중 더 작은 ID의 같은 이름 포트 등장 → 「바꿨다」, 「다시 연결했다」 없음 | 못 깨뜨림 | `lower_id_appearing_while_connected_says_switched` |
| 6 | 같은 이름 둘 중 쓰던(작은 ID) 포트가 재생 중 빠짐. 남은 포트로 옮김·프로그램 한 번·「다시 연결했다」, 「소리 내지 않는다」가 남지 않는가 | 깨짐(메시지 목록에 「없어 그 트랙은 소리 내지 않는다」가 남음. 소리·프로그램·마지막 문구는 맞음) | 부록 A |
| 7 | 쓰지 않는 포트 5개와 쓰는 포트 1개를 한 틱에 만들고 지움. 쓰는 창에 종류마다 한 줄, 이름 5개 모두, 쓰는 포트 이름 없음, 쓰는 포트는 「다시 연결했다」 | 못 깨뜨림 | `grouped_hotplug_one_line_per_kind_excluding_used` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 이름 포트는 그 이름의 목적지로만 보낸다 | 아니오(재현 못 함) | 시도 1·2·4. 리뷰 2 부록 A는 `review_s2_t5d_r2.rs`에서 통과 |
| 잃은 포트 처리는 재생 주인 창의 스냅샷만 바꾼다 | 아니오 | 시도 3·4. `transport_document_changed`의 주인·재생 확인 |
| 돌아온 포트에 음색을 채널마다 한 번 | 아니오 | 시도 1·6(같은 틱 두 번 교체에도 한 번) |
| 다른 목적지의 음색은 다시 보내지 않는다 | 아니오 | 시도 1·2·3 |
| 쓰는 창은 자기 포트의 핫플러그 문구를 받지 않는다 | 아니오 | 시도 7 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 | 충족(복구 절차, 1c2c9b6에서 바뀌지 않음). 서버 재시작 감지는 수동 확인 항목 | `crates/keyscore-app/src/engine.rs:186-222`, `crates/keyscore-app/src/ports.rs:261-265` |
| OUT-3 핫플러그·소멸 표시 | 충족 | `crates/keyscore-app/src/ports.rs:272-310`(쓰지 않는 창, 묶음), `ports.rs:227-241`(쓰는 창). 시도 7 |
| OUT-3 같은 이름 자동 재연결 | 충족 | `ports.rs:243-256`, `transport.rs:347-365`. 시도 1·5·6(소리·문구), 리뷰 2 시도 1·4·5 |
| OUT-4 재생 시작·복귀 때 프로그램·뱅크 | 충족 | `transport.rs:353-365`. 시도 1·6 |
| UI-5, 7.1 상태줄 | 충족 | `crates/keyscore-app/src/window.rs:676-741`. 상태줄은 마지막 메시지만 보인다 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `keyscore-midi`에 변경 없음.
- 메인 스레드: `Lost`마다 주인 창이 라이브 준비 한 번(목적지 열거 없음, `known_missing`). 같은 틱에 잃는 포트가 많아도 교체는 32개 이하라 명령 큐(256)가 차지 않는다.
- 제품 코드의 `unwrap`/`expect`: 1c2c9b6에 새로 들어온 것 없음.
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. `transport_port_lost`의 교체 실패는 `transport_document_changed`가 「재생 중 편집을 반영하지 못했다」로 알린다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 같은 이름 포트가 둘 이상일 때 쓰던 포트가 재생 중 빠지면 `Lost`와 `Reconnect`가 같은 틱에 오고, `transport_port_lost`의 라이브 준비가 「출력 포트 「X」이 없어 그 트랙은 소리 내지 않는다」를 남긴 뒤 곧바로 「다시 연결했다」가 온다(부록 A). 소리·프로그램·상태줄(마지막 메시지)은 맞아 사용자에게 보이는 오동작은 아니다. 다만 그 문장이 목록에 남아 있는 동안 `report_missing`의 중복 억제가 같은 문장을 다시 내지 않는다. `tick`에서 같은 이름의 `Reconnect`가 뒤따르는 `Lost`는 `transport_port_lost`를 건너뛰면 교체도 한 번으로 준다 | `crates/keyscore-app/src/ports.rs:227-241`, `crates/keyscore-app/src/transport.rs:347-349` |
| 2 | 제안 | 「같은 이름의 다른 포트로 바꿨다」 문구와 핫플러그 묶음(「N개가 …」)은 구현 쪽 테스트가 없다. 리뷰 테스트(시도 5·7)가 덮지만 구현 테스트로 옮겨 두기를 권한다 | `crates/keyscore-app/tests/port_watch.rs` |

리뷰 2 지적 1(재생 중 이름 바뀜)은 근본에서 고쳐졌다. 바뀐 동안 소리가 가지 않고, 되돌리면 다시 가며, 다른 창·다른 포트 트랙의 재생과 음색을 건드리지 않고, 메시지가 쌓이지 않는다(시도 1–4). 제안 2(문구 구분)와 제안 3(묶음)도 의도대로 동작한다(시도 5–7). `main` 대비 회귀, REQ·결정 위반, 사용자에게 보이는 오동작은 찾지 못했다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2는 제안이다. 구현자가 받아들이거나 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 깨진 테스트

아래 테스트는 `crates/keyscore-app/tests/review_s2_t5d_r3.rs`의 도우미(`start`, `open`, `run`, `messages`, `mine`, `destination_with`, `wait_count`, `wait_seen`, `hears`, `drain`, `collect`, `pump_until_heard`, `project_on`, `base_id`)를 쓴다. 그 파일 끝에 붙이면 돈다.

### A. 같은 이름 둘 중 쓰던 포트가 재생 중 빠지면 「소리 내지 않는다」가 목록에 남는다(지적 1, 제안)

~~~rust
/// 같은 이름 포트 둘 중 쓰던(더 작은 ID) 포트가 재생 중 빠지면 남은 포트로 옮긴다. 이것은 「다시 연결했다」다.
/// 같은 틱에 「사라졌다」 뒤 곧바로 옮기므로, 실제로는 소리가 계속 나는데 「없어 그 트랙은 소리 내지 않는다」가
/// 메시지 목록에 남는지 본다.
#[gpui_kit::test]
fn duplicate_lost_mid_playback_moves_without_silent_claim(cx: &mut TestAppContext) {
    start(cx, "dupl", true);
    let name = unique("dup dupl");
    let base = base_id(2);
    let low = destination_with(&name, Some(base + 1));
    let high = destination_with(&name, Some(base + 2));
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&low.rx, 0x91, Duration::from_secs(3)));
    drain(&high.rx);
    drop(low);
    wait_count(&name, 1);
    let mut got = pump_until_heard(cx, h, &high.rx, 0x91, "남은 포트로 옮기지 않았다");
    got.extend(collect(&high.rx, Duration::from_millis(400)));
    run(cx, h, transport::Stop);
    assert_eq!(got.iter().filter(|m| *m == &vec![0xC1, 5]).count(), 1, "{got:?}");
    let m = mine(messages(cx, &v), &name);
    assert!(m.last().unwrap().contains("다시 연결했다"), "{m:?}");
    assert!(m.iter().all(|x| !x.contains("바꿨다")), "{m:?}");
    assert!(
        m.iter().all(|x| !x.contains("소리 내지 않는다")),
        "소리가 계속 나는데 「소리 내지 않는다」가 남았다: {m:?}"
    );
}
~~~

실패 출력(소리·프로그램·마지막 문구 단언은 통과하고 마지막 단언만 실패):

~~~
---- duplicate_lost_mid_playback_moves_without_silent_claim stdout ----
panicked at crates/keyscore-app/tests/review_s2_t5d_r3.rs:542:5:
소리가 계속 나는데 「소리 내지 않는다」가 남았다: ["출력 포트 「KS rv5d3 60102 dup dupl」이 사라졌다. 같은 이름의 포트가 돌아오면 다시 연결한다", "출력 포트 「KS rv5d3 60102 dup dupl」이 없어 그 트랙은 소리 내지 않는다", "출력 포트 「KS rv5d3 60102 dup dupl」에 다시 연결했다"]
~~~
