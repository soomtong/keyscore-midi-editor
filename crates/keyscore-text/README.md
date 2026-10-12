# keyscore-text

계이름 텍스트(KeyScore Text, REQ 5.14) 파서·내보내기와 드럼 라벨 표(REQ 5.13).

## 공개 API

- `parse(text, Mode) -> Result<Parsed, ParseError>`: 헤더(`@tempo`·`@time`·`@key`)와 `@track` 블록을 읽는다. 블록마다 클립 하나(시작 0)를 만들고, 같은 이름의 블록은 한 클립의 성부로 합친다. `Mode::Lenient`(파일·클립보드)는 모르는 토큰·마디 길이 불일치·맞지 않는 타이를 경고하고, `Mode::Strict`(AI 응답)는 그것들과 `~` 보정을 오류로 낸다.
- `Parsed`·`ParsedTrack`·`Header`: 결과. 필드는 getter로만 읽는다.
- `Diagnostic`·`ParseError`: 1부터 세는 줄·열(문자 단위)과 메시지.
- `export(&Header, &[ExportTrack]) -> String`: 문법 표의 토큰만으로 적는다. 결정적이고, 다시 읽으면 노트(시작·길이·음높이·가사)·클립 길이·트랙 속성·헤더가 같다. 예외는 `export`의 문서 주석(같은 이름의 트랙, 쉼표로 닿지 않는 클립 길이).
- `drum::label_of`·`drum::note_of`: GM 퍼커션 라벨 표(REQ 5.13).

문법의 모호한 곳은 `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md`와 `docs/decisions/2026-10-12-ktext-export-rules.md`가 정하고, 이 crate가 정한 나머지 해석은 `parse`의 문서 주석에 모아 둔다.

## 불변식

- `keyscore-core`에만 의존한다.
- GPUI·CoreMIDI·네트워크 crate에 전이적으로도 의존하지 않는다(`scripts/check-deps.sh`).
- `unsafe` 금지.
- 파서는 어떤 문자열에도 패닉하지 않고, 입력 길이에 선형이다(되돌아가지 않는다).
- 파서가 만든 클립은 `keyscore-core`의 공개 API(`Clip::new`, `insert_note`)로 만든다.
- 속성 기반 테스트: `tests/round_trip.rs`(내보내기 → 가져오기 = 원본, 재내보내기 안정, 퍼즈).

## 테스트

~~~
cargo test -p keyscore-text
~~~
