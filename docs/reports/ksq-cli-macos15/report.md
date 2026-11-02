# Task 보고: ksq-cli 출력 경로의 macOS 15 차이

- PR: 브랜치 `fix/ksq-cli-macos15-output-paths`(PR 1 `claude/handoff-continuation-efszpp` 위에 쌓음)
- 계획 파일: `docs/reports/ksq-cli-macos15/plan.md`
- 계획 대비 달라진 점: 디렉터리 꼴 검사를 처음에는 임시 파일을 만들기 전에 두었다. 그러면 쓰기 권한 없는 디렉터리 안의 `ro/new/`가 커널의 `EACCES`(일반 안내) 대신 `stat`의 `ENOENT`(특수 안내)로 바뀌어 `review_s0_t8_r11`이 Linux에서 새로 실패했다. 이름 바꾸기 바로 앞으로 옮겼다. 계획의 「디렉터리 꼴」은 마지막 구성 요소가 `.`·`..`인 경로(맨 `.`, `..` 포함)까지 넓혔다.

## 원인

원격 CI(macOS 15 러너)에서만 실패하던 `ksq-cli` 시험 대상 넷(`cli`, `review_s0_t8_r6`, `r9`, `r11`)의 원인 셋.

1. 문구: `/dev/<없는 이름>` 만들기가 사람 Mac에서는 `EPERM`, 러너에서는 `EACCES`다. 문구 분기는 `EPERM`만 특수 쪽으로 봐 일반 안내가 나왔다(`cli`, `r11`).
2. 결함: `-o <끊긴 링크>/`에서 러너의 `rename`이 성공해 종료 0이었다(`r9`). 끝이 `/`인 대상은 일반 파일이 될 수 없다. `txt2ksq`는 `keyscore-core`의 `ksq::write_file`로, `ksq2txt`는 `ksq-cli`의 `write_atomic`으로 쓰므로 둘 다 고쳤다. 앱의 저장도 `ksq::write_file`을 쓰므로 함께 막힌다.
3. 문구: `l3 -> l2/`, `l2 -> /dev/null` 사슬을 러너의 `stat`은 문자 장치로 푼다(사람 Mac은 `ENOTDIR`). 둘 다 종료 1이고 대상은 그대로다(`r6`).

## 구현 요약

- `crates/keyscore-core/src/ksq.rs`, `tools/ksq-cli/src/cli.rs`: 비공개 `directory_form`(끝이 `/`이거나 마지막 구성 요소가 `.`·`..`), `refuse_directory_form`(그 꼴이면 `stat`의 오류, 디렉터리면 `EISDIR`, 아니면 `ENOTDIR`). `write_then_rename`이 `rename` 바로 앞에서 부른다. 실패하면 호출자가 임시 파일을 지운다(기존 경로).
- `tools/ksq-cli/src/cli.rs` `output_error`: 경로 글자 판정(`special_place`)에 더해 부모 디렉터리가 devfs이면 `EACCES`(13)도 특수 쪽(`parent_on_devfs`, 리뷰 1 지적 1·리뷰 2 제안 1). 그 밖의 `EACCES`는 그대로 일반 안내.
- `tools/ksq-cli/tests/review_s0_t8_r6.rs`: `ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory`의 `dev` 경우만 「이 경로는 문자 장치」 거부도 받는다. 종료 1과 일반 파일 보존 단언은 그대로다.
- 공개 API 변화 없음. 새 의존 없음.

## 테스트 결과

이 컨테이너는 Linux라 앱 crate를 빌드하지 못하고 macOS 커널 동작(원인 2·3)을 재현하지 못한다. `ksq-cli`, `keyscore-core`, `keyscore-text`는 Linux에서 빌드·실행된다. 루트로 돌리면 권한 시험의 조건이 서지 않아 일반 사용자(`setpriv --reuid=65534`)로 돌렸다.

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `ksq::tests::directory_form_matches_trailing_slash_dot_and_dotdot` | 통과(뼈대 `todo!()`로 실패 확인) |
| 2 | `cli::tests::directory_form_matches_trailing_slash_dot_and_dotdot` | 통과(뼈대로 실패 확인) |
| 3 | `ksq::tests::write_file_to_dangling_link_with_trailing_slash_fails_and_keeps_link` | 통과(뼈대로 실패 확인. Linux 커널은 수정 전에도 거부하므로 macOS 15 확인은 CI) |
| 4 | `cli::output_errors_show_redirect_example_for_each_binary` | Linux 일반 사용자에서 수정 전 실패, 수정 뒤 통과 |
| 5 | `review_s0_t8_r11::binaries_uncreatable_output_wording_splits_by_special_place_and_errno` | 같음 |
| 6 | `review_s0_t8_r9::binaries_output_uncreatable_missing_paths_fail_without_creating` | macOS 15 CI로 확인 |
| 7 | `review_s0_t8_r6::ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory` | macOS 15 CI로 확인 |
| 8 | `review_macos15_r1::binaries_output_link_with_slashed_target_to_missing_fails_without_creating` | macOS 15 CI 통과(실행 41455906843) |
| 9 | `review_macos15_r1::binaries_output_dev_dotdot_into_read_only_dir_gets_general_hint` | Linux 일반 사용자에서 수정 전 실패·수정 뒤 통과, macOS 15 CI 통과(실행 41455906843) |
| 10 | `review_macos15_r2::binaries_relative_output_in_dev_cwd_same_hint_with_or_without_dot_slash` | Linux 일반 사용자에서 수정 전 실패·수정 뒤 통과 |

macOS 15 CI 실행 41450337288(625fbc5)·41455906843(fe79322)에서 `ksq-cli` 시험 대상 넷(`cli`, `r6`, `r9`, `r11`)과 `keyscore-core` 시험이 모두 통과했다.

Linux 일반 사용자로 `ksq-cli`·`keyscore-text` 시험 바이너리 전부를 수정 전후로 돌려 실패 목록을 비교했다. 수정 뒤 사라진 실패 3(위 4·5와 Linux에서만 나던 `r6::ksq2txt_output_slash_and_directory_paths_exit_1_without_leftovers`), 새로 생긴 실패 0. 수정 전후 모두 남는 실패는 Linux devfs 차이(제품은 macOS 전용, REQ 8.3)와 일반 사용자의 `insta` 스냅샷 쓰기 권한 탓이다. `keyscore-core` 시험은 루트로 모두 통과하고, 루트에서만 실패하는 `ksq_write_file_failure_keeps_original`은 수정 전에도 같다(일반 사용자로는 통과).

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings: `keyscore-core`, `keyscore-text`, `ksq-cli`(Linux). 워크스페이스 전체는 PR CI
- [ ] test: 워크스페이스 전체는 PR CI(macOS 15, 결정 2026-11-02 A의 건너뛰기 목록 적용)
- [ ] deny: 이 컨테이너에 `cargo-deny`가 없어 PR CI. 의존 변화 없음
- [x] `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 16 passed, `scripts/bench-check.test.sh` 8 passed

## 알려진 한계

- 원인 2·3은 macOS 15에서만 재현된다. 이 PR의 CI가 첫 확인이다.
- 디렉터리 꼴 대상은 임시 파일을 만든 뒤 이름 바꾸기 앞에서 실패한다. 그 사이 임시 파일은 호출자가 지운다(기존 실패 경로와 같음).
- `ksq-cli`과 `keyscore-core`의 원자적 쓰기 도우미가 중복이다(계획 「범위 밖」).
- 문구 판정에서 EACCES는 경로 글자에 더해 부모 디렉터리의 장치 번호도 본다. ENOENT·ENOTDIR·EPERM은 지금처럼 경로 글자(`/dev`로 시작, 끝 `/`·`/.`)만 본다(S0-T8 리뷰 11 제안 1에서 문구 취향으로 둔 것). 그래서 `/dev/..`로 밖에 나간 경로의 EPERM(불변 디렉터리 등)은 특수 안내를 받는다(리뷰 2 제안 2, 기준부터 같은 동작).

## 리뷰어에게

- 문구 분기(`output_error`)에 `EACCES`를 더한 범위가 `/dev` 아래뿐인지, `/dev` 밖 `EACCES`(쓰기 권한 없는 디렉터리)가 일반 안내로 남는지.
- `refuse_directory_form`의 위치가 커널의 이전 오류를 가리지 않는지(쓰기 권한 없는 디렉터리, 없는 부모 디렉터리).
- 링크 대상 문자열 안의 끝 `/` 뒤가 없는 이름인 사슬(`l3 -> l2/`, `l2 -> 없음`)이 `resolve_symlink` 뒤에도 남아 막히는지(`l2 -> 일반 파일`은 출력 확인이 먼저 막는다).
- 리뷰어는 이 Linux 컨테이너에서 앱 crate를 빌드할 수 없다. `ksq-cli`·`keyscore-core` 시험은 일반 사용자로 돌린다(루트면 `/dev` 쓰기가 성공한다).

## 리뷰 대응

### 리뷰 1 (`review-1.md`, 수정 후 재리뷰)

macOS 15 CI(실행 41450337288)에서 `ksq-cli` 시험 대상 넷과 코어 시험이 모두 통과한 것을 리뷰어도 확인했다. 리뷰어의 재현 시험 `tools/ksq-cli/tests/review_macos15_r1.rs`를 리뷰 기록과 함께 커밋했다.

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 수정 | 받아들임. EACCES의 특수 판정을 경로 글자 대신 부모 디렉터리가 devfs인지(`parent_on_devfs`, `on_devfs`와 같은 장치 번호 비교)로 한다. 재현 시험이 수정 전 실패, 수정 뒤 통과(Linux 일반 사용자). `/dev/<새 이름>`·`/dev/<새 이름>/`의 부모는 `/dev`라 특수 안내 그대로 | 3a0077c |
| 2 | 수정 | 받아들임. 리뷰어의 시험 `binaries_output_link_with_slashed_target_to_missing_fails_without_creating`을 그대로 담았고 macOS 15 CI(실행 41455906843)에서 통과했다. 계획·보고서의 「`l2 -> 일반 파일`도 같은 길」을 고쳤다(그 경우는 출력 확인이 먼저 막음) | 9fbb6dc |
| 3 | 제안 | 받아들임. 코어 주석을 관찰한 사실(「종료 0, 무엇이 어디에 생겼는지는 보지 못함」)로 고쳤다. 커밋 173bfeb 본문은 고쳐 쓰지 않고 이 표로 바로잡는다 | 885096b |
| 4 | 제안 | 받아들임. 계획의 REQ를 5.14·9.5로 고쳤다. 이 브랜치의 앞선 커밋 본문 `REQ: S0-T8`은 그대로 두고, 대응 커밋부터 REQ ID를 적는다 | 9fbb6dc |
| 5 | 제안 | 받아들임. `review_s0_t8_r6` 사슬 시험 문서에서 사라진 `Slash` 판정 언급을 뺐다. 시험 이름은 그대로(사람 Mac에서는 여전히 ENOTDIR) | 78d995d |
| 6 | 제안 | 거절. 실패 때 남는 임시 디렉터리는 원인 확인에 도움이 되고, pid 이름이라 다음 실행과 겹치지 않는다 | — |

### 리뷰 2 (`review-2.md`, 병합 가능)

| # | 심각도 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 제안 | 받아들임. `parent_on_devfs`가 빈 부모를 `.`로 본다. 리뷰어 시험 `review_macos15_r2`가 수정 전 실패, 수정 뒤 통과(Linux 일반 사용자) | 1번 커밋 |
| 2 | 제안 | 거절. EPERM의 경로 글자 판정은 기준부터 같은 동작이고 「알려진 한계」에 적었다. 바꾸면 사람 Mac의 `/dev` 문구 경로가 달라져 재리뷰가 필요하다. 사람이 원하면 후속으로 연다 | — |
| 3 | 제안 | 받아들임. 주석·계획·보고서를 「경로 글자에 더해 부모 장치 번호」로 고쳤다 | 1번 커밋 |
| 4 | 제안 | 받아들임. 시험 결과 표에 8–10과 CI 실행 번호를 적었다 | 1번 커밋 |
