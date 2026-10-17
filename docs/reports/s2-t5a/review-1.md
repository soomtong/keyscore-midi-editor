# 적대적 리뷰: S2-T5a MIDI 엔진 연결과 트랜스포트 Action (리뷰 1회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 에이전트(구현자 세션과 다름)
- 대상 커밋: `b878817`(브랜치 `s2/t5-transport-panels`, 범위 `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트를 열기 전에 `engine.rs`, `transport.rs`, `window.rs`, `edit.rs`와 S1 `scheduler.rs`의 `handle`·`swap`·`chase`를 읽고 적었다.

1. 재생 중 템포가 바뀐 뒤(TRN-3) 일시정지 자리. `position`은 재생 시작 시각과 「지금의」 템포 맵으로 계산하는데, 스케줄러는 교체 때 보낸 구간을 옛 템포로 두고 이어 간다.
2. 재생 중 트랙 프로그램·뱅크를 바꾸면 나가는가. 렌더러는 프로그램을 틱 0에만 두고, 교체(`swap`)는 chase도 준비 송신도 하지 않는다.
3. `play_from`이 한 번에 넣는 명령 수(정지·교체·트랙마다 최대 3개 `send_now`·메트로놈 2개·재생)와 큐 크기 256. 큐가 넘치면 앞서 넣은 `stop`·`swap`은 이미 들어가 있다.
4. 실패 경로의 상태 일관성: 재생 실패 시 이 창과 앞 주인 창의 상태가 스케줄러와 맞는가.
5. 재생 중 창 닫기(`release`)가 노트를 끄는가, 다른 창을 닫으면 주인이 계속 재생하는가.
6. Play 연타에서 걸린 노트, 일시정지에서 울리는 노트를 끄는가, 재개 때 프로그램이 첫 노트보다 먼저인가.
7. 템포 변화가 있는 곡의 일시정지 자리, 곡 끝 뒤의 상태.
8. 메트로놈 토글: 재생 중 켜고 끄기, 재생하지 않는 창의 토글이 새지 않는가.
9. 포트 라우팅: 같은 이름 포트를 여러 트랙이 공유, 포트 상한(`MAX_SCHEDULER_PORTS` 32, 0번은 기본 가상 포트), 없는 포트.
10. 엔진 없음: 모든 트랜스포트 Action이 안전한가, 메시지 상한 32.
11. 결정 B: 한 undo 단위 안의 왕복, coalesce로 묶인 편집, 실패한 묶음의 원상 복구, 조성 없음 왕복, 같은 값 편집의 undo/redo, 히스토리 바이트 추정과 상한.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 템포 편집 뒤 일시정지 자리 | 없음 | 깨짐(지적 1) |
| 2 재생 중 프로그램 편집 | 없음 | 깨짐(지적 2). `edit_while_playing_is_heard`는 노트 추가만 본다 |
| 3 큰 프로젝트·연타의 큐 넘침 | 없음 | 깨짐(지적 3) |
| 4 실패 경로 상태 | 없음 | 지적 3에서 드러남 |
| 5 창 닫기 | 없음 | 못 깨뜨림 |
| 6 연타·일시정지 노트·재개 프로그램 | 일부(`pause_resumes_from_paused_position`은 자리만) | 못 깨뜨림. `play_action_sends_notes_to_virtual_port`의 「프로그램이 먼저」는 렌더러의 틱 0 이벤트만으로도 통과한다(제안 5) |
| 7 템포 맵·곡 끝 | 없음 | 템포 맵은 못 깨뜨림. 곡 끝은 제안 4 |
| 8 메트로놈 재생 중 토글·다른 창 | 일부(정지 상태 토글만) | 못 깨뜨림 |
| 9 포트 공유·상한 | 없는 포트만 | 못 깨뜨림. 상한 메시지 문구는 제안 2 |
| 10 엔진 없음 전 Action | Play·Panic만 | 못 깨뜨림 |
| 11 결정 B 경계 | 단일 편집·proptest | 못 깨뜨림 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 120 BPM 재생 약 1 s 뒤 템포를 60 BPM으로 편집하고 바로 일시정지 | 깨짐: `Paused(Tick(943))`(실제 위치 약 1920) | 부록 A `pause_after_tempo_edit_while_playing_keeps_position` |
| 2 | 재생 중 트랙 프로그램 5 → 9 편집 | 깨짐: 다음 노트까지 `[0xC0, 9]`가 오지 않음(받은 것 `[[128,60,0],[144,62,100]]`) | 부록 A `program_edit_while_playing_is_sent` |
| 3 | 뱅크·프로그램 있는 트랙 100개 Play | 깨짐: 「재생하지 못했다: 스케줄러 명령 큐가 가득 찼다」, 상태 Stopped | 부록 A `many_tracks_with_programs_can_play` |
| 4 | 64트랙(SEQ-1 실용 상한)에서 Play 두 번 연달아 | 깨짐(4회 중 3회): 창은 Playing인데 스케줄러는 멈춰 틱 1920 노트가 오지 않음 | 부록 A `sixty_four_tracks_survive_repeated_play` |
| 5 | 재생 중 창 닫기 | 못 깨뜨림(Note Off 수신) | `closing_playing_window_stops_playback` |
| 6 | 다른 창 닫기 | 못 깨뜨림(주인 계속 재생) | `closing_other_window_keeps_owner_playing` |
| 7 | Play 8회 연타 뒤 정지, Note On/Off 짝 | 못 깨뜨림 | `repeated_play_leaves_no_hanging_notes` |
| 8 | 긴 노트 중 일시정지 → 재개, 프로그램이 첫 노트보다 먼저 | 못 깨뜨림 | `pause_releases_sounding_notes_and_resume_sends_program` |
| 9 | 곡 안 템포 변화(틱 960에서 60 BPM) 뒤 일시정지 자리 | 못 깨뜨림 | `pause_position_follows_tempo_change_in_song` |
| 10 | 곡 끝 뒤 상태와 일시정지 자리 | 못 깨뜨림(동작 기록: 계속 Playing, 자리는 곡 끝 뒤) | `playback_past_song_end_stays_playing` |
| 11 | 재생 중 메트로놈 켜기·끄기 | 못 깨뜨림 | `metronome_toggle_while_playing_takes_effect` |
| 12 | 재생하지 않는 창의 메트로놈 토글 | 못 깨뜨림 | `metronome_flag_of_other_window_does_not_leak` |
| 13 | 같은 이름 포트 두 트랙(채널 1·2), 프로그램이 그 목적지로 | 못 깨뜨림 | `tracks_sharing_named_port_reach_that_destination` |
| 14 | 이름 포트 32개(상한 초과) | 못 깨뜨림(32번째 무음·알림) | `ports_over_scheduler_limit_are_silent_and_reported` |
| 15 | 엔진 없이 모든 Action, Play 40회, 창 닫기 | 못 깨뜨림 | `every_transport_action_without_engine_is_harmless` |
| 16 | 결정 B: 한 단위 안 A→B→A, coalesce, 실패 묶음, 조성 없음 왕복, 같은 값 undo/redo, 히스토리 바이트·상한 | 못 깨뜨림 | `crates/keyscore-core/tests/review_s2_t5a.rs` 7개 |

못 깨뜨린 시도는 `crates/keyscore-app/tests/review_s2_t5a.rs`(11개)와 `crates/keyscore-core/tests/review_s2_t5a.rs`(7개)에 남겼다. 모두 통과한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 포트·조성 값이 바뀌면 모르는 키를 지우고 undo가 되살린다(결정 B) | 없음(히스토리 경로) | 한 단위 왕복·coalesce·실패 묶음·조성 없음 모두 통과. 다만 `Project::set_key`, `Track::set_port`는 `pub`이라 히스토리 밖에서 값만 바꿀 수 있다(제안 7, 이 Task 이전부터) |
| 한 번에 한 창만 재생 | 상태 표시로는 깨짐 | 지적 3: 재생 실패 시 창 상태가 Playing으로 남고 스케줄러는 멈춤 |
| 앱의 첫 CoreMIDI 호출은 `MidiClient::new` | 없음 | `lib.rs` `run`이 창보다 먼저 `install`, `init`은 CoreMIDI 무관 |
| 실시간 스레드는 S1 그대로 | 없음 | 앱은 제어 쪽 API만 부름 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 재생/정지/일시정지 | 부분 충족 | 일시정지 자리가 재생 중 템포 편집 뒤 틀림(`transport.rs:42-51`), 큰 프로젝트에서 재생 실패(`transport.rs:186-197`). 루프·위치 이동은 S3·S4(범위 밖) |
| TRN-2 메트로놈 토글 | 충족(이 Task 범위) | `transport.rs:133-150`, `transport.rs:194-195`. 카운트인·시각 메트로놈은 뒤 Task |
| TRN-3 재생 중 편집 반영 | 부분 충족 | 노트는 반영, 트랙 프로그램·뱅크는 반영 안 됨(`transport.rs:153-174`가 `swap`만 부름, 렌더러는 틱 0에만 프로그램) |
| OUT-1 가상 포트 | 충족 | `engine.rs:83-84` |
| OUT-4 라우팅·시작 시 프로그램 송신 | 부분 충족 | 라우팅 `engine.rs:132-159` 충족. 시작 송신이 큐를 넘쳐 재생 자체가 실패(`engine.rs:160-170`, `transport.rs:191-193`) |
| OUT-6 패닉 | 충족 | `transport.rs:116-131` |
| MET-1 기본 설정 클릭 | 충족 | `transport.rs:194`(기본 `MetronomeConfig`) |
| SEQ-7 트랙 → 포트 | 충족 | `engine.rs:132-159` |
| IO-1 모르는 키(결정 B) | 충족 | `edit.rs:579-609` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `scheduler.rs`를 바꾸지 않았고, 앱은 `play`·`stop`·`swap`·`send_now`·`set_metronome*`·`panic`만 부른다.
- 제품 코드의 `unwrap`/`expect`: 없음(`engine.rs`, `transport.rs`).
- 에러가 삼켜지는 곳
  - `engine.rs:192` `OsOutput::open(..).ok()?`: 포트를 여는 오류가 「없는 포트」로 보고된다(제안 2).
  - `transport.rs:205` 앞 주인 창 갱신 실패 `let _`: 창이 이미 해제된 경우라 수용 가능.
  - `transport.rs:262` 창 닫힐 때 `stop` 실패 `let _`: 알릴 창이 없어 수용 가능.
  - `transport.rs:186-211` 재생 실패: 메시지는 남지만 이미 보낸 `stop`·`swap`의 결과(앞 재생 정지)를 상태에 반영하지 않는다(지적 3).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 재생 중 템포를 편집하면 일시정지 자리가 뒤로 튄다. `position`이 재생 시작 시각부터의 경과를 「편집 뒤」 템포 맵으로 다시 환산하는데, 스케줄러는 교체 때 보낸 구간을 옛 템포로 두고 이어 간다(`scheduler.rs` `swap`의 `move_base`). 재현: 120 BPM 약 1 s 재생 → 60 BPM 편집 → 일시정지 = `Tick(943)`(기대 약 1920), 재개하면 이미 들은 노트가 다시 울린다. 교체 때 (호스트 시각, 틱) 기준점을 옛 맵으로 옮겨 두는 등으로 고칠 것. TRN-1·TRN-3 | `transport.rs:42-51`, `transport.rs:153-174`, `transport.rs:202` |
| 2 | 수정 | 재생 중 트랙 프로그램·뱅크 편집이 나가지 않는다. 문서 변경은 `swap`만 하고, 새 스냅샷의 프로그램은 틱 0에만 있어 이미 지난 위치에서는 보내지 않는다. 재현: 프로그램 5 → 9 편집 뒤 다음 노트까지 `0xC0 9` 없음. 교체 때 바뀐 트랙의 뱅크·프로그램을 `send_now`로 보내거나, 인스펙터 Task(S2-T5c)로 넘긴다면 단계 파일·계획에 명시하고 그 Task의 테스트로 둘 것. TRN-3·OUT-4 | `transport.rs:153-174`, `engine.rs:160-170` |
| 3 | 차단 | 재생 시작 송신이 명령 큐(256)를 넘쳐 재생이 실패한다. 트랙마다 최대 3개 `send_now`를 한꺼번에 넣으므로 뱅크·프로그램 있는 트랙이 대략 80개를 넘으면 실패하고(100트랙에서 매번 재현), 64트랙(SEQ-1 실용 상한)에서도 Play 두 번 연달아면 실패한다(4회 중 3회). 실패 전에 `stop`·`swap`이 이미 큐에 들어가 스케줄러는 멈추는데, 창은 Playing으로 남고(연타의 경우) 주인도 그대로라 「재생 중인데 무음」이 된다. 다른 창이 재생 중일 때도 그 창을 소리 없이 멈추고 상태는 Playing으로 둔다. 시작 송신이 렌더러 틱 0 이벤트와 스케줄러 chase(`scheduler.rs:1277-1293`)와 겹친다는 점도 함께 볼 것. 실패 시 상태 일관성(이 창·앞 주인 모두)을 함께 고칠 것. TRN-1·OUT-4·SEQ-1 | `engine.rs:160-170`, `transport.rs:186-211` |

## 7-1. 제안

| # | 내용 | 파일:줄 |
| --- | --- | --- |
| 1 | 창 닫힘 처리가 `ProjectWindow` 엔티티 해제(`on_release`)에 기댄다. 창이 닫혀도 누가 강한 참조를 들고 있으면(테스트에서 뷰를 들고 있을 때 실제로 재현) 재생이 계속된다. 창 닫힘 자체(`on_window_closed` 류)에 묶는 편이 단단하다 | `window.rs:82`, `transport.rs:252-265` |
| 2 | 포트 상한 초과와 `OsOutput::open` 실패도 「출력 포트 X이 없어」로 보고된다. 원인별 문구를 나눌 것 | `engine.rs:139-153`, `engine.rs:192`, `transport.rs:237` |
| 3 | 없는 이름 포트가 있으면 재생 중 편집마다(TRN-3) 메인 스레드에서 `destinations()` 열거를 다시 한다. 키 입력마다 CoreMIDI 열거가 된다. `PortWatcher`(S2-T5c) 전까지 음성 결과를 캐시할 것 | `engine.rs:190` |
| 4 | 곡 끝에서 스스로 멈추지 않아 상태가 계속 Playing이고, 그 뒤 일시정지 자리는 곡 끝 뒤가 되어 재개하면 무음이다. REQ에 규정이 없으니 정할 것이면 결정 요청으로 | `transport.rs:42-51` |
| 5 | `play_action_sends_notes_to_virtual_port`의 「프로그램이 먼저」는 렌더러의 틱 0 이벤트만으로 통과해 `setup` 송신을 검증하지 못한다. 틱 0이 아닌 곳에서 시작하는 경우는 chase가 이미 프로그램을 보낸다. `setup`이 꼭 필요한 경우(뮤트 트랙 등)를 정하고 그 경우를 테스트할 것 | `engine.rs:160-170`, `tests/engine.rs:172-175` |
| 6 | 한 번 연 OS 포트(`opened`)는 쓰는 트랙이 없어도 닫히지 않는다. S2-T5c 포트 감시와 함께 정리 | `engine.rs:56` |
| 7 | `Project::set_key`, `Track::set_port`가 `pub`이라 히스토리 밖에서 값만 바뀌고 모르는 키가 남는다(결정 B 우회). 이 Task 이전부터의 API이니 별도로 판단 | `project.rs:67`, `track.rs:206` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·3은 이 Task가 들인 트랜스포트 경로의 버그이고, 지적 2는 이 Task가 맡은 TRN-3 교체 경로의 누락이다(넘기려면 계획에 명시).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(dev-dependency `coremidi` MIT, 워크스페이스 기존 버전)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 테스트

`crates/keyscore-app/tests/review_s2_t5a.rs`의 도우미(`start`, `open`, `run`, `state`, `wait_for`, `drain_for`, `note_on`, `project`)를 그대로 쓰고, `use keyscore_core::{Edit, Program, Project, Tempo, Tick, TrackProp, ksq};`로 가져온다. 수정 커밋에서 이 테스트를 저장소에 넣어 통과시킬 것.

~~~rust
fn edit(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    make: impl FnOnce(&Project) -> Edit,
) {
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            let e = make(doc.project());
            doc.apply(vec![e]).unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
}

/// 깨뜨림 1: 재생 중 템포를 바꾸면(TRN-3) 일시정지 자리가 새 템포로 처음부터 다시 계산되어 뒤로 튄다.
/// 스케줄러는 교체 때 지난 구간을 옛 템포로 두고 이어 가므로, 재개하면 이미 들은 노트가 다시 울린다.
#[gpui_kit::test]
fn pause_after_tempo_edit_while_playing_keeps_position(cx: &mut TestAppContext) {
    let lb = start(cx, "tempo-edit-pause");
    // 120 BPM: 62는 틱 1440(0.75 s)
    let (handle, view) = open(
        cx,
        project("[0, 240, 60, 51492], [1440, 240, 62, 51492], [2400, 240, 64, 51492]"),
    );
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    // 약 1.0 s(틱 1920 근처)에서 60 BPM으로
    std::thread::sleep(Duration::from_millis(220));
    edit(cx, &view, |_| Edit::SetTempo {
        at: Tick::ZERO,
        tempo: Some(Tempo::from_bpm(60).unwrap()),
    });
    run(cx, handle, transport::Pause);
    let TransportState::Paused(at) = state(cx, &view) else {
        panic!("{:?}", state(cx, &view));
    };
    assert!(at >= Tick::new(1700), "일시정지 자리가 뒤로 튀었다: {at:?}");
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    let got = drain_for(&lb.rx, Duration::from_millis(1200));
    assert!(
        !got.iter().any(|m| note_on(62)(m)),
        "이미 들은 62가 다시 울렸다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 깨뜨림 2: 재생 중 트랙 프로그램을 바꿔도(TRN-3, OUT-4) 프로그램 체인지가 나가지 않는다.
/// 스냅샷의 프로그램은 틱 0에만 있고 교체는 chase·준비 송신을 하지 않는다.
#[gpui_kit::test]
fn program_edit_while_playing_is_sent(cx: &mut TestAppContext) {
    let lb = start(cx, "program-edit");
    let (handle, view) = open(cx, project("[0, 240, 60, 51492], [1920, 240, 62, 51492]"));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| Edit::SetTrackProp {
        track: p.tracks()[0].id(),
        prop: TrackProp::Program(Some(Program::new(9).unwrap())),
    });
    let got = wait_for(&lb.rx, WAIT, note_on(62)).expect("Note On 62");
    assert!(
        got.iter().any(|m| m == &[0xC0, 9]),
        "재생 중 바꾼 프로그램이 나가지 않았다: {got:?}"
    );
    run(cx, handle, transport::Stop);
}

/// 깨뜨림 3: 뱅크·프로그램이 있는 트랙이 많으면 재생 시작의 `send_now`가 명령 큐(256)를 넘쳐 재생이
/// 실패한다. 실패 전에 이미 `stop`·`swap`을 보내 앞 재생도 멈춘다. SEQ-1은 트랙 수 제한이 없다.
#[gpui_kit::test]
fn many_tracks_with_programs_can_play(cx: &mut TestAppContext) {
    let lb = start(cx, "many-tracks");
    let tracks: Vec<String> = (0..100)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "bank": 1, "program": 2,
                  "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492]]}}]}}"#,
                i % 16 + 1
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert_eq!(state(cx, &view), TransportState::Playing, "{messages:?}");
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
}

/// 깨뜨림 3의 변형: 실용 상한(SEQ-1, 64트랙)에서 Play를 연달아 누르면 큐가 넘친다. 둘째 Play가
/// 실패해도 창은 「재생 중」으로 남지만, 실패 전에 보낸 `stop`으로 스케줄러는 멈춰 소리가 없다.
#[gpui_kit::test]
fn sixty_four_tracks_survive_repeated_play(cx: &mut TestAppContext) {
    let lb = start(cx, "sixty-four");
    let tracks: Vec<String> = (0..64)
        .map(|i| {
            format!(
                r#"{{"port": "virtual", "channel": {}, "bank": 1, "program": 2,
                  "clips": [{{"start": 0, "length": 3840, "notes": [[0, 240, 60, 51492], [1920, 240, 62, 51492]]}}]}}"#,
                i % 16 + 1
            )
        })
        .collect();
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    run(cx, handle, transport::Play);
    let messages = cx.read(|cx| view.read(cx).messages().to_vec());
    assert_eq!(state(cx, &view), TransportState::Playing, "{messages:?}");
    assert!(
        wait_for(&lb.rx, WAIT, note_on(62)).is_some(),
        "창은 재생 중인데 소리가 없다: {messages:?}"
    );
    run(cx, handle, transport::Stop);
}
~~~
