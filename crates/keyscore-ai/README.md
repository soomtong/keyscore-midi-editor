# keyscore-ai

OpenRouter 클라이언트, 프롬프트 템플릿, 컨텍스트 조립, 실험실 평가.

## 공개 API

아직 없다. S7에서 채운다.

## 불변식

- 워크스페이스 안에서는 `keyscore-core`, `keyscore-text`에만 의존한다.
- `unsafe` 금지.

## 테스트

~~~
cargo test -p keyscore-ai
~~~
