# 적대적 리뷰: S0-T8 `ksq-cli` (리뷰 6회차)

- 리뷰어 세션: 구현자·계획자·리뷰 1–5와 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `cb8c9d6` (브랜치 `s0/t8-ksq-cli`). 범위는 리뷰 5 수정 코드 커밋 `069fe59`(끝 `/` 경로는 원래 경로로 열어 커널 오류), `372c80c`(u64를 넘는 어림은 「어떤 상한으로도 변환 불가」)뿐이다(`git diff 688e1ee HEAD -- tools/`).
- 범위 밖: 1–5회차에서 확인된 성질(`..`·링크 사슬·별칭 판정, 어림의 위쪽 한계, 손실 보고 등)은 다시 훑지 않았다. 큰 `--max-output`으로 거대한 출력을 실제로 만드는 실험은 하지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 688e1ee HEAD -- tools/`의 `cli.rs`(`Special::Slash`, `special_output`, `write_special`), `convert.rs`(`check_estimate`), `lib.rs`(`OutputBeyondAnyLimit`)만 읽고 적었다.

1. 리뷰 1–5 테스트가 그대로인가. 두 커밋을 되돌린 사본에서 리뷰 5 실패 테스트 3개가 다시 실패하는가.
2. `Slash` 판정은 바이트 끝이 `/`인지만 본다. `Path::file_name`·`parent`는 끝의 `.` 구성요소도 버리므로(`foo/.`의 `file_name`은 `foo`), `/dev/null/.`·`/dev/stdout/.`·`fifo/.`·대상이 `x/.`인 링크가 같은 갈래로 `/dev/null` 등으로 다시 짜이지 않는가.
3. 링크 사슬 중간의 `/`(`l3 -> l2/`, `l2 -> /dev/null` 또는 일반 파일): `resolve_symlink`의 `lstat("l2/")`는 끝 `/` 때문에 링크를 따라가 실패하고 `l2/`를 돌려준다. 그래서 `Slash`가 되고 원래 경로를 열어 커널과 같은 오류가 나는가.
4. `//`, `/dev/null//`, `/dev/null/./`, `dir/`, 없는 `new/`, 디렉터리 `-o`가 모두 종료 1이고 아무것도 만들지 않는가. `Slash`의 열기는 `create` 없이 쓰기로 연다. 연 뒤 실패를 돌려주므로 쓰기·잘라 쓰기가 일어나지 않는가.
5. 일반 파일 `-o`는 여전히 원자적으로 바뀌는가(새 inode, 옛 꼬리 없음, 임시 파일 없음). `txt2ksq`도 같은가.
6. 어림이 u64 바로 아래면 `OutputTooLarge`의 `estimated`가 잘리지 않고 정확한가. 그보다 1 작은 상한은 다시 거부되는가(안내값이 최소인가). 바로 위면 상한 `u64::MAX`에서도 `OutputBeyondAnyLimit`인가. 메시지에 모순(같은 수가 같은 수를 넘는다, 안내와 불가가 함께)이 없는가.
7. 리뷰 4·5에서 통과한 `/dev/null`, `/dev/stdout`(일반 파일로 돌린 표준 출력)과 촘촘한 곡의 안내값 통과가 그대로인가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 리뷰 테스트 유지와 되돌린 사본 | 리뷰 5 실패 테스트 3개가 기준 | diff 0바이트, 사본에서 3개 실패 확인(시도 1) |
| 2 끝의 `/.` | 없음. 리뷰 5의 `<dir>/.`(디렉터리)만 있다 | 실패 테스트 넷 |
| 3 사슬 중간의 `/` | `tests/cli.rs`의 `binaries_output_with_trailing_slash_fail_with_kernel_error`는 대상이 `/dev/null/`인 링크 하나(사슬 끝) | 통과 테스트 하나(`/dev/null`과 일반 파일) |
| 4 `/` 끝 여러 꼴과 디렉터리 | 리뷰 5의 `/dev/null/`, `/dev/stdout/`, `out.txt/`, `<dir>/.` | `//`, `/dev/null//`, `/dev/null/./`, `dir/`, `new/`, 디렉터리를 통과 테스트로 더했다 |
| 5 원자적 쓰기 | 리뷰 1–5에 있다 | 두 바이너리를 한 번에 확인하는 통과 테스트를 더했다 |
| 6 u64 경계 | `convert.rs`의 `ksq2txt_estimate_beyond_u64_is_unconvertible_at_any_limit`(u64를 크게 넘는 한 점), 리뷰 5 테스트 | 바로 아래(`u64::MAX − 3007`)·바로 위(`u64::MAX + 833`) 정확한 값과 메시지 테스트를 더했다 |
| 7 `/dev/` 판정·안내값 | 리뷰 4·5 테스트 | 그대로 통과. `/dev/null`·`/dev/stdout` 통과 테스트 하나를 더했다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `git diff 688e1ee HEAD -- tools/ksq-cli/tests/review_s0_t8*.rs`, 그리고 `src/cli.rs`·`convert.rs`·`lib.rs`만 `688e1ee`로 되돌린 `/tmp` 사본(별도 target)에서 `review_s0_t8_r5` 실행 | 못 깨뜨림. diff 0바이트. 사본에서 8 통과 3 실패(리뷰 5 실패 테스트 3개 그대로). HEAD에서는 `ksq-cli` 테스트 전부 통과(리뷰 1–5: 10·7·10·10·11). 사본은 지웠다 | 없음 |
| 2 | `-o /dev/null/.` | 깨짐. 종료 0. 커널은 ENOTDIR(`: > /dev/null/.`가 실패). 경로가 `/`로 끝나지 않아 `Slash`를 비켜 가고, `file_name`·`parent`가 끝의 `.`을 버려 `Dev("/dev/null")`을 연다 | `ksq2txt_output_dev_null_with_trailing_dot_fails_not_a_directory`(실패) |
| 3 | `-o /dev/stdout/.`, 표준 출력을 일반 파일로 돌림 | 깨짐. 종료 0이고 돌린 파일에 텍스트 161바이트. 리뷰 5 시도 3과 같은 결과다 | `ksq2txt_output_dev_stdout_with_trailing_dot_fails_and_writes_nothing`(실패) |
| 4 | 대상이 `/dev/null/.`인 링크 | 깨짐. 종료 0. 링크를 따라간 경로도 `/`로 끝나지 않는다. 리뷰 4 전(`6b17d05`) 코드로 지은 사본은 이 링크를 ENOTDIR로 거부했다(리뷰 4 커밋이 연 회귀) | `ksq2txt_output_link_to_dev_null_with_trailing_dot_fails_not_a_directory`(실패) |
| 5 | `txt2ksq -o /dev/null/.` | 깨짐. 종료 0(같은 `write_special`) | `txt2ksq_output_dev_null_with_trailing_dot_fails_not_a_directory`(실패) |
| 6 | FIFO `ff/.`(읽는 쪽을 띄워 둠), 셸에서 | 깨짐. 종료 0이고 FIFO로 161바이트가 나갔다. `6b17d05` 사본도 종료 0이라 이 경로는 리뷰 4 이전부터다. 고치는 자리가 시도 2–5와 같아 테스트는 두지 않았다 | 없음(셸에서 실행) |
| 7 | 사슬 중간의 `/`: `l3 -> l2/`, `l2 -> /dev/null` / `l2 -> kept.txt` | 못 깨뜨림. 둘 다 `Slash`로 가 ENOTDIR, 종료 1, 일반 파일 그대로, 임시 파일 없음 | `ksq2txt_output_link_chain_with_slash_in_middle_fails_not_a_directory` |
| 8 | `//`, `/dev/null//`, `/dev/null/./`, `sub/`, 없는 `new/`, 디렉터리 `sub` | 못 깨뜨림. 차례로 EISDIR·ENOTDIR·ENOTDIR·EISDIR·ENOENT·EISDIR로 종료 1. `new`가 생기지 않고 `sub`가 비어 있고 임시 파일 없음. `<dir>/.`은 EINVAL로 종료 1(셸에서 확인, 리뷰 5에 테스트 있음) | `ksq2txt_output_slash_and_directory_paths_exit_1_without_leftovers` |
| 9 | 긴 기존 파일에 `ksq2txt -o out.txt`, `txt2ksq -o out.ksq` | 못 깨뜨림. 새 inode, 텍스트는 `-o` 없는 출력과 같고 `.ksq`는 읽히며 임시 파일 없음. `out.txt/.`은 원자적 쓰기에서 ENOTDIR, 종료 1(셸에서 확인) | `binaries_output_regular_file_still_replaced_atomically` |
| 10 | `-o /dev/null`, `-o /dev/stdout`(일반 파일로 돌림) | 못 깨뜨림. 둘 다 종료 0, 돌린 파일은 `-o` 없는 출력과 같다. 리뷰 4·5 테스트도 그대로 통과 | `ksq2txt_output_dev_paths_still_written_directly` |
| 11 | 어림 = `277,568 + 3840 × W`(트랙 "A", 노트 639개, 성부 많아야 640개)로 W를 골라 어림을 `u64::MAX − 3007`(바로 아래)로 맞춤. 상한 1,000,000과 `u64::MAX − 3008`로 `project_to_text_limited` | 못 깨뜨림. 두 상한 모두 `OutputTooLarge { estimated: u64::MAX − 3007 }`. 안내값보다 1 작은 상한이 거부되므로 안내값이 통과하는 가장 작은 상한이다. 안내값 자체로는 실행하지 않았다(지적 2) | `ksq2txt_estimate_just_below_u64_is_reported_exactly` |
| 12 | W를 1 늘려 어림 `u64::MAX + 833`(바로 위). 상한 1,000,000과 `u64::MAX` | 못 깨뜨림. 둘 다 `OutputBeyondAnyLimit` | `ksq2txt_estimate_just_above_u64_is_unconvertible_even_at_max_limit` |
| 13 | 시도 11·12의 `.ksq`(수 KB)를 바이너리로. 아래쪽은 기본 상한, 위쪽은 `--max-output 18446744073709551615` | 못 깨뜨림. 아래쪽 메시지는 「어림 18446744073709548608바이트가 상한 …바이트를 넘는다」와 `--max-output 18446744073709548608` 안내(어림과 같은 값). 위쪽은 「u64 범위(…)를 넘어 어떤 상한으로도 변환할 수 없다」만 있고 안내도 「바이트가 상한」 문구도 없다. 둘 다 표준 출력 비어 있음, 종료 1 | `ksq2txt_messages_around_u64_are_consistent` |
| 14 | 리뷰 6 테스트를 `688e1ee` 코드 사본(`OutputBeyondAnyLimit` 비교만 `false`로 바꿔 컴파일)에서 실행 | 확인. 3 통과 8 실패. 시도 7·8·12·13의 경계 테스트는 옛 코드에서 실패하고 HEAD에서 통과하므로, 두 커밋이 고친 것을 실제로 잡는다. 시도 2–5의 4개는 두 쪽 모두 실패 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 판정이 커널의 경로 풀이와 같다(쓰기로 열리지 않는 경로는 실패) | 깨짐 | 시도 2–6(끝의 `/.`). 끝의 `/`, `//`, 사슬 중간의 `/`는 같다(시도 7·8) |
| 일반 파일 `-o`는 원자적으로 바꾸고 옛 꼬리가 남지 않는다 | 못 깨뜨림 | 시도 9 |
| `/dev/` 아래로 가는 경로는 그대로 열어 쓴다 | 못 깨뜨림 | 시도 10, 리뷰 4·5 테스트 |
| 거부 메시지의 안내값은 어림 검사를 통과하는 가장 작은 상한이고, 그런 값이 없으면 안내하지 않는다 | 못 깨뜨림 | 시도 11–13 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 내보내기(도구 수준) | 충족 | `tools/ksq-cli/src/convert.rs`. 두 커밋은 출력 경로 판정과 거부 오류의 종류만 바꾼다 |
| 9.5 / IO-1 `.ksq`·`.ksqz` 입력 | 충족 | 리뷰 5 테스트(`.ksqz` 안내값) 통과 |
| AI-18 번들 픽스처 8곡 | 충족 | `tests/fixtures.rs` 통과 |
| S0-T8 게이트: 8개 `check` 통과, 텍스트 왕복 | 충족 | `tests/fixtures.rs`, `tests/round_trip.rs` 통과 |

`-o` 쓰기 규칙과 안내 문구는 REQ가 아니라 README와 리뷰 1–5의 보장이다. 아래 지적은 REQ 미충족이 아니라 그 보장의 버그로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(실시간 경로 없음).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `Slash`는 원래 경로를 열고 실패를 그대로 낸다. 열기가 성공하면 쓰지 않고 `InvalidInput`을 돌려준다. `create`가 없어 파일을 만들지 않고 `truncate`가 없어 자르지 않는다. 다만 열기 자체의 부수 효과(FIFO 열기, 장치 열기)는 남는다. 커널이 끝 `/` 경로를 쓰기로 열어 주는 경우를 찾지 못해 해롭지 않다. 끝의 `/.`에 대한 ENOTDIR은 여전히 판정이 경로를 고쳐 써서 사라진다(지적 1).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 끝이 `/.`인 `-o`를 받아들인다. `Slash` 판정은 바이트 끝 `/`만 보는데, `Path::file_name`·`parent`는 끝의 `.` 구성요소도 버린다. 그래서 커널이 ENOTDIR로 거부하는 `/dev/null/.`, `/dev/stdout/.`, 대상이 `/dev/null/.`인 링크, `fifo/.`을 `/dev/null`·`/dev/stdout`·`fifo`로 열어 쓰고 종료 0이다. 일반 파일로 돌린 표준 출력에도 쓴다. `txt2ksq`도 같다. 리뷰 5 지적 1과 같은 원인(경로를 다시 짜며 끝을 잃음)인데 `/`만 막았다. `Slash`의 문서 주석 「쓰기로 열리는 `/`로 끝나는 경로는 없다」의 전제도 `/.`에는 적용되지 않는다. 링크 경우는 리뷰 4 커밋이 연 회귀이고(`6b17d05`는 거부), 직접 쓴 `/dev/null/.`과 `fifo/.`은 그 전부터다. 데이터 손실은 없다. 고치는 방향의 예: 경로(와 링크를 따라간 경로)의 마지막 구성요소가 `.`이면(바이트가 `/.`로 끝나거나 `.` 자체이면) `/`와 똑같이 `Slash`로 보낸다. 더 일반적으로는 `target.parent().join(target.file_name())`이 `target`과 바이트로 다르면 특수 판정을 하지 않는다 | `tools/ksq-cli/src/cli.rs:305-313` |
| 2 | 제안 | 어림이 u64 바로 아래(예: `u64::MAX − 3007`)면 안내 「`--max-output 18446744073709548608` 이상을 주면 변환한다」는 어림 검사는 통과하지만, 그 실행은 실제로 엑사바이트급 텍스트를 만들려 한다. `372c80c`가 막은 「지킬 수 없는 약속」은 u64를 넘는 경우뿐이다. 구현자가 report.md에 「`--max-output`을 아주 크게 주면 거대한 출력도 만들려 한다」고 밝혔고 사용자가 고른 상한이라는 설계라 결함으로 세지 않는다. 문구를 「이 값 이상이면 크기 검사를 통과한다」로 바꾸면 약속과 동작이 맞는다 | `tools/ksq-cli/src/cli.rs:233-238`, `tools/ksq-cli/README.md:18` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 5 지적 1(끝의 `/`)과 지적 2(u64를 넘는 어림의 안내)는 해소되었다. 끝 `/`, `//`, 사슬 중간의 `/`, 디렉터리·없는 경로에서 커널과 같은 오류를 내고, 일반 파일의 원자적 쓰기와 `/dev/` 판정은 그대로다. u64 바로 아래에서는 어림이 정확히 안내되고 바로 위에서는 안내 없이 변환 불가를 알린다. 남은 것은 지적 1 하나다. 리뷰 5 지적 1과 같은 갈래를 `/.` 꼴로 다시 연다. 수정 범위는 `special_output`의 판정 한두 줄이고 실패 테스트 4개가 기준을 정한다. 재리뷰는 「`review_s0_t8_r6.rs` 11개 통과와 리뷰 1–5 테스트 유지」만 확인하는 좁은 확인으로 충분하다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(리뷰 테스트 diff만 먼저 보았다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p ksq-cli --test review_s0_t8_r6`: 7 통과, 4 실패. `688e1ee` 사본에서는 3 통과, 8 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`688e1ee` 뒤 변경은 `tools/ksq-cli`와 `docs/reports/s0-t8/report.md`뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰 1–5 테스트 diff 0바이트, `.proptest-regressions` 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
