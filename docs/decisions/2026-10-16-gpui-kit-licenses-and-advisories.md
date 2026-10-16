# 결정 요청: gpui-kit이 끌어오는 라이선스 둘과 유지보수 중단 권고 넷

- 날짜: 2026-10-16
- 상태: 결정 대기
- 관련: REQ 8.4(라이선스), 9.1(GPUI는 gpui-kit이 고정한 스냅샷), 9.4, `deny.toml`, `docs/decisions/2026-10-11-license-allowlist.md`, S2-T1

## 상황

S2-T1에서 `keyscore-app`에 `gpui-kit = "=0.7.1"`을 더하자 `cargo deny check`가 실패한다. 전부 gpui-kit과 그 GPUI 스냅샷(`gpui-pre =0.3.8`)의 전이 의존이다. REQ 9.1·9.4가 정한 의존이라 다른 버전으로 바꿀 수 없다.

라이선스(허용 목록 밖)

| crate | 라이선스 | 끌어오는 곳 | 성격 |
| --- | --- | --- | --- |
| `enum-iterator` 2.3.0, `enum-iterator-derive` 1.5.0 | 0BSD | `gpui-component` | BSD Zero Clause. 고지 의무도 없는 가장 느슨한 BSD 계열(OSI 승인) |
| `notify` 8.2.0 | CC0-1.0 | `gpui-component`(테마 디렉터리 감시) | 퍼블릭 도메인 헌정. 고지·조건 없음 |

유지보수 중단 권고(취약점이 아니라 「더 이상 고치지 않는다」는 알림, 안전한 업그레이드 없음)

| crate | 권고 | 끌어오는 곳 |
| --- | --- | --- |
| `instant` 0.1.13 | RUSTSEC-2024-0384 | GPUI 스냅샷 |
| `paste` 1.0.15 | RUSTSEC-2024-0436 | `gpui-component`, `metal` |
| `rustybuzz` 0.20.1 | RUSTSEC-2026-0206 | `usvg` ← `gpui-pre` |
| `ttf-parser` 0.25.1 | RUSTSEC-2026-0192 | 폰트 경로(`rustybuzz` 등) |

## 선택지

- A. 허용 목록에 `0BSD`, `CC0-1.0`을 더한다. 넷의 권고는 `deny.toml`의 `[advisories] ignore`에 ID와 이유(「gpui-kit 고정 스냅샷의 전이 의존, 안전한 업그레이드 없음, REQ 9.1」)를 적어 무시한다. gpui-kit을 올릴 때마다 무시 목록을 다시 본다.
- B. A와 같되, 무시를 `keyscore-app`의 의존 경로에만 한정하는 방법을 찾는다(cargo-deny는 crate 단위 무시만 지원해 실질적으로 A와 같다).
- C. 받지 않는다. gpui-kit 대신 다른 UI 바탕을 찾는다(REQ 9.1·9.4 변경 필요).

## 권고

A.

- 0BSD는 이미 허용한 BSD-1-Clause(S1-T4)보다도 조건이 적다. CC0-1.0은 권리를 포기한 퍼블릭 도메인 헌정이라 배포물에 지는 의무가 없다.
- 권고 넷은 보안 취약점이 아니라 유지보수 중단 알림이다. 고칠 수 있는 버전이 없고, 스냅샷을 고정하는 REQ 9.1과 바로 부딪친다.
- `NOTICE`에는 Apache-2.0 구성요소(gpui-kit, GPUI) 고지를 이 Task에서 더한다.

## 결정

(사람이 적는다)
