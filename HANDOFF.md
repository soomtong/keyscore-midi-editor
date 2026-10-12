# HANDOFF: KeyScore S0 진행 중 — S0-T5(문법 결정 대기)·S0-T8 남음

## 목표

`plan/` 계획(README → 00-conventions → 01-protocol → 02-spikes → stages)에 따라 KeyScore(Rust + GPUI, macOS 키보드 중심 MIDI 시퀀서)를 단계별로 구현한다. Task마다 「계획 → 테스트 먼저(실패 확인) → 구현 → 자동 게이트 → 보고 → 새 에이전트의 적대적 리뷰 → 수정 → `--no-ff` 병합」 루프를 돈다. 규칙은 `CLAUDE.md`와 `plan/01-protocol.md`가 기준이다.

## 현재 브랜치

`main` — S0-T1·T2·T3·T4·T6·T7 병합 완료(테스트 243개 통과). 진행 중인 Task 브랜치는 없다.

## 완료된 작업

- [x] 저장소 구조: 계획은 `plan/`, REQ 원문은 `docs/REQ.md`
- [x] 스파이크 SP-1~6 (`docs/spikes/`): S5-T2 분기 B, S3-T1 분기 A(`paint_layer` 조건), S1-T1 분기 B(`coremidi-sys`), S8-T1 안내만, SP-5 문법 결정 요청, SP-6 상한 2 s 유지
- [x] S0-T1 워크스페이스·CI (`scripts/check-deps.sh` + `.test.sh`, `deny.toml`, `.github/workflows/ci.yml`)
- [x] S0-T2 모델 타입 (Track/Clip/Note, 불변식, 숨김 꼬리)
- [x] S0-T3 템포·박자표 맵 (1/960 ns 정수 시각, `scripts/bench-check.py`, `benches/baseline.json`)
- [x] S0-T4 undo (`Document`가 프로젝트+히스토리 소유, 역편집은 crate 내부 `Op`, `TrackId`)
- [x] S0-T6 `.ksq` (결정적 쓰기, 모르는 키 보존, 임시 파일 + rename, 10만 노트 읽기 15 ms)
- [x] S0-T7 일괄 연산(퀀타이즈·조옮김·시간 이동·길이 스케일). 4회차 리뷰에서 병합 가능. 일괄 연산은 클립마다 `Edit::ReplaceNotes`(길이 늘리기 + 동시 치환, 꼬리는 편집 직전 길이로 판정)를 낸다

## 시도했으나 실패한 접근

- 공개 `Edit`에 역편집 변형을 두기: 위조·재적용으로 불변식·원자성이 깨졌다. 역편집은 crate 내부 `Op`에만 둔다.
- `History`를 공개하고 「히스토리로만 편집」을 문서로 약속: 복제·두 번째 히스토리·밖 수정으로 깨졌다. `Document`로 묶었다.
- 트랙을 인덱스로 가리키기: 밖에서 순서가 바뀌면 undo가 엉뚱한 트랙에 적용됐다. `TrackId`를 쓴다.
- `.ksq` 읽기에서 객체마다 `Value` 복제 + 노트마다 경로 문자열 미리 생성: 61 ms가 걸렸다. 소유권 이동 + 오류 때만 경로 생성으로 15 ms.
- `rm -f tests/*.proptest-regressions`: 커밋된 기존 시드까지 지웠다(S0-T6 리뷰 2 지적). 새로 생긴 파일만 이름을 지정해 지운다.
- `git add -A`: 계획 파일이 구현 커밋에 섞였다. 경로를 지정한다.
- zsh에서 `set -e`가 `cargo fmt --check` 실패 뒤에도 멈추지 않았다. 게이트 결과를 직접 확인한다.
- 일괄 연산을 `ExtendClip` + `ReplaceNote` 여러 개로 표현: 꼬리 판정 시점이 어긋나 `Ok` 묶음이 적용에서 실패했다. 클립 단위 `ReplaceNotes`로 합쳤다.
- `ReplaceNotes`를 pair 차례 적용: 앞 pair의 결과를 뒤 pair의 `from`으로 써서 분할 없이 꼬리가 생겼다. 동시 치환으로 바꿨다.
- 테스트를 커밋한 뒤 clippy를 돌림: 경고가 뒤늦게 발견됐다. 게이트는 커밋 전에 모두 돌린다.
- 단계 파일 수치를 검증 없이 옮기기: S0-T3 「1시간 = 3,456,000틱」은 60 BPM 기준이었다(정정함).

## 남은 작업

- [ ] S0-T5 계이름 텍스트 파서: `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md` 답을 기다리며 보류
- [ ] S0-T8 `ksq-cli`: 텍스트 변환이 S0-T5에 의존. `ksq check`·픽스처 생성은 먼저 할 수 있다
- [ ] S0 출구 게이트(`plan/stages/10-S0-foundation.md` 끝) + `docs/reports/stage-0-exit.md` 사람 서명. 수동 항목에 「원격 CI 첫 실행 확인」, 「벤치 기준값을 CI 러너에서 다시 잡기」 포함
- [ ] 사람의 수동 확인 대기: Surge XT 수신(SP-3), 실제 키보드·IME(SP-1), 120 Hz 디스플레이(SP-2)

결정 대기(`rg -l "상태: 결정 대기" docs/decisions/`):
- `2026-10-11-ktext-grammar-ambiguities.md` — S0-T5를 막는다(가장 급함)
- `2026-10-11-ai-model-defaults.md` — S7 전
- `2026-10-12-ksq-newer-version-save.md` — S2-T7 전
- `2026-10-12-ksq-unknown-keys-on-value-change.md` — S2-T5 전
- `2026-10-12-quantize-grid-lengths.md` — S5 전

사람이 직접 고칠 REQ 문구: 8.4(MPL-2.0 허용), LYR-1·9.5(노트 선택 속성 객체 `{"lyric": …}`).

## 핵심 파일

- `CLAUDE.md` — 저장소 규칙. 커밋 첫 줄 `<crate>: …` / `plan: …`, 에이전트가 직접 커밋·병합한다
- `plan/01-protocol.md` — Task 루프와 적대적 리뷰 규칙
- `plan/stages/10-S0-foundation.md` — S0 Task와 「이월」 줄
- `docs/reports/s0-t7/plan.md`, `report.md` — 현재 Task 계획과 리뷰 대응 기록
- `docs/decisions/2026-10-12-quantize-and-transform-rules.md` — S0-T7 규칙(개정·부록 포함)
- `crates/keyscore-core/src/transform.rs` — S0-T7 연산(`plan`, `snap`, `relocate`)
- `crates/keyscore-core/src/edit.rs` — 공개 `Edit`, 내부 `Op`(역편집), `approx_bytes`
- `crates/keyscore-core/src/document.rs` — 편집의 유일한 진입점
- `crates/keyscore-core/src/ksq.rs` — `.ksq` 읽기·쓰기
- `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.py`, `scripts/bench-check.test.sh` — 추가 게이트

## 다음 에이전트에게

1. `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md`의 「결정」 줄이 채워졌는지 본다. 채워졌으면 S0-T5를 시작한다(`plan/stages/10-S0-foundation.md` S0-T5, REQ 5.13·5.14·5.15, 결정 문서 6문항, `docs/spikes/sp5-llm-text-stability.md`). 비어 있으면 사람에게 묻는다.
2. Task 루프: 브랜치 `s0/t<번호>-<slug>` → `docs/reports/s0-t<번호>/plan.md` → 테스트 먼저(뼈대 `todo!()`로 실패 확인) → 구현 → 게이트 전부(커밋 전에) → `report.md` → 새 에이전트로 적대적 리뷰(`general-purpose`, 구현 맥락 없이, 양식 `plan/templates/adversarial-review.md`, 산출물만 남기고 커밋 금지, 기존 `.proptest-regressions` 삭제 금지) → 지적마다 커밋 하나 → 재리뷰(최대 3회) → `git merge --no-ff`.
3. 자동 게이트: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`. `core`·`text`에 외부 의존을 더하면 `scripts/check-deps.sh`의 허용 목록도 고친다.
4. REQ에 없는 결정은 임의로 정하지 말고 사람에게 묻거나 `docs/decisions/`에 결정 요청을 쓴다. 사람과의 대화는 한국어로 하고, 문서에 `**`를 쓰지 않는다.
5. S0-T5·T8이 끝나면 S0 출구 게이트를 돌리고 `docs/reports/stage-0-exit.md`를 쓴 뒤 사람 서명을 받는다. 서명 전에는 S1을 시작하지 않는다.
