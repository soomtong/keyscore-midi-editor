# keyscore-core

모델 계층. 프로젝트·트랙·클립·노트·CC 포인트, 템포 맵, undo, 퀀타이즈, `.ksq` 직렬화(REQ 9.6).

## 공개 API

- 값 타입: `Tick`(u64, `PPQ` = 960), `Pitch`(0–127), `PitchOffset`(반음 1/2^25 단위, ±12반음), `Velocity`(MIDI 2.0 16비트, `from_midi1`/`to_midi1`), `Channel`(1–16, `is_drum`), `TrackColor`(12색 팔레트), `Program`, `Bank`, `PitchClass`, `Key`, `CcPoint`.
- `Note`: 클립 기준 시작 틱, 길이(≥ 1), 음높이, 피치 오프셋, 벨로시티, 가사.
- `Clip`: 트랙 위 구간과 노트. `insert_note`는 노트 끝까지 클립 안이어야 받는다.
- `Track`: SEQ-7 속성과 클립. 클립의 삽입·삭제·이동·분할과 클립 안 노트 삽입·삭제는 `Track`의 메서드로만 한다.
- `Project`: 조성과 트랙 목록(템포·박자표 맵은 S0-T3).
- `TempoMap`·`MeterMap`·`MusicTime`·`TimeSignature`: 템포·박자표 맵과 정확한 틱↔시각 변환(1/960 ns 단위).
- `TrackId`: 트랙 식별자. 문서(`Document`) 안에서 유일하다. `Project`는 구성 단계에서 `track_mut` 대입으로 중복을 만들 수 있으나 `Document::new`가 거부한다.
- `Edit`·`TrackProp`: 모델을 바꾸는 정방향 편집. 역편집은 히스토리 안에만 있다.
- `Document`·`CoalesceKey`: 편집 중인 곡(프로젝트 + undo 히스토리). 편집의 유일한 진입점이며 `&mut Project`를 내주지 않는다. 묶음 적용은 원자적이고, 같은 키의 연속 편집은 한 단위로 묶인다.
- `ModelError`: 불변식을 깨뜨리는 입력의 거부 사유.

## 불변식

- 워크스페이스의 어떤 crate에도 의존하지 않는다.
- GPUI·CoreMIDI·네트워크 crate에 전이적으로도 의존하지 않는다(`scripts/check-deps.sh`).
- `unsafe` 금지(`#![deny(unsafe_code)]`).
- 같은 트랙의 클립 구간 `[start, end)`는 겹치지 않는다(맞닿음은 허용).
- 클립 안 노트의 시작은 `0 ≤ start < 클립 길이`다. 끝이 클립 길이를 넘는 노트(숨김 꼬리)는 분할로만 생긴다(`docs/decisions/2026-10-12-clip-split-and-velocity.md`).
- 실패한 편집 메서드는 상태를 바꾸지 않는다.
- 편집은 `Document`로만 한다. `Project`의 `&mut` API는 구성 단계용이며, 거기서 생긴 id 중복은 `Document::new`가 거부한다.
- 속성 기반 회귀 테스트: `tests/model_invariants.rs`, `tests/tempo_map.rs`, `tests/history.rs`.

## 테스트

~~~
cargo test -p keyscore-core
~~~
