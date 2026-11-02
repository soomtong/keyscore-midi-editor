# 적대적 리뷰: ksq-cli 출력 경로의 macOS 15 차이 (리뷰 2회차)

- 리뷰어 세션: 구현 세션과도, 리뷰 1회차 세션과도 다른 서브 에이전트 세션. 구현 컨텍스트를 물려받지 않았다
- 대상 커밋: `9ca2501`(브랜치 `fix/ksq-cli-macos15-output-paths`, 기준 `a1e85fd`). 리뷰 1 대응 커밋 `3a0077c`, `9fbb6dc`, `885096b`, `78d995d`, `9ca2501`. 뒤의 병합 커밋 `fe79322`는 기준 브랜치를 들이는 것뿐이라 범위 밖이다
- 실행 환경: Linux 컨테이너. `ksq-cli`·`keyscore-core` 시험은 일반 사용자(`setpriv --reuid=65534`)로 돌렸다. 비교를 위해 `a1e85fd`(기준), `625fbc5`(리뷰 1 대상), `9ca2501`(이번 대상)을 `/tmp/ksq-review2-{base,r1,after}` worktree에서 따로 빌드했다(리뷰 뒤 지움). macOS 15 동작은 이 브랜치의 원격 CI 실행 41455906843(`fe79322`, macOS 15)의 로그로 확인했다

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 리뷰 1 지적 1이 실제로 고쳐졌는가. `/dev/../<쓰기 권한 없는 디렉터리>/x`가 일반 안내로 돌아가고, `/dev/<새 이름>`·`/dev/<새 이름>/`·`/dev/<새 이름>/.`·`/dev/../dev/<새 이름>`은 특수 안내로 남는가.
2. `parent_on_devfs`가 `Path::parent`에 기대는 곳. 끝 `/`·`/.`는 `Path::parent`가 버리므로 `/dev/x/`의 부모는 `/dev`가 맞다. 한 구성 요소짜리 상대 경로(`x/`)의 부모는 빈 경로라 `fs::metadata("")`가 실패한다. 같은 파일의 `write_then_rename`은 빈 부모를 `.`로 바꾸는데 여기는 바꾸지 않는다.
3. `parent_on_devfs`는 사용자가 준 `-o` 경로의 부모를 보지만, 실제로 실패한 곳은 링크를 푼 경로의 부모다. `<링크 → /dev>/x`, `<링크 → /dev/x>`에서 안내가 어떻게 되는가. 특수 자리 판정(`special_place`)이 아직 경로 글자라 둘이 엇갈리지 않는가.
4. `/dev` 아래지만 devfs가 아닌 곳(Linux `/dev/pts`, `/dev/shm`, `/dev/fd -> /proc/self/fd`), devfs인 하위 디렉터리(`/dev/net`)의 EACCES.
5. EACCES만 장치 번호로 바꾸고 EPERM은 그대로 경로 글자다. `/dev/..`로 밖에 나간 경로의 EPERM(불변 디렉터리, macOS TCC)은 리뷰 1 지적 1과 같은 꼴로 틀린 안내를 받는가.
6. 리뷰 1 지적 2의 시험이 macOS 15 CI에서 실제로 돌고 통과하는가.
7. 코어와 cli의 `directory_form`·`refuse_directory_form`이 여전히 같은가. 문서·주석·보고서가 새 규칙을 맞게 적는가.
8. `clippy::pedantic`, fmt, 기존 시험의 전후 비교.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 지적 1 수정 | 있음 | `review_macos15_r1::binaries_output_dev_dotdot_into_read_only_dir_gets_general_hint`. `/dev/<새 이름>`·`/dev/<새 이름>/`는 `review_s0_t8_r11`, `cli::output_errors_show_redirect_example_for_each_binary`. `/dev/<새 이름>/.`, `/dev/../dev/<새 이름>`은 없다(아래 3번 #2로 실행 확인) |
| 2. 빈 부모 | 없음 | 아래 지적 1, 새 시험 `review_macos15_r2.rs` |
| 3. 링크로 `/dev`에 닿는 경로 | 없음(문구) | `review_s0_t8_r11::..._misleading_path_shape_...`는 종료 코드만 본다 |
| 4. `/dev` 아래 devfs 아닌 곳 | 없음 | 실행 확인(3번 #3) |
| 5. EPERM 경로 글자 | 없음 | 보고서 「알려진 한계」에 적혀 있음 |
| 6. 지적 2 시험의 macOS 15 확인 | 있음 | CI 41455906843에서 `review_macos15_r1` 2/2 통과 |
| 7. 코어·cli 규칙 일치 | 있음 | 두 `directory_form_matches_trailing_slash_dot_and_dotdot`가 같은 표 |

## 3. 깨뜨리기 시도

모두 일반 사용자로 실제 실행했다. 「기준」은 `a1e85fd`, 「r1」은 `625fbc5`, 「지금」은 `9ca2501` 빌드다. 두 바이너리(`ksq2txt`, `txt2ksq`) 결과는 모든 경우에 같았다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `-o /dev/../<tmp>/ro/x`, `.../ro/x/`(쓰기 권한 없는 디렉터리, EACCES) | 못 깨뜨림. 기준 일반, r1 특수, 지금 일반. 리뷰 1 지적 1은 실제로 고쳐졌다. 리뷰 1 시험이 Linux 일반 사용자와 macOS 15 CI 모두에서 통과 | — |
| 2 | `-o /dev/ksqr2-new`, `/dev/ksqr2-new/`, `/dev/ksqr2-new/.`, `/dev/ksqr2-new/./`, `/dev/../dev/ksqr2-new`(Linux devtmpfs는 EACCES) | 못 깨뜨림. 지금 모두 특수 안내(r1과 같음). `Path::parent`가 끝 `/`·`/.`를 버려 부모가 `/dev`로 나온다 | — |
| 3 | `-o /dev/net/x`, `/dev/net/x/`(devfs 하위 디렉터리), `/dev/pts/x`(devpts), `/dev/fd/ksqr2`, `/dev/fd/ksqr2/` | 못 깨뜨림. `/dev/net`은 특수, `/dev/pts/x`는 r1 특수에서 지금 일반(셸 리다이렉션도 EACCES로 실패하므로 일반 안내가 맞다), `/dev/fd/...`는 ENOENT라 전후 모두 특수 | — |
| 4 | 작업 디렉터리 `/dev`에서 `-o ksqr2-new/`와 `-o ./ksqr2-new/`, `ksqr2-new/.`, `../dev/ksqr2-new/` | 깨짐(문구 불일치). 커널에게 같은 자리인데 `ksqr2-new/`·`ksqr2-new/.`는 일반 안내, `./ksqr2-new/`·`../dev/ksqr2-new/`는 특수 안내. `Path::new("ksqr2-new/").parent()`가 빈 경로라 `fs::metadata("")`가 ENOENT로 실패한다. r1·기준은 넷 다 일반 안내였다(지적 1). 빈 부모를 `.`로 바꾸는 한 줄 수정으로 새 시험·`review_macos15_r1`·`r11`·`cli`가 모두 통과함을 확인했다(수정은 되돌림) | `tools/ksq-cli/tests/review_macos15_r2.rs::binaries_relative_output_in_dev_cwd_same_hint_with_or_without_dot_slash`(r1 통과, 지금 실패) |
| 5 | `<tmp>/devlink/ksqr2-new`와 `<tmp>/devlink/ksqr2-new/`(`devlink -> /dev`), `<tmp>/to-devnew`·`<tmp>/to-devnew/`(`to-devnew -> /dev/ksqr2-new`) | 못 깨뜨림(동작). 문구는 엇갈림: `devlink/ksqr2-new`는 일반, 끝에 `/`만 붙인 `devlink/ksqr2-new/`는 지금 특수(r1·기준 일반). `special_place`가 경로 글자, EACCES 판정이 부모 장치 번호라 둘을 함께 만족할 때만 특수가 된다. 문서 규칙(「경로가 `/dev/` 아래이거나 끝이 `/`·`/.`이고 … 부모가 devfs면 EACCES도」) 그대로의 결과다(지적 3) | — |
| 6 | 불변 디렉터리(`chattr +i`, 루트가 미리 만듦)에 일반 사용자로 `-o <tmp>/imm/x`, `-o /dev/../<tmp>/imm/x`(EPERM) | 깨짐(기존 한계). `/dev/..` 쪽은 기준·r1·지금 모두 특수 안내인데 셸 `> /dev/../<tmp>/imm/x`도 「Operation not permitted」로 실패한다. 리뷰 1 지적 1과 같은 꼴이 EPERM에 남아 있다. 보고서 「알려진 한계」에 적혀 있고 이번 변경의 회귀는 아니다(지적 2). 루트 준비가 필요해 시험으로 남기지 않았다 | — |
| 7 | `ksq-cli` 시험 바이너리 전부를 r1·지금 빌드로 돌려 실패 목록 비교 | 못 깨뜨림. 새 실패 0. 차이 하나(`r9::binaries_output_regular_file_off_devfs_accepted_through_dev_dotdot`)는 같은 빌드를 세 번 돌려 r1·지금 모두 통과·실패가 섞이는 기존의 불안정 시험임을 확인했다 | — |
| 8 | 코어 `ksq::tests` 일반 사용자 실행 | 못 깨뜨림. `directory_form_...`, `write_file_to_dangling_link_...` 통과. `ksq_snapshot_format`만 insta 스냅샷 쓰기 권한 탓으로 실패(전후 같음) | — |
| 9 | 원격 CI 41455906843(macOS 15, `fe79322`) 로그 읽기 | 못 깨뜨림(범위 안). `cli` 20, `review_macos15_r1` 2, `r6` 11, `r9` 8, `r11` 4 등 `ksq-cli` 시험 대상 전부와 `keyscore_core` 단위 시험 92 통과. 리뷰 1 지적 2의 시험이 macOS 15에서 처음 돌아 통과했다. 실행 전체는 `keyscore-app`의 `review_s2_t6b_r2::cached_render_is_cheap_with_big_keymap`(벽시계 시간 비교, 55.7ms vs 11.6ms)로 실패했는데, 이 Task와 무관하고 건너뛰기 목록에 없는 타이밍 시험이다 | — |
| 10 | `cargo clippy -p keyscore-core -p ksq-cli --all-targets -- -D warnings`(pedantic 켜짐), `cargo fmt --all -- --check` | 못 깨뜨림. 경고 0, 차이 없음. 새 리뷰 시험 파일 포함 | — |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `-o`는 없는 경로와 일반 파일만 받고 디렉터리 꼴 대상은 쓰지 않는다 | 못 깨뜨림. 대응 커밋은 쓰기 경로를 바꾸지 않았다. macOS 15 CI에서 링크 대상 끝 `/` 사슬(`l3 -> l2/`, `ls -> newdir/`, `ld -> x/.`, `l4 -> l2//`)이 두 바이너리 모두 종료 1, 이름 목록 불변, 링크 보존 | `cli.rs:473-501`, `ksq.rs:825-858` |
| 실패하면 대상·링크를 건드리지 않고 임시 파일을 남기지 않는다 | 못 깨뜨림(3번 #1–#6 모두 종료 1, 새 파일 없음) | `cli.rs:462-471` |
| 문구 분기는 메시지만 바꾸고 쓰기 동작은 바꾸지 않는다 | 못 깨뜨림. `parent_on_devfs`는 `output_error` 안에서만 불린다 | `cli.rs:305-309` |
| 코어와 cli의 디렉터리 꼴 규칙이 같다 | 못 깨뜨림. 두 함수 본문이 같고 같은 표로 시험한다 | `ksq.rs:886-890`, `cli.rs:524-528` |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 가져오기·내보내기(S0-T8 `ksq-cli`) | 충족(macOS 15 CI의 `ksq-cli` 시험 전부 통과) | `cli.rs:410-456` |
| 9.5 `.ksq` 형식(원자적 쓰기) | 충족 | `ksq.rs:808-858` |
| 8.3 macOS 전용 | 충족. Linux 전용 실패는 다루지 않음 | 계획 「범위 밖」 |

리뷰 1 제안 4대로 계획의 REQ가 5.14·9.5로 고쳐졌고, 대응 커밋 본문도 REQ ID를 적는다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(파일 쓰기 실패의 문구 경로만 바뀜)
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음
- 에러가 삼켜지는 곳: `parent_on_devfs`가 부모 `stat` 실패를 「devfs 아님」으로 바꾼다. 문구 선택에만 쓰이므로 의도된 것이지만, 빈 부모도 이 갈래로 빠진다(지적 1)

## 리뷰 1 지적 사항의 해결 확인

| 리뷰 1 # | 대응 | 확인 |
| --- | --- | --- |
| 지적 1 | `parent_on_devfs`(3a0077c) | 해결. 3번 #1·#2. 재현 시험이 Linux 일반 사용자, macOS 15 CI 모두 통과 |
| 지적 2 | 리뷰어 시험 커밋, 계획 문구 수정(b0ea65f, 9fbb6dc) | 해결. CI 41455906843에서 macOS 15 통과. 계획 원인 2의 「`l2 -> 일반 파일`은 출력 확인이 먼저 막는다」가 코드와 맞다 |
| 제안 3 | 코어 주석(885096b) | 해결. 관찰한 사실(종료 0, 위치는 보지 못함)만 적는다 |
| 제안 4 | 계획 REQ(9fbb6dc) | 해결 |
| 제안 5 | r6 시험 문서(78d995d) | 해결 |
| 제안 6 | 거절 | 거절 이유(원인 확인에 도움, pid 이름) 타당 |

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `parent_on_devfs`가 `Path::parent`의 빈 경로를 그대로 `stat`한다. 한 구성 요소짜리 상대 경로(`x/`, `x/.`)는 부모가 `""`라 `fs::metadata("")`가 ENOENT로 실패해 「devfs 아님」이 된다. 재현: 작업 디렉터리가 `/dev`일 때 macOS 15(EACCES)나 Linux 일반 사용자에서 `ksq2txt in.ksq -o ksq-new/`는 일반 안내, `-o ./ksq-new/`는 특수 안내. 커널에게 같은 자리이고 함수 문서(「부모 디렉터리(커널이 푼 것)」)와도 어긋난다. 같은 파일 `write_then_rename`(`cli.rs:495-498`)처럼 빈 부모를 `.`로 바꾸면 된다(`.map(\|p\| if p.as_os_str().is_empty() { Path::new(".") } else { p })`). 이렇게 고쳐 새 시험·`review_macos15_r1`·`r11`·`cli`가 Linux 일반 사용자에서 모두 통과함을 확인했다. 기준·r1도 `x/`는 일반 안내였으므로 회귀는 아니고, `cd /dev`라는 드문 조건의 문구 차이라 제안으로 둔다. 재현 시험: `tools/ksq-cli/tests/review_macos15_r2.rs::binaries_relative_output_in_dev_cwd_same_hint_with_or_without_dot_slash`(r1 통과, 지금 실패, 커밋하지 않음) | `tools/ksq-cli/src/cli.rs:343-348` |
| 2 | 제안 | EPERM은 아직 경로 글자로만 특수 쪽에 간다. `/dev/..`로 `/dev` 밖에 나간 경로의 EPERM(Linux 불변 디렉터리, macOS의 TCC 보호 폴더·`uchg` 등)은 리뷰 1 지적 1과 같은 꼴로 「셸로 리다이렉트하라」를 받지만 셸도 같은 EPERM으로 실패한다. 재현: 루트가 `chattr +i`한 디렉터리에 일반 사용자로 `-o /dev/../<tmp>/imm/x` → 특수 안내, `sh -c 'echo > /dev/../<tmp>/imm/x'` → 「Operation not permitted」. 기준부터 같은 동작이고 보고서 「알려진 한계」에 적혀 있다. 사람 Mac의 `/dev/<새 이름>` EPERM도 부모가 `/dev`라 `(EPERM \|\| EACCES) && parent_on_devfs(path)`로 묶으면 규칙이 하나가 되고 이 경우도 풀린다. 범위 밖이라고 거절해도 된다 | `tools/ksq-cli/src/cli.rs:305-309` |
| 3 | 제안 | 새 규칙을 설명하는 글이 코드와 조금씩 어긋난다. (가) `cli.rs:303-304` 주석 「경로 글자 … 가 아니라 부모 디렉터리가 실제로 devfs인지로 가른다」: 실제로는 `special_place`(경로 글자)와 부모 장치 번호를 함께 본다. 그래서 `<링크 → /dev>/x`는 일반, 끝에 `/`만 붙인 `<링크 → /dev>/x/`는 특수가 되고(3번 #5), `/dev/pts/x`는 `/dev` 아래인데 일반이다. (나) 보고서 「구현 요약」(`report.md:18`)과 계획 원인 1(`plan.md:12`)은 아직 「`/dev` 아래에서는 `EACCES`도 특수 쪽」이라는 고치기 전 규칙을 적는다. (다) 보고서 「알려진 한계」(`report.md:51`)의 「EACCES만 부모 디렉터리의 장치 번호로 본다」는 「EACCES는 경로 글자에 더해 부모 디렉터리의 장치 번호도 본다」가 맞다 | `tools/ksq-cli/src/cli.rs:301-304`, `docs/reports/ksq-cli-macos15/report.md:18`, `:51`, `docs/reports/ksq-cli-macos15/plan.md:12` |
| 4 | 제안 | 보고서 「테스트 결과」 표에 계획의 시험 8·9(`review_macos15_r1`의 두 시험)가 없다. 리뷰 1 대응 표는 「macOS 15 CI로 확인한다」에서 멈춰 있는데, CI 41455906843(`fe79322`)에서 두 시험과 `ksq-cli` 시험 전부가 macOS 15에서 통과했다. 실행 번호와 함께 적는다 | `docs/reports/ksq-cli-macos15/report.md:26-34`, `:69` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1의 지적 둘은 실제로 해결됐다. 지적 1은 Linux 일반 사용자와 macOS 15 CI에서 재현 시험이 통과했고, 지적 2의 시험은 macOS 15에서 처음 돌아 통과했다. 대응 커밋이 새로 들인 `parent_on_devfs`에서 빈 부모 문제(지적 1)를 실행해 찾았지만, 기준보다 나빠진 경우가 없고 `cd /dev`라는 드문 조건의 문구 차이라 제안으로 둔다. 쓰기 동작, 불변식, REQ에 걸리는 문제는 없다. 지적 1·3은 한 줄 수정과 문서 수정이라 병합 전에 반영하기를 권하지만, 반영하지 않으면 그 이유를 보고서에 적는다.

PR을 녹색으로 만드는 것은 따로 남는다. CI 41455906843은 이 Task와 무관한 `keyscore-app` 타이밍 시험 `review_s2_t6b_r2::cached_render_is_cheap_with_big_keymap`으로 실패했다. 결정 2026-11-02 A의 「반영」 절차로 다룬다(이 리뷰의 범위 밖).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(diff의 시험 파일은 대응 범위를 알려고 함께 봤지만, 1번 항목은 코드에서 뽑았다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`review_macos15_r2.rs`, 커밋하지 않음. r1 통과·지금 실패, 한 줄 수정 뒤 통과 확인)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff a1e85fd..9ca2501`에 Task 범위 밖의 변경이 없다(코어·cli 쓰기 도우미, r6 시험 문서·단언, 리뷰 1 시험, 계획·보고서·리뷰 기록뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`Cargo.toml`·`Cargo.lock` 변화 없음)
- [x] 스타일 지적을 적지 않았다
