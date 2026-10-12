# Task 계획: S0-T5 계이름 텍스트 파서·내보내기

- 브랜치: `s0/t5-ktext`
- 관련 REQ: 5.13(드럼 라벨 표), 5.14(문법 전체), 5.15 LYR-2(가사 표기), 6.6 IO-3, 6.8 AI-6(엄격 모드 재요청 사유)
- 선행 Task: S0-T2(모델), S0-T3(박자표)
- 관련 결정: `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md`(Q1–Q6), `docs/decisions/2026-10-12-ktext-export-rules.md`(옥타브, 성부 블록, 길이 접미)
- 관련 스파이크: `docs/spikes/sp5-llm-text-stability.md`(검증기 `spikes/sp5-llm-text-stability/ksparse.py`는 참고용, 복사하지 않음)

## 끝의 정의 (내 말로)

`keyscore-text` crate가 계이름 텍스트를 파싱해 클립(들)과 헤더 값을 만들고, 클립을 텍스트로 내보낸다.

- `parse(text, mode)`는 `@tempo`·`@time`·`@key` 헤더와 `@track` 블록들을 읽는다. 블록마다 클립 하나(시작 0, 노트는 클립 기준)를 만든다. 같은 텍스트 안의 같은 이름 블록은 한 클립의 성부로 합친다(결정 Q2). 클립 길이는 내용(가장 긴 성부의 끝)이다. (구현 중 해석) 성부의 끝은 노트의 실제 끝(보정 포함)과 쉼표 끝 가운데 가장 늦은 것이며, 노트 뒤의 명목 커서는 세지 않는다(`c8~+13`의 클립은 493틱). 커서로 세면 64분음표(60틱)보다 짧은 클립을 적을 수 없다.
- 두 모드가 있다. `Lenient`(파일·클립보드)는 모르는 토큰을 경고하고 건너뛴다. `Strict`(AI 응답)는 모르는 토큰, `~` 접미, 마디 길이 불일치를 오류로 낸다(문법 Q5·Q6).
- 오류와 경고는 1부터 세는 줄·열을 담는다.
- `export`는 클립을 문법 표의 토큰만으로 적는다. 결과를 다시 `parse`하면 원본과 같다(아래 「왕복」의 비교 기준).
- 파서는 입력 길이에 선형이다(되돌아가기 없음).

## 문법 (REQ 5.14 + 결정)

- 대소문자 구분 없음. 공백·줄바꿈은 토큰 구분이며 생략 가능(Q4). `|`는 마디선(검증만, 틱 계산에 안 씀, Q5).
- 주석: `#`가 음이름 또는 앞선 변화표 바로 뒤가 아니면 줄 끝까지 주석(Q1).
- 헤더: `@tempo <BPM>`(정수 또는 소수, `Tempo::from_bpm` 계열로 µs 반올림), `@time <분자>/<분모>`, `@key <으뜸음> major|minor` 또는 `@key none`. 으뜸음은 `C`, `C#`/`Db` … `B`. 헤더는 가져올 때만 쓰고, 클립 단위 텍스트에서는 무시한다(REQ).
- `@track <이름> ch<1–16> [prog<0–127>] [bank<0–16383>]`. 이름은 공백 없는 한 낱말 또는 큰따옴표 문자열. 채널 10이면 드럼 트랙이다.
- 음: `c d e f g a b`, 변화표 `+`/`#` 올림, `-` 내림(겹침 허용). 순서 `음이름 변화표 길이 점 ~보정 "가사" &`(Q6).
- 길이: 1·2·4·8·16·32·64와 점(`.`, `..`)만. 그 밖(`c3`, `c12`)은 오류. 없으면 현재 `l`. 틱: 온음표 3840, 점 ×1.5, 겹점 ×1.75.
- `r` 쉼표(길이 규칙 같음). `o<0–9>` 옥타브(스티키), `<` `>` ±1, `l<길이>[.]` 기본 길이. `o4 c` = MIDI 60(결정 Q1). 범위 밖 음은 오류.
- `[ … ]<길이>` 화음: 안의 음은 길이 없이. 길이·`~`·가사·`&`는 `]` 뒤. 화음 가사는 가장 높은 음에 저장(Q6).
- `n( x y z )` 셋잇단: 정확히 3개(음·쉼표·화음), 각 원소 길이(없으면 `l`)의 2/3. `)` 뒤 숫자는 오류(Q2).
- 드럼 트랙: 5.13 표의 라벨(`bd sd rs cp ch ph oh lt mt ht cr rd rb tb cb sh`)을 최장 일치로 읽는다. `n<번호>`(35–81 밖도 0–127이면 허용)도 받는다. 멜로디 트랙에서 음으로 읽을 수 없는 라벨(예: `ch`, `sd`, `oh`)은 오류다(Q4의 단점: `bd`는 `b`·`d`로 읽힌다).
- `v<0–127>`: 벨로시티(스티키, MIDI 1.0 값 → `Velocity::from_midi1`). 가져오기만. 기본은 트랙 기본 벨로시티(MIDI 1.0 100).
- `&` 타이: 다음 같은 음과 이어 한 노트로. 다음이 같은 음이 아니면 경고(Lenient)·오류(Strict).
- `"가사"`: 음 바로 뒤. 따옴표 안의 공백·기호는 그대로. `\"` 이스케이프는 받는다. (구현 중 추가) `\\`도 이스케이프다. 없으면 `\`로 끝나는 가사·이름을 적을 수 없다. 그 밖의 `\`는 글자 그대로다.
- `~<±시작>[:<±길이>]` 보정(결정 Q3). 커서(다음 토큰의 시작)는 보정 전 명목 길이로 진행한다. 보정 뒤 시작이 0 미만이거나 길이가 1 미만이면 오류. `Strict`에서는 금지.
- 마디 길이 검사: `|`를 만날 때 지난 마디선 이후의 명목 길이가 마디 길이(`@time`, 없으면 4/4)와 다르면 경고(Lenient)·오류(Strict).

## 내보내기

- 문법 표의 토큰만 쓴다(`v`, 피치 오프셋은 쓰지 않는다). 결정적이다: 같은 입력이면 같은 문자열.
- 한 성부로 적을 수 없으면(시작이나 길이가 다른 겹친 노트) 같은 이름의 블록을 더 써서 성부를 나눈다. 같은 시작·같은 길이의 노트들은 화음 `[ ]`로 묶는다.
- 그리드에 맞고 표기 가능한 길이의 노트는 접미 없이 쓴다. 그 밖은 `~` 보정을 붙인다. 쉼표로 간격을 채우고, 클립 끝까지 쉼표를 써서 가져온 클립 길이가 원본 길이와 같게 한다. (구현 중 확인한 한계) 쉼표 길이는 모두 15틱의 배수라, 클립 길이가 노트 끝도 아니고 쉼표 합으로도 닿지 않으면(15틱 배수가 아니거나 매우 짧음) 정확히 적을 수 없다. 이때는 닿는 가장 긴 길이로 줄이고, 노트가 없어 그 길이가 0이면 60틱으로 늘린다. 테스트 1의 생성기는 닿는 길이만 만들고, 테스트 2가 모든 길이에서 재내보내기 안정을 검사한다.
- 숨김 꼬리(끝 > 클립 길이)는 클립 끝에서 자른다. 드럼 트랙은 라벨이 있으면 라벨, 없으면 `n<번호>`.
- 구체 알고리즘은 구현자가 정하되, 아래 테스트를 통과해야 한다.

## 공개 인터페이스 초안

~~~rust
pub enum Mode { Lenient, Strict }
pub struct Diagnostic { line: usize, column: usize, message: String }   // 줄·열은 1부터. getter + Display
#[derive(thiserror::Error)] pub struct ParseError(Diagnostic);   // diagnostic() getter
pub struct Header { tempo: Option<Tempo>, time: Option<TimeSignature>, key: Option<Key> }   // Header::new(tempo, time, key) (구현 중 추가: export 입력용)
pub struct ParsedTrack { name: String, channel: Channel, program: Option<Program>, bank: Option<Bank>, clip: Clip }   // + into_clip()
pub struct Parsed { header: Header, tracks: Vec<ParsedTrack>, warnings: Vec<Diagnostic> }
pub fn parse(text: &str, mode: Mode) -> Result<Parsed, ParseError>;
pub struct ExportTrack<'a> { pub name: &'a str, pub channel: Channel, pub program: Option<Program>, pub bank: Option<Bank>, pub clip: &'a Clip }
pub fn export(header: &Header, tracks: &[ExportTrack<'_>]) -> String;
pub mod drum { pub fn label_of(note: u8) -> Option<&'static str>; pub fn note_of(label: &str) -> Option<u8>; }
~~~

필드는 비공개(getter)로 두되, `ExportTrack`은 입력 묶음이라 공개 필드로 둔다(`NoteRef`와 같은 이유). 클립의 노트는 `keyscore-core`의 공개 API(`Clip::new`, `insert_note`)로만 만든다. 화음·성부 때문에 같은 시작의 노트가 여럿일 수 있다.

## 왕복의 비교 기준

`export → parse`한 클립을 원본과 비교할 때 노트의 (시작, 길이, 음높이, 가사)와 클립 길이를 본다. 벨로시티(트랙 기본값을 받음), 피치 오프셋(표기 없음)은 뺀다. 원본은 숨김 꼬리를 자른 것으로 비교한다. 트랙 이름·채널·프로그램·뱅크와 헤더도 왕복한다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `export_parse_round_trip_random_clips` | 속성 | 임의 클립(겹친 노트·화음·비그리드 시작·임의 길이·가사·드럼) → 내보내기 → 파싱 = 원본(게이트) |
| 2 | `export_is_deterministic_and_reparse_stable` | 속성 | 내보내기 → 파싱 → 내보내기 = 같은 문자열 |
| 3 | `req_example_block_snapshot` | 스냅샷 | REQ 5.14 예시 블록의 파싱 결과(마디 길이 경고 3개 포함, 결정 Q3) |
| 4 | `export_snapshot` | 스냅샷 | 대표 클립의 내보내기 문자열 |
| 5 | `invalid_lengths_are_errors_with_position` | 단위 | `c3`, `c12` 오류, 줄·열 정확 |
| 6 | `drum_label_on_melody_track_is_error` | 단위 | `ch`·`sd` 오류, `bd`는 `b d` |
| 7 | `hash_is_sharp_after_note_else_comment` | 단위 | 결정 Q1 |
| 8 | `triplet_group_rules` | 단위 | 결정 Q2: 원소 3개, `)` 뒤 숫자 오류, 길이 2/3 |
| 9 | `voice_blocks_merge_into_one_clip` | 단위 | 결정 Q2(export rules) |
| 10 | `offset_suffix_start_and_length` | 단위 | `c4~+13:-20`, 커서는 명목 길이로 진행, 0 미만 오류 |
| 11 | `strict_mode_rejects_what_lenient_warns` | 단위 | 모르는 토큰, `~`, 마디 불일치 |
| 12 | `lyrics_chord_tie_order` | 단위 | 결정 Q6의 순서, 화음 가사는 가장 높은 음 |
| 13 | `octave_reference_o4_c_is_60` | 단위 | 결정 Q1(export rules), 범위 밖 오류 |
| 14 | `parse_is_linear_in_input_size` | 단위 | 입력 10배에 시간 ≈ 10배(여유 있게 30배 미만) |
| 15 | `headers_and_track_attributes` | 단위 | `@tempo` 소수, `@time`, `@key`(none, 플랫·샤프 으뜸음), `prog`, `bank`, 따옴표 이름 |

## 불변식

- 파서가 만든 클립은 `keyscore-core` 불변식을 만족한다(공개 API로 만든다).
- 파서는 패닉하지 않는다. 어떤 문자열에도 `Ok` 또는 `Err`다(퍼즈 속성 테스트를 하나 둔다).
- `keyscore-text`는 `keyscore-core`에만 의존한다(`scripts/check-deps.sh`).

## 범위 밖

- 커서 위치에 클립 놓기·트랙 매칭(S3·S7), 스트리밍 점진 파싱(S7-T2), AI 재요청(S7), 클립보드(S4).
- SMF(S1).

## 리스크

- REQ 5.14 예시의 마디 길이는 사람이 고칠 때까지 경고가 담긴 스냅샷으로 둔다(문법 Q3). 예시가 바뀌면 스냅샷만 갱신한다.
- 내보내기 가독성(그리드 노트는 접미 없음)과 정확한 왕복을 함께 만족하는 알고리즘이 핵심 위험이다.
