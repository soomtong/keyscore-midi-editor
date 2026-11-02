# 보고: 원격 CI 첫 실행 실패와 대응

- 브랜치: `claude/handoff-continuation-efszpp`
- 관련 남은 작업: HANDOFF 「원격 CI 첫 실행과 CI 러너 벤치 기준값」 중 첫 실행 부분. 벤치 기준값은 그대로 S8 이월.
- 범위(리뷰 2 「범위 재정의」 뒤): MIDI 서버 keepalive, 벽시계 여유로 실패한 일시정지 시험 둘(`review_s2_t5a`, `review_s2_t5a_r2`), `--no-fail-fast`, 그리고 드러난 실패 목록의 기록. 이 PR로 원격 CI가 녹색이 되지는 않는다. 나머지 실패는 결정 요청 `docs/decisions/2026-11-02-ci-runner-scope.md`로 넘긴다.

## 관찰

`main`에 원격(GitHub, `macos-15` 러너)이 생긴 뒤 CI 네 번이 모두 `cargo test --workspace`에서 실패했다. fmt·clippy는 통과했다.

| 실행 | 커밋 | 실패한 시험 | 메시지 |
| --- | --- | --- | --- |
| 1 | 73daaf2 | `review_s2_t5a::pause_position_follows_tempo_change_in_song` | `Tick(1601)` |
| 2 | b130832 | 같음 | `Tick(1603)` |
| 3 | 18ea43a | 같음 | `Tick(1604)` |
| 4 | 13087f8 | `inspector::send_test_note_reaches_track_port` | `MIDIClientCreate` `-50` |

실행 4는 `inspector`에서 멈춰 `review_s2_t5a`까지 가지 않았다(리뷰 1 지적 2 (가): 처음 쓸 때 실행 2를 「끝난 기록 없음」으로 잘못 적었다).

### PR 1의 CI(`--no-fail-fast`)

PR 실행 41442250458(d19b27b)에서 위 두 시험은 통과했고 keepalive는 시험 끝까지 살아 있었다. 첫 실패에서 멈추지 않자 `main`에 가려져 있던 실패가 시험 대상 14개로 드러났다(리뷰 2 지적 1).

- 벽시계·실시간 타이밍 8개: `keyscore-app` `review_s2_t5a_r2`(`Tick(2834)`·`Tick(3038)`, 이 PR이 고침), `review_s2_t5a_r3`(메트로놈 켜짐 없음), `review_s3_t8_r2`(첫 박 `[40, 41]`), `transport_bar`(탭 템포 84.4). `keyscore-midi` `scheduler`(지터 p99 4.478 ms, 기준 0.5 ms), `review_s1_t4_r2`(`None`), `review_s1_t5`(클릭 하나 더), `review_s3_t8_r3`(루프 끝 경계).
- CoreMIDI 환경 2개: `keyscore-app` `review_s2_t6c`(「감시가 시작되지 않았다」 둘), `keyscore-midi` `review_s1_t1_r3`(`MIDISourceCreateWithProtocol` `-307`).
- 파일 시스템 4개: `ksq-cli` `cli`, `review_s0_t8_r6`·`r9`·`r11`(러너의 `/dev` 쓰기가 `EACCES`, 링크 사슬의 종료 코드 등).

## 원인

1. 일시정지 자리 시험은 재생 뒤 벽시계로 1초 자고 일시정지한 자리가 1300–1600틱 안이기를 기대한다. 1600틱은 약 1.167초다. 러너에서 재생·일시정지 처리와 잠이 길어져 상한을 1–4틱 넘었다. 템포 지도를 따르지 않으면 같은 시간에 약 2240틱이 되므로, 넘친 값은 결함이 아니라 시간 여유 부족이다.
2. `inspector` 바이너리에서 처음 만드는 CoreMIDI 클라이언트가 10초 재시도(`CLIENT_CREATE_PATIENCE`) 동안 `-50`으로 실패했다. 원인은 확인하지 못했다. 가설은 「MIDI 서버 재기동이 가끔 10초 넘게 실패한다」이다. `MIDIServices.h`는 마지막 클라이언트가 사라지면 서버가 끝날 수 있다고 하고, 시험 바이너리는 저마다 프로세스라 바이너리 사이에 서버가 내려갈 수 있다.

   가설을 약하게 하는 근거(리뷰 1 지적 2):
   - 실행 1–3도 바이너리 순서(`files_playback` → `import_export` → `inspector`)와 「마지막 MIDI 바이너리가 끝난 뒤 5–6초에 클라이언트를 만듦」이 같은데 통과했다. 그러니 재기동 자체가 아니라 재기동이 가끔 길어지는 경우라야 맞다. 4회 중 1회다.
   - 이 저장소가 지금까지 본 재기동 중 오류는 `-304`·`-2`였고 `-50`(`paramErr`)은 처음이다(`client.rs`의 `CLIENT_CREATE_PATIENCE` 주석, `review_s1_t1.rs`).
   - 처음 쓴 「앞 시험 끝 11:49:03 → 실패 11:49:12」는 근거가 되지 못한다. 9.2초로 10초보다 짧아, 실패한 시험은 같은 바이너리 안에서 앞 시험과 병렬로 이미 시작했던 것이다.
   - 이 컨테이너(Linux)에서는 앱 crate를 빌드할 수 없어 재현하지 못했다.

## 대응

- `crates/keyscore-app/tests/review_s2_t5a.rs`: 상한을 고정값 대신 「재생 직전부터 일시정지가 돌아올 때까지 흐른 시간 + lookahead」를 템포 지도(0.5초까지 120 BPM, 뒤로 60 BPM)로 바꾼 틱으로 잡는다. 하한 1300은 그대로다. 템포 변화를 무시한 구현은 같은 시간에 틱이 거의 두 배라 여전히 실패한다. 시험을 지우거나 `#[ignore]`하지 않았다.
- `crates/keyscore-app/tests/review_s2_t5a_r2.rs`: `pause_after_several_tempo_edits_matches_elapsed_time`의 고정 구간(1800–2300)을 편집 호출 앞뒤 시각으로 잰 세 템포 구간의 길이로 바꿨다. 위 끝은 틱이 가장 많이 가게, 아래 끝은 가장 적게 가게 잡고 여유 100틱을 둔다. 템포 편집을 반영하지 않는 구현은 여전히 벗어난다.
- `.github/workflows/ci.yml`, `.github/ci/midi-keepalive.swift`: 시험 단계 동안 Swift 스크립트가 CoreMIDI 클라이언트 하나를 쥐고 있어 서버가 바이너리 사이에 내려가지 않게 한다. 띄우지 못해도 시험은 돌린다. 시험 앞뒤로 keepalive 상태를 남겨(`ready before tests=yes|no`, `alive after tests`, 살아 있지 않으면 그 로그), 다음 실패 때 「가설이 틀림」과 「keepalive가 없었음」을 가른다(리뷰 1 지적 1). 시험은 `--no-fail-fast`로 돌아 한 실패가 뒤 바이너리를 가리지 않는다(리뷰 1 제안 3). 제품 코드의 재시도 시간(10초)은 바꾸지 않았다. 앱 시작 동작을 CI 사정으로 바꾸지 않으려는 것이다.

## 확인

- `rustfmt --check`(바꾼 시험 파일), workflow YAML 구문 확인.
- macOS 빌드·시험은 이 컨테이너에서 돌릴 수 없어 PR 1의 CI로 확인했다. 796f57c·d19b27b 두 실행에서 `review_s2_t5a`와 `inspector`가 통과했다. `review_s2_t5a_r2` 수정(1d771b7)은 그 뒤 실행으로 본다.
- CI 단계는 셸 모의 실행(가짜 `swift`·`cargo`, `/bin/bash -e`)으로 keepalive가 살아 있음·도중에 죽음·띄우지 못함 세 경우의 로그와 종료 코드 보존을 확인했다.

## 알려진 한계

- 가설 2가 맞다면 CI만의 사정이 아니다. 사용자가 마지막 MIDI 앱을 끈 직후 KeyScore를 열면 10초 재시도를 넘겨 OUT-1 포트 없이 뜰 수 있다. keepalive는 CI에서 이 경로를 가린다. 녹색 실행 몇 번으로 가설을 확인하지는 못하므로, 제품 쪽(재시도 시간, 늦게라도 포트를 다시 등록하는 길)은 HANDOFF의 후속 항목으로 남긴다(리뷰 1 제안 4).
- 원격 CI는 이 PR 뒤에도 빨갛다(위 14개 대상 중 이 PR이 고친 하나를 뺀 나머지). 결정 요청 `2026-11-02-ci-runner-scope.md`.
- keepalive가 있으면 서버 유휴 종료 회귀 시험(`crates/keyscore-midi/tests/review_s1_t1.rs`의 프로세스 클라이언트를 해제하지 않음)이 CI에서 늘 통과한다. 시험 스스로 「다른 MIDI 클라이언트가 떠 있으면 늘 통과한다」고 적은 시험이다. 누가 `MIDIClientDispose`를 되살려도 CI는 잡지 못하고, 사람 Mac의 게이트(다른 MIDI 앱이 없을 때)만 잡는다(리뷰 2 제안 3).

## 리뷰 대응

### 리뷰 1 (`review-1.md`, 수정 후 재리뷰)

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 지적 | 받아들임. 스크립트 끝을 조건 없는 `sleep` 고리로 바꾸고, 시험 뒤 생존을 한 줄 남긴다. 단계 종료 코드는 `cargo test`의 것(셸 모의 실행으로 `exit=101` 보존 확인) | c86a67f |
| 2 | 지적 | 받아들임. 실행 2 기록(`Tick(1603)`)을 고치고, 가설을 「재기동이 가끔 10초 넘게 실패」로 좁혀 반대 근거 셋을 적었다. 커밋 85b041a 본문은 가설을 사실처럼 적었으나 기록을 고쳐 쓰지 않고 이 보고서로 바로잡는다 | 45f3712 |
| 3 | 제안 | 받아들임. `--no-fail-fast` | ba00d66 |
| 4 | 제안 | 받아들임. 「알려진 한계」와 HANDOFF 후속 항목 | 45f3712 |
| 5 | 제안 | 받아들임. 시험에 가정 주석 | c20dec4 |

### 리뷰 2 (`review-2.md`, 범위 재정의)

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 지적 | 받아들임. 범위를 위 「범위」 줄로 다시 정하고, 드러난 14개 대상을 「관찰」에 적고, 나머지는 결정 요청으로 넘겼다. HANDOFF의 「녹색이면 병합」을 고쳤다 | c5592a3, ea20f3c |
| 2 | 지적 | 받아들임. keepalive 머리 주석을 가설로 고치고(실행 4), `RunLoop.main.run()`이 러너에서 돌아오지 않았음을 적고, 시험 주석에 1603을 더했다. 커밋 c86a67f 본문은 고쳐 쓰지 않고 주석으로 바로잡는다 | f6e3a28 |
| 3 | 제안 | 받아들임. 「알려진 한계」에 적음 | ea20f3c |
| 4 | 제안 | 받아들임. `grep -qx`, 시험 앞 준비 여부, 살아 있지 않으면 로그 다시 보이기. 셸 모의 실행 세 경우로 확인 | 79dd066 |
| 5 | 제안 | 받아들임. HANDOFF에 병합 방식(merge commit, squash 금지)을 적고, keepalive가 살아 있는데 `-50`이 다시 나면 재기동 가설이 틀린 것이므로 원인 조사로 고쳤다 | ea20f3c |
