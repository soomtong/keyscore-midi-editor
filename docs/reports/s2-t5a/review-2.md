# 적대적 리뷰: S2-T5a MIDI 엔진 연결과 트랜스포트 Action (리뷰 2회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 에이전트(구현자 세션과 다름, 1회차 리뷰어와도 다른 세션)
- 대상 커밋: `f9b66b6`(브랜치 `s2/t5-transport-panels`, 범위 `git diff main...HEAD`, 리뷰 1 대응은 `git show f9b66b6`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트를 열기 전에 `engine.rs`, `transport.rs`, `window.rs`와 S1 `scheduler.rs`의 `swap`·`chase`·`move_base`·`step_inner`·`collect_garbage`를 읽고 적었다.

1. 기준점(`Anchor`) 교체: 자리(`here`)는 `prepare` 전에 구하고 새 기준점 시각(`HostTime::now()`)은 `prepare`·`swap` 뒤에 찍는다. 그 사이 시간이 편집마다 빠지는가(큰 프로젝트, 편집 여러 번).
2. 템포 편집 여러 번, 템포 변화가 있는 곡에서 템포 아닌 편집 거듭 뒤 일시정지 자리.
3. 교체 실패(`swap` 성공 뒤 `send_now` 실패 포함) 시 기준점·`setup`·메시지의 일관성.
4. `setup` 차이 비교가 (스냅샷 포트 번호, 메시지)다. 포트 번호는 스냅샷마다 트랙 순서로 다시 매겨진다. 번호가 당겨지거나 밀릴 때 빠지거나 넘치는가.
5. 뮤트·솔로: `setup`은 뮤트 트랙도 담지만 렌더러는 뮤트 트랙의 틱 0 프로그램을 빼고 chase도 그것만 본다. 재생 중 뮤트를 풀면 프로그램이 나가는가.
6. 뱅크만 바꾸면 바뀐 CC 32(또는 CC 0)만 나가고 프로그램 체인지가 따르지 않는가(뱅크 셀렉트는 다음 프로그램 체인지에서 적용된다).
7. chase 의존: 틱 0·중간·곡 끝 뒤 재개에서 프로그램이 첫 노트보다 먼저인가. 128트랙에서 Play 연타, 다른 창 재생 중 Play.
8. 앱이 `Scheduler::collect_garbage`를 부르는가. 스케줄러 문서(`scheduler.rs:397-399`)는 「교체 뒤 한 번 이상」 부르라고 한다. 안 부르면 교체가 수백 번 쌓인 뒤 무슨 일이 생기는가(Play마다, 재생 중 편집마다 교체가 하나).
9. 못 찾은 포트 기억(리뷰 1 제안 3): 재생 중 처음 가리키는 이름 포트는 찾는가, 문구가 편집마다 쌓이지 않는가, 다음 재생에서 다시 찾는가.
10. 실패 시 상태 맞춤: 창 Stopped, 스케줄러 정지, 앞 주인 정지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 기준점 시각 누락 | 없음 | 깨짐(지적 2) |
| 2 템포 편집 여러 번·곡 안 템포 | 한 번 편집만(`pause_after_tempo_edit_while_playing_keeps_position`) | 못 깨뜨림 |
| 3 교체 실패 일관성 | 없음 | 큐 넘침이 필요해 실행으로 재현하지 않음(제안 1) |
| 4 포트 번호 재매김 | 없음 | 깨짐(지적 5) |
| 5 뮤트·솔로 해제 | 없음 | 깨짐(지적 3) |
| 6 뱅크만 편집 | 없음(`program_edit_while_playing_is_sent`는 프로그램만) | 깨짐(지적 4) |
| 7 chase 시작 위치·연타·인수 | 일부(일시정지 재개, 100트랙 1회, 64트랙 2회) | 못 깨뜨림 |
| 8 `collect_garbage` | 없음 | 깨짐(지적 1) |
| 9 못 찾은 포트 기억 | 없음 | 못 깨뜨림 |
| 10 실패 시 상태 | 없음 | 지적 1에서 「재생 중인데 무음, 메시지 없음」으로 드러남 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 매 박 노트, Play 280회(3 ms 간격) 뒤 소리 | 깨짐: 창은 Playing, 메시지 없음, 2 s 동안 Note On 없음 | 부록 A `play_still_sounds_after_many_plays` |
| 2 | 재생 중 이름 편집 300회(2 ms 간격) 뒤 Stop | 깨짐: 창은 Stopped인데 1.2 s 동안 Note On 60이 계속 옴, 메시지 없음 | 부록 A `stop_works_after_many_live_edits` |
| 3 | 뮤트 트랙(채널 2, 프로그램 9)을 재생 중 뮤트 해제 | 깨짐: 받은 것 `[[128,60,0],[145,64,100]]`, `[0xC1, 9]` 없음 | 부록 A `unmuting_track_while_playing_sends_its_program` |
| 4 | 뱅크 1 → 2(프로그램 5 그대로) 재생 중 편집 | 깨짐: 받은 것 `[[176,32,2],[128,60,0],[144,62,100]]`, 뱅크 뒤 프로그램 체인지 없음 | 부록 A `bank_edit_while_playing_is_followed_by_program` |
| 5 | 트랙 A(X, 채널 1, 프로그램 5)를 재생 중 포트 Y(트랙 B가 쓰는 중)로 옮김 | 깨짐: Y가 받은 것 `[[129,64,0],[193,9],[128,60,0],[129,64,0],[144,62,100]]`, `[0xC0, 5]` 없음 | 부록 A `moving_track_to_other_named_port_sends_program_there` |
| 6 | 큰 프로젝트(16트랙 × 4000노트)에서 재생 중 편집 40회(2.1 s) 뒤 일시정지 | 깨짐: `Paused(Tick(155))`, 그 전에 박 9개(틱 3840까지)를 이미 들음 | 부록 A `pause_position_does_not_drift_with_many_live_edits` |
| 7 | 120 → 60 → 240 BPM을 0.3 s마다 편집 뒤 일시정지(기대 약 2016) | 못 깨뜨림 | `pause_after_several_tempo_edits_matches_elapsed_time` |
| 8 | 틱 960에 템포 변화가 있는 곡에서 이름 편집 10회 뒤 일시정지 | 못 깨뜨림(오차 150틱 안) | `non_tempo_edits_in_song_with_tempo_change_keep_position` |
| 9 | 곡 끝 뒤 일시정지 → 재개, 프로그램 chase | 못 깨뜨림 | `resume_past_song_end_chases_program` |
| 10 | 뱅크·프로그램 있는 128트랙 Play 20회 연타 | 못 깨뜨림(Playing, 메시지 없음, 틱 1920 노트 수신) | `many_tracks_survive_play_burst` |
| 11 | 128트랙 창 재생 중 다른 창 Play | 못 깨뜨림(앞 창 Stopped, 프로그램이 첫 노트보다 먼저) | `play_while_other_large_window_plays_takes_over` |
| 12 | 재생 중 가상 → 있는 이름 포트로 편집 | 못 깨뜨림(프로그램이 노트보다 먼저, 메시지 없음) | `live_port_change_to_existing_named_port_is_heard` |
| 13 | 없는 포트로 재생, 편집 5회, 포트 생성 뒤 다시 Play | 못 깨뜨림(문구 한 번, 다음 재생에서 찾음) | `missing_port_is_retried_on_next_play` |

못 깨뜨린 시도 7개는 `crates/keyscore-app/tests/review_s2_t5a_r2.rs`에 남겼다. 기본 병렬 실행으로 3회 모두 통과한다. 리뷰 1 회귀 테스트(`tests/engine.rs` 8개, `tests/review_s2_t5a.rs` 15개)도 이 환경에서 통과한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 창 상태가 스케줄러와 맞는다(Playing ⇔ 소리, Stopped ⇔ 무음) | 깨짐 | 지적 1: 교체 260여 번 뒤 「Playing인데 무음」 또는 「Stopped인데 재생」 |
| 한 번에 한 창만 재생 | 없음 | 시도 11 |
| 앱의 첫 CoreMIDI 호출은 `MidiClient::new` | 없음 | `lib.rs` `run` 변경 없음 |
| 실시간 스레드는 S1 그대로 | 없음 | 앱은 제어 쪽 API만 부른다. 다만 제어 쪽 계약(`collect_garbage`)을 지키지 않는다(지적 1) |
| 일시정지 자리 ≥ 이미 보낸 위치(재개가 들은 것을 되풀이하지 않는다) | 깨짐 | 지적 2 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 재생/정지/일시정지 | 미충족 | 교체가 쌓이면 Stop이 멈추지 않고 Play가 무음(`transport.rs:196`, `transport.rs:246`, `collect_garbage` 호출 없음). 일시정지 자리가 편집마다 뒤로 밀림(`transport.rs:184-204`) |
| TRN-2 메트로놈 토글 | 충족(이 Task 범위) | 1회차와 같음 |
| TRN-3 재생 중 편집 반영 | 미충족 | 편집 수백 번 뒤 반영 중단(지적 1). 뮤트·솔로 해제, 뱅크 편집, 포트 이동의 음색이 반영되지 않음(지적 3–5) |
| OUT-1 가상 포트 | 충족 | `engine.rs:99` |
| OUT-4 라우팅·프로그램·뱅크 송신 | 부분 충족 | 라우팅·재생 시작 chase는 충족. 재생 시작 때 뮤트(또는 솔로에 가려진) 트랙의 프로그램·뱅크를 보내지 않음(`engine.rs:193`이 렌더러 결과만 씀, 렌더러 `render.rs:51-58`). 1회차 코드는 모든 트랙의 `setup`을 보냈다 |
| OUT-6 패닉 | 충족 | `transport.rs:149`. 패닉은 큐와 따로 가서 지적 1 상황에서도 닿는다 |
| MET-1 기본 설정 클릭 | 충족 | `transport.rs:247` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `scheduler.rs`를 바꾸지 않았다.
- 제품 코드의 `unwrap`/`expect`: 없음(`engine.rs`, `transport.rs`).
- 에러가 삼켜지는 곳
  - 교체가 큐 머리에서 막혀도 `swap`·`stop`은 `Ok`다(큐에 들어가기만 하면). 앱은 막힘을 알 길이 없고 메시지도 남기지 않는다(지적 1).
  - `transport.rs:197-199`: `swap`이 들어간 뒤 `send_now`가 실패하면 `Err`로 빠져 기준점을 다시 잡지 않고 `setup`을 옛 것으로 돌리며 「반영하지 못했다」고 알린다. 실제로는 스냅샷이 바뀌었다(제안 1).
  - `transport.rs:233`, `transport.rs:262`, `transport.rs:327`의 `let _`: 1회차와 같은 판단(수용).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 앱이 `Scheduler::collect_garbage`를 한 번도 부르지 않는다(`rg collect_garbage crates/keyscore-app` 결과 없음). 옛 스냅샷은 돌려받기 큐(256)와 대기 자리(4)에 쌓이고, 차면 다음 `Swap` 명령이 큐 머리에서 영영 막혀 그 뒤의 모든 명령(정지·재생·메트로놈)이 처리되지 않는다(`scheduler.rs:682-688`, 계약은 `scheduler.rs:397-399`). Play마다, 재생 중 편집마다 교체가 하나라 일상 사용(키 입력 260여 번)으로 닿는다. 재현: Play 280회 뒤 창은 Playing인데 무음(막힌 Play의 `stop`만 처리됨), 재생 중 편집 300회 뒤 Stop을 눌러도 재생이 계속되고 편집도 들리지 않는다. 둘 다 메시지 없음. 패닉만 닿는다. 리뷰 1이 놓친 것으로, 1회차 코드부터 있었다. TRN-1·TRN-3 | `transport.rs:196`, `transport.rs:246`, `engine.rs:119-122` |
| 2 | 수정 | 기준점 교체가 편집마다 `prepare`·`swap` 시간을 잃는다. `here`는 `prepare` 전에 구하고(`transport.rs:184-188`) 새 기준점의 시각은 그 뒤 `Anchor::new`의 `HostTime::now()`다(`transport.rs:204`, `transport.rs:57`). 큰 프로젝트에서 편집이 이어지면 일시정지 자리가 실제보다 크게 뒤처지고, 재개하면 이미 들은 부분을 되풀이한다. 재현: 16트랙 × 4000노트, 편집 40회(2.1 s) 뒤 `Paused(Tick(155))`, 실제로는 틱 3840 박까지 들음. 리뷰 1 지적 1 수정이 만든 회귀(1회차 코드는 재생 시작 시각 하나로 계산해 템포 편집이 없으면 정확했다). 자리와 시각을 같은 순간에 잡을 것. TRN-1 | `transport.rs:184-204` |
| 3 | 수정 | 재생 시작 때 뮤트(또는 다른 트랙 솔로에 가려진) 트랙의 프로그램·뱅크가 나가지 않고, 재생 중 뮤트·솔로를 풀어도 나가지 않는다. 리뷰 1 지적 3 수정이 시작 송신을 chase에 맡겼는데, chase는 렌더러 결과만 보고 렌더러는 들리지 않는 트랙을 통째로 뺀다(`render.rs:51-58`). 반면 `setup`은 뮤트 트랙도 담아(`engine.rs:183-192`) 뮤트 해제 때 차이가 없다. 재현: 뮤트 트랙(채널 2, 프로그램 9) 재생 중 해제 → `[0xC1, 9]` 없이 Note On. 지적 3 수정의 회귀. OUT-4(「트랙별 프로그램 체인지·뱅크 셀렉트를 재생 시작 시 송신」), TRN-3 | `engine.rs:183-193`, `transport.rs:197` |
| 4 | 수정 | 재생 중 뱅크만 바꾸면 바뀐 CC(예: CC 32)만 보내고 프로그램 체인지를 보내지 않는다. 뱅크 셀렉트는 뒤따르는 프로그램 체인지에서 적용되므로 음색이 바뀌지 않는다. 재현: 뱅크 1 → 2 편집 → 받은 것 `[[176,32,2], ...]`, `[0xC0, 5]` 없음. 리뷰 1 지적 2 수정이 메시지 단위 차이로 거른 탓. 바뀐 트랙의 뱅크 MSB·LSB·프로그램을 한 묶음으로 보낼 것. OUT-4, TRN-3 | `transport.rs:197` |
| 5 | 수정 | `setup` 차이를 (스냅샷 포트 번호, 메시지)로 비교하는데, 포트 번호는 스냅샷마다 트랙 순서대로 다시 매겨진다(`engine.rs:148-171`). 트랙을 다른 이름 포트로 옮기면서 번호가 당겨지면 새 목적지에 처음 가는 프로그램이 옛 목적지의 같은 번호 항목과 같다고 보고 빠진다. 재현: A(X=1, 채널 1, 프로그램 5), B(Y=2, 채널 2) → A를 Y로 → Y=1이 되어 `(1, C0 5)`가 「이미 보냄」으로 걸러짐, Y에 `[0xC0, 5]` 없이 Note On. 비교 키를 포트 이름(또는 목적지)으로 할 것. OUT-4, TRN-3 | `transport.rs:190-205`, `engine.rs:148-171` |

## 7-1. 제안

| # | 내용 | 파일:줄 |
| --- | --- | --- |
| 1 | 교체 경로에서 `swap`이 들어간 뒤 `send_now`가 실패하면 기준점을 다시 잡지 않아(템포 편집이었으면 자리가 틀어짐) `setup`을 옛 것으로 되돌리며 「반영하지 못했다」고 알린다. 스냅샷은 실제로 바뀌었다. `swap` 성공 뒤에는 기준점·`setup`을 새 것으로 두고 송신 실패만 알리는 편이 맞다. 지적 5의 재매김으로 많은 트랙의 `setup`이 한꺼번에 「새것」이 되면 `send_now`가 큐(256)를 넘칠 수 있어(코드 읽기로만 확인) 이 경로가 실제로 열린다 | `transport.rs:191-213` |
| 2 | 같은 포트·채널을 여러 트랙이 공유하면 한 트랙의 프로그램 편집이 그 채널 전체의 음색을 바꾼다(재생 시작 때는 트랙 순서상 마지막 값이 이긴다). REQ에 규정이 없는 사용자 설정 충돌이라 결함은 아니지만, 트랙 목록·인스펙터(S2-T5b·c)에서 경고할지 정할 것 | `engine.rs:183-192` |
| 3 | 1회차 제안 1(창 닫힘을 엔티티 해제에 기댐), 제안 4(곡 끝), 제안 6(연 포트 캐시)의 한계 처리는 수용한다. 단계 파일의 S2-T5b·c에 넘긴 항목으로 적혀 있는지 확인할 것 | `docs/reports/s2-t5a/report.md` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1은 일상 사용으로 닿는 트랜스포트 마비(Stop이 듣지 않음)라 차단이다. 지적 2·3·4·5는 리뷰 1 대응이 근본까지 닿지 않았거나 새로 만든 결함이다. 부록 A의 테스트를 저장소에 넣어 통과시킬 것.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(리뷰 1 뒤 새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 테스트

`crates/keyscore-app/tests/review_s2_t5a_r2.rs`의 도우미(`start`, `open`, `run`, `state`, `edit`, `messages`, `destination`, `unique`, `wait_for`, `drain_for`, `note_on`, `project`)를 그대로 쓴다. 가져오기는 `use keyscore_core::{Bank, Edit, Note, OutputPort, Pitch, Project, Tempo, Tick, TrackProp, Velocity, ksq};`로 넓힌다. 2026-10-17 이 환경에서 6개 모두 실패했다(`cargo test -p keyscore-app --test review_s2_t5a_r2 -- --test-threads=1`).

~~~rust
/// 4분음표마다 같은 음(`pitch`)을 32번(시작은 델타).
fn every_beat(pitch: u8) -> String {
    (0..32)
        .map(|i| format!("[{}, 240, {pitch}, 51492]", if i == 0 { 0 } else { 480 }))
        .collect::<Vec<_>>()
        .join(", ")
}


/// 깨뜨림 1: 앱이 `Scheduler::collect_garbage`를 부르지 않는다. 교체가 260번쯤(돌려받기 큐 256 +
/// 대기 자리 4) 쌓이면 다음 교체가 큐 머리에서 영영 막히고, 그 뒤의 명령은 처리되지 않는다. Play마다
/// 교체가 하나다. 막힌 Play의 `stop`은 이미 처리돼 소리가 멎는데 창은 「재생 중」이고 메시지도 없다.
#[gpui_kit::test]
fn play_still_sounds_after_many_plays(cx: &mut TestAppContext) {
    let lb = start(cx, "garbage-play");
    let (handle, view) = open(cx, project(&every_beat(60)));
    for _ in 0..280 {
        run(cx, handle, transport::Play);
        std::thread::sleep(Duration::from_millis(3));
    }
    assert_eq!(state(cx, &view), TransportState::Playing, "{:?}", messages(cx, &view));
    drain_for(&lb.rx, Duration::from_millis(50));
    let heard = wait_for(&lb.rx, WAIT, note_on(60));
    let msgs = messages(cx, &view);
    run(cx, handle, transport::Stop);
    assert!(heard.is_some(), "창은 재생 중인데 소리가 없다: {msgs:?}");
}

/// 깨뜨림 1의 변형(TRN-3의 주 사용): 재생 중 편집을 300번 하면 그 뒤 교체가 막혀 편집이 들리지 않고,
/// 정지(Stop)도 막힌 교체 뒤에 줄 서서 재생이 멈추지 않는다. 메시지도 없다.
#[gpui_kit::test]
fn stop_works_after_many_live_edits(cx: &mut TestAppContext) {
    let lb = start(cx, "garbage-edit");
    let (handle, view) = open(cx, project(&every_beat(60)));
    run(cx, handle, transport::Play);
    for i in 0..300 {
        edit(cx, &view, |p| {
            vec![Edit::SetTrackProp {
                track: p.tracks()[0].id(),
                prop: TrackProp::Name(format!("n{i}")),
            }]
        });
        std::thread::sleep(Duration::from_millis(2));
    }
    run(cx, handle, transport::Stop);
    assert_eq!(state(cx, &view), TransportState::Stopped);
    drain_for(&lb.rx, Duration::from_millis(150));
    let after = drain_for(&lb.rx, Duration::from_millis(1200));
    let msgs = messages(cx, &view);
    run(cx, handle, transport::Panic);
    assert!(
        !after.iter().any(|m| note_on(60)(m)),
        "정지 뒤에도 재생된다: {after:?}, 메시지 {msgs:?}"
    );
}

/// 깨뜨림 2: 재생 시작 때 뮤트였던 트랙은 렌더러가 빼서 chase가 그 프로그램을 보내지 않는다. 재생 중
/// 뮤트를 풀면 `setup`은 그대로라(뮤트 트랙도 들어 있다) 차이가 없어 프로그램이 나가지 않는다(OUT-4, TRN-3).
#[gpui_kit::test]
fn unmuting_track_while_playing_sends_its_program(cx: &mut TestAppContext) {
    let lb = start(cx, "unmute-program");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"port": "virtual", "channel": 1, "program": 5,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492]]}]},
        {"port": "virtual", "channel": 2, "program": 9, "mute": true,
         "clips": [{"start": 0, "length": 7680, "notes": [[1920, 240, 64, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[1].id(),
            prop: TrackProp::Mute(false),
        }]
    });
    let got = wait_for(&lb.rx, WAIT, |m| note_on(64)(m) && m[0] == 0x91).expect("Note On 64");
    run(cx, handle, transport::Stop);
    assert!(
        got.iter().any(|m| m == &[0xC1, 9]),
        "뮤트를 푼 트랙의 프로그램이 나가지 않았다: {got:?}"
    );
}

/// 깨뜨림 3: 재생 중 뱅크만 바꾸면 바뀐 뱅크 셀렉트(CC 32)만 나가고 프로그램 체인지가 따라가지 않는다.
/// 뱅크 셀렉트는 다음 프로그램 체인지에서야 적용되므로 소리가 바뀌지 않는다(OUT-4, TRN-3).
#[gpui_kit::test]
fn bank_edit_while_playing_is_followed_by_program(cx: &mut TestAppContext) {
    let lb = start(cx, "bank-edit");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [{"port": "virtual", "channel": 1, "bank": 1, "program": 5,
        "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}]}]}"#;
    let (handle, view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Bank(Some(Bank::new(2).unwrap())),
        }]
    });
    let got = wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    run(cx, handle, transport::Stop);
    let bank = got.iter().position(|m| m == &[0xB0, 32, 2]);
    let program = got.iter().rposition(|m| m == &[0xC0, 5]);
    assert!(
        matches!((bank, program), (Some(b), Some(p)) if b < p),
        "바뀐 뱅크 뒤에 프로그램 체인지가 없다: {got:?}"
    );
}

/// 깨뜨림 4: `setup` 차이는 (스냅샷 포트 번호, 메시지)로 비교한다. 포트 번호는 스냅샷마다 트랙 순서로
/// 다시 매겨지므로, 트랙을 다른 이름 포트로 옮겨 번호가 당겨지면 새 목적지에 처음 가는 프로그램이 옛
/// 목적지의 같은 번호 항목과 같다고 보고 빠진다.
#[gpui_kit::test]
fn moving_track_to_other_named_port_sends_program_there(cx: &mut TestAppContext) {
    let _lb = start(cx, "move-port");
    let x = unique("move x");
    let y = unique("move y");
    let dx = destination(&x);
    let dy = destination(&y);
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
            {{"port": {{"named": "{x}"}}, "channel": 1, "program": 5,
              "clips": [{{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}}]}},
            {{"port": {{"named": "{y}"}}, "channel": 2, "program": 9,
              "clips": [{{"start": 0, "length": 7680, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&dx.rx, WAIT, note_on(60)).expect("X Note On 60");
    wait_for(&dy.rx, WAIT, note_on(64)).expect("Y Note On 64");
    let target = y.clone();
    edit(cx, &view, move |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[0].id(),
            prop: TrackProp::Port(OutputPort::Named(target)),
        }]
    });
    let got = wait_for(&dy.rx, WAIT, |m| note_on(62)(m) && m[0] == 0x90).expect("Y Note On 62");
    run(cx, handle, transport::Stop);
    assert!(
        got.iter().any(|m| m == &[0xC0, 5]),
        "옮긴 트랙의 프로그램이 새 포트에 가지 않았다: {got:?}"
    );
}

/// 깨뜨림 5: 기준점을 다시 잡을 때 자리는 교체 전에 구하고 시각은 `prepare`·`swap` 뒤에 찍는다. 그 사이
/// 시간(렌더링)이 편집마다 빠져 일시정지 자리가 실제보다 뒤로 밀린다. 큰 프로젝트에서 편집을 거듭하면
/// 재개 때 이미 들은 부분을 다시 연주한다.
#[gpui_kit::test]
fn pause_position_does_not_drift_with_many_live_edits(cx: &mut TestAppContext) {
    let lb = start(cx, "anchor-drift");
    // 노트는 아주 뒤에 두어 소리 없이 렌더링만 무겁게 한다
    let notes: Vec<String> = (0..4000)
        .map(|i| format!("[{}, 48, 60, 51492]", if i == 0 { 2_000_000 } else { 96 }))
        .collect();
    let notes = notes.join(", ");
    let tracks: Vec<String> = (0..16)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "clips": [{{"start": 0, "length": 3000000, "notes": [{notes}]}}]}}"#,
                i + 1
            )
        })
        .collect();
    // 실제 재생 위치를 재는 박 트랙(채널 16, 480틱마다 노트 70)
    let beats: Vec<String> = (0..32)
        .map(|i| format!("[{}, 120, 70, 51492]", if i == 0 { 0 } else { 480 }))
        .collect();
    let beat = format!(
        r#"{{"port": "virtual", "channel": 16, "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}}"#,
        beats.join(", ")
    );
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}, {beat}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let begin = Instant::now();
    for i in 0..40 {
        edit(cx, &view, |p| {
            vec![Edit::SetTrackProp {
                track: p.tracks()[0].id(),
                prop: TrackProp::Name(format!("n{i}")),
            }]
        });
    }
    let secs = begin.elapsed().as_secs_f64();
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    // 일시정지 전에 들은 박 수: 마지막 박의 틱이 실제 위치의 하한이다
    let heard = drain_for(&lb.rx, Duration::from_millis(100))
        .iter()
        .filter(|m| note_on(70)(m))
        .count();
    let floor = u64::try_from(heard.saturating_sub(1) * 480).unwrap();
    assert!(heard >= 4, "박을 {heard}개만 들었다");
    assert!(
        at.get() >= floor,
        "일시정지 자리 {at:?}가 이미 들은 박(틱 {floor}, {heard}개, 경과 {secs:.2} s)보다 앞이다"
    );
}
~~~
