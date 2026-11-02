# 보고: 원격 CI 첫 실행 실패와 대응

- 브랜치: `claude/handoff-continuation-efszpp`
- 관련 남은 작업: HANDOFF 「원격 CI 첫 실행과 CI 러너 벤치 기준값」 중 첫 실행 부분. 벤치 기준값은 그대로 S8 이월.

## 관찰

`main`에 원격(GitHub, `macos-15` 러너)이 생긴 뒤 CI 네 번 가운데 끝난 세 번이 모두 `cargo test --workspace`에서 실패했다. fmt·clippy는 통과했다.

| 실행 | 커밋 | 실패한 시험 | 메시지 |
| --- | --- | --- | --- |
| 1 | 73daaf2 | `review_s2_t5a::pause_position_follows_tempo_change_in_song` | `Tick(1601)` |
| 3 | 18ea43a | 같음 | `Tick(1604)` |
| 4 | 13087f8 | `inspector::send_test_note_reaches_track_port` | `MIDIClientCreate` `-50` |

실행 2(b130832)는 끝난 기록이 없다. 실행 4는 `inspector`에서 멈춰 `review_s2_t5a`까지 가지 않았다.

## 원인

1. 일시정지 자리 시험은 재생 뒤 벽시계로 1초 자고 일시정지한 자리가 1300–1600틱 안이기를 기대한다. 1600틱은 약 1.167초다. 러너에서 재생·일시정지 처리와 잠이 길어져 상한을 1–4틱 넘었다. 템포 지도를 따르지 않으면 같은 시간에 약 2240틱이 되므로, 넘친 값은 결함이 아니라 시간 여유 부족이다.
2. `inspector` 바이너리에서 처음 만드는 CoreMIDI 클라이언트가 10초 재시도(`CLIENT_CREATE_PATIENCE`) 내내 `-50`으로 실패했다(앞 시험 끝 11:49:03 → 실패 11:49:12). 같은 실행의 앞선 바이너리들은 클라이언트를 만들었다. `MIDIServices.h`는 마지막 클라이언트가 사라지면 MIDI 서버가 끝날 수 있다고 한다. 시험 바이너리마다 프로세스가 달라, 바이너리가 끝날 때 서버가 내려가고 다음 바이너리가 재기동을 기다리다 시간을 넘긴 것으로 본다. 러너에서만 재현되며 이 컨테이너(Linux)에서는 앱 crate를 빌드할 수 없어 가설로 남긴다.

## 대응

- `crates/keyscore-app/tests/review_s2_t5a.rs`: 상한을 고정값 대신 「재생 직전부터 일시정지가 돌아올 때까지 흐른 시간 + lookahead」를 템포 지도(0.5초까지 120 BPM, 뒤로 60 BPM)로 바꾼 틱으로 잡는다. 하한 1300은 그대로다. 템포 변화를 무시한 구현은 같은 시간에 틱이 거의 두 배라 여전히 실패한다. 시험을 지우거나 `#[ignore]`하지 않았다.
- `.github/workflows/ci.yml`, `.github/ci/midi-keepalive.swift`: 시험 단계 동안 Swift 스크립트가 CoreMIDI 클라이언트 하나를 쥐고 있어 서버가 바이너리 사이에 내려가지 않게 한다. 띄우지 못해도 시험은 돌리고, 결과를 로그에 한 줄 남긴다. 제품 코드의 재시도 시간(10초)은 바꾸지 않았다. 앱 시작 동작을 CI 사정으로 바꾸지 않으려는 것이다.

## 확인

- `rustfmt --check`(바꾼 시험 파일), workflow YAML 구문 확인.
- macOS 빌드·시험은 이 컨테이너에서 돌릴 수 없다. 이 브랜치의 PR(또는 `main` 병합)로 CI가 돌아야 확인된다. 대응 2가 가설대로가 아니면 `inspector` 실패가 다시 나온다. 그때는 실패 직전 시험 바이너리와 `midi-keepalive` 로그 줄을 본다.

## 알려진 한계

- CI에는 사람 Mac과 달리 늘 떠 있는 MIDI 앱이 없다. 다른 타이밍 시험(지터 0.5 ms 등)도 러너 부하에 따라 흔들릴 수 있다. 첫 녹색 실행 뒤 몇 번 더 보고 판단한다.
