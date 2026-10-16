# 변경 보고: 계이름 텍스트 주석을 `//`로 (결정 Q1 개정)

- 브랜치: `s0/t5-slash-comments`
- 근거: 사람 지시(2026-10-16) 「토큰 표: 주석은 `//`으로 통일하고 `#`을 주석으로 사용하지 않도록 한다(복잡함을 줄이자)」. `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md` Q1 개정
- 관련 REQ: 5.1(입력 예), 5.14(예시 블록, 토큰 표)
- 성격: S0-T5(계이름 텍스트 파서·내보내기)의 동작 변경. Task 루프(테스트 먼저 → 구현 → 게이트 → 적대적 리뷰 → 병합)를 따른다.

## 바뀐 동작

| 입력 | 전 | 후 |
| --- | --- | --- |
| `// 메모`, `c4// 메모` | 모르는 토큰 `/` 둘 | 줄 끝까지 주석 |
| `# 메모`(줄 첫머리·공백 뒤) | 주석 | 모르는 토큰 `#`. 엄격 모드 오류, 느슨 모드 경고 후 `#` 한 글자 건너뜀(뒤의 글자는 토큰으로 읽힘) |
| `c4# d` | `c4`, 뒤는 주석 | `c4`, 모르는 토큰 `#`, `d` |
| `c#`, `c+#`, `@key F# minor` | 올림표 | 그대로 |
| `@track Lead#2 ch1` | 이름 `Lead`, 나머지는 주석 | 이름 `Lead#2` |
| `@track A//x ch1` | 이름 `A//x` | 이름 `A`, 나머지는 주석 |
| 가사·따옴표 이름 안의 `#`, `//` | 글자 | 글자 |
| 내보내기 트랙 이름 | `#`가 들면 따옴표 | `//`가 들면 따옴표, `#`는 그대로 |

## 고친 곳

| 파일 | 내용 |
| --- | --- |
| `crates/keyscore-text/src/parse.rs` | `skip_trivia`가 `//`만 주석으로 본다(`Scanner::at_comment`). 따옴표 없는 트랙 이름의 끝 판정 |
| `crates/keyscore-text/src/export.rs` | 따옴표 없는 이름 판정(`//` 포함이면 따옴표) |
| `crates/keyscore-text/tests/parse.rs` | REQ 예시 상수, 주석 테스트, 새 테스트 `hash_is_not_a_comment`, `track_name_may_contain_hash_and_ends_at_comment` |
| `crates/keyscore-text/tests/export.rs` | 새 테스트 `track_names_with_hash_stay_bare_and_comment_marks_are_quoted` |
| `crates/keyscore-text/tests/review_s0_t5.rs` | 리뷰어 테스트 `grammar_decisions_hold`의 주석 입력을 `//`로(검사하는 결정이 바뀌었다) |
| `tools/ksq-cli/fixtures-src/*.txt` | 첫 줄 주석 8개 |
| `docs/REQ.md` | 5.1 입력 예, 5.14 예시 블록과 토큰 표(사람 지시에 따라 에이전트가 고침) |
| `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md` | Q1 개정 |

바꾸지 않은 것
- `docs/spikes/sp5-llm-text-stability.md`와 `spikes/`의 검증기는 당시 기록이라 그대로 둔다.
- proptest 회귀 파일의 `#`는 proptest 형식이다.
- README의 `#`는 셸 예시의 주석이다.

## 테스트 결과

~~~
keyscore-core, keyscore-text, ksq-cli 등(루프백 crate 제외): 479 passed, 0 failed
REQ 예시 insta 스냅샷, 픽스처 8곡 .ksq: 바뀌지 않음(주석은 결과에 남지 않는다)
~~~

keyscore-midi·ksq-play 루프백 테스트는 같은 시각에 돈 S1 출구 게이트의 1시간 측정을 흐트러뜨리지 않으려고 그 측정 뒤에 돌린다. 이 변경은 두 crate에 닿지 않는다.

## 리뷰어에게

1. 느슨 모드에서 `# 메모`는 `#` 한 글자만 건너뛰고 경고한다. 뒤의 `메모`는 글자마다 모르는 토큰이나 음으로 읽힐 수 있다. 옛 `#` 주석을 쓴 파일은 경고가 많아지고, 음이름 글자(`a`–`g`)가 있으면 음이 생길 수 있다. 결정의 결과로 받아들였다. 대신 `#`로 시작하는 줄 전체를 건너뛰는 이행 규칙을 둘지는 사람이 정할 일이다.
2. `@time 4/4`의 `/`와 주석의 `//`가 섞이는 입력(`@time 4//4`).
