# keyscore-midi

CoreMIDI 가상 포트·포트 열거, 스케줄러 스레드, MIDI 1.0 인코딩, SMF 입출력.

## 공개 API

아직 없다. S1에서 채운다. SP-3에 따라 가상 소스 생성과 송신 경로는 `coremidi-sys`를 직접 쓴다(`docs/spikes/sp3-coremidi.md`).

## 불변식

- `keyscore-core`에만 의존한다(워크스페이스 안에서).
- `unsafe`는 함수 단위 `#[allow(unsafe_code)]`와 이유 주석으로만.
- 스케줄러 스레드에서 할당·락·로그 금지(REQ 8.4).

## 테스트

~~~
cargo test -p keyscore-midi
~~~
