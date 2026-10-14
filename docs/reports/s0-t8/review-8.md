# 적대적 리뷰: S0-T8 `ksq-cli` (리뷰 8회차)

- 리뷰어 세션: 구현자·계획자·리뷰 1–7과 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `6202881` (브랜치 `s0/t8-ksq-cli`). 범위는 리뷰 7 대응 커밋 `546a776`(열기 실패 갈래), `c53641d`(열기 전 `/dev/` 분류와 핸들 신원 비교), `1b88fc7`(리뷰 5 테스트의 링크 대상)뿐이다(`git diff 95f3a9e HEAD -- tools/`).
- 범위 밖: 1–7회차에서 확인된 성질(어림 상한, u64 경계, 손실 보고, 두 번 직렬화 등)은 다시 훑지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 95f3a9e HEAD -- tools/`의 `cli.rs`(`DevClass`, `Identity`, `dev_classification`, `write_special`)와 README만 읽고 적었다.

1. 리뷰 테스트 파일의 변경이 `1b88fc7`의 두 줄뿐인가. 그 변경이 리뷰 5 테스트 의도(상대·`..`·`.`·절대 링크가 섞인 사슬의 `/dev/` 판정)를 약하게 했는가. `cli.rs`만 `95f3a9e`로 되돌린 사본에서 리뷰 7 실패 테스트 4개가 다시 실패하는가.
2. 열기 실패 갈래의 「경로가 없다」 판정은 `fs::metadata(path)`의 실패다. ENOENT가 아닌 실패(ELOOP, EACCES, ENAMETOOLONG)도 「없다」로 센다. 그런데 원자적 쓰기의 `resolve_symlink`는 커널과 다른 한도(40)로 링크를 따라간다. 커널(macOS 32)이 ELOOP로 거부하는 사슬 끝의 특수 파일을 원자적 쓰기가 찾아 바꾸지 않는가.
3. 대상 종류별 종료 코드·임시 파일·원래 대상: 없는 경로, 끊긴 링크, 없는 디렉터리 안을 가리키는 링크, 순환 링크, 권한 없는 부모, 읽기 전용 일반 파일, 쓰기 권한 없는 FIFO, 소켓, 블록 장치, 디렉터리, `/dev/` 아래 없는 이름, 제어 터미널 없는 `/dev/tty`.
4. `Identity`는 장치 번호가 `/dev`의 것이면 inode만 비교한다. 분류가 `/dev/stdout`(돌린 파일 X)을 보고 열기가 다른 볼륨의 같은 inode 번호 일반 파일 Y를 보면 `matches`가 참이 되어 Y에 자르지 않고 쓰지 않는가. 볼륨 사이 inode 번호 충돌이 실제로 쉽게 생기는가.
5. `/dev/fd/N`·`/dev/stdout`이 일반 파일·파이프·터미널을 가리키는 정상 경우가 그대로인가(신원 비교가 정상 경우를 막지 않는가).
6. 일반 파일에 옛 꼬리가 남는 다른 경로가 생겼는가.
7. 전체 `cargo test -p ksq-cli` 반복에서 흔들리는 테스트가 남았는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 리뷰 테스트 diff, 되돌린 사본 | 리뷰 7 실패 테스트 4개가 기준 | 시도 1·2 |
| 2 커널 한도를 넘는 링크 사슬 | 없음. 리뷰 1–7의 순환 링크 테스트는 두 링크 순환(양쪽 다 실패)뿐 | 실패 테스트 둘 |
| 3 대상 종류별 갈래 | 리뷰 7에 FIFO·소켓·`/dev/stdin` 단건 | 묶어 확인하는 통과 테스트 둘 |
| 4 inode만 비교 | 없음. `c53641d`에 신원 비교 테스트가 없다 | 선택 실행 실패 테스트 하나 |
| 5 `/dev/fd/N`·`/dev/stdout` 정상 경우 | 리뷰 3–7 테스트 | 통과 테스트 둘 |
| 6 옛 꼬리 | 리뷰 6·7의 원자적 쓰기 테스트 | 시도 6·7 |
| 7 반복 | 없음 | 시도 9 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `git diff 95f3a9e HEAD -- tools/ksq-cli/tests/review_s0_t8*.rs` | 못 깨뜨림. `review_s0_t8_r5.rs`의 두 줄(주석, 링크 대상 `dev/./fd/../stdout` → `dev/./stdout`)뿐이다. 의도는 유지된다: `..`는 `../../c/l2`와 `up_to_root`가 만든 `../../..`에 그대로 있고, 분류가 부모 `…/c/../../../dev/.`를 정규화해야 하는 것도 같다. 글자 그대로 `starts_with("/dev")`로 판정하는 구현은 바뀐 테스트에서도 실패한다. 잃은 것은 `/dev` 안의 하위 디렉터리를 `..`로 빠져나오는 한 단계뿐이고, `/dev/..` 경계는 리뷰 7 `ksq2txt_output_dev_dotdot_regular_file_replaced_atomically`가 따로 본다. 계획자가 적은 `fd/..` 조회 흔들림은 한가한 상태에서 `stat -L /dev/./fd/../stdout` 3000회로는 재현하지 못했다(0회 실패. pty를 만드는 부하가 없었다) | 없음 |
| 2 | `src/cli.rs`만 `95f3a9e`로 되돌린 `/tmp` 사본(별도 target)에서 `review_s0_t8_r7` | 확인. 7 통과 4 실패(리뷰 7 실패 테스트 4개 그대로). HEAD에서는 11개 모두 통과. 사본은 지웠다 | 없음 |
| 3 | 링크 35개 사슬(커널 한도 32 초과, `resolve_symlink` 한도 40 미만) 끝의 쓰기 가능한 FIFO·0444 FIFO에 `ksq2txt -o`, 소켓에 `txt2ksq -o` | 깨짐. 셋 다 종료 0이고 FIFO·소켓이 일반 파일로 바뀐다. 쓰기 열기는 ELOOP, `fs::metadata`도 ELOOP라 `special`이 거짓이 되어 `Ok(false)`로 가고, `resolve_symlink`가 사슬 끝을 찾아 이름 바꾸기 한다. `txt2ksq`는 core `ksq::write_file`(역시 한도 40)이 바꾼다. 리뷰 6 이전 코드(`724717a`) 사본에서는 소켓은 종료 1, FIFO는 풀린 경로를 열어 막힌 채 FIFO로 남았다. 즉 `3874b10`(원래 경로로 열기)이 연 회귀를 `546a776`이 닫지 못했다 | `ksq2txt_output_link_chain_over_kernel_limit_to_fifo_fails_and_keeps_fifo`(실패), `txt2ksq_output_link_chain_over_kernel_limit_to_socket_fails_and_keeps_socket`(실패) |
| 4 | 열기 실패 갈래의 대상 종류: 없는 경로, 끊긴 링크, 없는 디렉터리 안을 가리키는 링크, 두 링크 순환, 권한 없는(000) 부모 안 파일, 0444 일반 파일, 0444 FIFO, 소켓, `/dev/disk0` 링크(블록 장치), 디렉터리, `/dev/nonexist` 링크, `/dev/tty`(제어 터미널 없음) | 못 깨뜨림. 없는 경로·끊긴 링크는 종료 0으로 만든다(링크는 링크로 남음). 0444 일반 파일은 종료 0, 내용 교체, 권한 0444 유지. 나머지는 모두 종료 1이고 원래 대상 그대로: ENOENT·「심볼릭 링크가 너무 깊거나 순환한다」·EACCES·EOPNOTSUPP(102)·EPERM(1)·EISDIR·ENOENT·ENXIO(6). 임시 파일은 어디에도 남지 않았다 | `ksq2txt_output_open_failure_kinds_follow_documented_exit_codes`, `txt2ksq_output_open_failure_kinds_follow_documented_exit_codes` |
| 5 | `stat`으로 신원 확인: `/dev`는 dev 1791158680, `/dev/fd/3 3<reg`와 `/dev/stdout >reg`는 dev 1791158680에 `reg`의 inode | 확인. 주석대로 fdesc는 `/dev`의 장치 번호와 실제 inode를 낸다. 그래서 `/dev/fd/N`·`/dev/stdout`이 일반 파일이면 `Identity.dev`는 언제나 `None`이고 inode만 비교한다 | 없음 |
| 6 | RAM 디스크 둘(`hdiutil attach -nomount ram://4096`, `diskutil erasevolume HFS+`)을 붙이니 첫 파일이 둘 다 inode 20이었다. 볼륨 B의 읽기 전용 디렉터리에 긴 `Y`(inode 22), 볼륨 A에 같은 inode 22의 `X2`. 표준 출력을 `X2`로 돌리고, 링크 `l`을 `/dev/stdout`과 `Y` 사이에서 계속 바꾸며 `ksq2txt -o l` | 깨짐. 500회 중 74회 `Y`의 머리에 새 텍스트가 쓰이고 `OLDOLD…` 꼬리가 남았다(종료 0). 분류가 `/dev/stdout`(inode 22, dev 없음)을 보고 열기가 `Y`(inode 22)를 봐 `matches`가 참이 된다. 같은 볼륨(`/tmp`)의 `Y3`로 바꾼 대조군은 500회 중 0회. `c53641d`가 닫겠다고 한 리뷰 7 제안 3의 경합이 `/dev/fd` 경로에서는 그대로 열려 있다. 같은 inode 번호는 사용자 권한으로 몇 초 안에 만들 수 있다(HFS+·FAT·FUSE·NFS 등 inode가 작은 볼륨, 외장 디스크) | `ksq2txt_output_link_raced_to_same_inode_on_other_volume_keeps_file_unchanged`(`KSQ_R8_RAMDISK=1`일 때만 실행. 실행 시 첫 회에 실패, 종료 0) |
| 7 | 표준 출력을 일반 파일로 돌리고(셸처럼 자름) `-o /dev/fd/1`, `/dev/stdout`, 그 둘을 가리키는 링크. 표준 출력이 파이프일 때 `/dev/stdout`·`/dev/fd/1` | 못 깨뜨림. 모두 종료 0, 같은 inode에 직접 쓰고 `-o` 없는 출력과 같다. 신원 비교가 정상 경우를 막지 않는다 | `ksq2txt_output_dev_fd_to_redirected_regular_file_matches_identity`, `ksq2txt_output_dev_stdout_to_pipe_writes_pipe` |
| 8 | 셸에서 `-o /dev/stdout 1<>rw.txt`(자르지 않고 읽기·쓰기로 연 표준 출력) | 옛 꼬리가 남는다(종료 0). 다만 리뷰 3 이후 README가 약속한 「셸이 열어 둔 파일에 이어 쓴다」 동작이고 이 세 커밋 전부터라 지적하지 않는다 | 없음 |
| 9 | `cargo test -p ksq-cli --no-fail-fast`를 HEAD + 이 리뷰 테스트로 22회 | 흔들림 없음. 22회 모두 실패는 이 리뷰의 실패 테스트 2개뿐이었다. 리뷰 5 `ksq2txt_output_mixed_link_chain_to_dev_stdout_writes_redirected_file`를 포함해 나머지는 22회 모두 통과 | 없음 |
| 10 | 이 리뷰 테스트를 `724717a` 코드 사본에서 실행 | 확인. 소켓 사슬 테스트는 통과, FIFO 사슬 테스트는 막혀 시간 제한(30초)으로 실패. 시도 3이 리뷰 6 수정 뒤의 회귀임을 뒷받침한다. 사본은 지웠다 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 일반 파일이 아닌 기존 출력은 이름 바꾸기로 일반 파일로 바뀌지 않는다(리뷰 2 지적 3, `546a776`의 약속) | 깨짐 | 시도 3(커널 한도를 넘는 링크 사슬) |
| 열기가 실패하면 원자적 쓰기로 가는 것은 「없음」과 「일반 파일」뿐이다 | 깨짐 | 시도 3. ELOOP를 「없음」으로 센다 |
| 일반 파일에 직접 쓰는 것은 분류한 대상과 같은 파일일 때뿐이다(`c53641d`의 약속) | 깨짐(경합 + 다른 볼륨의 같은 inode) | 시도 6 |
| 쓰기로 열리는 경로의 판정이 커널의 경로 풀이와 같다 | 깨짐 | 시도 3. 분류·원자적 쓰기의 `resolve_symlink`(40)와 커널(32)이 다르다 |
| 실패 메시지는 실제 원인을 알린다 | 못 깨뜨림 | 시도 4 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 내보내기(도구 수준) | 충족 | `tools/ksq-cli/src/convert.rs`. 세 커밋은 출력 열기 갈래만 바꾼다 |
| 9.5 / IO-1 `.ksq`·`.ksqz` 입력 | 충족 | 리뷰 5–7 테스트 통과 |
| AI-18 번들 픽스처 8곡 | 충족 | `tests/fixtures.rs` 통과 |
| S0-T8 게이트: 8개 `check` 통과, 텍스트 왕복 | 충족 | `tests/fixtures.rs`, `tests/round_trip.rs` 통과 |

`-o` 쓰기 규칙은 REQ가 아니라 README와 리뷰 1–7의 보장이다. 아래 지적은 그 보장의 버그로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(실시간 경로 없음).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `tools/ksq-cli/src/cli.rs:375-380`. `fs::metadata(path)`의 실패를 종류와 상관없이 「없음」으로 보고 열기 오류를 버린다(ELOOP·ENAMETOOLONG 등). `dev_classification`(`:313-338`)도 `resolve_symlink`·`canonicalize`·`metadata` 실패를 모두 `Outside`/`None`으로 삼키지만, 그 결과는 원자적 쓰기나 열기 오류로 이어져 시도 4에서 해를 찾지 못했다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 커널 한도(32)를 넘고 `resolve_symlink` 한도(40) 안인 링크 사슬 끝의 FIFO·소켓이 일반 파일로 바뀐다(종료 0, 두 바이너리). 열기 실패 갈래가 `fs::metadata`의 ELOOP를 「경로 없음」으로 세고, 원자적 쓰기(`write_atomic`의 `resolve_symlink`, core `ksq::write_file`)는 커널보다 깊이 따라가 사슬 끝을 찾는다. `3874b10`의 회귀이고 `546a776`의 약속(README 28행 「특수 파일이 일반 파일로 바뀌지 않는다」)과 어긋난다. 사슬 끝이 일반 파일이면 커널은 거부하는 경로를 이 도구는 받아들인다(같은 원인). 고치는 방향의 예: 열기 오류가 `NotFound`이고 `fs::metadata`도 `NotFound`일 때만 「없음」으로 본다. 그 밖의 실패는 열기 오류를 그대로 낸다 | `tools/ksq-cli/src/cli.rs:370-383`, `:446-461` |
| 2 | 수정 | `c53641d`의 신원 비교는 `/dev/fd/N`·`/dev/stdout`(언제나 fdesc 장치 번호)에서 inode만 비교하므로, 분류와 열기 사이에 링크가 다른 볼륨의 같은 inode 번호 일반 파일로 바뀌면 그 파일에 자르지 않고 써 옛 꼬리가 남는다(종료 0). RAM 디스크 둘로 500회 중 74회 재현했다. 리뷰 7 제안 3과 같은 전제(출력 경로를 바꿀 수 있는 다른 주체)가 필요하지만, 커밋이 「닫는다」고 한 경합이 실제 쓰이는 `/dev/fd` 경로에서 열려 있다. 닫으려면 `/dev/fd/N`이면 경로 `stat` 대신 자기 fd N을 `fstat`해 (dev, ino)를 다 비교해야 하는데, 이것도 `/dev/fd` 이름 풀이를 손으로 흉내 내는 또 하나의 경계다 | `tools/ksq-cli/src/cli.rs:294-303`, `:333-337`, `:387` |
| 3 | 제안 | 경로 처리의 범위. 리뷰 2–8이 모두 `-o` 경로 처리에서 새 경계를 찾았다(FIFO·장치, `/dev/fd`, 끝 `/`·`/.`, `..` 링크, 열기 오류, 경합, 링크 한도, inode 충돌). 원인은 커널이 하는 경로 풀이(`/dev/fd` union, 링크 한도, 끝 `/.`)를 분류·`resolve_symlink`·신원으로 두 번째로 흉내 내는 구조다. 고칠 때마다 경계가 하나씩 더 생긴다. 판정 8 참고 | `tools/ksq-cli/src/cli.rs:283-461`, `tools/ksq-cli/README.md` |

## 8. 판정

- [ ] 병합 가능
- [ ] 수정 후 재리뷰
- [x] 범위 재정의 (이유: 아래)

리뷰 7 지적 1·2는 커널 한도 안의 경로에서는 해소되었다(시도 4). 그러나 같은 갈래가 링크 한도라는 새 경계에서 다시 특수 파일을 일반 파일로 바꾸고(지적 1), 경합을 닫는다는 신원 비교는 정작 직접 쓰기를 하는 `/dev/fd` 경로에서 inode만 본다(지적 2). 리뷰 2부터 매 회차가 `-o` 경로 처리에서 새 경계를 냈고, 이번 두 지적도 같은 구조(커널 경로 풀이를 손으로 다시 하기)에서 나왔다. 지적 1을 `NotFound`만 「없음」으로 보게 고치고 지적 2를 `fstat(N)`으로 고치는 것은 가능하지만, 다음 회차가 또 하나의 경계를 찾을 가능성이 높다고 본다.

범위를 줄이는 안(예):

- `-o`는 「없는 경로」와 「일반 파일(링크를 따라간 대상 포함)」만 받는다. 언제나 같은 디렉터리 임시 파일 + 이름 바꾸기로 쓴다. 대상이 있고 일반 파일이 아니면(FIFO, 소켓, 장치, 디렉터리, `/dev/` 아래 무엇이든) 쓰기 전에 오류로 종료 1이다. 이 판정은 `symlink_metadata`·`metadata`의 `NotFound`만 「없음」으로 보고 나머지 실패는 오류로 낸다.
- 표준 출력·파이프·`/dev/null`로 보내려면 `-o`를 빼고 셸 리다이렉션을 쓴다고 README에 적는다. 그러면 `DevClass`, `Identity`, `write_special`의 직접 쓰기, `/dev/` 분류가 모두 사라지고 리뷰 3–8의 `/dev/fd` 경계가 함께 없어진다.
- 남는 경합(검사와 이름 바꾸기 사이에 대상이 특수 파일로 바뀜)은 데이터를 섞지 않는다(이름 바꾸기는 원자적). README에 한 줄로 적는다.

이 안은 리뷰 2–7이 통과시킨 `-o /dev/stdout`·`-o fifo` 동작을 없애므로 기존 리뷰 테스트 가운데 그 동작을 요구하는 것과 충돌한다. 기존 테스트를 지우지 않는 규칙이 있으므로, 계획자가 `docs/decisions/`에 범위 결정을 먼저 적고 충돌하는 리뷰 테스트의 처리(기대를 「오류로 종료 1」로 바꾸는 것)를 그 결정에서 정해야 한다. REQ는 `-o`의 특수 파일 동작을 정하지 않는다.

범위를 줄이지 않기로 하면, 지적 1·2를 고치고 `review_s0_t8_r8.rs`의 실패 테스트 2개(그리고 `KSQ_R8_RAMDISK=1`의 선택 실행 테스트)가 통과하는지 보는 좁은 재리뷰가 필요하다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(리뷰 테스트는 diff와 리뷰 7 테스트의 도우미만 먼저 보았다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p ksq-cli --test review_s0_t8_r8`: 5 통과, 2 실패. `KSQ_R8_RAMDISK=1`로 선택 실행 테스트도 실패 확인)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`95f3a9e` 뒤 변경은 `tools/ksq-cli/src/cli.rs`, `tools/ksq-cli/README.md`, `tools/ksq-cli/tests/review_s0_t8_r5.rs` 두 줄, `docs/reports/s0-t8/report.md`)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 테스트 diff는 `1b88fc7`의 두 줄뿐, `.proptest-regressions` 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
