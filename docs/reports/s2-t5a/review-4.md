# 적대적 리뷰: S2-T5a MIDI 엔진 연결과 트랜스포트 Action (리뷰 4회차, 좁은 확인)

근거: `docs/decisions/2026-10-17-s2-t5a-review-limit.md` A. 리뷰 3의 지적 1·2를 닫았는지와 `prepare`·`Setup`·`transport_document_changed`의 회귀만 본다. 리뷰어는 구현자가 아니다.

## 확인한 것

- 이벤트 모으기: 들리는 트랙마다 `render(project, |j, _| (j == i).then_some(port))`를 따로 부른 결과를 트랙 순서로 잇는다. 트랙별 결과는 `(틱, order_class)` 안정 정렬이고 `Snapshot::new`도 같은 키의 안정 정렬(`scheduler.rs:152`)이라, 전체 렌더 한 번의 결과와 노트·CC·프로그램의 누락·중복·순서가 같다(렌더가 트랙을 독립으로 처리하고 `port_of`만 쓰기 때문). `Rendered.end`는 쓰지 않던 값이다.
- `Setup.track` 비교: 같은 트랙 id의 옛 설정과만 비교한다. 리뷰 3 지적 2의 undo 재현(부록 A 3)은 통과한다.
- 트랙 더하기: 새 id는 옛 설정에 없어 프로그램이 나간다. 확인 테스트 `inserting_track_while_playing_sends_its_program`(`tests/review_s2_t5a_r4.rs`) 통과.
- 지난 4개 테스트 파일(`engine` 8, `review_s2_t5a` 15, `_r2` 13, `_r3` 10)은 모두 통과한다. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과.

## 지적 사항

| # | 심각도 | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 지적 1은 들리지 않는 트랙이 트랙 순서에서 앞일 때만 닫혔다. 솔로(또는 들리는) 트랙 A가 앞이고 들리지 않는 B가 뒤인 같은 포트·채널에서는 B의 프로그램이 뒤에 들어가 A가 B의 음색으로 연주된다. 리뷰 3의 원래 증상(`[[192,5],[192,9],[144,60,100]]`)이 순서만 바꿔 그대로 난다. 리뷰 3 지적 1의 요구(들리는 트랙이 뮤트·솔로에 가려진 트랙의 음색으로 연주되면 안 됨, OUT-4)를 채우지 못한다. 단계 파일 이월 문구는 「뒤 트랙(들리지 않는 트랙 포함)이 이긴다」로 이를 한계로 적었지만, 리뷰 3은 그 기대를 받아들이지 않았다. 고치는 길 둘: 들리지 않는 트랙의 프로그램을 모두 들리는 트랙의 것보다 앞(틱 0, 먼저 넣기)에 두거나, 이 우선순위를 `docs/decisions/`에 결정 요청으로 올린다. 재현: 부록 A 1 | `crates/keyscore-app/src/engine.rs:241-262` |

## 제안(판정에 영향 없음)

| # | 내용 |
| --- | --- |
| 1 | 같은 채널을 나눠 쓰는 뒤 트랙을 재생 중 지우면 남은 트랙의 프로그램이 다시 나가지 않아 신디가 지운 트랙의 음색에 남는다(부록 A 2). 이번 커밋의 회귀가 아니다(이전 비교도 같았다). 재생 중 공유 채널 편집의 재송신은 이미 이월된 SEQ-8 처리(S2-T5b)에 이 사례를 함께 적을 것 |

## 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의

지적 2와 트랙 더하기, 이벤트 모으기의 동등성은 닫혔고 회귀는 없다. 지적 1은 절반만 닫혔다. 프로토콜상 마지막 재리뷰가 이미 지났으므로(결정 A), 고치는 대신 「공유 채널에서 누가 이기는가」를 결정 요청으로 올려 현재 동작(트랙 순서)을 사람이 인정하는 쪽도 가능하다. 그 경우 부록 A 1의 기대를 결정에 맞춰 바꾸고 병합한다.

## 체크리스트

- [x] 지적 1·2 수정 코드를 읽었다
- [x] 렌더 분할의 동등성을 정렬 코드로 확인했다
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 A 1·2 실패, 확인 테스트 통과)
- [x] 이전 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 프로덕션 코드를 고치지 않았다
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 테스트

`tests/review_s2_t5a_r4.rs`의 도우미에 `drain_for`(r3과 같음), `last_program_before`(r3과 같음)와 아래 `shared`를 더하면 돌아간다. 2026-10-17 이 환경에서 둘 다 실패했다.

~~~rust
fn shared(a: &str, b: &str) -> Project {
    let text = format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [
        {{"port": "virtual", "channel": 1, "program": 5{a},
         "clips": [{{"start": 0, "length": 15360, "notes": [{}]}}]}},
        {{"port": "virtual", "channel": 1, "program": 9{b},
         "clips": [{{"start": 0, "length": 15360, "notes": [[0, 240, 64, 51492]]}}]}}]}}"#,
        every_beat(60)
    );
    ksq::from_str(&text).unwrap().0
}

// 1: 받은 것 [[192, 5], [192, 9], [144, 60, 100]], 기대 마지막 프로그램 5
#[gpui_kit::test]
fn soloed_first_track_on_shared_channel_plays_its_own_program(cx: &mut TestAppContext) {
    let lb = start(cx, "solo-first");
    let (handle, _view) = open(cx, shared(r#", "solo": true"#, ""));
    run(cx, handle, transport::Play);
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    assert_eq!(last_program_before(&got, got.len() - 1, 0xC0), Some(5), "{got:?}");
}

// 2: 받은 것 [[128, 60, 0], [144, 60, 100]], 프로그램 5가 나가지 않는다
#[gpui_kit::test]
fn removing_later_shared_track_while_playing_restores_remaining_program(cx: &mut TestAppContext) {
    let lb = start(cx, "remove");
    let (handle, view) = open(cx, shared("", ""));
    run(cx, handle, transport::Play);
    wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    edit(cx, &view, |p| vec![Edit::RemoveTrack { track: p.tracks()[1].id() }]);
    drain_for(&lb.rx, Duration::from_millis(50));
    let got = wait_for(&lb.rx, WAIT, note_on(60)).expect("Note On 60");
    run(cx, handle, transport::Stop);
    assert!(got.iter().any(|m| m == &[0xC0, 5]), "{got:?}");
}
~~~
