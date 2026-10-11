# Task 계획: S0-T1 워크스페이스와 CI 뼈대

- 브랜치: `s0/t1-workspace-ci`
- 관련 REQ: 2(라이선스·CI 결정), 8.4(라이선스), 9.1, 9.6
- 선행 Task: 없음
- 관련 스파이크: 없음(SP-2의 crates.io `gpui-kit =0.7.1` 확인은 S2에서 쓴다)

## 끝의 정의 (내 말로)

- 루트 `Cargo.toml`이 다섯 crate(`keyscore-core`, `-text`, `-midi`, `-ai`, `-app`)를 멤버로 갖는 워크스페이스다. `spikes/`는 멤버가 아니다.
- crate 사이 path 의존이 9.6의 방향 그대로 선언되어 있다. `core`는 아무 crate에도 의존하지 않는다. `text → core`, `midi → core`, `ai → core, text`, `app → core, text, midi, ai`.
- 외부 의존(serde, coremidi, gpui-kit, reqwest 등)은 이 Task에서 추가하지 않는다. 각자를 쓰는 Task에서 추가한다.
- 린트가 워크스페이스 단위로 설정되어 있다. `clippy::all`과 `clippy::pedantic`은 warn이고 CI에서 `-D warnings`로 막는다. `core`·`text`·`ai`의 `lib.rs`에는 `#![deny(unsafe_code)]`를 둔다.
- `rust-toolchain.toml`이 1.95.0과 rustfmt·clippy 컴포넌트를 고정한다.
- `deny.toml`이 라이선스 허용 목록(MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, Zlib, ISC), 알 수 없는 레지스트리·git 소스 거부, 보안 권고 검사를 설정한다.
- `LICENSE`(BSD-3-Clause), `NOTICE`(gpui-kit·Ely 등 Apache-2.0 의존성 고지 자리), crate마다 `README.md`가 있다.
- `scripts/check-deps.sh`가 crate 사이 의존 방향과 「`core`·`text`가 GPUI·CoreMIDI·reqwest에 의존하지 않음」을 `cargo tree`로 검사한다.
- `.github/workflows/ci.yml`이 macOS 러너에서 fmt·clippy·test·deny·check-deps를 돌린다.
- 원문과 다른 점: 원문 게이트는 「CI가 초록」이다. 저장소에 remote가 없어서 GitHub Actions를 실제로 돌릴 수 없다. 대신 CI의 각 단계를 로컬에서 같은 명령으로 실행해 통과를 확인하고, 원격 CI 초록 확인은 remote가 생긴 뒤의 수동 항목으로 넘긴다. 또 S0 출구 게이트의 `cargo tree` 검사를 이 Task에서 미리 만든다(리뷰 초점이 의존 방향이므로).

## 공개 인터페이스 초안

이 Task는 빈 crate만 만든다. 공개 API는 없다. 각 `lib.rs`는 crate 문서 주석과 린트 속성만 갖는다.

~~~text
scripts/check-deps.sh            # 종료 코드 0 = 통과, 1 = 위반. 위반 내용을 stderr에
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `scripts/check-deps.sh` (워크스페이스) | 스크립트 | 다섯 crate의 직접 의존이 허용 목록과 정확히 같다 |
| 2 | `scripts/check-deps.sh` (금지 crate) | 스크립트 | `core`·`text`의 전이 의존에 `gpui*`, `coremidi*`, `reqwest`가 없다 |
| 3 | 위반 주입 시 실패 | `scripts/check-deps.test.sh` (리뷰 1 뒤 자동화) | 직접·플랫폼별·이름 바꾼·dev·build 의존으로 넣은 역방향 내부 의존과 금지 crate를 모두 잡는다 |
| 4 | 깨끗한 워크스페이스 통과 | `scripts/check-deps.test.sh` | 위반이 없으면 통과한다(검사가 무조건 실패하는 퇴화 방지) |
| 5 | `cargo fmt/clippy/test/deny` | 자동 게이트 | 빈 워크스페이스에서 네 게이트 통과 |
| 6 | `unsafe` 거부 확인 | 수동 실험 | `core`에 `unsafe {}` 블록을 임시로 넣으면 컴파일 실패 |

테스트 먼저: 1·2는 워크스페이스가 없는 상태에서 실행해 실패를 확인한 뒤 워크스페이스를 만든다.

## 불변식

- 의존 방향: `app → ai → text → core`, `app → midi → core`. 역방향 또는 건너뛴 방향(`text → midi` 등)이 생기면 `check-deps.sh`가 실패한다.
- `core`·`text`는 GPUI·CoreMIDI·네트워크 crate에 전이적으로도 의존하지 않는다.
- `core`·`text`의 직접 외부 의존은 `check-deps.sh`의 허용 목록 안에 있다. 새 crate는 그 목록을 고치는 PR에서 리뷰받는다.
- 검사는 모든 타깃(`--target all`)과 normal·build·dev 의존 전부에 적용한다.

## 범위 밖

- 외부 의존 추가(serde, proptest, insta, criterion, coremidi, gpui-kit, reqwest). 사용하는 Task에서 추가한다.
- `tools/ksq-cli`(S0-T8), `assets/`(S0-T8·S7), `benches/baseline.json`(S0-T3).
- 서명 빌드·야간 1시간 재생 CI(S8).
- 원격 CI 실행(remote 없음).

## 리스크·결정 요청

- 라이선스 허용 목록: `plan/00-conventions.md`는 BSD·MIT·Apache-2.0·Zlib·ISC만 허용한다. `proptest`·`serde` 등이 끌어오는 `unicode-ident`는 `(MIT OR Apache-2.0) AND Unicode-3.0`이라 S0-T2부터 `cargo deny`가 막을 수 있다. GPUI 의존 트리에는 더 많은 라이선스가 있을 것이다. 결정 요청: `docs/decisions/2026-10-11-license-allowlist.md`. 이 Task 자체는 외부 의존이 없어서 결정 없이 끝난다.
