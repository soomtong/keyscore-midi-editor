# 적대적 리뷰: S0-T5 레거시 `#` 주석 줄(결정 Q1 2차 개정) (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `920fd60` (브랜치 `s0/t5-legacy-hash-comments`, `main..HEAD`)
- 추가한 테스트: `crates/keyscore-text/tests/review_legacy_hash.rs`(12개, proptest 2개 포함), `tools/ksq-cli/tests/review_legacy_hash.rs`(3개)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. CRLF·탭·전각 공백 뒤 줄 첫머리의 `#`가 주석인가.
2. `c\n#d`: 다음 줄 첫머리의 `#`는 올림표가 아니라 주석인가. 같은 줄 `c##`는 올림표 둘인가.
3. 화음 `[ ]`·셋잇단 `n( )` 안에서 줄을 넘길 때 줄 첫머리 `#` 줄이 주석인가(`]`·`)`가 주석 안에 있으면 닫기가 아닌가).
4. 줄을 넘는 가사 따옴표 안의 `#` 줄은 글자로 남는가.
5. 헤더: `@key F#`가 줄 첫머리 `@`라 깨지지 않는가, `@key F` 다음 줄 `# minor`는 어떻게 되는가.
6. BOM + `#`: 라이브러리와 CLI가 다르게 보는가.
7. `at_line_start`가 뒤로 걷는 비용: 긴 줄에 `#`가 많을 때 제곱 시간이 되는가.
8. 레거시 꺼짐의 결과가 바뀌지 않았는가, 줄 첫머리 `#`가 없는 입력에서 켜짐과 꺼짐이 같은가.
9. 엄격·느슨 두 모드 모두 같은가.
10. `txt2ksq --legacy` 옵션 위치·중복·변형, 다른 도구에서의 거절.
11. 내보내기가 따옴표 밖 줄 첫머리에 `#`를 쓰지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 CRLF·들여쓰기 | 부분(공백·탭만) | CRLF·전각 공백 없음. 추가 |
| 2 `c\n#d` | 없음 | 추가 |
| 3 화음·셋잇단 안 | 없음 | 추가 |
| 4 가사 안 `#` 줄 | 있음 | `legacy_mode_keeps_sharp_and_error_rules_for_other_hashes` |
| 5 `@key` | 없음 | 추가 |
| 6 BOM | 없음(레거시와 조합) | 추가 |
| 7 성능 | 없음 | 추가 |
| 8 켜짐 = 꺼짐(줄 첫머리 `#` 없음) | 없음 | proptest 추가 |
| 9 두 모드 | 있음 | |
| 10 CLI 옵션 | 부분(입력 뒤 위치만) | 추가 |
| 11 내보내기 | 없음 | 추가 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | CRLF, 탭, 전각 공백(U+3000) 뒤 `#` 줄, 두 모드 | 못 깨뜨림 | `legacy_hash_after_crlf_and_indent_is_comment_in_both_modes` |
| 2 | `c\n#d e\nf` → c, f만. `c##`는 올림표 둘 | 못 깨뜨림 | `legacy_hash_at_next_line_start_after_note_is_comment_not_sharp` |
| 3 | `[c e\n# ] 가짜 닫기\n g]`, `n(c d\n  # x )\ne)` | 못 깨뜨림 | `legacy_hash_line_inside_chord_and_triplet_is_comment` |
| 4 | 가사 `"가\n# 나 // 다"` | 못 깨뜨림 | `legacy_hash_inside_lyric_spanning_lines_stays_text` |
| 5 | `@key F# minor`, `# @key C major` 줄, `@key F\n# minor`(오류), `@tempo\n# 주석\n90` | 못 깨뜨림(규칙대로) | `legacy_key_sharp_at_line_start_is_header_not_comment` |
| 6 | `@track #1 ch1`은 이름 `#1`, `@track\n#1 ch1`은 오류 | 못 깨뜨림(규칙대로) | `legacy_track_name_with_hash_mid_line_is_name` |
| 7 | BOM + `#`: 라이브러리는 오류, CLI는 BOM을 떼고 받음 | 못 깨뜨림(층 분리 일관) | `legacy_bom_is_not_whitespace_at_library_level`, `txt2ksq_legacy_reads_bom_crlf_hash_lines_with_option_anywhere` |
| 8 | 한 줄에 `   #` 2만 개 + 20만 칸 들여쓰기 뒤 주석(느슨). `at_line_start`는 첫 공백 아닌 글자에서 멈춰 선형. debug 빌드 0.06초 안 | 못 깨뜨림 | `legacy_many_hashes_on_one_long_line_is_linear` |
| 9 | 옛 기록의 줄 끝 `#` 주석 `c d # e f`: 느슨 모드는 경고 하나 뒤 `e f`를 음으로 읽음 | 규칙대로이나 지적 1 | `legacy_trailing_hash_comment_words_become_notes_in_lenient` |
| 10 | 내보내기 후 레거시로 다시 읽기: 따옴표 밖 줄 첫머리 `#` 없음, 결과 같음 | 못 깨뜨림 | `legacy_export_never_writes_hash_at_line_start_and_rereads_same` |
| 11 | proptest: 줄 첫머리 `#`가 없으면 레거시 켜짐 = 꺼짐(오류 위치 포함). 5만 회 | 못 깨뜨림 | `legacy_without_line_start_hash_equals_plain` |
| 12 | proptest: 레거시에서 줄 첫머리 `#` 하나를 `//`로 바꾼 텍스트와 결과(경고·오류 위치 포함)가 같음. `@key F`, `@key F#`, 화음, 셋잇단, CRLF, 전각 공백 섞음. 5만 회 | 못 깨뜨림 | `legacy_hash_line_equals_slash_line` |
| 13 | `--legacy`를 입력 앞·뒤·`-o` 사이, 두 번, `--strict`와 함께 | 못 깨뜨림 | `txt2ksq_legacy_reads_bom_crlf_hash_lines_with_option_anywhere` |
| 14 | `ksq2txt --legacy`, `--legacy=1`, `--Legacy`, `-legacy` → 종료 코드 2 | 못 깨뜨림 | `legacy_option_is_rejected_by_other_tools_and_variants` |
| 15 | `text_to_project` = `text_to_project_with(default)` | 못 깨뜨림 | `text_to_project_default_equals_with_legacy_off` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 모델 불변식(클립 겹침, 노트는 클립 안, 채널) | 아니오 | 주석은 `skip_trivia`에서만 건너뛰고 노트 생성 경로는 바뀌지 않음 |
| 레거시 꺼짐의 결과 불변 | 아니오 | `parse`는 `parse_with(default)`, proptest 11 |
| 내보내기는 `#` 주석을 쓰지 않음 | 아니오 | 시도 10 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 5.14 주석 `//`, `#`는 올림표 | 충족 | 꺼짐 기본, `crates/keyscore-text/src/parse.rs:333` |
| 결정 Q1 2차 개정(레거시 `#` 줄) | 충족 | `crates/keyscore-text/src/parse.rs:164`, `:333`, `tools/ksq-cli/src/cli.rs` `--legacy` |

REQ 5.14 토큰 표(`docs/REQ.md:604`)는 레거시 읽기 선택 사항을 언급하지 않는다. 레거시 모드는 문법이 아니라 읽기 선택 사항이라 미충족으로 세지 않고 지적 2(제안)로 둔다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(텍스트 파서·CLI만)
- 제품 코드의 `unwrap`/`expect`: 이번 변경에 없음
- 에러가 삼켜지는 곳: 없음. 줄 중간 `#`는 엄격 오류, 느슨 경고를 그대로 낸다

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 옛 기록의 줄 끝 `#` 주석(`c d # 2절`)은 레거시 모드에서도 줄 첫머리가 아니라서, 느슨 모드는 `#` 하나만 경고하고 뒤의 주석 글자를 음·명령으로 읽는다(시도 9). 사람 규칙대로이지만 레거시 모드의 경고 안내를 「줄 끝 `#` 주석은 레거시 모드에서도 받지 않는다. `//`로 바꾼다」처럼 더 구체적으로 하거나, README·`--legacy` 도움말에 「줄 첫머리 `#` 줄만」이라고 적으면 옛 기록 변환에서 놀람이 줄어든다 | `crates/keyscore-text/src/parse.rs:1063`, `tools/ksq-cli/README.md:11` |
| 2 | 제안 | REQ 5.14 토큰 표의 「`#`는 주석이 아니다」 옆에 레거시 읽기 선택 사항(줄 첫머리 `#` 줄)을 한 줄 덧붙인다. 2026-10-17 문구 수정 승인 범위다 | `docs/REQ.md:604` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(`cargo test -p keyscore-text -p ksq-cli -p keyscore-core` 통과, proptest는 `PROPTEST_CASES=50000`으로도 실행)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`MEMORY.md`, 결정 문서 2개, 보고서는 이번 개정의 산출물)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`main..HEAD` 테스트 diff는 추가만)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
