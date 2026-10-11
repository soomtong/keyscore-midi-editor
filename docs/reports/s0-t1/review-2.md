# 적대적 리뷰: S0-T1 워크스페이스와 CI 뼈대 (리뷰 2회차)

- 리뷰어 세션: 구현 세션 및 1회차 리뷰 세션과 분리된 별도 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 3135624 (브랜치 `s0/t1-workspace-ci`, `main` 대비 11커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `ci.yml`, `deny.toml`만 읽고, review-1.md·report.md를 열기 전에 적었다.

1. `cargo tree`가 보는 범위: 타깃(`--target all`), 의존 종류(`normal,build,dev`), 그리고 feature. optional 의존은 기본 feature에서 꺼져 있으면 그래프에 안 나온다. `--all-features` 또는 feature 합집합이 필요하지 않은가.
2. 허용 목록이 이름만 비교한다. 허용된 이름(`serde` 등)을 다른 소스(path, git)의 crate가 차지해도 통과하는가.
3. `[patch]`·`[replace]`가 허용 이름의 소스를 바꿔치기하는 경로.
4. 전이 금지 정규식이 놓치는 이름과, 허용 crate가 끌고 오는 전이 의존(`serde` → `serde_derive`는 UI·네트워크가 아니라 문제없음).
5. sanity 검사(첫 줄 == crate 이름)가 실제로 작동하는지, 그것을 지키는 테스트가 있는지.
6. 회귀 테스트가 「엉뚱한 이유로」 통과하는가. 테스트가 종료 코드만 보고 실패 메시지를 보지 않으므로, 스크립트의 한 검사를 지워도 다른 검사가 대신 잡아 주는 사례가 있는가.
7. 테스트의 격리: 임시 복사본에서만 도는가, 실제 저장소·`Cargo.lock`을 건드리는가, 오프라인에서 되는가.
8. CI에 새 단계가 들어갔을 때 순서와 실패 전파.
9. 직접 의존 파싱이 `cargo tree --depth 1` 출력의 첫 열에 의존하는데, 중복 제거 후 `dev` 전용과 `normal` 의존이 합쳐지는 것은 의도인가(의도다. 어느 종류든 금지).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| target-specific 의존 | 있음(A, F) | `--target all`을 지우면 A·F가 실패로 바뀜을 확인(깨뜨리기 2) |
| dev 의존 | 있음(D, E) | `-e`에서 dev를 지우면 D·E가 실패함을 확인(깨뜨리기 3) |
| renamed 의존 | 있음(C) | 패키지 이름으로 판정 |
| build 의존 | 있음(text의 ureq) | |
| 허용 목록 밖의 비금지 이름 | 없음 | 가짜 crate 이름이 `reqwest`·`ureq`라 금지 목록과 허용 목록이 둘 다 잡는다. 허용 목록 검사만 단독으로 지켜지지 않음 |
| optional + feature 의존 | 없음 | 스크립트에 구멍이 있음(깨뜨리기 1) |
| 허용 이름의 다른 소스 | 없음 | 깨뜨리기 4 |
| sanity 검사 | 없음 | 깨뜨리기 6 |
| 격리·오프라인 | 있음(구조상) | 깨뜨리기 7 |

## 3. 깨뜨리기 시도

모든 시도는 `/tmp/rv2` 아래 워크스페이스 복사본과 가짜 crate에서 했다. 실제 저장소 파일은 바꾸지 않았다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | core에 `reqwest = { path = 가짜, optional = true }` 추가 + `[features] net = ["dep:reqwest"]`. 또 keyscore-app이 `keyscore-core`의 `net` feature를 켜게 함 | 깨짐. `check-deps: ok`(exit 0). `cargo tree -p keyscore-app`은 reqwest를 보여 주고, `cargo tree -p keyscore-core --all-features`도 직접 의존으로 보여 준다. 스크립트가 기본 feature로만 트리를 본다. 1회차 「검사할 것」 3번에 optional/feature가 있었으나 재현·수정되지 않았다 | 없음(지적 1) |
| 2 | 스크립트에서 `--target all`만 제거하고 `check-deps.test.sh` 실행 | 테스트가 잡음. A, F가 「기대 fail, 실제 ok」. 테스트가 엉터리로 통과하지 않음 | 없음 |
| 3 | `-e normal,build,dev`를 `-e normal,build`로 바꾸고 테스트 실행 | 테스트가 잡음. D, E 실패 | 없음 |
| 4 | core에 `serde = { path = 가짜 }`(허용 이름, 로컬 소스) | 깨짐. exit 0. `cargo deny`의 sources도 path 소스는 막지 않는다. 가짜 serde가 reqwest를 끌고 오면 전이 금지 목록이 잡는 것도 확인(`package = "inner"` 변형은 허용 목록·금지 목록 둘 다에서 실패) | 없음(제안 2) |
| 5 | 패키지 이름을 `Reqwest`(대문자)로 | 해당 없음. cargo가 해석 단계에서 거부한다(이름은 대소문자를 구분해 찾는다). 우회 불가 | 없음 |
| 6 | sanity 검사 변경: `--prefix none`을 `--prefix depth`로 바꿔 출력 형식을 깨뜨림 | 스크립트는 exit 1로 실패하지만, 실패 이유는 sanity 메시지가 아니라 허용 목록·내부 의존 불일치(`1keyscore-core`)다. 즉 sanity 검사를 지켜 주는 테스트가 없다. 지적 5의 수정은 코드에만 있고 회귀로 남지 않았다 | 없음(제안 1) |
| 7 | 격리·오프라인: `check-deps.test.sh`는 `mktemp -d` 복사본에서만 돌고 `CARGO_NET_OFFLINE=true`를 건다. 실행 후 실제 저장소 `git status` 깨끗함. 가짜 crate는 모두 path라 레지스트리가 필요 없음 | 못 깨뜨림 | 없음 |
| 8 | 자동 게이트 6종 실행 | 전부 통과(5절 아래 참조) | 없음 |

실행한 게이트: `cargo fmt --all -- --check` 통과, `cargo clippy --workspace --all-targets -- -D warnings` 통과, `cargo test --workspace` 통과(테스트 0개), `cargo deny check`는 `advisories ok, bans ok, licenses ok, sources ok`(미사용 라이선스 경고만), `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 9/9.

1회차 지적 1~5 재현 확인: 1(타깃), 3(dev), 2(허용 목록과 renamed `ureq`), 4(회귀 테스트 존재, 위 2·3의 변형으로 유효성 확인), 5(코드 존재, 테스트 없음). 이 항목들은 해결되었다. 다만 지적 2의 취지(core가 정확히 허용 집합만 쓰는가)는 optional 의존 경로로 여전히 우회된다(지적 1).

## 4. 불변식 점검

이 Task에는 모델 불변식이 없다(제품 코드 없음). REQ 9.6의 의존 방향 불변식을 대신 점검한다.

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| core·text는 GPUI·CoreMIDI·네트워크에 의존하지 않는다 | 예, 게이트를 통과한 채로 | optional 의존 + feature(깨뜨리기 1). 다른 crate가 feature를 켜면 실제 빌드에 들어간다 |
| 내부 의존 방향 | 못 깨뜨림 | normal, build, dev, target-specific 모두 검사됨 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 9.6 저장소 구조: 다섯 크레이트, 의존 방향 | 충족 | `Cargo.toml` members, `scripts/check-deps.sh:55-62` |
| REQ 9.6 `core`·`text`는 GPUI·CoreMIDI·네트워크에 의존하지 않음 | 부분 충족(=미충족) | optional + feature 경로가 게이트를 통과함. `scripts/check-deps.sh:25` |
| REQ 8.3 macOS 전용 | 충족 | `ci.yml` macos-15, `deny.toml` targets |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(제품 코드 없음)
- 제품 코드의 `unwrap`/`expect`: 없음
- 에러가 삼켜지는 곳: `check-deps.sh`의 `|| true`는 `grep` 무일치용이며 정당하다. 그 외 없음

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | optional 의존과 feature로 들어온 금지·목록 밖 crate가 통과한다. `cargo tree`에 `--all-features`가 없다(`core`에 `reqwest` optional + feature를 두고 app이 켜도 `check-deps: ok`). 인자에 `--all-features`를 추가하면 `core`의 직접 의존으로 `reqwest`가 나타남을 확인했다. 같은 사례를 `check-deps.test.sh`에 추가할 것(예: core에 optional `reqwest`와 feature 선언) | `scripts/check-deps.sh:25` |
| 2 | 제안 | 허용 목록이 이름만 보므로 허용 이름의 path·git 소스 crate가 통과한다(`serde = { path = ... }`). `cargo deny`도 path 소스는 막지 않는다. `core`·`text`의 외부 의존에 `path` 소스가 없음을 확인하는 검사(트리의 `{p}` 출력에 경로 표시가 붙는 것을 이용)는 이 Task의 범위에 비해 과할 수 있어 제안으로 둔다. 거절 가능 | `scripts/check-deps.sh:76-82` |
| 3 | 제안 | sanity 검사에 회귀 테스트가 없다. 출력 형식을 깨뜨린 변형에서 스크립트는 다른 이유로 실패해 지금은 안전하지만, 그 보호가 약해지면 알 수 없다. 테스트가 종료 코드만 보지 않고 기대 메시지 일부도 비교하면 「엉뚱한 이유로 통과」를 함께 막는다 | `scripts/check-deps.test.sh:41-52` |
| 4 | 제안 | 원격 CI 첫 실행, 지적 6~8의 구현자 답변(받음 2, 거절 1)은 타당하다. 6은 stage-0-exit.md 체크리스트에 실제로 들어갔는지 S0 종료 때 확인하면 된다. 7은 보안 권고를 막는 것이 의도이고 `ignore`에 이유를 적는 절차가 있어 거절을 인정한다. 8은 S8 이월이 conventions 문구와 맞는다 | `docs/reports/s0-t1/report.md` |

지적 4는 조치 요구가 아니라 6~8 대응 평가의 기록이다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

근거: 1회차 지적 1~5는 해결되었고 회귀 테스트도 엉터리 없이 작동한다(깨뜨리기 2, 3). 그러나 이 Task의 핵심 판정 기준인 「core·text가 GPUI·네트워크에 의존하지 않는다」가 optional 의존 + feature 경로로 게이트를 통과한 채 깨진다(지적 1, 수정). 수정은 `--all-features` 한 인자와 테스트 사례 하나로 끝나는 작은 범위다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다 (이 세션에서 추가·수정한 파일은 이 문서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다 (외부 의존 없음)
- [x] 스타일 지적을 적지 않았다
