# 적대적 리뷰: S0-T5 계이름 텍스트 주석을 `//`로 (결정 Q1 개정) (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 세션. 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `b75f7ae` (브랜치 `s0/t5-slash-comments`, `main` 대비)
- 실행 환경 제약: 백그라운드 MIDI 측정 때문에 keyscore-midi·ksq-play 테스트와 `cargo test --workspace`는 돌리지 않았다. `CARGO_TARGET_DIR=target/alt`, `cargo test --no-fail-fast -p keyscore-text -p ksq-cli -p keyscore-core`.

## 1. 코드만 읽고 적은 「내가 검사할 것」

변경 diff(테스트 diff 포함)를 한 번에 받아 읽었으므로 테스트를 전혀 보지 않은 상태는 아니다. 아래는 기존 테스트 파일을 열기 전에 `parse.rs`의 trivia 호출 자리를 따라가며 적었다.

1. `skip_trivia`를 부르는 모든 자리(문서 첫머리, `@tempo/@time/@key` 뒤, `@track` 이름·`ch`·`prog`·`bank` 사이, 화음 `[ ]` 안, 셋잇단 `n( )` 안, `&`·`~±`·`"가사"`·`|`·`o`·`l`·`<`·`>`·드럼 라벨 뒤)에서 `//` 주석이 먹히는가.
2. `@time 4//4`가 4/4로도, 「4/ 뒤 주석」으로도 읽히지 않고 오류인가.
3. 파일 끝 줄바꿈 없는 `//`, `///`, 빈 `//`, `//` 뒤 같은 줄의 `@track`.
4. 홑 `/`가 모든 자리(본문, 화음, 셋잇단, 파일 끝)에서 모르는 토큰인가.
5. 토큰 자리의 `#`가 엄격 모드 오류, 느슨 모드 경고 후 한 글자 건너뜀인가. 특히 첫 `@track` 앞(헤더 영역)에서도 그런가.
6. 따옴표 없는 트랙 이름의 끝 판정(`/`, `#`, `//`)과 내보내기의 따옴표 판정이 서로 맞는가. `/`·`#`·`"`·`\`·공백이 섞인 이름과 가사의 export → parse 왕복.
7. 기존 왕복 proptest가 여전히 통과하는가, 그 생성기가 `/`를 만들기는 하는가.
8. `#` 주석을 설명하던 문서·코드가 모두 바뀌었는가(REQ 5.1·5.14, 결정, 픽스처 원본, keyscore-text 문서).
9. 기존 테스트가 지워지거나 `#[ignore]`되지 않았는가, `review_s0_t5.rs`의 수정이 결정 변경만 따르는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 모든 trivia 자리의 `//` | 부분 | `parse.rs`의 `hash_is_sharp_after_note_and_comment_starts_with_slashes`는 본문 음 뒤·줄 첫머리만. 헤더·화음·셋잇단·`&`·`~`·가사 뒤는 없었다 |
| 2. `@time 4//4` | 없음 | 보고서가 리뷰어에게 넘긴 질문 |
| 3. EOF·`///`·빈 `//` | 없음 | |
| 4. 홑 `/` | 부분 | `c / d` 하나 |
| 5. 토큰 자리 `#` | 부분 | 트랙 블록 안 세 자리만. 헤더 영역은 없다 |
| 6. 트랙 이름·가사 왕복 | 부분 | 예시 세 개(`Lead#2`, `A//B`, `A/B`). 기존 proptest 글자 집합(`round_trip.rs:46`)에 `/`가 없어 `//`가 생성되지 않는다 |
| 7. 기존 proptest | 있음 | 통과 |
| 8. 문서 | 해당 없음 | 수동 확인 |
| 9. 테스트 삭제 | 해당 없음 | 수동 확인 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-text/tests/review_comments.rs`에 있고 실제로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 헤더 셋, `@track` 줄과 다음 줄의 `prog`, `|`, `o`, `l`, 음, 화음 안 두 곳과 `]4` 뒤, 셋잇단 안 두 곳과 `)` 뒤, `&` 뒤, 가사 뒤, `>`, `<c4.` 뒤에 `//` 주석. 주석 없는 입력과 헤더·트랙이 같다(두 모드). 가사 안 `//`는 글자 | 못 깨뜨림 | `comments_in_every_trivia_position_are_skipped` |
| 2 | `~+13//`, `~+0:-20//`(느슨), 드럼 `bd4//`, `n36//`, `[bd sd// z\n]4` | 못 깨뜨림 | `comments_after_offset_and_drum_labels_are_skipped` |
| 3 | `@time 4//4` 두 모드 오류(1행, `@time` 메시지), `@time 4/4//x`는 4/4 | 못 깨뜨림 | `time_with_double_slash_is_error` |
| 4 | `c// 끝`(EOF), `c//`, `c///`, `/// d`, 빈 `//`, `///@track B ch2`(주석 안 지시어), 주석뿐인 문서 | 못 깨뜨림 | `comment_edges_at_eof_triple_slash_and_empty` |
| 5 | 홑 `/`: `c /`(EOF), `c/`, `c / / d`(경고 2), 화음·셋잇단 안. 엄격 오류, 느슨 경고 | 못 깨뜨림 | `lone_slash_is_unknown_token_everywhere` |
| 6 | 트랙 블록 안 토큰 자리 `#`: 화음·셋잇단 안, `|` 뒤, `&` 뒤, 가사 뒤, `c4.#`, `@track A ch1 #`, `bank0#`. 엄격 오류(메시지에 `#`), 느슨 `#` 경고 1. `c#//`, `d-#4//`, `c##` | 못 깨뜨림 | `hash_in_token_positions_inside_track_is_unknown` |
| 7 | 첫 `@track` 앞의 `#`: `# 옛 주석`, `@tempo 120 # 선택`, `@key C major # 선택`을 느슨 모드로 | 깨짐. 느슨 모드도 `@track 블록 밖에는 헤더(@tempo·@time·@key)만 쓸 수 있다` 오류로 끝난다 | `hash_before_first_track_is_lenient_warning` (실패) |
| 8 | 따옴표 없는 이름 `A/`, `/`, `#/#`, `A/B// x`(다음 줄 `ch1`), `Lead#2// ch9`(주석 속 ch9 무시), 따옴표 `"A // B"`, `"#"`. `@track //x`는 이름 없음 오류 | 못 깨뜨림 | `bare_track_names_with_slash_and_hash` |
| 9 | proptest: `a / # 공백 " \ 가 @ c h 1 탭 줄바꿈`으로 만든 트랙 이름과 가사 → export → parse(엄격) → 이름·노트 같음, 다시 export하면 같은 문자열 | 못 깨뜨림 | `names_and_lyrics_with_slashes_round_trip` |

결과: 9개 중 8개 통과, 1개 실패. 기존 테스트 전체(keyscore-core, keyscore-text, ksq-cli)는 통과(487 통과, 실패는 위 1개뿐).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| export → parse 왕복(이름·가사) | 못 깨뜨림 | 시도 8, 9. `export.rs:114`의 `contains("//")`와 `parse.rs:451–455`의 `at_comment` 판정이 같은 규칙이다 |
| 주석은 결과에 남지 않는다 | 못 깨뜨림 | 시도 1, 4. 픽스처 `.ksq`가 바뀌지 않는 테스트(`ksq-cli` `fixtures.rs`) 통과 |
| 엄격 모드는 경고가 없다(오류로 바뀐다) | 못 깨뜨림 | 시도 5, 6 |
| 느슨 모드의 모르는 토큰은 경고 후 건너뜀(결정 Q1 개정 문구) | 깨뜨림 | 시도 7. 첫 `@track` 앞에서는 오류 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| REQ 5.14 토큰 표 `//` 줄 끝까지 주석 | 충족 | `crates/keyscore-text/src/parse.rs:147`, `:297–307`; 시도 1–4 |
| REQ 5.14 「`#`는 주석이 아니다(음이름 뒤 올림표에만)」 | 충족 | `parse.rs:716–724`, `:412`; 시도 6 |
| REQ 5.14 예시 블록 | 충족 | `docs/REQ.md:569–572`, `tests/parse.rs` `req_example_block_snapshot`(스냅샷 불변) |
| REQ 5.1 입력 예 주석 | 충족 | `docs/REQ.md:138` |
| REQ 5.14 느슨한 가져오기(모르는 토큰 경고 후 건너뜀, `plan/stages/10-S0-foundation.md:38`) + 결정 Q1 개정 「느슨 모드는 경고 후 그 한 글자를 건너뛴다」 | 부분 충족(미충족으로 셈) | `parse.rs:283–287` 헤더 영역의 모든 비 `@` 토큰은 모드와 무관하게 오류. 시도 7 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 없음(keyscore-text).
- 제품 코드의 `unwrap`/`expect`: 이번 diff에 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: 없음. 반대로 시도 7은 경고로 끝나야 할 자리가 오류가 된다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 첫 `@track` 앞의 `#`(줄 첫머리 `# 메모`, `@tempo 120 # 메모`, `@key C major # 메모`)가 느슨 모드에서도 `@track 블록 밖에는 헤더(@tempo·@time·@key)만 쓸 수 있다` 오류로 파싱 전체를 끝낸다. 결정 Q1 개정(「토큰 자리의 `#`와 홑 `/`는 … 느슨 모드는 경고 후 그 한 글자를 건너뛴다」)과 변경 보고서의 표(「`# 메모`(줄 첫머리·공백 뒤) → 느슨 모드 경고 후 `#` 한 글자 건너뜀」), 리뷰어에게 1(「옛 `#` 주석 파일은 경고가 많아진다」)과 어긋난다. 옛 REQ 예시와 옛 픽스처 원본처럼 첫 줄이 `#` 주석인 파일이 바로 이 경우라, 이행 영향이 보고서 설명(경고 증가)보다 크다(가져오기 실패). 메시지도 원인(`#`는 이제 주석이 아니다, `//`를 쓴다)을 알려 주지 않는다. 고치는 방법은 둘 중 하나: (a) 느슨 모드에서 헤더 영역의 모르는 토큰도 경고 후 건너뛰게 해 결정 문구에 맞추거나, (b) 헤더 영역의 오류를 의도로 결정·보고서 문구를 좁히고 `#`일 때 `//` 안내를 메시지에 넣는다. (b)는 결정 문구 변경이므로 결정 문서에 먼저 적는다 | `crates/keyscore-text/src/parse.rs:283–287`, `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md:21`, `docs/reports/s0-t5-comments/report.md` 「바뀐 동작」 표 2행·「리뷰어에게」 1. 테스트 `hash_before_first_track_is_lenient_warning` |
| 2 | 제안 | 기존 왕복 proptest의 글자 집합에 `/`가 없어 `//`가 든 이름·가사를 만들지 않는다. 이번 변경의 핵심 경계(따옴표 판정)가 기존 proptest로는 덮이지 않는다. 글자 집합에 `/`를 더하면 된다. 이번 리뷰의 proptest가 같은 경계를 덮으므로 거절해도 된다 | `crates/keyscore-text/tests/round_trip.rs:46`, `tools/ksq-cli/tests/round_trip.rs:63`. 테스트 `names_and_lyrics_with_slashes_round_trip` |
| 3 | 제안 | 트랙 블록 안의 `#` 경고·오류 메시지가 `모르는 토큰 '#'를 건너뛴다`뿐이다. 옛 `#` 주석을 쓴 사용자와 AI 응답(엄격 모드 오류 메시지가 재요청 프롬프트로 갈 수 있다)에 「주석은 `//`」 안내를 붙이면 복구가 쉽다 | `crates/keyscore-text/src/parse.rs:1003–1028`. 테스트 `hash_in_token_positions_inside_track_is_unknown` |

문서 점검(지적 없음)
- REQ 5.1(`docs/REQ.md:138`), 5.14 예시 블록(`:569–572`)과 토큰 표(`:604`), 결정 Q1 개정, 픽스처 원본 8개 첫 줄, `parse.rs`·`export.rs` 주석이 모두 `//`로 바뀌었다. `crates/keyscore-text/README.md`와 keyscore-ai에는 주석 문법 설명이 없다.
- `docs/spikes/sp5-llm-text-stability.md`, `spikes/*/ksparse.py`, `docs/reports/s0-t5/plan.md`의 `#` 주석 서술은 당시 기록이라 그대로인 것이 맞다.
- `tools/ksq-cli/tests/review_s0_t8.rs:262`의 이름 `a#b`, `//x` 왕복 테스트도 통과한다.

기존 테스트 점검
- `tests/parse.rs`의 `hash_is_sharp_after_note_else_comment`는 이름이 `hash_is_sharp_after_note_and_comment_starts_with_slashes`로 바뀌고 주석 입력이 `#`에서 `//`로 바뀌었다. 기대값과 검사 항목 수는 그대로이고 따옴표 안 `//` 검사가 더해졌다. 삭제·`#[ignore]` 없음.
- `tests/review_s0_t5.rs` `grammar_decisions_hold`: `d4#` → `d4//`와 주석 한 줄만 바뀌었다. 「길이 뒤 주석」이라는 검사 의도를 바뀐 결정에 맞춰 옮긴 것이고 기대값은 같다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(수정)이 결정 문구와 실제 동작의 불일치이고, 실패 테스트가 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다 (diff에 포함된 테스트 변경은 이미 봤다. 1절 머리말)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다 (새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
