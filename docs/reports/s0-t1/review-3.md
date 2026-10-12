# 적대적 리뷰: S0-T1 워크스페이스와 CI 뼈대 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰어 역할, 독립 에이전트). 직접 수정하지 않았다.
- 대상 커밋: 9173ab4 (`main` 대비 `git log main..HEAD` 15개)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 소스 판정 `read -r dep _version source`가 레지스트리 crate를 소스로 오인하지 않는가(실제 레지스트리 crate를 normal·dev에 넣어 본다).
2. path 소스, git 소스, proc-macro 표시(`(proc-macro)`)가 붙은 path crate가 모두 잡히는가.
3. `--all-features`를 되돌린 사본에서 review-2 지적 1 사례가 실제로 실패로 바뀌는가(테스트가 그 수정을 정말 보호하는가).
4. 소스 검사를 끈 사본에서 review-2 제안 2 사례가 실패로 바뀌는가.
5. macOS 기본 bash 3.2에서 `< <(...)`, `${5:-}`가 동작하는가. CI는 `scripts/*.sh`를 직접 실행하므로 git 모드가 100755인가.
6. 공백이 든 경로에서 스크립트가 동작하는가.
7. 회귀 테스트의 fail 사례가 `reason`으로 사유를 확인하는가(엉뚱한 이유 통과 방지).
8. 자동 게이트 6종과 Task 범위 밖 변경 여부, 기존 테스트 삭제·`#[ignore]` 여부.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| optional 의존 + feature | 있음(`check-deps.test.sh` review-2 #1) | 아래 시도 1에서 수정을 되돌리면 실제로 실패함 |
| 허용 이름의 path 소스 | 있음(review-2 #2) | 시도 2에서 확인 |
| git 소스, proc-macro path 소스 | 없음 | 시도 4·5에서 수동 확인, 모두 잡힘. 오프라인 테스트로 넣기 어려워 제안으로 둠 |
| 레지스트리 crate 오탐 없음 | 없음(네트워크 필요) | 시도 6에서 수동 확인, 구현자도 수동 확인했다고 보고 |
| bash 3.2 | 없음 | 시도 7에서 확인 |
| fail 사례의 사유 확인 | 있음(`reason` 비교) | review-2 제안 3 해결 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | /tmp 사본에서 `--all-features ` 제거 후 `check-deps.test.sh` | 사례 「review-2 #1」이 기대 fail, 실제 ok로 FAIL. 10 passed, 1 failed. 테스트가 수정을 보호한다 | 없음 |
| 2 | 사본에서 `elif [ -n "$source" ]`를 `elif false`로 | 사례 「review-2 #2」가 FAIL. 10 passed, 1 failed. 보호됨 | 없음 |
| 3 | `serde`로 위장한 proc-macro path crate(`thiserror` 이름, `proc-macro = true`)를 core에 추가 | 못 깨뜨림. `레지스트리 crate가 아니다: (proc-macro) (/tmp/r3/pm)`로 실패(표시 제거 sed가 ` (proc-macro)`는 줄 끝에서만 지우는데 path 뒤에 붙어 있어 소스로 남아 잡힘) | 없음 |
| 4 | 허용 이름 `thiserror`를 git 소스(`file://`)로 | 못 깨뜨림. `레지스트리 crate가 아니다: (file:///tmp/r3/gitdep#9d48d94d)` | 없음 |
| 5 | 공백이 든 경로(`/tmp/r3/sp ace/ws`)에서 `check-deps.sh` | 못 깨뜨림. ok | 없음 |
| 6 | 레지스트리의 실제 `thiserror`·`proptest`(normal)와 `insta`(dev)를 core에 추가 | 못 깨뜨림. 오탐 없이 `check-deps: ok`. 소스 판정이 레지스트리 crate를 오인하지 않는다 | 없음 |
| 7 | `/bin/bash`(3.2.57)로 `check-deps.sh`와 `check-deps.test.sh` 실행 | 못 깨뜨림. 11/11 통과 | 없음 |
| 8 | `git ls-files -s scripts` 모드 확인 | 두 스크립트 모두 100755. CI의 직접 실행이 막히지 않는다 | 없음 |

실험은 모두 /tmp/r3 사본에서 했고 저장소 파일은 건드리지 않았다. 실험 뒤 `git status`는 이 리뷰 문서 외에 깨끗하다(`Cargo.lock` 변경 없음).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| core는 내부 의존 0 | 아니오. `Cargo.toml`에 의존이 없고 `check-deps.sh`가 dev·build·target·feature까지 본다 | 시도 1~4 |
| 의존 방향 `app → ai → text → core`, `app → midi → core` | 아니오. 정확히 일치하는 집합 비교 | `scripts/check-deps.sh:84-91` |
| core·text에 GPUI·CoreMIDI·네트워크 없음 | 허용 목록(직접) + 금지 정규식(전이)로 막힘. 목록 밖 신규 네트워크 crate가 허용된 crate의 전이 의존으로 들어오는 경우만 구멍(제안 1) | `scripts/check-deps.sh:73,121-131` |
| `core`·`text`·`ai` `#![deny(unsafe_code)]` | 충족(소스 확인). `midi`·`app`은 함수 단위 허용 예정 | `crates/*/src` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 9.6 저장소 구조(다섯 crate, 의존 방향, core·text의 GPUI·CoreMIDI·네트워크 비의존) | 충족 | `Cargo.toml`, `crates/*/Cargo.toml`, `scripts/check-deps.sh` |
| REQ 8.3 CI(macOS 러너) | 충족(원격 첫 실행은 보고서 「알려진 한계」로 이월, 출구 보고서 항목) | `.github/workflows/ci.yml:13-15` |
| REQ 라이선스 BSD-3-Clause | 충족 | `Cargo.toml` `license`, `LICENSE`, `deny.toml` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(코드 없음).
- 제품 코드의 `unwrap`/`expect`: 없음.
- 에러가 삼켜지는 곳: `grep ... || true`는 매치 없음을 정상으로 보는 용도이며, 직전에 첫 줄 sanity 검사가 있어 빈 출력으로 통과할 수 없다. `cargo tree` 실패는 `fail=1`로 전파된다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 허용 목록 crate의 전이 의존에서 목록 밖 신규 네트워크·UI crate는 닫힌 금지 정규식에 없으면 통과한다. 직접 의존은 허용 목록, 허용 목록 crate(serde 등)가 평소 네트워크를 끌어오지는 않으므로 S0-T1 범위에선 충분하다. 새 외부 의존이 들어가는 S0-T2·T6 PR에서 `cargo tree` 결과를 리뷰하는 규칙으로 갈음해도 된다. 거절 가능 | `scripts/check-deps.sh:73` |
| 2 | 제안 | git 소스·proc-macro path 소스·레지스트리 오탐 없음은 수동으로만 확인되었다. 오프라인 회귀 사례로 git(`file://` 로컬 저장소) 하나를 추가할 수 있으나 CI 오프라인 가정(`CARGO_NET_OFFLINE`)과 충돌하므로 거절 가능 | `scripts/check-deps.test.sh` |
| 3 | 제안 | 보고서 「리뷰 대응 (review-1)」 절의 자동 게이트에 `9/9`가 남아 있고 최신 수치는 review-2 절의 `11/11`이다. 시점 기록이므로 오류는 아니다. 출구 보고서에는 최신 수치를 쓸 것 | `docs/reports/s0-t1/report.md:70,97` |

차단·수정 항목은 없다. review-2의 지적 1(수정), 제안 2·3은 모두 해결되었고 되돌린 사본에서 실제로 실패로 바뀌는 것을 확인했다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

자동 게이트(fmt, clippy -D warnings, test, `cargo deny check`, `check-deps.sh`, `check-deps.test.sh` 11/11)를 이 리뷰에서 직접 실행해 통과를 확인했다. `cargo deny`는 쓰이지 않은 라이선스 허용에 대한 경고만 있다. 원격 CI 초록은 remote가 없어 확인하지 못했고, 병합 조건이 아니라 S0 출구 항목이다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(스크립트·설정을 읽은 뒤, 테스트·이전 리뷰·보고서 전에 정리했다. 단, `check-deps.test.sh`는 스크립트와 같은 호출에서 열었으므로 2번 표는 그 이후 대조다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(외부 의존 없음, `cargo deny` 통과)
- [x] 스타일 지적을 적지 않았다
