# 적대적 리뷰: S0-T8 `ksq-cli` (리뷰 7회차)

- 리뷰어 세션: 구현자·계획자·리뷰 1–6과 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `30e9c4e` (브랜치 `s0/t8-ksq-cli`). 범위는 리뷰 6 수정 코드 커밋 `3874b10`(`-o`를 언제나 사용자가 준 원래 경로로 열고 연 핸들의 fstat으로 고르기), `89b794d`(안내 문구 「크기 검사를 통과한다」)뿐이다(`git diff 724717a HEAD -- tools/`).
- 범위 밖: 1–6회차에서 확인된 성질(어림의 위쪽 한계, u64 경계, 손실 보고 등)은 다시 훑지 않았다. 거대한 출력을 만드는 실험은 하지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 724717a HEAD -- tools/`의 `cli.rs`(`classified_under_dev`, `write_special`, `run_txt2ksq`)와 README만 읽고 적었다.

1. 리뷰 1–6 테스트가 그대로인가. `cli.rs`만 `724717a`로 되돌린 사본에서 리뷰 6 실패 테스트 4개가 다시 실패하는가.
2. `Err(_) => return Ok(false)`: ENOTDIR·EISDIR 말고는 열기 오류를 모두 버리고 원자적 쓰기로 간다. 이전에는 `Special::Other`(이미 있고 일반 파일도 디렉터리도 아닌 것)를 열다 난 오류를 그대로 냈다. 쓰기로 열 수 없는 특수 파일(권한 없는 FIFO, 소켓, 읽기 전용으로 열린 `/dev/fd/N`)이 이제 이름 바꾸기로 일반 파일로 바뀌거나 엉뚱한 오류를 내지 않는가.
3. 열기만으로 생기는 부작용: 읽는 쪽 없는 FIFO에서 막히는가(이전과 비교), 읽기 전용 파일(EACCES → 원자적 쓰기), 쓰기 권한 없는 디렉터리 안의 쓰기 가능 파일, 터미널.
4. 일반 파일인데 직접 쓰기로 가서 옛 꼬리가 남는 경우. 연 핸들은 일반 파일이고, 분류는 경로를 다시 풀어(`classified_under_dev(path)`) 한다. 열기와 분류 사이에 경로가 `/dev/stdout` 링크로 바뀌면 연 일반 파일에 자르지 않고 쓴다.
5. 열기와 원자적 쓰기 사이 경합, 열린 핸들이 닫히는가(`Ok(false)` 갈래에서 `file`이 드롭되는가).
6. `txt2ksq`가 일반 파일에서 `to_bytes`와 `ksq::write_file`로 두 번 직렬화한다. 두 결과가 같은가(`.ksq`·`.ksqz`).
7. 리뷰 3–6에서 통과한 경로: `/dev/stdout`, `/dev/fd/1`, 링크, 끝 `/`·`/.`, `/dev/..`, 일반 파일 rename.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 리뷰 테스트 유지와 되돌린 사본 | 리뷰 6 실패 테스트 4개가 기준 | diff 0바이트, 사본에서 4개 실패 확인(시도 1) |
| 2 열기 오류를 버림 | 없음. 리뷰 2–6의 FIFO 테스트는 모두 쓰기 가능한 FIFO | 실패 테스트 넷 |
| 3 열기의 부작용 | 읽기 전용 파일은 리뷰 2 이후 테스트에 있다. 읽기 전용 디렉터리·읽는 쪽 있는 FIFO는 이 범위에서 다시 확인할 테스트가 없다 | 통과 테스트 둘 |
| 4 일반 파일 직접 쓰기 | 리뷰 6의 원자적 쓰기 테스트 | 링크 포함 통과 테스트 하나. 경합은 셸에서 400회 시도 |
| 5 경합·핸들 | 없음 | 코드 확인. `Ok(false)` 갈래는 `file` 드롭으로 닫힌다 |
| 6 이중 직렬화 | 없음 | 통과 테스트 하나 |
| 7 리뷰 3–6 경로 | 리뷰 3–6 테스트 | 그대로 통과. 묶어 확인하는 통과 테스트 셋 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `git diff 724717a HEAD -- tools/ksq-cli/tests/review_s0_t8*.rs`, 그리고 `src/cli.rs`만 `724717a`로 되돌린 `/tmp` 사본(별도 target)에서 `review_s0_t8_r6` 실행 | 못 깨뜨림. diff 0바이트. 사본에서 7 통과 4 실패(리뷰 6 실패 테스트 4개 그대로). HEAD에서는 11개 모두 통과. 사본은 지웠다 | 없음 |
| 2 | 쓰기 권한 없는(0444) FIFO에 `ksq2txt -o` | 깨짐. 종료 0이고 FIFO가 일반 파일(0444)로 바뀐다. 쓰기 열기의 EACCES를 버리고 원자적 쓰기로 가기 때문이다. `724717a` 바이너리는 「ff: Permission denied」로 종료 1, FIFO 그대로. 리뷰 2 지적 3(FIFO가 일반 파일로 바뀜)의 회귀 | `ksq2txt_output_fifo_without_write_permission_fails_and_keeps_fifo`(실패) |
| 3 | 같은 FIFO에 `txt2ksq -o` | 깨짐. 종료 0, `ksq::write_file`이 일반 파일로 바꾼다. 이전 종료 1 | `txt2ksq_output_fifo_without_write_permission_fails_and_keeps_fifo`(실패) |
| 4 | 유닉스 소켓에 `ksq2txt -o` | 깨짐. 쓰기 열기는 EOPNOTSUPP(os error 102)인데 버려지고 소켓이 일반 파일로 바뀐다(종료 0). 이전 종료 1 | `ksq2txt_output_unix_socket_fails_and_keeps_socket`(실패) |
| 5 | 표준 입력을 읽기 전용 일반 파일로 두고 `-o /dev/stdin`, 셸에서 `-o /dev/fd/3 3<ro.txt`도 | 깨짐(오류 보고). 종료 1이고 파일은 그대로지만 메시지가 「No such file or directory (os error 2)」다. 실제 원인 EACCES를 버리고 원자적 쓰기가 `/dev/fd`에 임시 파일을 만들다 난 오류를 낸다. 이전에는 「Permission denied (os error 13)」 | `ksq2txt_output_read_only_dev_stdin_reports_permission_denied`(실패) |
| 6 | 읽는 쪽 없는 FIFO에 `-o`(셸, `timeout 3`) | 못 깨뜨림(변화 없음). HEAD와 `724717a` 모두 열기에서 막혀 시간 초과(124). 이전과 같은 동작이다 | 없음 |
| 7 | 읽는 쪽(`cat`)이 있는 FIFO | 못 깨뜨림. 종료 0, `cat`이 `-o` 없는 출력과 같은 텍스트를 받고 FIFO로 남는다 | `ksq2txt_output_fifo_with_reader_written_directly_and_stays_fifo` |
| 8 | 긴 일반 파일, 일반 파일을 가리키는 링크(`ksq2txt`), 긴 `.ksq`(`txt2ksq`) | 못 깨뜨림. 셋 다 새 inode, 옛 꼬리 없음, 링크는 링크로 남고 대상이 바뀜, 임시 파일 없음 | `binaries_output_regular_file_and_link_replaced_without_old_tail` |
| 9 | 읽기 전용(0444) 일반 파일, 읽기 전용 디렉터리 안의 쓰기 가능 파일 | 못 깨뜨림. 앞은 종료 0, 내용 교체, 권한 0444 유지. 뒤는 쓰기 열기는 되지만 원자적 쓰기에서 종료 1, 파일 그대로, 임시 파일 없음. README와 같다 | `ksq2txt_output_read_only_file_and_read_only_dir_behave_as_documented` |
| 10 | 링크 `l`을 `A`(긴 일반 파일)와 `/dev/stdout` 사이에서 계속 바꾸며 `-o l`을 400회(셸) | 못 깨뜨림(재현 안 됨). `A`에 새 텍스트와 옛 꼬리가 섞인 경우 0회. 열기와 `classified_under_dev(path)` 사이 창이 아주 좁다(지적 3) | 없음 |
| 11 | `/dev/null`, `/dev/stdout`·`/dev/fd/1`·`/dev/stdout` 링크(표준 출력을 일반 파일로 돌림) | 못 깨뜨림. 모두 종료 0, 돌린 파일은 `-o` 없는 출력과 같다 | `ksq2txt_output_dev_paths_and_links_write_redirected_stdout` |
| 12 | `/dev/null/`, `/dev/null/.`, `out.txt/.`, 대상이 `/dev/null/.`인 링크, `fifo/.`(읽는 쪽 없음), `/dev/stdout/.`(돌린 표준 출력) | 못 깨뜨림. 앞 다섯은 ENOTDIR(os error 20)로 종료 1이고 FIFO는 막히지 않는다. `/dev/stdout/.`은 커널이 ENOENT를 내 원자적 쓰기로 넘어간 뒤 `/dev`에서 실패해 종료 1, 돌린 파일은 비어 있다. 일반 파일·FIFO 그대로, 임시 파일 없음 | `ksq2txt_output_trailing_slash_or_dot_fails_not_a_directory` |
| 13 | `/dev/../<임시>/out.txt` | 못 깨뜨림. 분류가 `/dev/` 밖이라 원자적 쓰기, 새 inode, 옛 꼬리 없음 | `ksq2txt_output_dev_dotdot_regular_file_replaced_atomically` |
| 14 | `txt2ksq`의 두 번 직렬화: `out.ksq`·`out.ksqz`와, 같은 확장자의 `/dev/stdout` 링크로 쓴 바이트 비교. 셸에서 `724717a` 바이너리 출력과도 비교 | 못 깨뜨림. 바이트가 같고 `.ksqz`는 다시 읽힌다. `724717a`와도 `cmp`가 같다. 비용은 일반 파일에서 직렬화 한 번과 그 바이트 버퍼가 더 드는 것뿐 | `txt2ksq_double_serialization_gives_same_bytes_as_direct_write` |
| 15 | 이 리뷰 테스트를 `724717a` 코드 사본에서 실행 | 확인. 10 통과 1 실패(`/.` 묶음 테스트, 리뷰 6이 고친 것). 시도 2–5의 4개는 옛 코드에서 통과하고 HEAD에서 실패하므로 `3874b10`이 연 회귀다 | 없음 |
| 16 | 자동 게이트 중 `cargo test -p ksq-cli`를 HEAD에서 31회 반복 | 범위 밖에서 발견. 리뷰 5 테스트 `ksq2txt_output_mixed_link_chain_to_dev_stdout_writes_redirected_file`가 2회 실패(「…/abs: No such file or directory (os error 2)」, 종료 1). `724717a` 사본에서도 40회 중 7회 실패해 이 두 커밋 전부터다. 원인은 좁히지 못했다(디버그 기록을 넣은 사본에서는 40회 재현 안 됨) | 없음(지적 4) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 일반 파일이 아닌 기존 출력은 이름 바꾸기로 일반 파일로 바뀌지 않는다(리뷰 2 지적 3) | 깨짐 | 시도 2–4(쓰기로 열 수 없는 FIFO·소켓) |
| 쓰기로 열리는 경로의 판정이 커널의 경로 풀이와 같다 | 못 깨뜨림 | 시도 12 |
| 일반 파일 `-o`는 원자적으로 바꾸고 옛 꼬리가 남지 않는다 | 못 깨뜨림(경합은 재현 안 됨) | 시도 8–10, 13 |
| `/dev/` 아래로 가는 경로는 그대로 열어 쓴다 | 못 깨뜨림 | 시도 11 |
| 실패 메시지는 실제 원인을 알린다 | 깨짐 | 시도 5 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 내보내기(도구 수준) | 충족 | `tools/ksq-cli/src/convert.rs`. 두 커밋은 출력 열기 순서와 안내 문구만 바꾼다 |
| 9.5 / IO-1 `.ksq`·`.ksqz` 입력 | 충족 | 리뷰 5·6 테스트 통과 |
| AI-18 번들 픽스처 8곡 | 충족 | `tests/fixtures.rs` 통과 |
| S0-T8 게이트: 8개 `check` 통과, 텍스트 왕복 | 충족 | `tests/fixtures.rs`, `tests/round_trip.rs` 통과 |

`-o` 쓰기 규칙은 REQ가 아니라 README와 리뷰 1–6의 보장이다. 아래 지적은 그 보장의 버그로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(실시간 경로 없음).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `tools/ksq-cli/src/cli.rs:329` `Err(_) => return Ok(false)`. ENOTDIR·EISDIR 말고는 열기 오류를 모두 버린다. 없는 파일(ENOENT)과 읽기 전용 일반 파일(EACCES)은 원자적 쓰기가 맡아야 하므로 버려도 되지만, 이미 있는 특수 파일의 열기 오류까지 버려 지적 1·2가 생긴다. 문서 주석(`:318-319`)의 「원자적 쓰기가 만들거나 같은 오류를 낸다」는 특수 파일에서 성립하지 않는다.
- 열린 핸들: `Ok(false)` 갈래와 `?` 갈래 모두 `file`이 함수 끝에서 드롭되어 원자적 쓰기 전에 닫힌다. 새는 곳은 없다.
- 열기의 부작용: 읽는 쪽 없는 FIFO에서 막히는 것은 이전과 같다(시도 6). 일반 파일은 자르기 없이 열었다 닫을 뿐 내용·크기가 바뀌지 않는다(시도 8–9).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 쓰기로 열 수 없는 기존 특수 파일을 일반 파일로 바꾼다. `write_special`이 ENOTDIR·EISDIR 말고는 열기 오류를 버리고 원자적 쓰기로 가므로, 쓰기 권한 없는 FIFO(EACCES)와 유닉스 소켓(EOPNOTSUPP)이 이름 바꾸기로 일반 파일이 되고 종료 0이다. 두 바이너리 모두. `724717a`는 `Special::Other`로 연 오류를 그대로 내 종료 1이었다. 리뷰 2 지적 3(FIFO가 일반 파일로 바뀜)의 회귀이고, README의 「그 밖은 원자적 쓰기로 간다」도 이 경우를 말하지 않는다. 고치는 방향의 예: 열기가 실패하면 원래 경로를 `fs::metadata`(링크 따라감)로 보고, 있고 일반 파일이 아니면(디렉터리 포함) 열기 오류를 그대로 낸다. ENOENT와 일반 파일의 EACCES만 원자적 쓰기로 보낸다 | `tools/ksq-cli/src/cli.rs:318-330` |
| 2 | 수정 | 같은 원인으로 오류 원인이 바뀐다. 읽기 전용으로 열린 `/dev/stdin`·`/dev/fd/N`에 `-o`를 주면 EACCES 대신 원자적 쓰기가 `/dev/fd`에 임시 파일을 만들다 낸 ENOENT(「No such file or directory」)를 알린다. 종료 1이고 데이터는 그대로라 해롭지 않지만 사용자가 엉뚱한 원인을 받는다. 지적 1의 수정(문자 장치·`/dev/fd` 대상은 일반 파일이 아니거나 `/dev/` 분류이므로 열기 오류를 그대로 낸다)으로 함께 닫힌다 | `tools/ksq-cli/src/cli.rs:329` |
| 3 | 제안 | 열기와 분류 사이 창. 연 핸들이 일반 파일이면 `classified_under_dev(path)`가 경로를 다시 푼다. 그 사이에 경로(링크)가 `/dev/stdout`을 가리키게 바뀌면 이미 연 `/dev/` 밖 일반 파일에 자르지 않고 써 옛 꼬리가 남는다. `724717a`는 분류한 경로를 열었으므로 이 조합은 없었다. 400회 시도로 재현하지 못했고 출력 디렉터리에 쓸 수 있는 다른 주체가 필요하다. 분류를 먼저 하고, 연 핸들의 (dev, ino)가 분류한 경로의 `fs::metadata`와 같을 때만 직접 쓰면 닫힌다 | `tools/ksq-cli/src/cli.rs:331-334` |
| 4 | 제안(범위 밖) | 리뷰 5 테스트 `ksq2txt_output_mixed_link_chain_to_dev_stdout_writes_redirected_file`가 가끔 실패한다(HEAD 31회 중 2회, `724717a` 40회 중 7회. 「os error 2」로 종료 1). 두 커밋 전부터라 이 리뷰 판정에는 넣지 않는다. 자동 게이트 `cargo test --workspace`가 흔들릴 수 있으니 원인(`/dev/./fd/../stdout` 사슬의 분류나 열기가 가끔 실패)을 따로 확인하길 권한다 | `tools/ksq-cli/tests/review_s0_t8_r5.rs:188-216`, `tools/ksq-cli/src/cli.rs:290-307` |

`89b794d`(안내 문구)는 README와 코드 문구가 같고(「`--max-output <어림>` 이상을 주면 크기 검사를 통과한다」), 리뷰 4–6의 안내값 테스트가 그대로 통과한다. 지적 없음.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 6 지적 1(끝 `/.`)은 해소되었다. 원래 경로로 여는 원칙 덕에 `/dev/null/.`, `fifo/.`, 링크 대상의 `/.`이 모두 커널 오류로 실패하고, 일반 파일·링크의 원자적 쓰기, `/dev/stdout`·`/dev/fd/1`, `/dev/..`, `txt2ksq`의 바이트도 그대로다. 남은 것은 지적 1이다. 열기 오류를 모두 버리는 한 줄이 리뷰 2에서 막은 「특수 파일이 일반 파일로 바뀜」을 쓰기 권한 없는 FIFO·소켓에서 다시 연다. 수정 범위는 `write_special`의 오류 갈래이고 실패 테스트 4개(`review_s0_t8_r7.rs`)가 기준이다. 재리뷰는 「`review_s0_t8_r7.rs` 11개 통과와 리뷰 1–6 테스트 유지」를 보는 좁은 확인으로 충분하다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(리뷰 테스트 diff만 먼저 보았다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p ksq-cli --test review_s0_t8_r7`: 7 통과, 4 실패. `724717a` 사본에서는 10 통과, 1 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`724717a` 뒤 변경은 `tools/ksq-cli/src/cli.rs`, `tools/ksq-cli/README.md`, `docs/reports/s0-t8/report.md`뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 1–6 테스트 diff 0바이트, `.proptest-regressions` 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
