# 결정 요청: 의존성 라이선스 허용 목록

- 관련 REQ: 2(라이선스 BSD-3-Clause), 8.4(「의존성은 BSD와 호환되는 허용적 라이선스(Apache-2.0, MIT, BSD)만」), 9.1(gpui-kit 필수)
- 관련 규약: `plan/00-conventions.md` 「의존성 추가는 허용적 라이선스(BSD·MIT·Apache-2.0·Zlib·ISC)만」
- 관련 Task: S0-T2부터(serde·proptest가 `unicode-ident`를 끌어옴), S2-T1(gpui-kit)
- 상태: 결정됨 (2026-10-12, B)

## 상황

`spikes/sp2-element-instancing`(crates.io `gpui-kit =0.7.1`, `default-features = false`)의 의존 트리를 macOS 두 타깃(`aarch64-apple-darwin`, `x86_64-apple-darwin`)으로 한정하고, 허용 목록을 규약 그대로(MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, Zlib, ISC) 둔 `cargo deny check licenses` 결과(2026-10-11):

| 라이선스 | crate | 성격 |
| --- | --- | --- |
| Unicode-3.0 | `icu_*`(8개), `litemap`, `potential_utf`, `tinystr`, `writeable`, `yoke`(+derive), `zerofrom`(+derive), `zerotrie`, `zerovec`(+derive) | OSI 승인 허용적 라이선스. URL 처리(`idna`) 경로로 들어온다 |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | `unicode-ident` | 거의 모든 proc-macro가 쓴다. S0-T2의 `serde_derive`·`proptest`에서 바로 걸린다 |
| MPL-2.0 | `cbindgen`(빌드 도구, 바이너리에 링크되지 않음), `option-ext` | 파일 단위 약한 copyleft. 수정하지 않고 쓰면 BSD 앱 배포에 의무가 생기지 않으나, 「허용적」의 범위 밖 |
| bzip2-1.0.6 | `libbz2-rs-sys` | 허용적(BSD 계열 문구) |

규약의 목록만으로는 S0-T2(serde)부터 `cargo deny`가 실패하고, REQ 9.1이 요구하는 gpui-kit은 쓸 수 없다.

## 선택지

- A. 허용 목록에 Unicode-3.0과 bzip2-1.0.6을 더한다. MPL-2.0은 목록에 넣지 않고 `cbindgen`·`option-ext` 두 crate만 `deny.toml`의 crate별 예외(`[[licenses.exceptions]]`)로 허용한다. 새 예외는 결정 요청을 거친다.
- B. A에 더해 MPL-2.0도 허용 목록에 넣는다.
- C. 목록을 그대로 둔다. gpui-kit을 쓰는 한 실현할 수 없으므로 REQ 9.1을 바꿔야 한다.

## 구현자 추천

A. Unicode-3.0·bzip2-1.0.6은 BSD 앱과 충돌하지 않는 허용적 라이선스이고, REQ 8.4의 「BSD와 호환되는 허용적 라이선스」 취지에 맞는다. MPL-2.0은 crate 단위 예외로 좁혀 두면 다른 MPL crate가 조용히 들어오는 것을 `cargo deny`가 계속 잡는다. NOTICE에 Unicode-3.0 고지를 함께 넣는다.

결정이 오기 전까지 S0-T1의 `deny.toml`은 규약의 목록 그대로 두고, S0-T2는 외부 의존을 추가하는 시점에 이 결정에 막히면 보류한다.

- 결정: B. Unicode-3.0, bzip2-1.0.6, MPL-2.0을 허용 목록에 더한다(사람, 2026-10-12).
- 반영: `deny.toml`의 `[licenses] allow`, `plan/00-conventions.md`의 툴체인 절.
- 후속(사람): REQ 8.4는 「BSD와 호환되는 허용적 라이선스(Apache-2.0, MIT, BSD)만」이라고 적혀 있다. MPL-2.0은 파일 단위 약한 copyleft라 이 문구와 맞지 않는다. REQ 문구 갱신이 필요하다. MPL-2.0 파일을 수정해 배포하면 그 파일의 소스 공개 의무가 생기므로, MPL crate는 수정 없이 쓴다.
