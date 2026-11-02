# Task 계획: ksq-cli 출력 경로의 macOS 15 차이

- 브랜치: `fix/ksq-cli-macos15-output-paths`(사람이 이름과 새 브랜치를 허락, 2026-11-02). PR 1(`claude/handoff-continuation-efszpp`) 위에 쌓아, PR 1의 CI 건너뛰기 목록과 함께 녹색을 확인한다
- 관련 REQ: 5.14(계이름 텍스트 가져오기·내보내기), 9.5(`.ksq` 형식, 원자적 쓰기). Task S0-T8 `ksq-cli`(리뷰 1 제안 4로 고침. 처음에 9.2로 잘못 적었다). 결정 `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`, `docs/decisions/2026-11-02-ci-runner-scope.md` A
- 선행 Task: S0-T8, PR 1(원격 CI 첫 실행)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

원격 CI(macOS 15 러너)에서 `ksq-cli` 시험 대상 넷(`cli`, `review_s0_t8_r6`, `r9`, `r11`)이 통과한다. 사람 Mac에서 통과하던 동작은 그대로다.

- 원인 1(문구): 러너에서 `/dev/<없는 이름>` 만들기가 `EPERM`이 아니라 `EACCES`를 돌려준다. `output_error`의 「특수 자리」 문구는 `/dev` 아래에서 `EACCES`도 특수 쪽으로 본다. `/dev` 밖의 `EACCES`(쓰기 권한 없는 디렉터리)는 지금처럼 일반 안내다.
- 원인 2(결함): 끝이 `/`인 경로가 끊긴 링크를 가리키면(`-o dl/`, `dl -> 없음`) 러너에서 이름 바꾸기가 성공해 종료 0이다. 끝이 `/`·`/.`·`/..`인 경로는 일반 파일을 가리킬 수 없으므로, 링크를 푼 뒤의 경로가 그 꼴이면 이름 바꾸기 전에 실패한다(임시 파일 만들기까지는 커널이 먼저 실패하게 둔다). `ksq-cli`의 `write_atomic`과 `keyscore-core`의 `ksq::write_file`(`txt2ksq`가 씀) 둘 다다. 오류는 커널이 그 경로에 `stat`으로 답한 것을 쓴다(없으면 그 오류, 있으면 디렉터리는 `EISDIR`, 아니면 `ENOTDIR`). 링크 대상 문자열 안의 끝 `/` 뒤가 없는 이름·끊긴 링크인 사슬(`l3 -> l2/`, `l2 -> 없음`, `ls -> newdir/`)도 링크를 푼 경로에 끝 `/`가 남아 같은 길로 막힌다. `l2 -> 일반 파일`은 이 검사에 닿기 전에 출력 확인(`guard_output`)의 `stat`이 ENOTDIR로 막는다(리뷰 1 지적 2로 고침).
- 원인 3(문구): `l3 -> l2/`, `l2 -> /dev/null` 사슬을 러너의 `stat`은 문자 장치로 푼다(사람 Mac은 `ENOTDIR`). 어느 쪽이든 종료 1이고 대상은 그대로다. 시험 `ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory`의 `dev` 경우는 두 답 중 하나를 받는다.

## 공개 인터페이스 초안

~~~rust
// 공개 API는 바뀌지 않는다. 비공개 도우미만 더한다.
// keyscore-core/src/ksq.rs, ksq-cli/src/cli.rs 각각:
fn directory_form(path: &Path) -> bool; // 끝이 `/`·`/.`·`/..`인가
fn refuse_directory_form(path: &Path) -> std::io::Result<()>; // 그 꼴이면 커널의 답으로 실패
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `ksq::tests::directory_form_matches_trailing_slash_dot_and_dotdot` | 단위 | `a/`, `a/.`, `a/./`, `a/..`, `/`는 참, `a`, `a/b`, `a.`, `a/.b`는 거짓 |
| 2 | `cli::tests::directory_form_matches_trailing_slash_dot_and_dotdot` | 단위 | 같은 표 |
| 3 | `ksq::tests::write_file_to_dangling_link_with_trailing_slash_fails_and_keeps_link` | 단위 | `dl/` 쓰기가 실패하고 링크·디렉터리 내용이 그대로(러너의 성공을 막음. Linux에서는 수정 전에도 통과할 수 있어 macOS 15 CI로 확인) |
| 4 | 기존 `cli::output_errors_show_redirect_example_for_each_binary` | 통합 | `/dev/<없는 이름>` 특수 안내. Linux 일반 사용자에서 수정 전 실패 확인 |
| 5 | 기존 `review_s0_t8_r11::binaries_uncreatable_output_wording_splits_by_special_place_and_errno` | 통합 | 같은 문구 분기, `/dev` 밖 `EACCES`는 일반 안내 그대로 |
| 6 | 기존 `review_s0_t8_r9::binaries_output_uncreatable_missing_paths_fail_without_creating` | 통합 | `dl/` 종료 1, 링크 그대로(러너에서 실패하던 것) |
| 7 | 기존 `review_s0_t8_r6::ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory` | 통합 | `dev`는 `Not a directory` 또는 일반 파일 아님 거부, `reg`는 `Not a directory`. 단언만 넓힘 |
| 8 | `review_macos15_r1::binaries_output_link_with_slashed_target_to_missing_fails_without_creating`(리뷰 1) | 통합 | 링크 대상 끝 `/` 뒤가 없는 이름인 사슬. 이 검사만 막는 경우(macOS 15 CI로 확인) |
| 9 | `review_macos15_r1::binaries_output_dev_dotdot_into_read_only_dir_gets_general_hint`(리뷰 1) | 통합 | `/dev/..`로 시작하는 `/dev` 밖 EACCES는 일반 안내 |

## 불변식

- `-o`는 없는 경로와 일반 파일만 받는다(결정 2026-10-14). 실패하면 대상과 링크를 건드리지 않고 임시 파일을 남기지 않는다.
- 문구 분기는 경로 글자와 커널 오류만 본다. 쓰기 동작은 문구 때문에 바뀌지 않는다.
- 커널의 경로 풀이를 흉내 내지 않는다. 끝 꼴 검사는 「일반 파일 이름이 될 수 없음」만 보고, 오류의 원인은 커널의 `stat` 답을 쓴다.

## 범위 밖

- 앱의 저장(`files.rs`)이 같은 `ksq::write_file`을 쓰므로 함께 고쳐지지만, 앱 쪽 시험은 더하지 않는다(파일 대화상자가 끝 `/` 경로를 내지 않는다).
- `ksq-cli`과 `keyscore-core`의 원자적 쓰기 도우미 중복(`resolve_symlink`, `temp_path`, `write_then_rename`)은 합치지 않는다.
- Linux에서만 실패하는 `ksq-cli` 시험(devfs 차이)은 다루지 않는다. 제품은 macOS 전용이다(REQ 8.3).

## 리스크·결정 요청

- 이 컨테이너는 Linux라 macOS 15의 커널 동작(원인 2·3)을 재현하지 못한다. 시험 6·7은 PR CI로만 확인된다.
- 문구 분기에 `EACCES`를 더하는 것은 결정 2026-10-14의 「특수 자리」 뜻(`/dev` 아래는 특수 파일 자리)을 바꾸지 않고 커널 판 차이를 메운다고 본다. 결정 요청 없음.
