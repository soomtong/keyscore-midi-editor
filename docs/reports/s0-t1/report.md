# Task 보고: S0-T1 워크스페이스와 CI 뼈대

- PR: 로컬 브랜치 `s0/t1-workspace-ci` (remote 없음)
- 계획 파일: docs/reports/s0-t1/plan.md
- 계획 대비 달라진 점
  - `clippy.toml`을 추가했다. `clippy::doc_markdown`(pedantic)이 `KeyScore`·`CoreMIDI` 같은 고유명사에 백틱을 요구해서, `doc-valid-idents`에 등록했다.
  - `deny.toml`에 `allow-wildcard-paths = true`를 넣었다. 버전 없는 내부 path 의존을 `wildcards = "deny"`가 막았기 때문이다.
  - CI의 deny 단계는 Linux용 Docker 액션 대신 macOS 잡 안에서 `taiki-e/install-action`으로 설치해 돌린다. 계획의 「macOS 러너」를 지키려는 것이다.

## 구현 요약

- `Cargo.toml`: 워크스페이스(resolver 3, edition 2024), 멤버 다섯 개, `exclude = ["spikes"]`, 내부 crate의 `workspace.dependencies`, 워크스페이스 린트(`clippy::all`·`clippy::pedantic` warn, `unsafe_op_in_unsafe_fn` deny).
- `crates/keyscore-{core,text,midi,ai}/src/lib.rs`, `crates/keyscore-app/src/main.rs`: crate 문서 주석과 린트 속성만 있다. `core`·`text`·`ai`에는 `#![deny(unsafe_code)]`를 둔다.
- `crates/*/Cargo.toml`: 내부 의존은 9.6의 방향대로만 선언했고, 외부 의존은 없다.
- `crates/*/README.md`: 역할, 공개 API(없음), 불변식, 테스트 실행법.
- `rust-toolchain.toml`: 1.95.0, rustfmt·clippy.
- `deny.toml`: 라이선스는 규약 목록 그대로 두고, 대상은 macOS 타깃 두 개로 한정했다. 알 수 없는 레지스트리·git 소스와 yanked crate를 거부한다.
- `LICENSE`(BSD-3-Clause, SPDX 원문), `NOTICE`(고지 자리와 추가 예정 목록).
- `scripts/check-deps.sh`: crate 사이 직접 의존이 기대 집합과 정확히 같은지 보고, `core`·`text`의 전이 의존에 금지 crate가 없는지 `cargo tree`로 검사한다.
- `.github/workflows/ci.yml`: macOS 러너에서 fmt → clippy → test → check-deps → deny 순서로 돈다.

## 테스트 결과

~~~
cargo test --workspace: test result: ok. 0 passed (9개 대상 모두 ok, 테스트 대상 코드 없음)
scripts/check-deps.sh: check-deps: ok
~~~

| 계획의 테스트 # | 실제 테스트 | 상태 |
| --- | --- | --- |
| 1 | `scripts/check-deps.sh` 내부 의존 검사 | 통과. 워크스페이스가 없을 때 실패하는 것을 먼저 확인했다 |
| 2 | `scripts/check-deps.sh` 금지 crate 검사 | 통과 |
| 3 | `text`에 `keyscore-midi` 의존 주입 | 실패를 확인했다(`실제 [keyscore-core keyscore-midi], 기대 [keyscore-core]`). 되돌렸다 |
| 4 | `text`에 가짜 `reqwest` path crate 주입 | 실패를 확인했다(`keyscore-text가 금지 crate에 의존한다: reqwest`). 되돌렸다 |
| 5 | fmt/clippy/test/deny | 통과 |
| 6 | `core`에 `unsafe {}` 주입 | 컴파일 오류(`usage of an unsafe block`)를 확인했다. 되돌렸다 |

테스트 4에서 스크립트의 결함을 하나 찾아 고쳤다. `cargo tree`의 stderr 경고가 출력에 섞이면 「첫 줄 = 자기 자신」 가정이 깨져서, `core`가 자기 자신에게 의존한다는 오탐이 났다. 이제 stderr는 따로 받고, 자기 자신은 위치가 아니라 이름으로 걸러낸다. 고친 뒤 3·4를 다시 실행해 결과를 확인했다.

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test
- [x] deny: `advisories ok, bans ok, licenses ok, sources ok`. 허용 목록 중 아직 쓰이지 않은 라이선스 6개에 대한 `license-not-encountered` 경고만 있다.
- [x] 단계별 추가 게이트: `scripts/check-deps.sh` ok
- [ ] 원격 CI 초록: remote가 없어서 실행하지 못했다. CI의 각 단계는 로컬에서 같은 명령으로 실행했다.

## 알려진 한계

- GitHub Actions 워크플로는 실제로 한 번도 돌려 보지 않았다. 원격 저장소가 생기면 첫 실행 결과를 확인해야 한다. 특히 `macos-15` 러너 이름, `rust-toolchain.toml` 자동 설치, `taiki-e/install-action`의 cargo-deny 설치를 봐야 한다.
- 라이선스 허용 목록이 규약 그대로라서, S0-T2에서 proc-macro 계열(`unicode-ident`, Unicode-3.0)을 추가하는 순간 `cargo deny`가 실패한다. `docs/decisions/2026-10-11-license-allowlist.md`가 답을 받아야 한다.
- `LICENSE`의 저작권자는 「the KeyScore authors」로 적었다. 실제 이름이나 조직명이 필요하면 사람이 정해야 한다.

## 리뷰어에게

- `scripts/check-deps.sh`가 핵심이다. 정규식 `forbidden`이 놓치는 이름이 없는지 봐 달라. 예를 들어 `objc2`, `core-foundation`처럼 macOS 프레임워크 바인딩은 지금 금지하지 않는다. `-e normal,build`만 보고 dev 의존은 보지 않는 판단이 맞는지도 봐 달라.
- `deny.toml`의 `targets`를 macOS로 한정한 판단(REQ 8.3)과 `private = { ignore = true }`를 봐 달라.
