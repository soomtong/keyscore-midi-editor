# keyscore-text

계이름 텍스트(KeyScore Text, REQ 5.14) 파서·내보내기와 드럼 라벨 표(REQ 5.13).

## 공개 API

아직 없다. S0-T5에서 채운다. 문법 결정 요청(`docs/decisions/2026-10-11-ktext-grammar-ambiguities.md`)이 답을 받은 뒤 착수한다.

## 불변식

- `keyscore-core`에만 의존한다.
- GPUI·CoreMIDI·네트워크 crate에 전이적으로도 의존하지 않는다(`scripts/check-deps.sh`).
- `unsafe` 금지.

## 테스트

~~~
cargo test -p keyscore-text
~~~
