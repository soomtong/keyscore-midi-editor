# 적대적 리뷰: S2-T5d 포트 감시와 재연결 (리뷰 2회차)

- 리뷰어 세션: 구현 세션·리뷰 1 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 82652cc (`s2/t5d-port-watch`, `git diff main...HEAD`). 리뷰 1 대응 수정은 eaaa415
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t5d_r2.rs`(깨뜨리지 못한 시도 8개, 병렬 3회·단일 스레드 1회 연속 통과). 깨진 시도 1개와 판정하지 못한 시도 1개는 부록.

## 1. 코드만 읽고 적은 「내가 검사할 것」

eaaa415의 diff와 `ports.rs`·`engine.rs`(`recover`, `reregister_main`, `prepare`, `open_named`, `port_lost`, `port_returned`)·`transport.rs`(`transport_document_changed`, `transport_port_returned`)·`window.rs` 상태줄, `keyscore-midi`의 `Reconnector`·`diff_ports`·`watcher.rs`·`scheduler.rs`의 교체 시 노트 정리만 읽고 적었다.

1. 지적 1: 엔진의 `best_match`와 `Reconnector`의 `best_match`가 같은 규칙인가(고유 ID 최소, 이름 정확 일치). 재생 중 더 작은 ID의 같은 이름 포트가 나타나면 `Reconnect` → 엔진도 그쪽으로 옮기고, 옛 포트에 울리던 노트가 꺼지며(`release_changed_ports`), 새 포트에 프로그램이 한 번 가는가.
2. 지적 1: 정지 중 더 작은 ID 포트가 나타난 뒤 재생 시작이 그쪽을 여는가(비재생 확인의 `best_match` 비교), 그 포트가 사라지면 남은 포트로 옮기는가.
3. 지적 2: 감시 없이 이름이 바뀌었다가 원래 이름으로 돌아오면, 놓은 핸들 대신 다시 열어 소리가 가는가.
4. 지적 2의 감시 중 경로: `Lost`는 `opened`에서 핸들을 빼고 `known_missing`에 넣을 뿐 스냅샷을 바꾸지 않는다. 재생 중 이름이 바뀌면 「사라졌다」·「포트 없음」이 보이는데 소리는 이름이 바뀐 포트로 계속 갈 것이라고 예상한다.
5. 비재생 확인이 재생 시작마다 목적지 목록을 읽는다. 메인 스레드 비용.
6. 제안 5: `transport_port_returned`가 그 목적지의 `setup`을 지운 뒤 교체한다. 채널 둘(뱅크 포함), 같은 채널의 뮤트 트랙, 가상 포트 트랙이 섞였을 때 채널마다 정확히 한 번, 뮤트 트랙의 음색은 보내지 않고, 다른 목적지는 다시 보내지 않는가. 두 번 잃고 되찾을 때도 매번 가는가. 교체 실패 때 `old_setup`으로 되돌리면 그 목적지의 음색이 빠진 채 남는다(다음 편집이 다시 보냄).
7. 지적 3: 실패를 처음 한 번만 알리고 10 s 간격으로 되풀이. `failures`를 틱 앞에서 읽어 넘김. 성공하면 0으로. 감시자 재시작은 성공 때만이라 등록이 실패하는 동안 옛 감시자가 남는다.
8. 제안 6: 쓰지 않는 창에만 「새로 보인다」·「없어졌다」. 이름 바뀜(옛 이름 사라짐 + 새 이름 나타남)에서 쓰는 창은 새 이름의 「새로 보인다」를 받고 마지막 메시지가 무엇인가(핫플러그 루프가 재연결 루프보다 먼저 돈다). 포트를 많이 내는 장치에서 메시지 상한(32)과 상태줄 마지막 메시지. 키맵 오류가 있으면 상태줄 오른쪽은 키맵 오류가 차지한다(기존 설계).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 재생 중 더 작은 ID 포트 등장 | 없음 | 리뷰 테스트로 확인, 통과 |
| 2 정지 중 더 작은 ID 포트 등장 뒤 재생·소멸 | 없음 | 단독 실행은 통과, 같은 프로세스에서 다른 테스트 뒤에는 감시가 그 포트의 생성·소멸을 받지 못함(부록 B, 판정 못 함) |
| 3 감시 없이 이름 바뀜 후 복귀 | 없음(`break_is_present_ignores_rename`은 바뀐 쪽만) | 리뷰 테스트로 확인, 통과 |
| 4 감시 중 재생하며 이름 바뀜 | 없음(`renamed_in_use_port_is_reported_lost`는 정지 중 표시만) | 리뷰가 깨뜨림(부록 A) |
| 5 열거 비용 | 없음 | 목적지 30여 개에서 `destinations()` 한 번 약 1.3 ms(측정 뒤 테스트는 지움: 테스트 스레드에서 먼저 만든 CoreMIDI 클라이언트가 같은 프로세스의 알림을 막아 다른 테스트를 깨뜨렸다) |
| 6 돌아온 포트의 뱅크·프로그램 | 일부(`break_program_sent_once_on_return_after_edit`: 프로그램 하나) | 리뷰 테스트로 확인, 통과 |
| 7 서버 확인 반복 | 없음 | 서버 재시작을 일으킬 수 없어 코드로만 확인 |
| 8 핫플러그 문구·상한 | 일부(`vanish_and_return_messages_only_user_window_and_other_keeps_playing`) | 리뷰 테스트로 확인, 통과 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 재생 중 더 작은 ID의 같은 이름 포트가 나타남. 새 포트에 노트·프로그램 한 번, 옛 포트에 노트가 더 가지 않음, 쓰는 창에 「새로 보인다」 없음 | 못 깨뜨림 | `lower_id_duplicate_appearing_mid_playback_moves_playback_once` |
| 2 | 감시 없이: 정지 중 이름 바뀜 → 재생(없음) → 원래 이름으로 → 재생 | 못 깨뜨림(다시 열어 프로그램·노트가 감) | `rename_and_back_without_monitor_reopens` |
| 3 | 감시 중 재생하며 쓰는 포트의 이름을 바꿈 | 깨짐: 「사라졌다」·「포트 없음」이 보이는데 노트가 이름이 바뀐 포트로 계속 간다 | 부록 A `rename_while_playing_with_monitor_stops_sound_to_renamed_port` |
| 4 | 채널 둘(ch2 뱅크 3·프로그램 5, ch3 프로그램 7), ch3 뮤트 트랙(프로그램 9), 가상 포트 트랙(프로그램 11). 재생 중 소멸·복귀 | 못 깨뜨림(ch2·ch3 각 한 번, 뮤트 음색 없음, 뱅크 MSB 있음, 프로그램이 첫 노트보다 먼저) | `returned_port_gets_each_channel_setup_once` |
| 5 | 재생 중 같은 포트를 두 번 잃고 되찾음 | 못 깨뜨림(매번 프로그램 한 번) | `returned_twice_sends_program_each_time` |
| 6 | 목적지 40개를 한꺼번에 만들고 지움(포트 많은 장치). 쓰는 포트가 없는 창 | 못 깨뜨림(목록 32 이하, 상태줄이 마지막 메시지, 「포트 없음」 유지) | `many_port_hotplug_is_bounded_and_status_shows_last` |
| 7 | 쓰는 창·쓰지 않는 창에서 이름 바뀜: 문구와 마지막 메시지 | 못 깨뜨림(쓰는 창은 「사라졌다」가 마지막이고 「없어졌다」 없음, 쓰지 않는 창은 「없어졌다」·새 이름 「새로 보인다」, 「사라졌다」 없음) | `hotplug_wording_per_window_and_rename_order` |
| 8 | 만든 직후·0.3 s 뒤 고유 ID를 바꾸는 목적지(저장된 ID를 되살리는 앱)를 감시자가 새 ID로 보는가. 못 보면 엔진(새 열거)과 `Reconnector`(이벤트)가 다시 갈린다 | 못 깨뜨림(옛 ID 사라짐·새 ID 나타남 이벤트가 온다) | `probe_watcher_sees_unique_id_change`, `probe_watcher_sees_late_unique_id_change` |
| 9 | 정지 중 더 작은 ID 포트 등장 → 재생 → 그 포트 소멸 | 판정 못 함(부록 B) | 없음(부록 B) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 스케줄러 스레드는 바뀌지 않는다 | 아니오 | `keyscore-midi`에 변경 없음 |
| 엔진이 쓰는 포트와 감시가 연결됐다고 보는 포트가 같다 | 아니오(재현 못 함) | 둘 다 고유 ID 최소. 시도 1·8 |
| 이름 포트는 그 이름의 목적지로만 보낸다 | 예(감시 중, 재생 중 이름 바뀜부터 다음 편집까지) | 부록 A. `Lost`가 스냅샷을 바꾸지 않는다 |
| 돌아온 포트에 음색을 채널마다 한 번 | 아니오 | 시도 4·5 |
| 창 메시지는 32개를 넘지 않는다 | 아니오 | 시도 6 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 | 충족(복구 절차). 서버 재시작 감지는 수동 확인 항목 | `engine.rs:186-222`, `ports.rs:244-248` |
| OUT-3 핫플러그·소멸 표시 | 충족 | `ports.rs:196-213`(쓰지 않는 창), `ports.rs:217-230`(쓰는 창). 시도 6·7 |
| OUT-3 같은 이름 자동 재연결 | 부분 충족(미충족으로 셈) | 소멸·복귀·같은 이름 여럿은 충족(시도 1·4·5, 리뷰 1 테스트). 재생 중 이름이 바뀐 포트는 「사라졌다」로 알리면서 그 포트로 계속 보낸다(부록 A, `ports.rs:217-218`, `engine.rs:244-247`) |
| OUT-4 재생 시작·복귀 때 프로그램·뱅크 | 충족 | `transport.rs:348-360`. 시도 4·5 |
| UI-5, 7.1 상태줄 | 충족 | `window.rs:676-741` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `keyscore-midi` 변경 없음.
- 메인 스레드 블로킹: 비재생 준비가 이름 포트를 하나라도 다시 쓸 때 목적지 목록을 한 번 읽는다(약 1.3 ms, 목적지 30여 개). 서버 확인 실패 중에는 10 s마다 `reregister_main`이 최대 2 s 기다릴 수 있다(서버가 내려가 소리가 나지 않는 동안이라 받아들인다).
- 제품 코드의 `unwrap`/`expect`: 이 수정에 새로 들어온 것 없음.
- 에러가 삼켜지는 곳
  - `recover(cx, false)`: 잇단 실패의 두 번째부터 알리지 않는다. 의도된 억제다.
  - `transport_port_returned`가 `setup`을 지운 뒤 교체가 실패하면 `old_setup`(그 목적지 음색이 빠진 것)으로 되돌린다. 교체 실패 자체는 「재생 중 편집을 반영하지 못했다」로 알리고, 다음 편집이 그 목적지 음색을 「바뀐 것」으로 보내므로 잃지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 감시 중 재생하는 동안 쓰는 포트의 이름이 바뀌면(Audio MIDI Setup에서 IAC 버스·네트워크 세션 이름 바꾸기, 다른 앱의 가상 포트 이름 바꾸기) `Lost`가 「사라졌다」를 알리고 상태줄에 「포트 없음」이 뜨지만, 스냅샷은 옛 핸들을 그대로 들고 있어 그 트랙의 노트가 이름이 다른 포트로 계속 간다(부록 A). 다음 편집이 오면 그때 갑자기 끊긴다. 리뷰 1 지적 2는 「감시가 돌면 `Lost`가 핸들을 놓아 막힌다」를 전제로 감시 없는 경로만 지적했고, 대응도 「재생 중 변화는 감시가 알린다」에 기댔으나 `port_lost`는 `opened`에서만 빼므로 재생 중에는 막히지 않는다. `Lost`를 받은 쓰는 창이 재생 중이면 `transport_port_returned`처럼 스냅샷을 다시 만들어 교체한다(`known_missing`이라 그 포트가 빠지고, 스케줄러가 옛 포트의 울리는 노트를 끈다). 이름이 바뀌지 않고 사라진 경우는 보낼 곳이 없어 소리 차이가 없다 | `crates/keyscore-app/src/ports.rs:217-230`, `crates/keyscore-app/src/engine.rs:244-247` |
| 2 | 제안 | 쓰는 창은 같은 이름의 더 작은 ID 포트가 새로 꽂혀도 「출력 포트 「X」에 다시 연결했다」를 받는다(정지 중에도). 끊긴 적이 없는데 「다시」라 오해할 수 있다. `Reconnect` 직전 상태가 `Connected`였으면 「X(같은 이름의 다른 포트)로 바꿨다」처럼 구분하기를 권한다 | `crates/keyscore-app/src/ports.rs:231-239` |
| 3 | 제안 | 쓰지 않는 창의 핫플러그 알림은 창마다 포트마다 한 줄이다. 포트가 32개를 넘는 장치(대형 인터페이스, 다수 IAC 버스)를 꽂으면 창의 메시지가 모두 밀려 난다(시도 6). 한 틱의 같은 종류 이벤트를 한 줄로 묶는(「출력 포트 N개가 새로 보인다: …」) 방식을 권한다 | `crates/keyscore-app/src/ports.rs:196-213` |

지적 1은 이 Task가 맡은 OUT-3 「소멸 감지·이름 기반 재연결」과 리뷰 1 지적 2의 목적(이름 포트는 그 이름의 목적지로만)에 걸리는 사용자에게 보이는 오동작이다. `main`에도 같은 소리 흐름이 있으나 `main`은 소멸을 알리지 않으므로, 「사라졌다」·「포트 없음」과 실제 소리가 어긋나는 것은 이 Task가 새로 만든 모순이다. 고칠 범위가 작다(`Lost` 분기에서 재생 중인 쓰는 창에 교체를 부름).

리뷰 1 지적 1·2(감시 없는 경로)·3과 제안 4·5·6은 근본에서 고쳐졌다. 시도 1·2·4·5·6·7로 확인했고 지적 3은 코드로 확인했다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1을 고치고 부록 A를 테스트로 들인 뒤 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 깨진 테스트

아래 테스트는 `crates/keyscore-app/tests/review_s2_t5d_r2.rs`의 도우미(`start`, `open`, `run`, `messages`, `pump`, `destination`, `wait_count`, `wait_seen`, `hears`, `drain`, `collect`, `project_on`)를 쓴다. 그 파일 끝에 붙이면 돈다.

### A. 감시 중 재생하며 쓰는 포트의 이름이 바뀌면 이름이 다른 포트로 계속 보낸다(지적 1)

~~~rust
/// 감시 중 재생하는 동안 쓰는 포트의 이름이 바뀌면 「사라졌다」를 알린다. 그 뒤 트랙 소리가 이름이 바뀐
/// 포트로 계속 가면 안 된다.
#[gpui_kit::test]
fn rename_while_playing_with_monitor_stops_sound_to_renamed_port(cx: &mut TestAppContext) {
    start(cx, "rnplay", true);
    let name = unique("synth rnplay");
    let dest = destination(&name);
    wait_seen(cx, &name, true);
    let (h, v) = open(cx, project_on(&name));
    run(cx, h, transport::Play);
    assert!(hears(&dest.rx, 0x91, Duration::from_secs(3)));
    let renamed = format!("{name} 2");
    dest.dest.set_property_string("name", &renamed).unwrap();
    wait_count(&name, 0);
    pump(
        cx,
        Some(h),
        |cx| messages(cx, &v).iter().any(|m| m.contains("사라졌다")),
        "사라짐 메시지가 없다",
    );
    drain(&dest.rx);
    let got = collect(&dest.rx, Duration::from_millis(1500));
    run(cx, h, transport::Stop);
    assert!(
        got.iter().all(|m| m[0] != 0x91),
        "「사라졌다」 뒤에도 「{name}」 트랙 소리가 「{renamed}」로 간다: {got:?}"
    );
}
~~~

실패 출력(3회 모두 같음):

~~~
---- rename_while_playing_with_monitor_stops_sound_to_renamed_port stdout ----
panicked at crates/keyscore-app/tests/review_s2_t5d_r2.rs:628:5:
「사라졌다」 뒤에도 「KS rv5d2 49036 synth rnplay」 트랙 소리가 「KS rv5d2 49036 synth rnplay 2」로 간다: [[145, 60, 100], [129, 60, 0], [145, 60, 100], [129, 60, 0], [145, 60, 100], [129, 60, 0]]
~~~

### B. 판정하지 못한 시도: 정지 중 더 작은 ID 포트 등장 → 재생 → 그 포트 소멸

정지 중 같은 이름의 더 작은 ID 포트(`base + 1`)를 만들고, 재생 시작이 그쪽을 여는 것을 소리로 확인한 뒤 지워 남은 포트(`base + 2`)로 옮기는지 보았다.

- 이 테스트만 따로 돌리면 4회 모두 통과했다. 메시지는 「다시 연결했다」(등장), 「사라졌다」, 「다시 연결했다」(소멸 뒤) 순서다.
- 같은 테스트 바이너리에서 다른 `#[gpui_kit::test]` 하나라도 먼저 돌면(단일 스레드) 매번 실패했다. 그때는 앱의 감시뿐 아니라 테스트 안에서 따로 띄운 `PortWatcher`도 그 포트(`base + 1`)의 생성·소멸 이벤트를 받지 못했다. 같은 시점에 만든 다른 이름의 목적지는 앱 감시가 「새로 보인다」로 알렸다.
- 엔진·감시의 선택 규칙 문제라면 단독 실행에서도 깨져야 하고, 이벤트 자체가 오지 않으므로 이 Task의 코드가 아니라 같은 프로세스 안에서 만든 CoreMIDI 클라이언트·목적지의 알림 전달(테스트 환경)로 본다. 지적으로 올리지 않았고, 다른 테스트를 불안정하게 하므로 파일에서 뺐다.

~~~
test lower_id_duplicate_appearing_while_stopped_then_vanishing ... LOWSTOP before []
LOWSTOP after [] monitoring=true
LOWSTOP probe ["출력 포트 「KS rv5d2 43172 lowstop probe」이 새로 보인다"]
panicked: 엔진이 쓰던 포트가 사라졌는데 남은 같은 이름 포트로 옮기지 않았다: []
~~~
