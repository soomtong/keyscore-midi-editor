# 적대적 리뷰: S0-T8 `ksq-cli` (리뷰 9회차)

- 리뷰어 세션: 구현자·계획자·리뷰 1–8과 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `8c5697e`(`-o`는 없는 경로와 일반 파일만), `784f715`(리뷰 2–8 테스트 기대 변경). 브랜치 `s0/t8-ksq-cli`, 기준 `cd5c138`(`git diff cd5c138 HEAD -- tools/`).
- 근거 결정: `docs/decisions/2026-10-14-ksq-cli-output-path-scope.md`(A).
- 범위 밖: 1–8회차에서 확인된 성질(어림 상한, 손실 보고, 두 번 직렬화 등). README 「남는 한계」의 확인–쓰기 사이 경합은 결함으로 세지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff cd5c138 HEAD -- tools/ksq-cli/src tools/ksq-cli/README.md`와 결정 문서만 읽고 적었다.

1. `guard_output`의 판정(`fs::metadata` 하나)과 실제 쓰기 대상(`resolve_symlink`, core `ksq::write_file`)이 갈리는 경로가 있는가. 확인은 커널 풀이(한도 32, 경로 안 디렉터리 링크 포함), 쓰기는 도구의 마지막 성분 따라가기(한도 40)다. 커널이 「없음」이나 「일반 파일」이라 했는데 도구가 다른 파일(FIFO 등)에 닿는 경우.
2. 끊긴 링크: `NotFound`로 통과한 뒤 링크 대상 경로에 새로 만드는가. 끝 `/`가 붙은 끊긴 링크가 링크 자체를 일반 파일로 바꾸지 않는가. `ksq2txt`와 `txt2ksq`(core)가 같은가.
3. `NotFound`가 아닌 확인 오류(ELOOP, EACCES, ENOTDIR, 순환)가 모두 종료 1이고 대상·링크가 그대로인가. 임시 파일이 남지 않는가.
4. `on_devfs`가 `/dev` 밖 일반 파일을 잘못 거르는가(장치 번호 충돌, firmlink, 다른 볼륨, `/dev/..` 경유). 반대로 `/dev/fd/N`·`/dev/stdout`이 일반 파일일 때 빠져나가는가.
5. 입력과 같은 파일의 거부(`same_file`)가 범위 판정보다 먼저이고 그대로인가.
6. `txt2ksq`에서 `write_special`이 빠진 뒤 core `write_file`만 남았는가. 그 사이에 쓰기 전 직렬화가 남아 있지 않은가.
7. 오류 메시지의 셸 리다이렉션 안내가 두 바이너리 모두에 맞는가.
8. 테스트 18개 변경: 입력이 그대로인가, 결정과 무관한 검사가 지워지거나 약해졌는가, 기대가 「종료 1, 대상 그대로」인가. 특수 파일을 열지 않게 된 뒤 FIFO 읽는 쪽을 두는 테스트가 멈추지 않는가.
9. 리뷰 8 실패 테스트 2개가 기대값 그대로 통과하는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 확인과 쓰기 대상의 갈림 | 리뷰 8 사슬 35개 테스트(특수 파일 끝)만 | 일반 파일 끝 사슬의 한도 경계는 없음. 통과 테스트 추가 |
| 2 끊긴 링크 | 리뷰 8 `open_failure_kinds`에 한 단계 끊긴 링크 | 두 단계 사슬, 끝 `/`, 두 바이너리 비교는 없음. 통과 테스트 추가 |
| 3 확인 오류 | 리뷰 8 `open_failure_kinds`(순환, 권한 없는 부모) | 메시지(「확인할 수 없다」, 안내)와 ENOTDIR는 없음. 통과 테스트 추가 |
| 4 `on_devfs` 오판 | 리뷰 7 `/dev/..` 일반 파일 테스트(`ksq2txt`만) | `on_devfs`를 겨냥한 테스트 없음. 통과 테스트 추가 |
| 5 같은 파일 거부 | 리뷰 1 테스트 | 범위 판정과의 순서는 없음. 통과 테스트 추가 |
| 6 `txt2ksq` 경로 | 리뷰 7 두 번 직렬화 테스트(이제 링크 거부만 봄) | 시도 9 |
| 7 메시지 | 바뀐 테스트들이 「리다이렉트」 포함만 봄 | 시도 10 |
| 8 테스트 변경 | 18개 | 시도 1·2·11 |
| 9 리뷰 8 실패 테스트 | `review_s0_t8_r8.rs` 172·189행 | 시도 2 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `git diff cd5c138 HEAD -- tools/ksq-cli/tests/`로 기대 변경 18개 확인(`cli.rs` 1, r2 1, r3 4, r4 1, r5 3, r6 1, r7 4, r8 3) | 못 깨뜨림(대체로). 실행 인자(입력·`-o` 대상)는 모두 그대로다. 결정과 무관한 검사는 지워지지 않았다: r5 `devlink`의 `d/..` 원자적 쓰기, r5 `fd1`의 `-o` 없는 실행, r7 `twice`의 `.ksqz` 되읽기, r8 `fd-regular`의 inode·임시 파일 검사가 남았다. `cli.rs` FIFO 테스트에서 빠진 「`txt2ksq -o` 바이트 = 픽스처」는 `binaries_round_trip_files_via_cli`가 본다. 바뀐 기대는 모두 「종료 1, 대상 그대로」이고, 많은 경우 `!expected.is_empty()`로 빈 기대가 우연히 맞는 것을 막았다. 다만 보고서의 「임시 파일 없음」은 18개 중 5개(`cli.rs`, r3 `fifo-epipe`, r5 `devlink`, r8 `fd-regular`, r8 `kinds`)만 실제로 검사한다(지적 2) | 없음 |
| 2 | 리뷰 8 실패 테스트 `ksq2txt_output_link_chain_over_kernel_limit_to_fifo_fails_and_keeps_fifo`, `txt2ksq_output_link_chain_over_kernel_limit_to_socket_fails_and_keeps_socket` | 못 깨뜨림. `cd5c138` 뒤 이 두 함수는 바뀌지 않았고(`git diff`의 r8 변경은 206행 뒤 통과 테스트 3개뿐) 통과한다 | 없음 |
| 3 | 끊긴 링크(한 단계·두 단계)로 두 바이너리 `-o` | 못 깨뜨림. 종료 0, 링크 대상 경로에 만들고 링크는 링크로 남는다. `ksq2txt`(`cli.rs` `resolve_symlink`)와 `txt2ksq`(core `resolve_symlink`, 같은 코드)가 같다. 결정의 「core와 같이」에 맞다 | `binaries_output_dangling_link_creates_link_target_and_keeps_link` |
| 4 | 일반 파일로 끝나는 상대 링크 사슬 31·32·33개(손으로), 그리고 `/var` 경유 절대 경로 | 못 깨뜨림. 32개까지 종료 0으로 대상 파일을 바꾸고, 33개는 ELOOP(os error 62)로 종료 1, 대상 그대로. 판정은 커널이 한다: 처음 쓴 테스트는 정규화하지 않은 `/var/folders/…` 경로라 `/var` 링크 하나가 더해져 32개에서 ELOOP였다. 도구 한도 40은 판정에 끼지 않는다(확인이 먼저 ELOOP로 막는다) | `binaries_output_link_chain_to_regular_file_kernel_limit_decides` |
| 5 | ENOTDIR(`regular/x`), 두 링크 순환, 권한 000 부모 안 파일, 변경 불가(`chflags uchg`) 일반 파일 | 못 깨뜨림. 앞 셋은 확인에서 종료 1(「확인할 수 없다」+ 안내), 대상·링크 그대로, 임시 파일 없음. `uchg`는 확인을 통과하고 이름 바꾸기가 EPERM으로 종료 1, 내용 그대로, 임시 파일 없음 | `binaries_output_unchecked_errors_exit_1_and_keep_targets`, `binaries_output_immutable_regular_file_fails_without_temp_files` |
| 6 | 읽는 쪽 없는 FIFO, FIFO 링크, 세 단계 사슬, 유닉스 소켓(두 바이너리) | 못 깨뜨림. 모두 종료 1, 종류(FIFO·소켓)와 안내, 대상·링크 그대로. 도구가 FIFO를 열었다면 쓰기 열기에서 막혔을 텐데 즉시 끝난다 | `binaries_output_fifo_socket_and_links_refused_without_opening_or_temp_files` |
| 7 | `on_devfs` 오판: `stat -f %d`로 `/dev`(1791158680), `/`·`/tmp`·`/System/Volumes/Data`(16777232), 외장·TimeMachine·Recovery(16777233·41·43), autofs `/System/Volumes/Data/home`(889192449), NFS `~/OrbStack`(436207626) 비교. 일반 파일을 그대로, `/dev/..` 경유 절대 경로로, `/dev/..`를 가리키는 링크로 `-o` | 못 깨뜨림. `/dev`와 장치 번호가 같은 다른 마운트는 없었고, 세 경로 모두 종료 0으로 새 inode가 된다. `/dev/stdout`·`/dev/fd/1`은 일반 파일을 가리켜도 장치 번호가 devfs라 거부된다(리뷰 3–8 바뀐 테스트) | `binaries_output_regular_file_off_devfs_accepted_through_dev_dotdot` |
| 8 | 끝 `/`·`/.`인 없는 경로, 끝 `/`인 끊긴 링크, `/dev/fd/<없는 이름>`, `/dev/zz-r9`, 빈 경로 `-o ""`, 빈 대상 링크(`ln -s "" emp`), 닫힌 `/dev/fd/9` | 못 깨뜨림. 앞 것들은 `NotFound`로 확인을 통과하지만 만들기에서 ENOENT·EPERM으로 종료 1이고 아무것도 생기지 않으며 링크는 그대로다(임시 파일은 지운다). 닫힌 `/dev/fd/9`는 EBADF로 확인에서 종료 1. 이 경우들의 메시지는 「입출력 오류: …」라 리다이렉션 안내가 없다(지적 5) | `binaries_output_uncreatable_missing_paths_fail_without_creating`(빈 경로·빈 링크·`/dev/zz`는 손으로만) |
| 9 | `txt2ksq`의 `guard_output` 뒤 경로를 코드로 확인 | 못 깨뜨림. `run_txt2ksq`는 변환 전에 `guard_output`을 부르고, 쓰기는 core `ksq::write_file` 하나뿐이다. 이전의 `to_bytes` 선직렬화와 `write_special`이 사라졌다 | 시도 3–8의 `txt2ksq` 쪽 |
| 10 | 같은 파일: 입력 자체, 하드 링크, 심볼릭 링크(두 바이너리) | 못 깨뜨림. 종료 1, 「입력 파일과 같다」, 입력 바이트 그대로. `same_file`이 범위 판정보다 먼저다. 범위 밖 관찰: 입력을 `/dev/stdin`으로 주고 표준 입력을 출력 파일에서 돌리면(`txt2ksq /dev/stdin -o same.txt < same.txt`) 입력이 `.ksq`로 덮인다. `/dev/stdin`의 장치 번호가 devfs라 `same_file`이 다르다고 본다. `cd5c138` 전에도 같았다(`same_file`·원자적 쓰기 갈래가 같다)(지적 4) | `binaries_output_same_file_as_input_refused_and_input_kept` |
| 11 | `cargo test -p ksq-cli` 반복 | 깨짐(흔들림). 첫 반복 루프의 첫 실행에서 `review_s0_t8_r3`이 35분 넘게 끝나지 않았다. `sample`로 보니 `ksq2txt_output_fifo_reader_closes_early_exits_1_and_keeps_fifo`의 읽는 쪽 스레드가 FIFO `read()`에서 막혀 있고(fd 7r), 테스트 스레드는 `join_reader`의 `join`에서 기다린다. FIFO를 쓰기로 연 프로세스는 없었다. `review_s0_t8_r3` 바이너리를 셋씩 동시에 10회(30회) 돌리자 1회는 `ksq2txt_output_symlinks_to_dev_null_and_fifo_write_through_and_keep_links`가 90초를 넘겨 죽였다. 따로 15회 돌리면 0회. 원인은 확정하지 못했다: 읽는 쪽이 FIFO 열기에서 막힌 뒤 `join_reader`가 `O_RDWR`로 열었다 곧 닫는 풀기가 macOS에서 그 뒤의 `read()`에 EOF를 보장하지 않는 것으로 보인다. 변경 전에는 도구가 FIFO를 열고 써서 이 풀기는 실패할 때만 탔는데, `784f715` 뒤로는 매번 탄다(그래서 r3이 매번 약 20초 걸린다). `join`에 시간 제한이 없어 게이트가 멈춘다. 나머지 반복(시간 제한 240초로 12회)은 아래 9절 | 없음(확률적이라 결정적 테스트로 못 남김. 재현: `target/debug/deps/review_s0_t8_r3-*`를 셋씩 동시에 반복) |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `-o`는 없는 경로·일반 파일만, 그 밖은 대상 그대로 종료 1(결정 A) | 아니오(경합 제외) | 시도 3–8 |
| 일반 파일은 임시 파일 + 이름 바꾸기, 실패해도 임시 파일 없음 | 아니오 | 시도 5(`uchg`), 시도 8 |
| 링크는 링크로 남고 대상 파일이 바뀜(core와 같음) | 아니오 | 시도 3·4·7 |
| 출력이 입력과 같은 파일이면 거부 | 경로·링크·하드 링크로는 아니오. `/dev/stdin` 입력으로는 예(기존, 범위 밖) | 시도 10 |
| `core` 모델 불변식 | 이 커밋은 건드리지 않음 | - |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.14 계이름 텍스트 내보내기(도구 수준) | 충족 | `tools/ksq-cli/src/cli.rs:298-336`. 두 커밋은 `-o` 대상 판정만 바꾼다. 표준 출력 경로는 그대로 |
| 9.5 / IO-1 `.ksq`·`.ksqz` 입력 | 충족 | `cargo test -p ksq-cli` 통과(9절) |

`-o` 대상 범위는 REQ가 아니라 결정 문서의 규칙이다. 아래 지적은 그 규칙과 테스트 품질의 결함으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(실시간 경로 없음).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `on_devfs`(`tools/ksq-cli/src/cli.rs:234-237`)는 `fs::metadata("/dev")` 실패를 거짓으로 삼킨다. macOS에서 `/dev`가 없을 수 없어 해를 찾지 못했다. `write_atomic`의 임시 파일 지우기 실패 무시는 기존 그대로다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `784f715`이 기대를 바꾼 리뷰 3 테스트 둘이 가끔 끝나지 않는다. 도구가 이제 FIFO를 열지 않으므로 읽는 쪽 스레드는 언제나 `join_reader`의 `O_RDWR` 열고 닫기로만 풀린다. 이 풀기 뒤에도 읽는 쪽이 `read()`에서 막힌 채 남는 일이 있고(관찰 3회: 시간 제한 없는 첫 전체 실행에서 35분 넘게, 동시 30회 중 1회, 시간 제한 둔 전체 실행 12회 중 1회), `join`에 시간 제한이 없어 `cargo test`가 멈춘다. 같은 풀기를 쓰는 리뷰 7 `ksq2txt_output_fifo_with_reader_written_directly_and_stays_fifo`는 `cat`을 시간 제한 있는 `wait`로 기다리므로 멈추지는 않지만 같은 원인으로 실패할 수 있다. 고치는 방향의 예: 바뀐 테스트에서 읽는 쪽을 두지 않는다(도구가 FIFO를 열면 쓰기 열기에서 막혀 바이너리 시간 제한이 잡는다. `review_s0_t8_r9.rs`가 이 방식이다). 또는 읽는 쪽을 `O_NONBLOCK`으로 열거나 `join`에 시간 제한을 둔다. 리뷰어 테스트의 입력은 그대로 두고 기다리는 방식만 바꾸는 것이라 결정 문서의 「입력은 바꾸지 않는다」와 어긋나지 않는다 | `tools/ksq-cli/tests/review_s0_t8_r3.rs:105-120`, `:346-387`, `:397-439`, `tools/ksq-cli/tests/review_s0_t8_r7.rs:258-287` |
| 2 | 제안 | 보고서는 18개 기대를 「종료 1, 대상 그대로, 임시 파일 없음」으로 바꿨다고 적지만, 임시 파일을 실제로 보는 것은 5개다. 거부가 쓰기 전이라 지금은 남을 수 없지만, 대상이 디렉터리 안 FIFO·링크인 테스트(r3 `special-links`, r7 `fifo`, r4 `rel-stdout`, r5 `chain`)는 검사를 더하거나 보고서 문구를 고친다. `review_s0_t8_r9.rs`가 FIFO·링크·소켓의 임시 파일을 본다 | `docs/reports/s0-t8/report.md` 「리뷰 대응 (review-8)」 끝 문단 |
| 3 | 제안 | `OutputNotRegular` 안내의 예가 언제나 `ksq2txt in.ksq > /dev/null`이다. `txt2ksq`가 거부할 때도 `ksq2txt` 예를 보인다. 바이너리 이름을 넣거나 예를 「`-o` 없이 실행하고 `> /dev/null`」로 일반화한다 | `tools/ksq-cli/src/lib.rs:76` |
| 4 | 제안(범위 밖, 기존) | 입력을 `/dev/stdin`(또는 `/dev/fd/N`)으로 주고 그 fd를 출력 파일에서 열면 `same_file`이 장치 번호(devfs 대 실제 볼륨) 차이로 다르다고 보아 입력 파일을 덮는다(`txt2ksq /dev/stdin -o same.txt < same.txt`가 종료 0, `same.txt`가 `.ksq`가 된다). README 「`-o`가 입력과 같은 파일이면 거부한다」와 계획 「CLI는 입력 파일을 덮어쓰지 않는다」에 어긋난다. 두 커밋이 만든 것이 아니고 `cd5c138` 전에도 같아서 이 회차의 판정에 넣지 않았다. 별도 결정이나 Task에서 다룰지 계획자가 정한다(예: 입력도 일반 파일·파이프로 제한, 또는 장치 번호가 devfs인 쪽은 inode만 비교) | `tools/ksq-cli/src/cli.rs:183-196` |
| 5 | 제안 | `NotFound`로 확인을 통과했지만 만들 수 없는 경로(`/dev/fd/<없는 이름>`, `/dev/<없는 이름>`, 끝 `/`)는 「입출력 오류: No such file or directory / Operation not permitted」로만 끝나 리다이렉션 안내가 없다. 해는 없고(종료 1, 아무것도 안 생김) 결정 문서도 이 경우를 따로 정하지 않는다 | `tools/ksq-cli/src/cli.rs:218`, `:341-350` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

결정 A의 제품 코드(`8c5697e`)는 깨뜨리지 못했다. 확인은 커널 풀이 하나(`fs::metadata`)이고, 쓰기 대상이 확인과 갈리는 경로(끊긴 링크, 커널 한도 근처 사슬, `/var` 경유, `/dev/..` 경유, 끝 `/`·`/.`)를 찾지 못했다. `on_devfs`도 이 기계의 어느 마운트와도 장치 번호가 겹치지 않았다. 리뷰 8 실패 테스트 2개는 기대값 그대로 통과한다.

그러나 `784f715`이 기대를 바꾼 테스트 가운데 리뷰 3 둘이 자동 게이트를 가끔 멈추게 한다(지적 1). 테스트 쪽 기다리는 방식만 고치면 되는 좁은 수정이다. 재리뷰는 지적 1의 수정 diff와, `review_s0_t8_r3` 바이너리를 동시에 여러 개 반복해도 멈추지 않는지만 보면 된다. 지적 2–5는 제안이다.

## 9. 반복 실행

- 첫 루프(`cargo test -p ksq-cli` 반복, 시간 제한 없음): 첫 실행에서 `review_s0_t8_r3`이 멈춰 35분 뒤 리뷰어가 자기 프로세스를 죽였다(지적 1).
- `review_s0_t8_r3` 바이너리의 FIFO 테스트 둘만 15회 연속: 15회 통과, 매번 약 20초(`join_reader` 시간 한도를 매번 탄다).
- `review_s0_t8_r3` 바이너리 전체를 셋씩 동시에 10회(30회, 시간 제한 90초): 29회 통과, 1회 멈춤.
- `review_s0_t8_r9.rs`를 더한 뒤 `cargo test -p ksq-cli` 12회(회당 시간 제한 240초): 11회 통과(17개 테스트 바이너리 모두 ok, `review_s0_t8_r9` 포함), 7번째 실행 1회는 `review_s0_t8_r3`의 `ksq2txt_output_symlinks_to_dev_null_and_fifo_write_through_and_keep_links`가 60초를 넘겨 240초에 죽였다(지적 1과 같은 멈춤). 그 밖의 테스트에서 흔들림은 없었다

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 diff와 결정 문서만 먼저 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p ksq-cli --test review_s0_t8_r9`: 8 통과. 시도 4의 첫 판은 `/var` 링크 때문에 실패해 경로를 정규화했다)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`cd5c138` 뒤 변경은 결정 문서, `report.md`, `tools/ksq-cli/` 아래 README·`cli.rs`·`lib.rs`·리뷰 테스트)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(이름은 그대로이고 기대만 바뀌었다. `.proptest-regressions` 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
