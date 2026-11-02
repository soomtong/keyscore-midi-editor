# KeyScore

컴퓨터 키보드만으로 악보를 입력하고, 같은 Mac의 스탠드얼론 신디사이저에 MIDI로 보내는 macOS 전용 시퀀서다. Rust와 [GPUI](https://www.gpui.rs/)로 만든다.

앱은 소리를 만들지 않는다. 가상 MIDI 포트를 내고, 소리는 그 포트를 잡은 신디사이저가 낸다. v1은 MIDI 1.0으로 보내며 MIDI 2.0(UMP)은 이후 버전에서 다룬다.

## 상태

개발 중이며 아직 릴리스가 없다. 단계별로 진행하고, 단계마다 출구 게이트와 사람 서명을 거친다.

- S0 기반, S1 MIDI 엔진, S2 앱 셸: 완료·서명
- S3 Step 입력과 피아노롤: 진행 중
- S4 이후(클립·배치, 시퀀스 입력, 표현, AI, 릴리스): 예정

## 특징

- 입력·편집·재생·저장까지 마우스 없이 키보드로 끝난다.
- Step 입력, 메트로놈에 맞추는 Sequence 입력, 피아노롤 편집, 건반 시뮬레이션 모드를 둔다.
- 곡은 선형 트랙 위의 클립으로 구성한다. 노트는 클립 안에 있다.
- 기본 프리셋 키맵과 vim 별칭 키맵을 제공한다.
- 계이름 텍스트 문법(KeyScore Text)으로 가져오고 내보낸다.
- 외부 LLM으로 클립을 생성하는 AI 모드를 둔다(S7 예정).

요구사항 원문은 [docs/REQ.md](docs/REQ.md)에 있다.

## 구성

~~~
crates/
  keyscore-core/   모델, undo, 템포 맵, 퀀타이즈, .ksq 직렬화
  keyscore-text/   계이름 텍스트 파서·내보내기, 드럼 라벨
  keyscore-midi/   CoreMIDI 가상 포트, 스케줄러 스레드, SMF
  keyscore-ai/     OpenRouter 클라이언트, 프롬프트 조립
  keyscore-app/    GPUI UI, 키맵, 모드 FSM, 설정
tools/
  ksq-cli/         ksq, mid, txt 변환 CLI
  ksq-play/        .ksq 재생 CLI
docs/              REQ, 스파이크 결과, 결정 요청, 보고서
plan/              구현 계획과 진행 규칙
spikes/            기술 검증 코드 (제품 빌드와 분리)
~~~

crate 의존 방향은 `app → ai → text → core`, `app → midi → core`이다. `core`는 GPUI·CoreMIDI·네트워크에 의존하지 않는다.

## 빌드

macOS 14 이상, Apple Silicon, Rust stable(1.95 이상)이 필요하다.

~~~
cargo build --workspace
cargo test --workspace
~~~

PR 전 자동 게이트는 다음과 같다.

~~~
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
~~~

## 개발 문서

- [CLAUDE.md](CLAUDE.md): 에이전트용 작업 규칙
- [plan/README.md](plan/README.md): 계획 문서 구성과 진행 규칙
- [plan/00-conventions.md](plan/00-conventions.md): 저장소 구조, 코딩·테스트·커밋 규칙

## 라이선스

[BSD-3-Clause](LICENSE). 의존성은 허용적 라이선스만 쓴다.
