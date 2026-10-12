# 적대적 리뷰: S0-T1 워크스페이스와 CI 뼈대 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 분리된 별도 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 5ef315b (브랜치 `s0/t1-workspace-ci`, `main` 대비 6커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

보고서·계획을 읽기 전에 코드(Cargo.toml, scripts/check-deps.sh, deny.toml, ci.yml)만 보고 적었다.

1. 내부 의존이 REQ 9.6 방향과 정확히 같은가. 역방향, 건너뛴 방향(`text → midi`), 빠진 의존이 모두 실패하는가.
2. `core`·`text`에 금지 crate(GPUI·CoreMIDI·네트워크)가 전이적으로도 못 들어오는가. 금지 목록이 닫혀 있는가, 열려 있는가.
3. 의존을 숨기는 경로가 스크립트를 통과하는가: renamed dependency, target-specific 의존, optional/feature 의존, build-dependencies, dev-dependencies.
4. `cargo tree` 기본 타깃이 호스트 하나뿐인데 `deny.toml`은 macOS 두 타깃이다. 스크립트와 deny의 검사 범위가 어긋나지 않는가.
5. `deny.toml`의 허용 목록이 conventions와 같은가, `private.ignore`·`allow-wildcard-paths`가 외부 crate 검사를 약화시키지 않는가.
6. `#![deny(unsafe_code)]`가 `core`·`text`·`ai`에 있고 `midi`·`app`에는 없다는 conventions와 일치하는가. 하위 모듈의 `#[allow]`로 우회되는가.
7. CI가 로컬 게이트와 같은 명령을 같은 순서로 실행하는가. 툴체인 고정, 캐시, cargo-deny 설치 실패 시 동작.
8. 위 검사들을 회귀로 남기는 자동 테스트가 저장소에 있는가.
9. LICENSE 본문, NOTICE의 REQ 8.4 요구(gpui-kit·Ely 고지 자리), crate별 README(conventions 71행).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 내부 의존 방향 | 있음 (`scripts/check-deps.sh` 직접 의존 집합 비교) | 역방향·건너뜀·누락 모두 잡음. 보고서는 `text → midi` 주입으로 확인 |
| 2 금지 crate 전이 의존 | 부분 | 금지 목록이 닫힌 정규식이라 목록 밖 이름은 통과(깨뜨리기 C) |
| 3 숨기는 경로 | 없음 | renamed는 `cargo tree`가 패키지명을 써서 잡히지만, target-specific·dev는 통과(A, D, E, F) |
| 4 타깃 범위 | 없음 | 스크립트는 호스트 타깃만 봄 |
| 5 deny 정책 | `cargo deny check` 통과만 | 허용 목록은 conventions와 일치. 정책 위반 주입 테스트는 없음 |
| 6 unsafe 거부 | 수동 실험 1회 (보고서 6번) | 자동 테스트 아님. 회귀로 남지 않음 |
| 7 CI | 없음 | remote가 없어 한 번도 실행되지 않음. 보고서가 명시 |
| 8 회귀 자동화 | 없음 | 계획의 테스트 3·4·6은 「수동 실험」이고 스크립트로 남지 않았다 |
| 9 문서 | 해당 없음 | LICENSE는 BSD-3-Clause 표준 문구와 일치, NOTICE·README 있음 |

## 3. 깨뜨리기 시도

모든 실험은 작업 트리에서 `crates/*/Cargo.toml`을 임시로 고치고 `scripts/check-deps.sh`를 실행한 뒤 `git checkout -- .`로 되돌렸다. 가짜 crate는 `/tmp/fake/{reqwest,ureq}`(빈 lib, 실험 후 삭제). 종료 후 `git status --short`는 비어 있다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| A | `core`에 `[target.'cfg(windows)'.dependencies] reqwest = { path = "/tmp/fake/reqwest" }` | 깨짐. `check-deps: ok` | 없음(아래 재현 절차) |
| B | `core`에 `reqwest` 직접 의존(대조군) | 못 깨뜨림. core·text 둘 다 금지 crate로 실패 | 없음 |
| C | `core`에 `foo = { package = "ureq", path = "/tmp/fake/ureq" }` (renamed + 금지 목록에 없는 네트워크 crate) | 깨짐. `check-deps: ok`. renamed 자체는 `cargo tree`가 패키지명을 출력하므로 문제 아님. 목록이 열린 것이 원인 | 없음 |
| D | `core`에 `[dev-dependencies] keyscore-text.workspace = true` (dev 사이클 `core → text`) | 깨짐. `check-deps: ok`. 코드상 `core`가 `text`의 타입을 테스트에서 쓸 수 있게 되어 방향 규칙이 무너짐 | 없음 |
| E | `core`에 `[dev-dependencies] reqwest` | 깨짐. `check-deps: ok` | 없음 |
| F | `text`에 `[target.'cfg(windows)'.dependencies] keyscore-midi.workspace = true` (역방향·건너뛴 내부 의존) | 깨짐. `check-deps: ok` | 없음 |
| G | 자동 게이트 5종 실행 | 못 깨뜨림 (아래) | 없음 |

재현 명령(A, 저장소 루트에서):

~~~
mkdir -p /tmp/fake/reqwest/src && : > /tmp/fake/reqwest/src/lib.rs
printf '[package]\nname="reqwest"\nversion="0.1.0"\nedition="2021"\n' > /tmp/fake/reqwest/Cargo.toml
printf "\n[target.'cfg(windows)'.dependencies]\nreqwest = { path = \"/tmp/fake/reqwest\" }\n" >> crates/keyscore-core/Cargo.toml
scripts/check-deps.sh      # 기대: 실패. 실제: "check-deps: ok"
git checkout -- . && rm -rf /tmp/fake
~~~

C는 reqwest 대신 `foo = { package = "ureq", path = "/tmp/fake/ureq" }`, D는 `[dev-dependencies]\nkeyscore-text.workspace = true`, F는 위 A와 같은 방식으로 `crates/keyscore-text/Cargo.toml`에 `keyscore-midi.workspace = true`를 target-specific 표에 넣으면 재현된다.

원인:
- `tree_of`가 `cargo tree -e normal,build`만 호출하고 `--target`을 주지 않아 호스트 타깃(aarch64-apple-darwin) 밖의 cfg 의존이 그래프에서 빠진다(A, F). `--target all`을 쓰면 잡힌다.
- dev 의존은 의도적으로 제외했지만(스크립트 주석 「제품 그래프가 아니다」), `core`·`text`의 dev 의존도 REQ 9.6의 「속성 기반 테스트를 UI 없이 돌린다」를 깨뜨리고, 내부 dev 사이클은 방향 규칙을 사실상 우회한다(D, E).
- `forbidden` 정규식은 닫힌 목록이다. `ureq`, `hyper-util`, `h2`, `curl`, `isahc`, `surf`, `async-std`, `smol`, `native-tls`, `rustls`, `openssl`, `security-framework`, `objc2*`, `core-foundation*`, `cocoa`, `metal`이 빠져 있다. 네트워크 금지는 REQ 9.6의 문구이므로 목록보다 소켓/HTTP 계열을 포괄하는 방식(또는 허용 목록: `core`는 외부 의존이 serde·serde_json뿐이어야 함 — REQ 9.6 표의 「의존」 열)이 맞다.

자동 게이트 실행 결과(대상 5ef315b, 로컬):

| 게이트 | 결과 |
| --- | --- |
| `cargo fmt --all -- --check` | 통과 (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 통과 |
| `cargo test --workspace` | 통과 (테스트 0개) |
| `cargo deny check` | 통과 (`advisories ok, bans ok, licenses ok, sources ok`, `Zlib` 미사용 경고만) |
| `scripts/check-deps.sh` | `check-deps: ok` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 의존 방향 `app → ai → text → core`, `app → midi → core` | 게이트 우회 가능 | F(target-specific 내부 의존), D(dev 사이클). 단 직접 `[dependencies]`에 넣는 경우는 막힘 |
| `core`·`text`는 GPUI·CoreMIDI·네트워크 무의존 | 게이트 우회 가능 | A, C, E |
| `core` 외부 의존은 serde, serde_json뿐 (REQ 9.6) | 미검사 | 어떤 외부 crate든 이름이 금지 목록에만 없으면 통과 |
| `core`·`text`·`ai`의 `unsafe` 금지 | 크레이트 수준에서는 막힘 | `#![deny(unsafe_code)]`는 하위 `#[allow(unsafe_code)]`로 풀 수 있으나 conventions가 허용하는 방식과 같고 리뷰에서 잡는 항목. 자동 검사는 없음 |

pub 필드·모델 불변식은 이 Task에 코드가 없어 해당 없음.

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 2 라이선스 BSD-3-Clause | 충족 | `LICENSE`, `Cargo.toml:19`(`license = "BSD-3-Clause"`) |
| REQ 2 CI: GitHub Actions macOS 러너 빌드·테스트 | 충족(서명 빌드는 S8 범위) | `.github/workflows/ci.yml:15` |
| REQ 8.3 macOS 전용 | 충족 | `deny.toml:7`, `ci.yml:15` |
| REQ 8.4 허용적 라이선스만, NOTICE 고지 | 충족 | `deny.toml:16-23`, `NOTICE` |
| REQ 9.1 gpui-kit 스냅샷만 | 해당 없음(의존 없음) | — |
| REQ 9.6 다섯 crate와 방향 | 충족 | `Cargo.toml:2-8`, `crates/*/Cargo.toml` |
| REQ 9.6 「`core`·`text`는 GPUI·CoreMIDI·네트워크에 의존하지 않는다」를 지키는 게이트 | 부분 충족 = 미충족 | 깨뜨리기 A, C, E |
| REQ 9.6 「core 의존: serde, serde_json」 | 해당 없음 (S0-T2에서 추가) | — |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 코드 없음.
- 제품 코드의 `unwrap`/`expect`: 없음.
- 에러가 삼켜지는 곳: `scripts/check-deps.sh`의 `|| true`는 grep 무매칭 때문이며 `cargo tree` 실패는 `tree_of`가 별도로 잡는다. 다만 `grep -E '^keyscore-'` 파이프에서 `cargo tree` 출력 형식이 바뀌어 모든 줄이 걸러지면 `actual`이 빈 문자열이 되어 `core`(기대값 빈 문자열)는 항상 통과한다. 금지 crate 검사도 같은 이유로 출력 형식 변화에 조용히 통과한다. 「tree 출력이 비어 있지 않고 crate 자신을 포함한다」는 sanity 검사가 없다.
- CI: `rustup show active-toolchain`은 툴체인이 없으면 자동 설치를 유발하는 부수효과에 의존한다. 별도 실패 조건이 없다(제안).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `cargo tree`에 `--target`이 없어 target-specific 의존(금지 crate, 역방향 내부 의존)이 검사에서 빠진다. `--target all` 사용 필요. 깨뜨리기 A, F | `scripts/check-deps.sh:15` |
| 2 | 수정 | 금지 목록이 닫혀 있어 `ureq`·`hyper-util`·`h2`·`rustls`·`openssl`·`curl` 등이 통과한다. 이 Task의 리뷰 초점(`core`가 아무것도 의존하지 않는가)이므로, `core`는 외부 의존 허용 집합(지금은 빈 집합, S0-T2에서 serde·serde_json)을 정확히 비교하는 방식을 권한다. 깨뜨리기 C | `scripts/check-deps.sh:53` |
| 3 | 수정 | dev-dependencies가 전혀 검사되지 않는다. `core`·`text`의 dev 의존으로 금지 crate나 상위 내부 crate(`core → text` 사이클)가 들어온다. 최소한 내부 crate dev 의존과 금지 crate는 `-e dev`로 검사해야 한다. 깨뜨리기 D, E | `scripts/check-deps.sh:15` |
| 4 | 수정 | 계획의 테스트 3·4·6은 수동 실험이었고 회귀로 남지 않았다. 위반 주입이 실제로 실패하는지를 확인하는 `scripts/check-deps.test.sh`(임시 복사본에서 주입 후 실패 기대)가 없어, 스크립트가 조용히 약해져도 알 수 없다. 이 리뷰의 A, C~F 재현을 그 스크립트의 케이스로 넣을 것 | `docs/reports/s0-t1/plan.md`(테스트 목록 3·4·6) |
| 5 | 수정 | `cargo tree` 출력이 비거나 형식이 바뀌면 조용히 통과한다. 출력에 crate 자신이 있는지 확인하는 sanity 검사 필요 | `scripts/check-deps.sh:41` |
| 6 | 제안 | CI가 한 번도 실행되지 않았다. 수용 가능하지만(remote 없음), 병합 전 `actionlint` 등 정적 검사나 첫 실행 확인 항목을 `docs/reports/stage-0-exit.md` 체크리스트에 명시할 것 | `.github/workflows/ci.yml` |
| 7 | 제안 | `cargo deny`의 advisories는 매 실행 때 DB를 받는다. 새 권고가 생기면 무관한 PR이 막힐 수 있다. 정책 결정이므로 거절 가능 | `deny.toml:12-13` |
| 8 | 제안 | conventions 36행의 「Intel 빌드는 CI에서만 확인」이 CI에 없다. `deny.toml`은 x86_64를 포함하지만 빌드는 aarch64 러너 하나. S8 범위로 미뤄도 되며 보고서에 명시 권장 | `.github/workflows/ci.yml:15` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의

근거: 이 Task의 리뷰 초점은 「`core`가 아무것도 의존하지 않는가」인데, 의존 방향·무의존을 지키는 유일한 자동 게이트인 `check-deps.sh`가 target-specific, dev, 목록 밖 crate 세 경로로 우회된다(지적 1-3). 직접 `[dependencies]` 위반과 워크스페이스 구조, fmt·clippy·test·deny 게이트, LICENSE·NOTICE·deny.toml은 문제없다. 수정 범위는 스크립트와 그 회귀 스크립트에 한정되며 범위 재정의는 필요 없다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다 (`main` 대비 27개 파일 모두 계획 범위)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다 (테스트 없음)
- [x] 새 의존성이 있으면 라이선스를 확인했다 (외부 의존 없음)
- [x] 스타일 지적을 적지 않았다
