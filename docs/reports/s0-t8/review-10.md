# 적대적 리뷰: S0-T8 `ksq-cli` (리뷰 10회차)

- 리뷰어 세션: 구현자·계획자·리뷰 1–9와 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `f5dceee`(FIFO 읽는 쪽 제거), `845a917`(연 입력 핸들과 같은 파일 비교), `ba8047a`(바이너리별 안내, `OutputUncreatable`), `4cd5dad`(gzip 검사 단언 메시지). 브랜치 `s0/t8-ksq-cli`, 기준 `b0670c9`(`git diff b0670c9 HEAD -- tools/`), HEAD `4814103`.
- 근거 결정: `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`(A).
- 범위 밖: 1–9회차에서 확인된 성질. 결정 문서·README의 「남는 한계」(확인–쓰기 경합)는 결함으로 세지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff b0670c9 HEAD -- tools/ksq-cli/src tools/ksq-cli/README.md`와 결정 문서만 읽고 적었다.

1. `open_input`이 `guard_output`보다 먼저 불린다. 입력을 여는 것만으로 생기는 부작용이 있는가: FIFO 입력의 열기 멈춤, 쓰는 쪽에 주는 SIGPIPE, 디렉터리 입력, 입력 열기 실패와 `-o` 거부 가운데 어느 오류가 먼저 나오는가.
2. fstat(입력 핸들)과 stat(출력 경로)의 (dev, ino) 비교가 같은 파일을 놓치는 경로: `/dev/stdin`, `/dev/fd/N`, 하드 링크, 심볼릭 링크(양쪽), firmlink(`/System/Volumes/Data`), `.ksqz` 입력.
3. 반대로 다른 파일을 같다고 보는 경로: 표준 입력이 파이프일 때 `/dev/stdin`과 있는 일반 파일 `-o`.
4. 같은 파일 판정이 `match` 첫 갈래라 범위 판정보다 먼저다. `-o`가 `/dev/stdin`·`/dev/fd/0`이고 입력이 같은 fd일 때 무엇으로 거부되는가.
5. `OutputUncreatable`: `was_missing`이 참일 때 모든 쓰기 실패(EACCES, ENOSPC, core `to_bytes` 오류)를 「만들지 못했다」로 바꾼다. 문구가 맞는가, 경로를 두 번 적지 않는가, 있는 파일 실패에는 붙지 않는가.
6. 바이너리별 예가 두 바이너리와 세 오류(`OutputNotRegular`, `OutputUnchecked`, `OutputUncreatable`)에 모두 맞는가.
7. 테스트 변경: 읽는 쪽만 지웠고 입력·기대는 그대로인가. 리뷰 9 경계 테스트(`review_s0_t8_r9.rs`)가 그대로인가.
8. 멈춤·흔들림: 리뷰 9의 r3 멈춤이 사라졌는가. 신호로 끝난 자식(`code() == None`)이 다시 나오는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 입력을 먼저 여는 부작용 | 없음 | 시도 5·6. 실패 테스트 추가 |
| 2 같은 파일 놓침 | `cli.rs` `binaries_refuse_output_equal_to_input_given_as_dev_stdin`(`/dev/stdin`만), 리뷰 1·9(경로·링크) | `/dev/fd/3`, firmlink, `.ksqz`, 입력 쪽 링크는 없음. 통과 테스트 추가 |
| 3 파이프 표준 입력 | 없음 | 통과 테스트 추가 |
| 4 `-o /dev/stdin` 순서 | 리뷰 7 `read_only_dev_stdin`(입력은 별도 파일) | 시도 4(손으로) |
| 5 `OutputUncreatable` | `cli.rs` `output_errors_show_redirect_example_for_each_binary`(없는 디렉터리, `/dev/<없는 이름>`) | EACCES, 끊긴 링크, 경로 한 번, 있는 파일 실패는 없음. 통과 테스트 추가 |
| 6 바이너리별 예 | 위 `cli.rs` 테스트(`/dev/null`·없는 디렉터리·`/dev/<없는 이름>`) | 시도 7 |
| 7 테스트 변경 | - | 시도 1·2 |
| 8 멈춤·흔들림 | - | 9절 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `git diff b0670c9 HEAD -- tools/ksq-cli/tests/`로 FIFO 테스트 셋 확인(r3 `special-links`·`fifo-epipe`, r7 `fifo`) | 못 깨뜨림. 지운 것은 읽는 쪽(스레드·`cat`), `join_reader`, 그것에 딸린 단언(`got.is_empty()`, `read == 0`, `read.stdout.is_empty()`, r7의 `!melody_text().is_empty()`)뿐이다. 실행 인자(입력·`-o` 대상)와 기대(종료 1, 링크·FIFO 그대로, 「리다이렉트」 안내)는 그대로이고, r3 `special-links`에 디렉터리 목록 검사, r7 `fifo`에 `temp_leftovers` 검사가 더해졌다. 도구가 FIFO를 쓰기로 열면 읽는 쪽이 없어 열기에서 막히므로 바이너리 시간 제한(r3 `wait_with_timeout`, r7 `wait` 30초)이 잡는다 | 없음 |
| 2 | `review_s0_t8_r9.rs` 변경 여부(`git diff b0670c9 HEAD --stat -- tools/ksq-cli/tests/review_s0_t8_r9.rs`) | 못 깨뜨림. 바뀌지 않았다. `cli.rs`의 변경은 새 테스트 둘과 `check_reads_gzip_and_warns_without_version` 단언 메시지뿐이고 기대는 그대로다 | 없음 |
| 3 | 같은 파일: `/dev/stdin`(표준 입력=출력 파일), `/dev/fd/3`(`3<` 출력 파일), 하드 링크, 출력→입력 심볼릭 링크, 입력→출력 심볼릭 링크, `.ksqz` 입력(경로·`/dev/stdin`), `/System/Volumes/Data/private/tmp/…` firmlink, `/private/tmp/…` 대 `/tmp/../p/./…`, 디렉터리 입력과 같은 디렉터리 `-o`. 두 바이너리 | 못 깨뜨림. 모두 종료 1 「입력 파일과 같다」, 파일 바이트 그대로. fstat이 `/dev/fd` 너머의 실제 파일 (dev, ino)를 준다. firmlink 두 경로는 `stat -f '%d %i'`가 같다(16777232). 다른 볼륨의 같은 inode 번호는 만들 수단이 없어 실행하지 못했다(dev가 비교에 들어가는 것은 코드로 확인) | `binaries_refuse_output_equal_to_input_given_as_dev_fd_3`, `binaries_refuse_output_equal_to_input_through_links_and_firmlink`, `binaries_directory_input_fails_without_creating_output` |
| 4 | 표준 입력이 파이프일 때 `/dev/stdin` 입력과 새 `-o`·있는 일반 파일 `-o`. 그리고 `ksq2txt q.ksq -o /dev/stdin < q.ksq`, `/dev/stdin -o /dev/stdin`, `/dev/fd/0 -o /dev/fd/0` | 못 깨뜨림. 파이프는 종료 0이고 출력 바이트가 파일 입력 때와 같다. `-o /dev/stdin`류는 출력 stat의 dev가 devfs라 같은 파일 갈래를 지나 `OutputNotRegular`(「/dev/fd 등 장치 파일 시스템의 파일」)로 종료 1, 입력 그대로 | `binaries_dev_stdin_pipe_input_writes_existing_regular_output` |
| 5 | FIFO 입력, 쓰는 쪽 없음, 거부될 `-o`(`/dev/null`, 같은 FIFO, 디렉터리, `txt2ksq -o /dev/stdout`(표준 출력이 파이프)). `b0670c9`와 HEAD 바이너리를 3초 시간 제한으로 비교 | 깨짐. `b0670c9`는 다섯 경우 모두 1초 안에 종료 1(범위 거부 또는 「입력 파일과 같다」). HEAD는 다섯 경우 모두 `open(O_RDONLY)`에서 멈춰 SIGKILL(137)로 끝났다. `845a917`이 `open_input`을 `guard_output`보다 앞에 두었기 때문이다. 대상은 건드리지 않으므로 데이터 손실은 없지만, 거부할 명령이 쓰는 쪽을 기다리며 끝나지 않는다(지적 1) | `binaries_fifo_input_with_refused_output_exit_1_without_waiting_for_writer`(HEAD 실패, `b0670c9`에서 통과 확인) |
| 6 | FIFO 입력에 쓰는 쪽(`cat in.ksq > ff`)이 있을 때 `-o /dev/null` | 깨짐(시도 5와 같은 원인). HEAD는 입력을 열고 곧 거부해 닫으므로 쓰는 쪽이 SIGPIPE로 죽는다(종료 141). `b0670c9`는 입력을 열지 않아 쓰는 쪽은 다른 읽는 쪽을 계속 기다린다 | 시도 5의 테스트로 함께 잡는다(쓰는 쪽은 테스트에 두지 않았다) |
| 7 | 디렉터리 입력, 없는 입력 + 거부될 `-o`, 읽기 권한 없는 입력 + 거부될 `-o` | 못 깨뜨림. 디렉터리는 열기가 되고 읽기에서 「d: 입출력 오류: Is a directory」로 종료 1, `-o`는 생기지 않는다. 없는·읽기 금지 입력은 이제 입력 오류가 먼저 나온다(`b0670c9`는 `-o` 거부가 먼저). 둘 다 종료 1이고 대상은 그대로라 결함으로 세지 않았다 | `binaries_directory_input_fails_without_creating_output` |
| 8 | `OutputUncreatable`: 쓰기 권한 없는 디렉터리 안 새 경로(EACCES), 없는 디렉터리를 가리키는 끊긴 링크, 그리고 같은 디렉터리의 있는 일반 파일(`was_missing` 거짓). 두 바이너리, 경고가 있는 텍스트 입력 | 못 깨뜨림(문구만 제안). 종료 1, 경로 한 번, 그 바이너리의 예(`ksq2txt in.ksq > out.txt` / `txt2ksq in.txt > out.ksq`), 아무것도 안 생김, 링크 그대로. 있는 파일 실패는 「입출력 오류: Permission denied」로 안내가 붙지 않는다. 다만 EACCES에도 「-o는 없는 경로나 일반 파일만 받는다. 특수 파일에 쓰려면 … 리다이렉트하라」가 붙는데, 대상은 바로 허용된 「없는 경로」이고 셸 리다이렉션도 같은 EACCES로 실패한다(지적 2). core `to_bytes` 실패도 이 갈래로 가지만 메모리 안 gzip이라 일으킬 수 없었다 | `binaries_uncreatable_output_reports_once_with_own_example` |
| 9 | 바이너리별 예를 세 오류에서 확인(`OutputNotRegular`: `/dev/null`·디렉터리·`/dev/stdin`, `OutputUnchecked`: 리뷰 9 경우, `OutputUncreatable`: 시도 8) | 못 깨뜨림. `Tool`이 `Options`에 실려 세 오류 모두 그 바이너리의 예를 보인다 | 시도 8의 테스트 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 출력이 입력과 같은 파일이면 거부, 입력 그대로 | 아니오(리뷰 9 지적 4의 `/dev/stdin` 경로가 닫혔다) | 시도 3·4 |
| `-o`는 없는 경로·일반 파일만, 그 밖은 대상 그대로 종료 1(결정 A) | 대상 쪽은 아니오. 다만 FIFO 입력이면 종료 1에 이르지 못하고 멈춘다 | 시도 5 |
| 거부할 때 아무것도 하지 않는다(`guard_output` 주석 「변환하기 전에 하므로 거부할 때 아무것도 쓰지 않는다」) | 쓰기는 아니오. 입력 FIFO를 여는 부작용(멈춤, 쓰는 쪽 SIGPIPE)은 예 | 시도 5·6 |
| `core` 모델 불변식 | 이 커밋들은 건드리지 않음 | - |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 내보내기(도구 수준) | 충족 | `tools/ksq-cli/src/cli.rs:337-385`. 네 커밋은 입력 열기 순서, 같은 파일 판정, 오류 문구만 바꾼다. 표준 출력 경로는 그대로 |
| 9.5 / IO-1 `.ksq`·`.ksqz` 입력 | 충족 | 시도 3의 `.ksqz`, `cargo test -p ksq-cli` 통과(9절) |

`-o` 범위와 입력 보호는 REQ가 아니라 결정 문서·계획의 규칙이다. 지적 1은 그 규칙의 동작 결함으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(실시간 경로 없음).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `output_error`(`tools/ksq-cli/src/cli.rs:251-266`)가 `CliError::Io`·`Ksq`의 원래 오류를 문자열 `detail`로 바꾼다. `source` 사슬은 끊기지만 문구는 남아 정보 손실은 없다. 그 밖은 기존 그대로.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `845a917`이 입력을 `-o` 확인보다 먼저 연다. 입력이 FIFO이고 쓰는 쪽이 없으면, `b0670c9`에서 곧바로 종료 1이던 거부(`-o /dev/null`, 디렉터리, `/dev/stdout`, 입력과 같은 FIFO)가 `open(O_RDONLY)`에서 끝없이 멈춘다. 쓰는 쪽이 있으면 도구가 열자마자 거부하고 닫아 쓰는 쪽이 SIGPIPE로 죽는다. 데이터 손실은 아니지만 「거부할 때 아무것도 하지 않는다」에서 물러난 회귀이고 테스트가 없다. 고치는 방향의 예: 경로만 보는 범위 판정(`fs::metadata(output)`로 일반 파일·`NotFound` 여부)을 입력을 열기 전에 하고, (dev, ino) 비교만 입력을 연 뒤에 한다. 그러면 같은 FIFO는 범위 판정에서 먼저 거부된다. `review_s0_t8_r10.rs`의 실패 테스트가 이것을 본다 | `tools/ksq-cli/src/cli.rs:343-344`, `:495-496` |
| 2 | 제안 | `OutputUncreatable`이 `was_missing`인 모든 쓰기 실패에 「-o는 없는 경로나 일반 파일만 받는다. 특수 파일에 쓰려면 … 리다이렉트하라」를 붙인다. 쓰기 권한 없는 디렉터리의 새 경로(EACCES)는 허용된 「없는 경로」이고 셸 리다이렉션도 같은 이유로 실패하므로 안내가 오해를 부른다. 안내를 ENOENT·EPERM(없는 디렉터리, `/dev/<없는 이름>`, 끝 `/`)에만 붙이거나, 문구를 「만들 수 없는 경로다」 쪽으로 바꾼다 | `tools/ksq-cli/src/lib.rs:95-104`, `tools/ksq-cli/src/cli.rs:251-266` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 9의 지적 1(멈춤)은 고쳐졌다. FIFO 테스트 셋은 읽는 쪽만 지웠고 입력과 기대는 그대로다. 리뷰 9 경계 테스트는 바뀌지 않았다. 같은 파일 판정(입력 핸들 fstat 대 출력 stat)은 `/dev/stdin`, `/dev/fd/3`, 하드·심볼릭 링크 양쪽, firmlink, `.ksqz`, 파이프 표준 입력 어디서도 깨뜨리지 못했다. 반복 실행에서 멈춤·실패·신호 종료는 0이다.

그러나 같은 커밋 `845a917`이 입력 열기를 `-o` 확인 앞으로 옮기면서 FIFO 입력의 거부가 멈추게 되었다(지적 1). 좁은 순서 수정으로 고칠 수 있다. 재리뷰는 지적 1의 수정 diff, `review_s0_t8_r10.rs`의 실패 테스트가 기대값 그대로 통과하는지, 같은 파일 통과 테스트가 여전히 통과하는지만 보면 된다.

## 9. 반복 실행

모두 HEAD `4814103`을 `git archive`로 `/tmp`에 풀어 별도 타깃 디렉터리에서 돌렸다(리뷰 10 테스트 파일이 섞이지 않게). `timeout -s KILL 300`.

- `cargo test -p ksq-cli` 단독 15회: 15회 통과(테스트 바이너리 17개 모두 ok), 회당 27–50초(첫 회만 50초).
- `cargo test -p ksq-cli` 동시 3개 × 6묶음(18회, 타깃 디렉터리를 셋으로 복제해 실제로 동시에 돌림): 18회 통과, 회당 26–56초.
- 리뷰 9 재현 방식: `review_s0_t8_r3`·`review_s0_t8_r7` 바이너리를 셋씩 동시에 10묶음(각 30회, 시간 제한 90초): 60회 모두 통과. r3는 회당 약 2.3초(리뷰 9 때 `join_reader` 시간 한도로 약 20초).
- 모든 로그에서 `FAILED`, `panicked`, 신호 종료(`signal`, `None`)는 0건이다. `check_reads_gzip_and_warns_without_version`의 신호 종료는 다시 나오지 않았다.
- `review_s0_t8_r10`: HEAD에서 6개 가운데 5 통과, 1 실패(지적 1, 다섯 경우 모두 5초 시간 제한에서 멈춤). 같은 파일을 `b0670c9` 소스에 넣어 돌리면 지적 1 테스트는 통과하고, 리뷰 9가 고친 `/dev/fd/3` 같은 파일 테스트와 `OutputUncreatable` 테스트는 실패한다(이번 커밋들이 고친 것을 확인).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 diff와 결정 문서만 먼저 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p ksq-cli --test review_s0_t8_r10`: 5 통과, 1 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`b0670c9` 뒤 변경은 `report.md`, `tools/ksq-cli/` 아래 README·`cli.rs`·`lib.rs`·테스트)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(이름은 그대로이고 읽는 쪽과 그에 딸린 단언만 빠졌다. `.proptest-regressions` 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
