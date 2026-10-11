# keyscore-app

KeyScore macOS 앱. GPUI(gpui-kit) UI, 키맵, 모드 FSM, 온보딩, 설정.

## 공개 API

아직 없다. S2에서 앱 뼈대를 만든다. 지금은 의존 방향을 고정하는 빈 바이너리다.

## 불변식

- 다른 crate는 이 crate에 의존하지 않는다.
- GPUI는 gpui-kit이 고정한 `gpui-pre` 스냅샷만 쓴다(REQ 9.1).
- `unsafe`는 함수 단위 `#[allow(unsafe_code)]`와 이유 주석으로만.

## 테스트

~~~
cargo test -p keyscore-app
~~~
