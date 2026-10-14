# 단계 출구: S0 기반

- 기준 커밋: `main` `9354eef`(S0-T8 병합)
- 작성: 계획자 에이전트, 2026-10-14
- 서명: YoungTaek, 2026-10-15

## Task 결과

| Task | 내용 | 리뷰 | 보고서 |
| --- | --- | --- | --- |
| S0-T1 | 워크스페이스·CI, 의존 검사 스크립트 | 병합 가능 | `docs/reports/s0-t1/` |
| S0-T2 | 모델 타입(Track/Clip/Note, 불변식, 숨김 꼬리) | 병합 가능 | `docs/reports/s0-t2/` |
| S0-T3 | 템포·박자표 맵, 정수 시각, 벤치 게이트 | 병합 가능 | `docs/reports/s0-t3/` |
| S0-T4 | undo(`Document`, 내부 역편집 `Op`) | 병합 가능 | `docs/reports/s0-t4/` |
| S0-T5 | 계이름 텍스트 파서·내보내기 | 리뷰 9회, 병합 가능 | `docs/reports/s0-t5/` |
| S0-T6 | `.ksq` 읽기·쓰기 | 병합 가능 | `docs/reports/s0-t6/` |
| S0-T7 | 일괄 연산(퀀타이즈·조옮김·이동·스케일) | 리뷰 4회, 병합 가능 | `docs/reports/s0-t7/` |
| S0-T8 | `ksq-cli`(`ksq check`, `ksq2txt`, `txt2ksq`), 픽스처 8곡 | 리뷰 11회, 병합 가능 | `docs/reports/s0-t8/` |

## 자동 게이트 (계획자가 `9354eef`에서 실행)

- [x] `cargo fmt --all -- --check`: 통과
- [x] `cargo clippy --workspace --all-targets -- -D warnings`: 경고 0
- [x] `cargo test --workspace`: 471 passed, 0 failed
- [x] `cargo deny check`: advisories·bans·licenses·sources ok
- [x] `scripts/check-deps.sh`: ok
  - `cargo tree`로 core·text·ksq-cli의 실제 의존 그래프를 보고, GPUI·CoreMIDI·reqwest를 비롯한 금지 crate가 없음을 확인한다.
  - CI(`.github/workflows/ci.yml`)에서도 같은 스크립트가 돈다.
- [x] `scripts/check-deps.test.sh`: 15 passed
- [x] `scripts/bench-check.test.sh`: 8 passed
- [x] 픽스처 게이트: `assets/fixtures/*.ksq` 8개가 `ksq check`를 통과하고 텍스트로 왕복한다(`tools/ksq-cli/tests/fixtures.rs`).

## 사람이 확인할 항목

- [x] (사람: confirmed) 텍스트 하나를 손으로 써서 `txt2ksq → ksq2txt` 왕복 결과를 눈으로 확인한다. 예:
  ~~~
  cargo run -q -p ksq-cli --bin txt2ksq -- my.txt -o my.ksq
  cargo run -q -p ksq-cli --bin ksq2txt -- my.ksq
  ~~~
  쓰는 법은 `tools/ksq-cli/fixtures-src/*.txt`와 REQ 5.14를 참고한다. 같은 시작·길이 노트는 화음으로 묶이고, 마디를 넘는 타이는 한 음으로 적히는 등 표기가 바뀔 수 있다. 음(시작·길이·음높이·가사)이 같으면 왕복 성공이다.
- [x] (사람: it's ok) 픽스처 8곡(`tools/ksq-cli/fixtures-src/*.txt`)이 원곡인지 확인한다. 널리 알려진 선율을 베끼지 않았는지 사람이 들어 보거나 읽어 본다. 리뷰어도 눈으로는 겹치는 것을 찾지 못했다.
- [ ] (사람: 건너뜀, remote 없음) 원격 CI 첫 실행: 저장소에 remote가 아직 없다. remote를 만들고 push한 뒤 `.github/workflows/ci.yml`이 통과하는지 확인한다.
- [ ] (remote 없음으로 함께 미룸, S8-T3) 벤치 기준값: `benches/baseline.json`은 개발 기계에서 잡은 값이다. CI 러너에서 다시 잡는다(S8-T3 이월과 함께).
- [x] REQ 문구(결정 문서에서 넘어온 것): 사람이 에이전트에게 수정을 허락했고(2026-10-15), `fab2fa2`에서 고쳤다. 5.14 예시가 바뀌어 스냅샷 테스트도 갱신했다(`d2acdd3`)
  - 8.4: MPL-2.0 허용
  - LYR-1·9.5: 노트 선택 속성 객체 `{"lyric": …}`
  - 5.14: 예시 블록의 마디 길이, 같은 이름 성부 블록, `~` 길이 접미

## 결정 요약 (S0에서 사람이 정한 것)

- `docs/decisions/` 결정됨: 라이선스 허용 목록, 클립 분할·벨로시티·피치 오프셋, 템포 단위·박자 변경, 절대 음높이 범위, `.ksq` 노트 형식, 퀀타이즈·일괄 연산 규칙, 계이름 텍스트 문법 6문항, 내보내기 규칙, 구현 세부(위임), 내보내기 성부 예산(개정 포함), `ksq-cli` `-o` 범위
- 결정 대기(S0를 막지 않음):
  - `2026-10-11-ai-model-defaults.md`: S7 전
  - `2026-10-12-ksq-newer-version-save.md`: S2-T7 전
  - `2026-10-12-ksq-unknown-keys-on-value-change.md`: S2-T5 전
  - `2026-10-12-quantize-grid-lengths.md`: S5 전

## 알려진 한계와 이월

- 계이름 텍스트 엄격 모드 왕복의 알려진 예외와 출력 크기 상한(선형, 256배): `docs/decisions/2026-10-12-ktext-implementation-details.md`
- `ksq2txt`
  - 텍스트가 나르지 않는 정보(템포·박자 변화, 클립 배치, 벨로시티 등)는 손실로 보고한다.
  - 아주 긴 촘촘한 곡(4분음표 트랙 약 1,400마디 이상)은 크기 어림 때문에 거부될 수 있고, `--max-output`으로 푼다.
  - `-o`는 일반 파일만 받는다.
- 이월 항목(단계 파일의 「이월」 줄): S2-T5, S2-T6, S2-T7, S3, S4-T2, S4-T3, S7-T2, S8-T3

## 서명

S1을 시작해도 좋다면 아래에 이름과 날짜를 적는다.

- 확인자: YoungTaek
- 날짜: 2026-10-15
