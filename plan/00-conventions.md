# 00 — 규약

모든 단계·Task에 적용한다. 이 파일과 충돌하는 지시가 단계 파일에 있으면 단계 파일이 우선하고, 그 사실을 PR 설명에 적는다.

## 저장소

~~~
keyscore/
  Cargo.toml                 # workspace
  LICENSE                    # BSD-3-Clause
  NOTICE                     # 의존성 고지 (gpui-kit, Ely 등 Apache-2.0)
  crates/
    keyscore-core/           # 모델·undo·템포 맵·퀀타이즈·.ksq 직렬화. GPUI/CoreMIDI/네트워크 의존 금지
    keyscore-text/           # 계이름 텍스트 파서·내보내기 (REQ 5.14), 드럼 라벨 (REQ 5.13)
    keyscore-midi/           # CoreMIDI 가상 포트, 스케줄러 스레드, MIDI 1.0 인코딩, SMF
    keyscore-ai/             # OpenRouter 클라이언트, 템플릿, 컨텍스트 조립, 실험실 평가
    keyscore-app/            # GPUI UI (gpui-kit), 키맵, 모드 FSM, 온보딩, 설정
  tools/
    ksq-cli/                 # core+text만 쓰는 CLI (ksq↔mid↔txt 변환, 픽스처 생성)
  assets/
    prompts/                 # 시스템·과제 템플릿 (Markdown)
    fixtures/                # 실험실 픽스처 (.ksq)
    i18n/                    # ko.ftl, en.ftl
  docs/
    spikes/                  # 스파이크 결과
    decisions/               # 결정 요청·답
    reports/                 # Task 보고서, 적대적 리뷰 결과
  plan/                      # 이 폴더
~~~

의존 방향: `app → ai → text → core`, `app → midi → core`. 역방향 의존이 생기면 리뷰에서 거부한다.

## 툴체인

- Rust stable 최신. `rust-toolchain.toml`로 고정한다.
- macOS 14 이상, Apple Silicon. Intel 빌드는 CI에서만 확인한다.
- GPUI 버전은 gpui-kit이 고정한 `gpui-pre-*` 스냅샷을 따른다. crates.io의 `gpui`를 따로 추가하지 않는다(REQ 9.1).
- 의존성 추가는 허용 목록의 라이선스(BSD·MIT·Apache-2.0·Zlib·ISC·Unicode-3.0·bzip2-1.0.6·MPL-2.0)만. `cargo deny`로 검사한다. MPL-2.0 crate는 수정 없이 쓴다(`docs/decisions/2026-10-11-license-allowlist.md`).

## 코딩 규칙

- `#![deny(unsafe_code)]`를 `core`, `text`, `ai`에 둔다. `midi`와 `app`에서 unsafe가 필요하면 함수 단위로 `#[allow]`하고 이유를 주석으로 적는다.
- `clippy::all` + `clippy::pedantic` 경고 0. 예외는 항목별 `#[allow]`와 이유.
- 공개 API에는 문서 주석. 주석은 「왜」를 적는다. 코드가 말하는 「무엇」은 적지 않는다.
- 문서·주석에 `**` 강조를 쓰지 않는다.
- 에러는 `thiserror`로 타입화한다. `anyhow`는 바이너리(app, cli)에서만.
- 실시간 스레드(스케줄러)에서는 할당·락·로그 금지. 디버그 빌드에서 `assert_no_alloc`로 검사한다(REQ 8.4).
- 모델 불변식은 `core`의 타입이 지킨다. 「같은 트랙의 클립은 겹치지 않는다」「노트는 클립 안에 있다」는 생성자·메서드에서 보장하고, 깨뜨릴 수 있는 공개 API를 두지 않는다.

## 테스트 규칙

- 단위 테스트는 같은 파일의 `#[cfg(test)] mod tests`.
- 속성 기반 테스트는 `proptest`. `core`(불변식), `text`(왕복: 내보내기 → 가져오기 = 원본)에 필수.
- 스냅샷 테스트는 `insta`. 텍스트 내보내기·`.ksq` 직렬화·프롬프트 조립 결과에 쓴다.
- 통합 테스트는 `crates/<name>/tests/`. `midi`는 가상 포트 루프백(송신 → 수신 테스트 앱)으로 지연·지터를 측정한다.
- UI 테스트는 GPUI의 테스트 컨텍스트로 키 입력 → 모델 상태 변화를 검사한다. 픽셀 비교는 하지 않는다.
- 성능 테스트는 `criterion`. 기준값은 `benches/baseline.json`에 커밋하고, 10% 이상 느려지면 CI가 실패한다.
- 테스트 이름은 `동작_조건_기대결과` 형식. 예: `split_note_at_cursor_inside_note_produces_two_notes`.

## 커밋·브랜치·PR

- 브랜치: `s<단계>/t<번호>-<slug>`. 예: `s0/t3-tempo-map`.
- 커밋 메시지 첫 줄: `<crate>: <무엇을>`. 본문에 REQ ID.
- PR 하나 = Task 하나. PR 설명은 `templates/task-report.md` 형식.
- 병합 조건: 자동 게이트 통과 + 적대적 리뷰 「병합 가능」 판정 + 리뷰 지적 사항 전부 해결 또는 결정 요청으로 전환.
- `main`은 항상 빌드되고 모든 테스트가 통과한다.

## 문서

- 변경이 REQ의 의미를 바꾸면 코드가 아니라 `docs/decisions/`에 먼저 적는다. REQ 자체는 사람이 고친다.
- 각 crate에 `README.md`: 역할, 공개 API 요약, 불변식, 테스트 실행법.
- 한국어로 쓴다. 식별자·명령·파일명은 영어.
