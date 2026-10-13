# ksq-cli

`.ksq` 프로젝트와 계이름 텍스트(REQ 5.14)를 오가는 명령줄 도구와, 프롬프트 실험실 번들 픽스처 8곡(AI-18)의 생성기.

## 명령

~~~
ksq check <파일.ksq|.ksqz>...        # 불변식·형식 버전 검사. 하나라도 실패하면 종료 코드 1
ksq fixtures <디렉터리>               # 번들 픽스처 8곡을 .ksq로 쓴다
ksq2txt <입력.ksq> [-o 출력.txt]      # 프로젝트 → 계이름 텍스트. 잃는 정보는 표준 오류에 한 줄씩
txt2ksq <입력.txt> [-o 출력.ksq] [--strict]   # 텍스트 → 프로젝트. 기본은 느슨 모드, .ksqz면 gzip
~~~

- 결과는 표준 출력(또는 `-o`)에, 경고·손실·오류는 표준 오류에 쓴다.
- 종료 코드: 0 성공, 1 실패, 2 잘못된 사용법.
- `-o`가 입력과 같은 파일(심볼릭 링크·하드 링크 포함)이면 거부한다.
- 새 형식 버전 파일은 앱이 경고와 함께 열지만, `ksq check`는 실패로 본다.
- `ksq2txt`는 텍스트가 정규 `.ksq` 바이트의 256배를 넘을 것이면(먼 틱의 클립 등) 만들기 전에 거부한다. 되읽은 결과가 손실로 설명되지 않게 다르면(`Loss::Mismatch`) 출력을 쓰지 않는다. 둘 다 종료 코드 1.

## 공개 API

- `check(path) -> Result<CheckReport, CliError>`
- `project_to_text(&Project) -> Result<(String, Vec<Loss>), CliError>`: 헤더는 틱 0의 템포·박자표와 조. 트랙마다 클립을 시작 0 클립 하나로 합친다. 쓴 텍스트를 다시 읽어 대조한다.
- `text_to_project(&str, Mode) -> Result<(Project, Vec<Diagnostic>), CliError>`: 블록마다 트랙 하나와 시작 0 클립 하나. 트랙 색은 순번대로 팔레트를 돈다.
- `fixtures()`, `write_fixtures(dir)`: 픽스처 8곡.
- `cli::{ksq, ksq2txt, txt2ksq}`: 바이너리 본체.

## 불변식

- 내부 의존은 `keyscore-core`와 `keyscore-text`뿐이다(`scripts/check-deps.sh`).
- `Loss`가 비어 있으면 `ksq2txt → txt2ksq` 결과의 `.ksq`가 원본과 바이트까지 같다.
- 픽스처는 `fixtures-src/*.txt`(엄격 모드로 읽히는 손으로 쓴 원곡)에서 만들고, `assets/fixtures/*.ksq`는 생성기 결과와 바이트까지 같다. 원본 텍스트를 고치면 다시 만든다.

~~~
cargo run -p ksq-cli --bin ksq -- fixtures assets/fixtures
~~~

## 테스트

~~~
cargo test -p ksq-cli
~~~
