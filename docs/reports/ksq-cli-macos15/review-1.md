# 적대적 리뷰: ksq-cli 출력 경로의 macOS 15 차이 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다
- 대상 커밋: `625fbc5`(브랜치 `fix/ksq-cli-macos15-output-paths`, 기준 `a1e85fd`). 코드 커밋 `173bfeb`, `e407db7`, `ec5270d`
- 실행 환경: Linux 컨테이너, 일반 사용자(`setpriv --reuid=65534`)로 실행. 수정 전 비교는 `a1e85fd`를 별도 worktree(`/tmp/ksq-review-before`)에서 빌드했다. macOS 15 커널 동작은 재현하지 못해 이 브랜치의 원격 CI(실행 41450337288, macOS 15)를 기다려 결과를 읽었다. fmt·clippy 통과. `keyscore-core` 단위 테스트와 `ksq-cli` 시험 대상 전부(`cli` 20, `r6` 11, `r9` 8, `r11` 4 등) 통과. 실행 전체는 `keyscore-midi`의 벽시계 타이밍 시험 둘(`review_s1_t4_r4::swap_after_pass_capped_right_before_clock_sends_events_and_clocks_once`, `scheduler::play_sends_events_at_scheduled_host_times`)로 실패했는데, 이 변경과 무관하고 결정 2026-11-02 A의 분류 1에 드는 것이다

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `refuse_directory_form`을 `rename` 바로 앞에 둔 탓에, 전에는 맞던 오류(쓰기 권한 없는 디렉터리의 EACCES, 없는 부모의 ENOENT, ELOOP, `-o .`·`-o ..`)의 문구·종료 코드가 바뀌는가.
2. 디렉터리 꼴 대상이 여전히 써지는 길이 있는가. 링크 대상 문자열 안의 끝 `/`(`l -> newdir/`), `a/./`, `a//`, 사슬 중간의 `l3 -> l2/`, `l2 -> 없는 이름`, `resolve_symlink`가 끝 `/`를 잃는 경우.
3. 실패할 때 임시 파일이 남는가. `temp_path`가 `dl/`, `a/.`, `a/..`에서 어느 디렉터리에 임시 파일을 만드는가.
4. `/dev` 아래 EACCES를 특수 쪽으로 보는 규칙이 너무 넓은가. `Path::starts_with`는 구성 요소 단위라 `/devices/x`는 빠지지만 `/dev/../tmp/ro/x`는 들어온다.
5. `keyscore-core`의 `ksq::write_file`은 앱 저장(`files.rs`)도 쓴다. 사용자가 볼 차이가 있는가(`PermissionDenied` 매핑 등).
6. 넓힌 `review_s0_t8_r6` 단언이 여전히 뜻이 있는가.
7. 새 테스트가 수정 전 코드에서 실패하는가(Linux에서는 커널이 먼저 거부하므로 실패하지 않을 것 같다).
8. `clippy::pedantic`, fmt.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 이전 오류의 문구 보존 | 일부 있음 | `review_s0_t8_r11` 문구 분기 표가 `ro/new`, `ro/new/`, `nodir/x`를 다룬다. `/dev/..`로 시작하는 EACCES는 없다 |
| 2. 디렉터리 꼴 대상 거부 | 일부 있음 | `dl/`(r9, 코어 단위 테스트), `new/`·`new/.`(r9, r6). 링크 대상 문자열이 `/`로 끝나고 그 뒤가 없는 이름·끊긴 링크인 경우는 없다. r6 사슬 테스트의 `l2`는 `/dev/null`·있는 일반 파일이라 새 코드가 아니라 `guard_output`이 막는다 |
| 3. 임시 파일 정리 | 있음 | r6·r9의 `temp_leftovers`, 코어 테스트의 디렉터리 이름 목록 |
| 4. `/dev` 밖 EACCES 문구 | 일부 있음 | `/dev/..` 철자는 r11 `..._misleading_path_shape_...`가 ENOENT만 다루고 문구는 단언하지 않는다 |
| 5. 앱 저장 | 없음 | 계획 「범위 밖」. 코드로 확인(아래 3번 표 #9) |
| 6. 넓힌 r6 단언 | 해당 | 종료 1, 일반 파일 보존, 임시 파일 없음 단언이 그대로라 뜻은 남는다 |
| 7. 수정 전 실패 확인 | 부분 | 코어 단위 테스트는 Linux에서 수정 전에도 통과한다(보고서가 밝힘). macOS 15 확인은 CI뿐 |

## 3. 깨뜨리기 시도

모두 일반 사용자로 실제 실행했다. 「수정 전」은 `a1e85fd` 빌드, 「수정 후」는 `625fbc5` 빌드다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `ksq-cli`·`keyscore-text` 시험 바이너리 전부를 수정 전후로 돌려 실패 목록 비교 | 못 깨뜨림. 사라진 실패 3(`cli::output_errors_show_redirect_example_for_each_binary`, `r11::binaries_uncreatable_output_wording_splits_by_special_place_and_errno`, `r6::ksq2txt_output_slash_and_directory_paths_exit_1_without_leftovers`), 새 실패 0. 보고서와 같다 | — |
| 2 | 두 바이너리에 `-o dl/`, `dl/.`, `dl//`, `new//`, `a/./`, `rw/new/.`, `x.ksqz/` | 못 깨뜨림. 모두 종료 1, 아무것도 생기지 않고 링크 그대로, 임시 파일 없음. Linux 커널 답이 ENOTDIR에서 `stat`의 ENOENT로 바뀌었고 macOS 15가 `new/`에 내던 「No such file」(r6 기대)과 같아졌다 | — |
| 3 | 링크 대상 문자열이 `/`로 끝남: `l3 -> l2/`·`l2 -> missing`, `ls -> newdir/`, `labs -> /abs/nope/`, `dlslash -> dl/`, `ld -> ./x/.`, `l4 -> l2//` | 못 깨뜨림(Linux). 모두 종료 1, 아무것도 생기지 않음. `resolve_symlink`는 `parent.join(target)`이라 끝 `/`를 보존하고, `lstat("dir/l2/")`는 끝 `/` 때문에 링크를 따라가 ENOENT로 멈추므로 결과 경로가 디렉터리 꼴로 남는다. 다만 Linux에서는 수정 전에도 커널이 막으므로 이 시도는 새 코드를 검증하지 못한다(지적 2) | `tools/ksq-cli/tests/review_macos15_r1.rs::binaries_output_link_with_slashed_target_to_missing_fails_without_creating`(수정 전후 모두 통과) |
| 4 | `-o ro/new`, `ro/new/`(쓰기 권한 없는 디렉터리), `missing/new/`(없는 부모) | 못 깨뜨림. 임시 파일 만들기에서 먼저 실패해 수정 전과 같은 오류·문구(EACCES 일반 안내, ENOENT 특수 안내) | — |
| 5 | `-o .`, `-o ..`, `-o sub/..`, `-o /dev/fd/77` | 못 깨뜨림. `.`·`..`·`sub/..`는 `guard_output`이 디렉터리로 먼저 거부해 새 코드에 닿지 않는다. `/dev/fd/77`은 수정 전후 같다 | — |
| 6 | `-o /dev/ksq-adv-node`, `/dev/ksq-adv-node/`(Linux devfs는 EACCES) | 의도대로 바뀜. 일반 안내에서 특수 안내로 | — |
| 7 | `-o /dev/../<tmp>/ro/x`(실제로는 `/dev` 밖의 쓰기 권한 없는 디렉터리, EACCES) | 깨짐. 수정 전은 「다른 경로를 주거나」 일반 안내였는데 수정 후 「특수 파일에 쓰려면 -o 없이 실행하고 셸로 리다이렉트하라」가 나온다. 셸 리다이렉션 `> /dev/../<tmp>/ro/x`도 같은 EACCES로 실패하므로 안내가 틀리다. 새 주석(`cli.rs:302-304`)이 스스로 세운 기준(「`/dev` 밖의 EACCES는 … 일반 안내로 둔다」)과 어긋난다(지적 1) | `review_macos15_r1.rs::binaries_output_dev_dotdot_into_read_only_dir_gets_general_hint`(수정 전 통과, 수정 후 실패) |
| 8 | 코어 `ksq::tests` 루트·일반 사용자 실행, `directory_form` 표 | 못 깨뜨림. 10/10 통과. 일반 사용자에서 `ksq_snapshot_format`만 실패하는데 insta 스냅샷 쓰기 권한 탓으로 수정 전과 같다 | — |
| 9 | 앱 저장 `save_project`(`crates/keyscore-app/src/files.rs:71-87`)가 보는 차이 | 못 깨뜨림(코드 확인, 앱은 Linux에서 빌드 불가). 파일 대화상자는 끝 `/` 경로를 내지 않고, 디렉터리 꼴 경로는 링크 대상 문자열이 `/`로 끝날 때만 생긴다. 그때 오류 종류가 rename의 ENOTDIR·EISDIR에서 `stat`의 답으로 바뀔 뿐 `PermissionDenied` 매핑은 그대로다 | — |
| 10 | `cargo clippy -p keyscore-core -p ksq-cli --all-targets -- -D warnings`, `cargo fmt --all -- --check` | 못 깨뜨림. 경고 0, fmt 차이 없음. 원격 CI의 macOS clippy·fmt 단계도 통과 | — |
| 11 | 원격 CI(macOS 15, 실행 41450337288) 결과 읽기. 수정 전 실패하던 `cli::output_errors_show_redirect_example_for_each_binary`, `r6::ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory`, `r9::binaries_output_uncreatable_missing_paths_fail_without_creating`, `r11::binaries_uncreatable_output_wording_splits_by_special_place_and_errno`와 코어 `write_file_to_dangling_link_with_trailing_slash_fails_and_keeps_link` | 못 깨뜨림. 모두 통과. r9와 코어 테스트는 디렉터리 이름 목록까지 단언하므로 macOS 15의 `lstat("dl/")`가 링크를 따라가(ENOENT) `resolve_symlink`가 `dir/dl/`로 멈추고 새 검사가 막는다는 것이 확인됐다(`dl-target`으로 비켜 가는 경우가 아님) | — |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `-o`는 없는 경로와 일반 파일만 받는다. 디렉터리 꼴 대상은 쓰지 않는다 | Linux에서는 못 깨뜨림. `directory_form`이 참이면 `refuse_directory_form`은 어떤 `stat` 답이든 `Err`를 돌려주므로 글자 판정만으로 막히고 경합이 없다. macOS 15에서 `lstat("dl/")`가 링크 자체를 돌려줬다면 `resolve_symlink`가 `dir/dl-target`으로 풀어 새 검사를 비켜 갔겠지만, CI의 r9·코어 테스트(이름 목록 단언)가 통과해 그렇지 않음을 확인했다 | `cli.rs:497-512`, `ksq.rs:863-882` |
| 실패하면 대상·링크를 건드리지 않고 임시 파일을 남기지 않는다 | 못 깨뜨림. 거부는 `write_then_rename` 안이라 호출자의 `remove_file(&temp)`가 지운다. `temp_path`는 `dl/`·`a/.`에서 부모 디렉터리(`Path::file_name`이 끝 `/`·`.`을 버림), `a/..`에서 `a/../`에 만든다 | `cli.rs:455-464`, `ksq.rs:816-822` |
| 문구 분기는 경로 글자와 커널 오류만 보고, 쓰기 동작은 바뀌지 않는다 | 동작은 못 깨뜨림. 문구는 #7처럼 어긋난다 | `cli.rs:301-311` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 가져오기·내보내기(S0-T8 `ksq-cli`) | 충족(Linux 일반 사용자 실행, macOS 15 CI의 `ksq-cli` 시험 전부 통과) | `cli.rs:485`, `ksq.rs:849` |
| 9.5 `.ksq` 형식(원자적 쓰기) | 충족 | `ksq.rs:808-852` |
| 8.3 macOS 전용 | 충족. Linux 전용 실패는 다루지 않음 | 계획 「범위 밖」 |

계획의 「관련 REQ: 9.2(텍스트 왕복 도구)」는 틀린 ID다. REQ 9.2는 「스레드 모델」이다. S0-T8 계획(`docs/reports/s0-t8/plan.md:4`)은 5.14·9.5를 든다(제안 4).

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(파일 쓰기 경로만 바뀜)
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. `directory_form`의 `unwrap_or_default`는 `rsplit`가 항상 한 조각 이상을 내므로 기본값에 닿지 않는다
- 에러가 삼켜지는 곳: 새로 생긴 것 없음. 기존의 임시 파일 지우기 실패·디렉터리 fsync 실패 무시는 그대로다

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `/dev` 아래 EACCES 판정이 경로 글자(`Path::starts_with("/dev")`)라 `/dev/..`로 시작하는 `/dev` 밖 경로의 권한 오류까지 특수 안내로 바뀐다. 재현: 쓰기 권한 없는 `/tmp/x/ro`에 `ksq2txt in.ksq -o /dev/../tmp/x/ro/y` → 수정 전 「다른 경로를 주거나 표준 출력으로 받는다」, 수정 후 「특수 파일에 쓰려면 -o 없이 실행하고 셸로 리다이렉트하라」. 셸 리다이렉션도 EACCES로 실패하므로 안내가 틀리고, 바로 위 새 주석의 기준과도 어긋난다. 리뷰 11 제안 1은 같은 글자 판정의 ENOENT 경우를 「문구 취향」으로 두었지만, 이번 변경은 전에 맞던 EACCES 문구를 새로 틀리게 만든 회귀다. 고치는 방법 예: EACCES 쪽만은 경로 글자 대신 커널 답으로 판정한다(링크를 푼 대상의 부모를 `fs::metadata`해 `on_devfs`와 같은 장치 번호 비교. `/dev/<새 이름>`의 부모는 `/dev`라 특수, `/dev/../tmp/ro/x`의 부모는 `/tmp/ro`라 일반). 고치지 않으려면 결정 근거를 보고서에 적고 주석의 「`/dev` 밖」을 「경로 글자가 `/dev`로 시작하지 않는」으로 고친다. 재현 테스트: `tools/ksq-cli/tests/review_macos15_r1.rs::binaries_output_dev_dotdot_into_read_only_dir_gets_general_hint` | `tools/ksq-cli/src/cli.rs:301-309` |
| 2 | 수정 | 새 코드만이 막는 경우의 테스트가 `dl/` 하나뿐이다. 링크 대상 문자열이 `/`로 끝나고 그 뒤가 끊긴 링크·없는 이름인 사슬(`l3 -> l2/`, `l2 -> missing`)은 링크를 푼 경로가 `dir/l2/`가 되어 `dl/`와 같은 rename에 닿는다. macOS 15에서 수정 전에는 `dl/`처럼 종료 0이었을 가능성이 높지만 어느 테스트도 이 꼴을 돌리지 않는다. 계획·보고서는 「링크 대상 문자열 안의 끝 `/`(`l3 -> l2/`, `l2 -> 일반 파일`)도 같은 길로 막힌다」고 적었지만, `l2 -> 일반 파일`은 `guard_output`의 `stat`이 ENOTDIR로 먼저 막으므로 새 코드를 지나지 않는다(r6 사슬 테스트도 같다). `l2 -> missing`, `ls -> newdir/`, `l4 -> l2//`를 두 바이너리에 돌려 종료 1·이름 목록 불변·링크 보존을 단언하는 테스트를 더해 macOS 15 CI로 확인한다. 초안: `review_macos15_r1.rs::binaries_output_link_with_slashed_target_to_missing_fails_without_creating`(Linux에서는 수정 전후 모두 통과) | `docs/reports/ksq-cli-macos15/plan.md:13`, `tools/ksq-cli/tests/review_s0_t8_r9.rs:431-459` |
| 3 | 제안 | 코어 주석과 커밋 본문이 원격 CI에서 보지 않은 사실을 적는다. 「macOS 15에서는 … 이름 바꾸기가 성공해 링크 자리에 파일이 생겼다(원격 CI)」, 커밋 `173bfeb` 「(원격 CI, txt2ksq)」. 실행 41442250458의 r9 실패는 `review_s0_t8_r9.rs:451`의 종료 코드 단언(첫 결과인 `ksq2txt`, `left: Some(0)`)에서 멈췄으므로 무엇이 어디에 생겼는지(링크 자리인지 `dl-target`인지)와 `txt2ksq`의 결과는 로그에 없다. 이번 CI(41450337288)에서 수정 뒤 테스트가 통과해 `lstat("dl/")`가 링크를 따라간다는 것(새 검사가 막는 쪽)은 확인됐지만, 수정 전에 파일이 어디에 생겼는지는 여전히 관찰되지 않았다. 주석·커밋 본문을 관찰한 사실(「이름 바꾸기가 성공해 종료 0이었다」)로 고친다 | `crates/keyscore-core/src/ksq.rs:892-894`(cli 쪽 `cli.rs:523-525`는 「성공했다」만 적어 맞다) |
| 4 | 제안 | 계획의 「관련 REQ: 9.2(텍스트 왕복 도구)」는 REQ 9.2(스레드 모델)와 맞지 않는다. S0-T8과 같이 5.14·9.5로 고친다. 커밋 본문의 `REQ: S0-T8`도 REQ ID가 아니라 Task ID다 | `docs/reports/ksq-cli-macos15/plan.md:4` |
| 5 | 제안 | `review_s0_t8_r6.rs`의 사슬 테스트 이름(`..._fails_not_a_directory`)과 첫 문단(「판정도 `Slash`로 가」, 지금은 없는 판정)이 넓힌 단언과 맞지 않는다. 단언 자체는 종료 1·일반 파일 보존·임시 파일 없음이 남아 뜻이 있다. 첫 문단의 사라진 `Slash` 언급만 고쳐도 된다 | `tools/ksq-cli/tests/review_s0_t8_r6.rs:227-229` |
| 6 | 제안 | 코어 단위 테스트 `write_file_to_dangling_link_with_trailing_slash_fails_and_keeps_link`는 단언이 실패하면 임시 디렉터리를 지우지 않고 남긴다(pid 이름이라 다음 실행과 겹치지는 않음). 실패 원인 확인에는 오히려 도움이 되므로 그대로 두어도 된다 | `crates/keyscore-core/src/ksq.rs:1145-1165` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

핵심 수정(디렉터리 꼴 대상 거부, `/dev` 아래 EACCES 문구)은 macOS 15 CI에서 효력이 확인됐다. 그래도 지적 1(전에 맞던 EACCES 문구의 회귀, 실행해 재현함)과 지적 2(새 코드만 막는 링크 대상 끝 `/` 경우의 테스트 누락. 계획·보고서가 이 경우를 다룬다고 적었으나 실제로 지나는 테스트가 없음)를 고친 뒤 재리뷰한다. 지적 2의 테스트는 macOS 15 CI에서 통과를 확인한다. 이 실행의 CI 전체 실패(`keyscore-midi` 타이밍 시험 둘)는 이 변경과 무관하지만 PR을 녹색으로 만들려면 결정 2026-11-02 A의 「반영」 절차(경과 시간 기준으로 고칠 수 있는지 먼저 보고, 안 되면 근거를 적고 건너뛰기 목록에 더함)가 따로 필요하다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_macos15_r1.rs`, 커밋하지 않음. 수정 전 통과·수정 후 실패 확인)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(코어·cli의 쓰기 도우미, r6 단언, 계획·보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(r6 단언 하나를 넓힘)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존 없음)
- [x] 스타일 지적을 적지 않았다
