# 적대적 리뷰: S2-T5a MIDI 엔진 연결과 트랜스포트 Action (리뷰 3회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 리뷰 에이전트(구현자 세션과 다름, 1·2회차 리뷰어와도 다른 세션)
- 대상 커밋: `3abedba`(브랜치 `s2/t5-transport-panels`, 범위 `git diff main...HEAD`, 리뷰 2 대응은 `git show 3abedba`)
- 프로토콜상 마지막 재리뷰다(`plan/01-protocol.md` [8]: 3회 재리뷰에도 판정이 안 나면 결정 요청으로 전환).

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트를 열기 전에 `engine.rs`, `transport.rs`와 S1 `scheduler.rs`의 `swap`·`step_inner`·`flush_garbage`·`collect_garbage`·`chase`, `Snapshot::new` 정렬, `render.rs`를 읽고 적었다.

1. `collect_garbage`를 교체 직전에만 부른다. 돌려받기 큐는 교체마다 하나씩 차고, 앱은 다음 교체 때 비운다. Play·편집·메트로놈 토글·패닉·일시정지를 섞어 교체가 수백 번 일어나도 명령이 막히지 않는가. 교체 없는 명령(메트로놈 토글)만 아주 많이 보내면 명령 큐(256)가 넘치는가.
2. 들리지 않는 트랙의 뱅크·프로그램을 렌더러 결과 뒤에 틱 0 이벤트로 붙인다(`engine.rs:235-260`). `Snapshot::new`는 (틱, 무리)로 안정 정렬하므로(`scheduler.rs:152`) 같은 틱의 프로그램끼리는 넣은 순서다. 같은 포트·채널을 들리는 트랙과 공유하면 들리지 않는 트랙의 프로그램이 항상 마지막이 되어 chase·재생 시작에서 이기는가. 단계 파일 이월은 「트랙 순서대로」라고 적었다.
3. 중간 위치에서 재개할 때 들리지 않는 트랙의 뱅크·프로그램이 chase로 첫 노트보다 먼저 나가는가. 재생 중 솔로를 옮기면.
4. `Setup` 비교는 「새 항목과 같은 옛 항목이 하나라도 있으면 보내지 않음」이다(`transport.rs:211`). 같은 목적지·채널에 같은 묶음을 가진 다른 트랙이 있으면, 한 트랙을 바꿨다가 되돌릴 때 되돌린 값이 다른 트랙의 옛 항목에 걸러지는가.
5. 채널만 바꾸기, 가상 → 이름 → 가상 왕복에서 묶음이 다시 나가는가.
6. 기준점: `now`를 `prepare` 전에 찍고 그 순간부터 새 템포를 쓴다. 스케줄러는 교체를 처리한 뒤(보낸 구간 뒤)에야 새 템포다. 큰 프로젝트에서 템포 편집이면 그 차이만큼 틀어지는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 섞인 긴 시퀀스, 토글 다수 | Play 280회, 이름 편집 300회만(`review_s2_t5a_r2.rs`) | 못 깨뜨림 |
| 2 공유 채널에서 들리지 않는 트랙의 프로그램 | 없음 | 깨짐(지적 1) |
| 3 중간 재개 chase, 솔로 이동 | 뮤트 해제 1건(틱 0 시작)만 | 못 깨뜨림 |
| 4 같은 묶음을 가진 트랙이 있을 때 되돌리기 | 없음 | 깨짐(지적 2) |
| 5 채널 편집, 포트 왕복 | 다른 이름 포트로 옮기기 1건만 | 못 깨뜨림 |
| 6 큰 프로젝트 템포 편집 기준점 | 작은 프로젝트 템포 편집만 | 실행하지 않음. 근삿값 한계로 문서화돼 있어 제안 1로만 둔다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | Play·일시정지·Toggle 320회, 그때마다 재생 중이면 이름 편집, 메트로놈 토글, 50회마다 패닉(교체 600회 넘음) 뒤 Play·Stop | 못 깨뜨림(Play 들림, Stop 뒤 무음, 메시지 없음) | `mixed_long_sequence_keeps_transport_working` |
| 2 | 재생 중 메트로놈 토글 401회(1 ms 간격) | 못 깨뜨림(클릭 켜짐, 메시지 없음) | `many_metronome_toggles_while_playing_stay_in_sync` |
| 3 | 뮤트 트랙(채널 2, 뱅크 3, 프로그램 9)이 있는 곡을 일시정지 뒤 중간 위치에서 재개 | 못 깨뜨림(뱅크 → 프로그램 → 첫 노트 순) | `resume_mid_song_chases_muted_track_program` |
| 4 | 다른 채널 두 트랙에서 재생 중 솔로를 A → B로 이동 | 못 깨뜨림 | `solo_switch_while_playing_keeps_program` |
| 5 | 둘 다 들리는 두 트랙이 채널 1을 공유(9, 5 순서) | 못 깨뜨림(트랙 순서상 마지막 5가 이김, 이월 문구와 일치) | `shared_channel_audible_tracks_follow_track_order` |
| 6 | 재생 중 채널 1 → 3 | 못 깨뜨림(`[0xB2,32,2]` 뒤 `[0xC2,5]`) | `channel_edit_while_playing_sends_setup_on_new_channel` |
| 7 | 재생 중 가상 → 이름 포트 Y → 가상 | 못 깨뜨림(되돌아온 가상 포트에 `[0xC0,5]`) | `port_round_trip_while_playing_resends_program` |
| 8 | 채널 1 공유: B(프로그램 9, 트랙 순서 앞), A(프로그램 5, 솔로). 재생 시작 | 깨짐: 받은 것 `[[192,5],[192,9],[144,60,100]]`. 솔로한 A가 B의 음색 9로 연주 | 부록 A `soloed_track_on_shared_channel_plays_its_own_program` |
| 9 | 채널 1 공유: B(프로그램 9, 뮤트, 앞), A(프로그램 5). 재생 시작, 일시정지 뒤 재개 | 깨짐: 시작 `[[192,5],[192,9],[144,60,100]]`, 재개 `[[192,9],[128,60,0],[144,60,100]]`. 둘 다 9 | 부록 A `muted_track_on_shared_channel_does_not_override_audible_program` |
| 10 | 채널 1 공유, 두 트랙 모두 프로그램 5. 재생 중 B를 9로 바꾼 뒤 되돌리기(undo) | 깨짐: 9는 나가지만 되돌린 뒤 받은 것 `[[128,60,0],[144,60,100]]`, `[0xC0,5]` 없음. 문서는 두 트랙 모두 5인데 신디는 9에 남는다 | 부록 A `undoing_program_edit_on_shared_channel_restores_program` |

못 깨뜨린 시도 7개는 `crates/keyscore-app/tests/review_s2_t5a_r3.rs`에 남겼다. 기본 병렬 실행으로 3회 모두 통과한다. 1·2회차 회귀 테스트(`tests/engine.rs` 8개, `tests/review_s2_t5a.rs` 15개, `tests/review_s2_t5a_r2.rs` 13개)도 이 환경에서 통과한다. 리뷰 2 지적 1의 막힘은 교체 직전 `collect_garbage`로 해소됐다: 돌려받기 큐에 쌓이는 것은 「지난 수거 뒤의 교체 수」라 앱 교체마다 비워지고, 대기 자리(4)가 차려면 큐(256)가 먼저 차야 한다(`scheduler.rs:682-688`, `scheduler.rs:923-932`).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 창 상태가 스케줄러와 맞는다 | 없음 | 시도 1·2 |
| 한 번에 한 창만 재생 | 없음 | 2회차 시도 11 그대로 |
| 일시정지 자리 ≥ 이미 보낸 위치 | 없음(비템포 편집) | 2회차 `pause_position_does_not_drift_with_many_live_edits` 통과. 템포 편집은 제안 1 |
| 들리는 트랙은 자기 뱅크·프로그램으로 연주된다(OUT-4) | 깨짐 | 지적 1(들리지 않는 트랙이 공유 채널을 덮음), 지적 2(되돌린 프로그램이 나가지 않음) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1 재생/정지/일시정지 | 충족(이 Task 범위) | `transport.rs:124-139`, `transport.rs:244-296`. 시도 1에서 교체 600회 뒤에도 Play·Stop 동작 |
| TRN-2 메트로놈 토글 | 충족(이 Task 범위) | `transport.rs:160-178`, 시도 2 |
| TRN-3 재생 중 편집 반영 | 부분 충족 | 이름·채널·포트·뱅크·뮤트·솔로 편집 반영. 공유 채널에서 되돌린 프로그램이 반영되지 않음(지적 2, `transport.rs:211`) |
| OUT-4 라우팅·트랙별 프로그램·뱅크를 재생 시작 시 송신 | 부분 충족 | 공유 채널에서 솔로·들리는 트랙이 뮤트(또는 솔로에 가려진) 트랙의 프로그램으로 시작한다(지적 1, `engine.rs:235-260`, 정렬 `scheduler.rs:152`) |
| OUT-6 패닉 | 충족 | `transport.rs:143-158` |
| MET-1 기본 설정 클릭 | 충족 | `transport.rs:275-276` |
| SEQ-8 같은 포트·채널 공유 허용(한 신디에 두 성부) | 이 Task 범위 밖(경고는 S2-T5b). 다만 지적 2는 프로그램이 같아 경고 대상도 아닌 정상 설정에서 생긴다 | `docs/REQ.md` SEQ-8 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `scheduler.rs`를 바꾸지 않았다.
- 제품 코드의 `unwrap`/`expect`: 없음(`engine.rs`, `transport.rs`, `unwrap_or`만).
- 에러가 삼켜지는 곳: 2회차와 같다(`transport.rs`의 `let _` 3곳, 수용). 리뷰 2 제안 1(교체 뒤 송신 실패)은 `transport.rs:205-232`에서 반영됐다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 2 지적 3 수정의 회귀. 들리지 않는 트랙의 뱅크·프로그램을 렌더러 결과 뒤에 틱 0으로 붙이는데, `Snapshot::new`는 같은 틱·같은 무리를 넣은 순서대로 두어(`scheduler.rs:152`) 같은 포트·채널에서는 들리지 않는 트랙의 프로그램이 트랙 순서와 무관하게 항상 마지막이 된다. 재생 시작(틱 0 이벤트)과 중간 재개(chase 마지막 값) 모두 들리는 트랙이 뮤트·솔로에 가려진 트랙의 음색으로 연주된다. 솔로의 뜻(그 트랙만 듣기)과 정반대이고, 단계 파일 이월 문구(「트랙 순서대로」, `12-S2-app-shell.md:42`)와도 다르다. 재현: 시도 8·9, 받은 것 `[[192,5],[192,9],[144,60,100]]`. 들리지 않는 트랙의 이벤트를 들리는 트랙 것보다 앞에 두거나(안정 정렬이라 들리는 쪽이 이긴다), 들리는 트랙이 쓰는 (포트, 채널)에는 넣지 말 것. OUT-4 | `crates/keyscore-app/src/engine.rs:230-260` |
| 2 | 수정 | `Setup` 차이 비교가 「같은 옛 항목이 어디든 하나라도 있으면 보내지 않음」이라, 같은 목적지·채널에 같은 묶음을 가진 트랙이 둘 이상이면 한 트랙의 프로그램을 바꿨다가 되돌린 값이 다른 트랙의 옛 항목에 걸러진다. 그 채널의 실제 마지막 값(되돌리기 전 9)은 비교에 들어가지 않는다. SEQ-8이 허용하는 「한 신디에 두 성부」(프로그램이 같아 경고도 없는 설정)에서, 프로그램 편집 뒤 되돌리기(undo)만으로 문서와 신디 음색이 어긋나 다음 재생까지 남는다. 재현: 시도 10. 비교 단위를 트랙(트랙 ID별 옛 묶음)으로 하거나, (목적지, 채널)별로 「마지막으로 보낸 묶음」을 기억해 그것과 비교할 것. TRN-3, OUT-4 | `crates/keyscore-app/src/transport.rs:207-218`, `crates/keyscore-app/src/engine.rs:79-86` |

## 7-1. 제안

| # | 내용 | 파일:줄 |
| --- | --- | --- |
| 1 | 템포 편집 때 기준점은 `prepare` 전의 `now`부터 새 템포를 쓰지만, 스케줄러는 교체를 처리한 뒤 보낸 구간(`sent_through`) 다음부터 새 템포다(`scheduler.rs:874-921`). 차이는 대략 (`prepare` 시간 + 미리 보기) × 템포 변화율이라 큰 프로젝트에서 템포를 크게 바꾸면 일시정지 자리가 수십 틱 틀어질 수 있다(코드 읽기로만 확인, 실행하지 않음). 일시정지 자리는 근삿값이라는 한계와 S3 위치 조회 이월(`13-S3-step-and-pianoroll.md:27`)에 포함되므로 결함으로 치지 않는다. 위치 조회를 정할 때 이 경우를 함께 볼 것 | `crates/keyscore-app/src/transport.rs:188-195`, `transport.rs:223-227` |
| 2 | 지적 1을 고칠 때 이월 문구(「재생 시작의 chase와 재생 중 송신 모두 트랙 순서대로」)가 실제 동작과 맞는지 다시 확인할 것. 재생 중 솔로·뮤트 전환은 `setup`이 그대로라 아무것도 보내지 않으므로, 공유 채널에서 솔로를 옮기면 새로 들리는 트랙은 그 채널에 마지막으로 나간 값으로 연주된다. SEQ-8 경고(S2-T5b)와 함께 다룰 내용이다 | `plan/stages/12-S2-app-shell.md:42` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2의 지적 1(차단, `collect_garbage`)·2·4·5와 제안 1은 근본까지 닿았고 섞인 긴 시퀀스에서도 다시 깨지지 않았다. 지적 3의 수정은 채널을 공유하지 않으면 맞게 동작하지만, 공유 채널에서는 들리지 않는 트랙이 들리는 트랙의 음색을 덮는 새 회귀를 만들었다(지적 1). 지적 2는 1·2회차가 놓친 비교 의미의 결함이다. 둘 다 고치는 범위가 작고(이벤트를 넣는 자리, 비교 키) 부록 A 테스트 3개로 확인할 수 있다.

이번이 프로토콜상 마지막 재리뷰다(`plan/01-protocol.md:25`). 구현자가 지적 1·2를 고치고 부록 A를 통과시킨 뒤에도 판정이 나지 않으면 결정 요청으로 전환한다. 공유 채널의 프로그램 우선순위 자체(누가 이기는가)를 REQ가 정하지 않았다고 판단하면, 지적 1의 기대(들리는 트랙이 이긴다)를 `docs/decisions/`에 결정 요청으로 올리는 것도 방법이다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 2 대응 커밋은 `engine.rs`, `transport.rs`, 리뷰 테스트, 보고서만)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 2 부록의 `unmuting_track_while_playing_sends_its_program` 기대 변경은 보고서에 이유가 있고, 바뀐 기대도 「첫 노트보다 먼저」로 의미를 지킨다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(리뷰 2 뒤 새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 테스트

`crates/keyscore-app/tests/review_s2_t5a_r3.rs`의 도우미(`start`, `open`, `run`, `edit`, `wait_for`, `drain_for`, `note_on`, `every_beat`, `last_program_before`)를 그대로 쓴다. 가져오기에 `Program`을 더하고 아래 도우미 `set_program`을 함께 넣는다. 2026-10-17 이 환경에서 3개 모두 실패했다(`cargo test -p keyscore-app --test <파일>`).

~~~rust
fn set_program(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, track: usize, value: u8) {
    edit(cx, view, |p| {
        vec![Edit::SetTrackProp {
            track: p.tracks()[track].id(),
            prop: TrackProp::Program(Some(Program::new(value).unwrap())),
        }]
    });
}

/// 깨뜨림 1: 같은 포트·채널의 두 트랙 가운데 A(프로그램 5)만 솔로다. 들리지 않는 B(프로그램 9)의 프로그램이
/// 틱 0 이벤트로 렌더러 결과 뒤에 붙어(같은 틱·같은 무리는 넣은 순서) chase·재생에서 마지막이 되어, 솔로한
/// A가 B의 음색으로 연주된다. B를 트랙 순서상 앞에 두어 「트랙 순서대로」 규칙으로도 5여야 함을 보인다.
#[gpui_kit::test]
fn soloed_track_on_shared_channel_plays_its_own_program(cx: &mut TestAppContext) {
    let lb = start(cx, "solo-shared");
    let text = r#"{"tempo": [[0, 500000]], "tracks": [
        {"port": "virtual", "channel": 1, "program": 9,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 62, 51492]]}]},
        {"port": "virtual", "channel": 1, "program": 5, "solo": true,
         "clips": [{"start": 0, "length": 7680, "notes": [[0, 240, 60, 51492]]}]}]}"#;
    let (handle, _view) = open(cx, ksq::from_str(text).unwrap().0);
    run(cx, handle, transport::Play);
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    let note = got.len() - 1;
    assert_eq!(
        last_program_before(&got, note, 0xC0),
        Some(5),
        "솔로한 트랙이 다른 트랙의 프로그램으로 연주된다: {got:?}"
    );
}

/// 깨뜨림 1의 뮤트 변형: 뮤트한 B(프로그램 9)가 들리는 A(프로그램 5)의 음색을 덮는다. 일시정지 뒤 중간
/// 위치에서 재개해도(chase) 같다.
#[gpui_kit::test]
fn muted_track_on_shared_channel_does_not_override_audible_program(cx: &mut TestAppContext) {
    let lb = start(cx, "mute-shared");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 9, "mute": true,
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 62, 51492]]}}]}},
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}}]}}"#,
        every_beat(60)
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    let start_got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    std::thread::sleep(Duration::from_millis(600));
    run(cx, handle, transport::Pause);
    drain_for(&lb.rx, Duration::from_millis(100));
    run(cx, handle, transport::Play);
    let resume_got = wait_for(&lb.rx, WAIT, note_on(60)).expect("재개 Note On 60");
    run(cx, handle, transport::Stop);
    let _ = view;
    assert_eq!(
        (
            last_program_before(&start_got, start_got.len() - 1, 0xC0),
            last_program_before(&resume_got, resume_got.len() - 1, 0xC0)
        ),
        (Some(5), Some(5)),
        "뮤트한 트랙의 프로그램이 들리는 트랙의 음색을 덮는다: 시작 {start_got:?}, 재개 {resume_got:?}"
    );
}

/// 깨뜨림 2: 같은 포트·채널의 두 트랙이 같은 프로그램 5다(SEQ-8의 「한 신디에 두 성부」, 경고 대상 아님).
/// 재생 중 B를 9로 바꾸면 9가 나간다. 되돌리기(undo)로 B를 5로 돌리면, 새 항목 B(채널 1, 5)가 옛 A 항목과
/// 같다고 보고 걸러져 5가 나가지 않는다. 두 트랙 모두 5인데 신디는 9에 남는다.
#[gpui_kit::test]
fn undoing_program_edit_on_shared_channel_restores_program(cx: &mut TestAppContext) {
    let lb = start(cx, "shared-undo");
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}},
        {{"port": "virtual", "channel": 1, "program": 5,
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#,
        every_beat(60)
    );
    let (handle, view) = open(cx, ksq::from_str(&text).unwrap().0);
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    set_program(cx, &view, 1, 9);
    let first = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60(편집 뒤)");
    assert!(first.iter().any(|m| m == &[0xC0, 9]), "{first:?}");
    let document = cx.read(|cx| view.read(cx).document().clone());
    cx.update(|cx| {
        document.update(cx, |doc, cx| {
            assert!(doc.undo().unwrap());
            cx.notify();
        });
    });
    cx.run_until_parked();
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60(되돌린 뒤)");
    let programs: Vec<u8> = cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .map(|t| t.program().unwrap().get())
            .collect()
    });
    run(cx, handle, transport::Stop);
    assert_eq!(programs, vec![5, 5]);
    assert!(
        got.iter().any(|m| m == &[0xC0, 5]),
        "두 트랙 모두 프로그램 5인데 신디는 9에 남는다: {got:?}"
    );
}
~~~
