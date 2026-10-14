# 적대적 리뷰: S0-T8 `ksq-cli` (리뷰 11회차)

- 리뷰어 세션: 구현자·계획자·리뷰 1–10과 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `af2c855`(`-o` 범위 판정을 입력 열기 앞으로), `473d0d2`(만들지 못한 `-o` 문구를 원인별로 나눔). 브랜치 `s0/t8-ksq-cli`, 기준 `005ccb5`(`git diff 005ccb5 HEAD -- tools/`), HEAD `b866b9a`.
- 근거 결정: `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`(A).
- 범위 밖: 1–10회차에서 확인된 성질. 결정 문서·README의 「남는 한계」(확인–쓰기 경합)는 결함으로 세지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 005ccb5 HEAD -- tools/ksq-cli/src tools/ksq-cli/README.md`와 결정 문서만 읽고 적었다.

1. 리뷰 1–10 테스트 파일이 바뀌지 않았는가. `cli.rs` 테스트 변경은 단언 추가뿐인가.
2. `guard_output`이 입력을 열기 전에 끝나므로, 쓰는 쪽 없는 FIFO 입력에 거부될 `-o`(장치, 디렉터리, `/dev/stdout`, `/dev/stdin`, `/dev/fd/9`, 같은 FIFO와 그 하드·심볼릭 링크)가 곧바로 종료 1인가. 받아들여질 `-o`(없는 경로)는 입력 열기에서 기다리는가(정상).
3. 새 갈래 `same_identity`는 입력 경로를 `stat`만 하는가. FIFO 입력을 열지 않는가. 입력이 없거나 디렉터리일 때 무엇을 알리는가.
4. 같은 파일 판정이 `refuse_same_file`로 옮겨진 뒤에도 하드 링크·심볼릭 링크·`/dev/stdin`·`/dev/fd/3` 입력에서 입력을 지키는가.
5. 문구 분기: `special_place`가 `Path::starts_with("/dev")`(글자 기준)라 `/dev/..`로 시작하는 경로, cwd가 `/dev`인 상대 경로, `/dev`를 가리키는 끊긴 링크에서 원인과 문구가 어긋나는가.
6. `special_kind`: EPERM을 `raw_os_error() == Some(1)`로 본다. EACCES에 끝 `/`가 붙으면 어느 쪽인가.
7. 두 바이너리에서 경로가 한 번만 적히는가. `txt2ksq`의 `KsqError::Io` 문구가 경로를 덧붙이지 않는가.
8. 반복 실행에서 멈춤·실패·신호 종료가 다시 나오는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 리뷰 테스트 불변 | 해당 없음 | `git diff 005ccb5 HEAD -- tools/ksq-cli/tests/review_s0_t8*.rs` 비어 있음. `cli.rs`는 주석과 `리다이렉트` 포함 여부 단언 하나만 더함 |
| 2 FIFO 입력 + 거부될 `-o` | `review_s0_t8_r10.rs`(`/dev/null`, 같은 FIFO, 디렉터리) | 하드·심볼릭 링크 FIFO는 없음. 통과 테스트 추가 |
| 3 `same_identity` | r10 디렉터리 입력 테스트 | FIFO 별칭은 없음. 위 테스트에 포함 |
| 4 같은 파일 | r10(`/dev/fd/3`, 링크, firmlink), `cli.rs`(`/dev/stdin`) | `/dev/stdin` 입력 + 링크 `-o` 조합은 없음. 통과 테스트 추가 |
| 5 경로 모양 오분류 | 없음 | 제안 1. 동작만 확인하는 통과 테스트 추가 |
| 6 EACCES + 끝 `/` | 없음 | 통과 테스트 추가 |
| 7 경로 한 번 | r10 `binaries_uncreatable_output_reports_once_with_own_example`(EACCES, 끊긴 링크) | `/dev/<없는 이름>`·끝 `/`·`/.`은 없음. 통과 테스트에 포함 |
| 8 반복 실행 | 해당 없음 | 9절 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 리뷰 1–10 테스트 파일 diff | 못 깨뜨림. 비어 있다. `.proptest-regressions` 그대로 | 없음 |
| 2 | 쓰는 쪽 없는 FIFO 입력(`ksq2txt ff`)에 `-o` `/dev/null`, `ff`, 하드 링크 `ffh`, 심볼릭 링크 `ffs`, 디렉터리, `/dev/stdout`, `/dev/stdin`, `/dev/fd/9`. 3초 알람 | 못 깨뜨림. 모두 1초 안에 종료 1. 같은 FIFO·링크는 「입력 파일과 같다」, 나머지는 범위 거부·확인 오류(EBADF). FIFO 그대로 | `binaries_fifo_input_with_fifo_alias_or_special_output_exit_1_without_opening_input` |
| 3 | 같은 FIFO 입력에 받아들여질 `-o`(`/dev/zz`, `nodir/x`, `x/`, `x/.`) | 예상대로 입력 열기에서 기다린다(알람 종료). `-o`가 「없는 경로」라 범위 안이고, 어떤 리더든 쓰는 쪽을 기다리는 것은 정상이라 결함으로 세지 않았다 | 없음(FIFO에 상대를 두지 않는 규칙) |
| 4 | 표준 입력을 FIFO에 `O_RDWR`로 열어 두고 `ksq2txt ff -o /dev/stdin`, `ksq2txt /dev/stdin -o ff`, `txt2ksq /dev/stdin -o ff` | 못 깨뜨림. 셋 다 곧바로 종료 1 「FIFO」 범위 거부(macOS fdesc가 다른 신원을 줘 「입력과 같다」가 아닌 범위 거부로 나온다. 문구 차이일 뿐) | 없음 |
| 5 | 디렉터리 입력 `dd`에 `-o dd/.`, `dd/` 입력에 `-o ddl`(→`dd`), 없는 입력에 `-o dd`, 없는 입력·디렉터리 입력에 새 `-o` | 못 깨뜨림. 앞 둘 「입력 파일과 같다」, 셋째 범위 거부, 나머지는 입력 오류(ENOENT, EISDIR). 아무것도 생기지 않음 | 없음(r10 디렉터리 테스트와 겹침) |
| 6 | 같은 파일: 입력 경로·`/dev/stdin`(표준 입력 = 그 파일)·`/dev/fd/3` × `-o` 같은 경로·하드 링크·심볼릭 링크, 두 바이너리. `-o /dev/stdin`(표준 입력 = 입력 파일·링크) | 못 깨뜨림. 모두 종료 1 「입력 파일과 같다」, `-o /dev/stdin`은 devfs 범위 거부. `shasum -c` 그대로 | `binaries_same_file_after_reorder_through_links_and_dev_stdin_keep_input` |
| 7 | 문구 분기, 두 바이너리: `/dev/zz`·`/dev/zz/`·`/dev/zz/.`·`/dev/./zz`·`//dev/zz`·`/./dev/zz`(EPERM), `/dev/fd/new`(ENOENT), `new/`·`new/.`·`nodir/x/`·`nodir/x/.`(ENOENT), `nodir/x`(ENOENT), 쓰기 금지 디렉터리 `ro/new`·`ro/new/`·`ro/new/.`(EACCES) | 못 깨뜨림. 앞 다섯 묶음은 리다이렉트 안내, `nodir/x`와 `ro/…`는 「다른 경로를 주거나」. 경로는 한 번, 그 바이너리의 예, `ro`·cwd에 임시 파일 없음 | `binaries_uncreatable_output_wording_splits_by_special_place_and_errno` |
| 8 | 경로 모양과 원인이 어긋나는 경우: `/dev/../tmp/r11/nodir/x`(실제는 없는 디렉터리, ENOENT), cwd `/dev`에서 상대 `-o zz`(EPERM), `/dev/zz`·`nodir/x`를 가리키는 끊긴 링크 | 문구만 어긋남(제안 1). 첫째는 리다이렉트 안내, 둘째·셋째는 「다른 경로를 주거나」. 동작은 모두 종료 1, 아무것도 생기지 않음. 링크 경우는 구현자 보고서에 이미 적혀 있다 | `binaries_uncreatable_output_with_misleading_path_shape_exit_1_without_creating`(문구는 단언하지 않음) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 출력이 입력과 같은 파일이면 거부, 입력 그대로 | 아니오 | 시도 2·5·6 |
| `-o`는 없는 경로·일반 파일만, 그 밖은 대상 그대로 종료 1(결정 A) | 아니오 | 시도 2·4·5 |
| 거부할 때 입력을 열지 않는다(리뷰 10 지적 1) | 아니오. 거부될 `-o`는 FIFO 입력을 열지 않고 끝난다 | 시도 2·4 |
| `core` 모델 불변식 | 이 커밋들은 건드리지 않음 | - |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 내보내기(도구 수준) | 충족 | `tools/ksq-cli/src/cli.rs:391-436`. 두 커밋은 판정 순서와 오류 문구만 바꾼다. 표준 출력 경로는 그대로 |
| 9.5 / IO-1 `.ksq`·`.ksqz` 입력 | 충족 | `cargo test -p ksq-cli` 반복 통과(9절) |

`-o` 범위와 입력 보호는 REQ가 아니라 결정 문서·계획의 규칙이다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(실시간 경로 없음).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `same_identity`·`refuse_same_file`이 `fs::metadata` 실패를 「같지 않다」로 본다(`tools/ksq-cli/src/cli.rs:249-268`). `same_identity`는 이미 거부될 대상의 문구만 고르고, `refuse_same_file`은 `guard_output`이 본 대상이 그 사이 사라진 경합에서만 실패하므로 결정의 「남는 한계」 안이다. `output_error`의 `detail` 문자열화는 리뷰 10에서 본 그대로다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `special_place`는 경로 글자(`Path::starts_with("/dev")`, 끝 `/`·`/.`)만 본다. `/dev/../<없는 디렉터리>/x`(ENOENT)는 리다이렉트 안내가 나오고, cwd가 `/dev`인 상대 경로 `zz`(EPERM)나 `/dev/<없는 이름>`을 가리키는 끊긴 링크는 「다른 경로를 주거나」가 나온다. 동작(종료 1, 아무것도 안 생김)은 같고 결정·README가 「경로 모양은 메시지를 고르는 데만 쓴다」고 밝혔으므로 문구 취향으로 둔다. 바꾼다면 `resolve_symlink` 뒤의 경로의 부모를 `canonicalize`해 `/dev`와 비교하는 정도면 된다 | `tools/ksq-cli/src/cli.rs:299` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 10 지적 1(FIFO 입력에서 거부가 멈춤)은 고쳐졌다. 거부될 `-o`는 FIFO 입력을 열지 않고 곧바로 종료 1이고, 같은 FIFO의 하드·심볼릭 링크도 `stat`만으로 「입력과 같다」로 끝난다. 같은 파일 판정은 순서를 바꾼 뒤에도 하드 링크·심볼릭 링크·`/dev/stdin`·`/dev/fd/3`에서 입력을 지킨다. 문구 분기는 결정 A의 정의대로 갈리고 두 바이너리 모두 경로를 한 번만 적는다. 남은 것은 문구만의 제안 하나다. 리뷰 1–10 테스트는 바뀌지 않았고 반복 실행에서 멈춤·실패·신호 종료는 없다.

## 9. 반복 실행

작업 트리(HEAD `b866b9a` + `review_s0_t8_r11.rs`)에서 `timeout 300 cargo test -p ksq-cli`. 빌드는 먼저 한 번 해 두었다.

- 단독 10회: 10회 모두 종료 0. 회마다 테스트 바이너리 19개 결과가 모두 ok, 139개 통과.
- 동시 3개 × 4묶음(12회): 12회 모두 종료 0, 같은 139개 통과. 가장 긴 테스트 바이너리 시간 약 24초.
- 모든 로그에서 `FAILED`, `panicked`, `signal`은 0건이다. `timeout`으로 끝난 실행도 없다.
- `review_s0_t8_r11`: 4개 모두 통과(약 0.7초).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 diff와 결정 문서만 먼저 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p ksq-cli --test review_s0_t8_r11`: 4 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`005ccb5` 뒤 변경은 `report.md`, `tools/ksq-cli/` 아래 README·`cli.rs`·`lib.rs`·`tests/cli.rs`)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`tests/cli.rs`는 단언 하나가 더해졌을 뿐. `.proptest-regressions` 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
