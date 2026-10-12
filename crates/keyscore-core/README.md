# keyscore-core

모델 계층. 프로젝트·트랙·클립·노트·CC 포인트, 템포 맵, undo, 퀀타이즈, `.ksq` 직렬화(REQ 9.6).

## 공개 API

- 값 타입: `Tick`(u64, `PPQ` = 960), `Pitch`(0–127), `PitchOffset`(센트, ±1200), `Velocity`(v1 0–127), `Channel`(1–16, `is_drum`), `TrackColor`(12색 팔레트), `Program`, `Bank`, `PitchClass`, `Key`, `CcPoint`.
- `Note`: 클립 기준 시작 틱, 길이(≥ 1), 음높이, 피치 오프셋, 벨로시티, 가사.
- `Clip`: 트랙 위 구간과 노트. `insert_note`는 노트 끝까지 클립 안이어야 받는다.
- `Track`: SEQ-7 속성과 클립. 클립의 삽입·삭제·이동·분할과 클립 안 노트 삽입·삭제는 `Track`의 메서드로만 한다.
- `Project`: 조성과 트랙 목록(템포·박자표 맵은 S0-T3).
- `ModelError`: 불변식을 깨뜨리는 입력의 거부 사유.

## 불변식

- 워크스페이스의 어떤 crate에도 의존하지 않는다.
- GPUI·CoreMIDI·네트워크 crate에 전이적으로도 의존하지 않는다(`scripts/check-deps.sh`).
- `unsafe` 금지(`#![deny(unsafe_code)]`).
- 같은 트랙의 클립 구간 `[start, end)`는 겹치지 않는다(맞닿음은 허용).
- 클립 안 노트의 시작은 `0 ≤ start < 클립 길이`다. 끝이 클립 길이를 넘는 노트(숨김 꼬리)는 분할로만 생긴다(`docs/decisions/2026-10-12-clip-split-and-velocity.md`).
- 실패한 편집 메서드는 상태를 바꾸지 않는다.
- 속성 기반 회귀 테스트: `tests/model_invariants.rs`.

## 테스트

~~~
cargo test -p keyscore-core
~~~
