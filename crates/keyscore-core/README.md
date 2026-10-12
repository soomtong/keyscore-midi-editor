# keyscore-core

모델 계층. 프로젝트·트랙·클립·노트·CC 포인트, 템포 맵, undo, 퀀타이즈, `.ksq` 직렬화(REQ 9.6).

## 공개 API

아직 없다. S0-T2(모델)부터 채운다.

## 불변식

- 워크스페이스의 어떤 crate에도 의존하지 않는다.
- GPUI·CoreMIDI·네트워크 crate에 전이적으로도 의존하지 않는다(`scripts/check-deps.sh`).
- `unsafe` 금지(`#![deny(unsafe_code)]`).

## 테스트

~~~
cargo test -p keyscore-core
~~~
