# HANDOFF: KeyScore S1 진행 중 — S1 Task 전부 병합, S1 출구 서명 대기

## 목표

`plan/` 계획(README → 00-conventions → 01-protocol → 02-spikes → stages)에 따라 KeyScore(Rust + GPUI, macOS 키보드 중심 MIDI 시퀀서)를 단계별로 구현한다. Task마다 「계획 → 테스트 먼저(실패 확인) → 구현 → 자동 게이트 → 보고 → 새 에이전트의 적대적 리뷰 → 수정 → `--no-ff` 병합」 루프를 돈다. 규칙은 `CLAUDE.md`와 `plan/01-protocol.md`가 기준이다.

## 현재 브랜치

`main` — S0 완료·서명(2026-10-15), S1-T1~T7과 계이름 텍스트 주석 `//` 변경 병합(테스트 818개 통과). `docs/reports/stage-1-exit.md` 사람 서명 대기. 서명 전에는 S2를 시작하지 않는다.

## 완료된 작업

- [x] 저장소 구조: 계획은 `plan/`, REQ 원문은 `docs/REQ.md`
- [x] 스파이크 SP-1~6 (`docs/spikes/`): S5-T2 분기 B, S3-T1 분기 A(`paint_layer` 조건), S1-T1 분기 B(`coremidi-sys`), S8-T1 안내만, SP-5 문법 결정 요청, SP-6 상한 2 s 유지
- [x] S0-T1 워크스페이스·CI (`scripts/check-deps.sh` + `.test.sh`, `deny.toml`, `.github/workflows/ci.yml`)
- [x] S0-T2 모델 타입 (Track/Clip/Note, 불변식, 숨김 꼬리)
- [x] S0-T3 템포·박자표 맵 (1/960 ns 정수 시각, `scripts/bench-check.py`, `benches/baseline.json`)
- [x] S0-T4 undo (`Document`가 프로젝트+히스토리 소유, 역편집은 crate 내부 `Op`, `TrackId`)
- [x] S0-T6 `.ksq` (결정적 쓰기, 모르는 키 보존, 임시 파일 + rename, 10만 노트 읽기 15 ms)
- [x] S0-T5 계이름 텍스트 파서·내보내기(리뷰 9회, 결정 `2026-10-13-ktext-export-voice-budget.md`와 개정). 엄격 모드 왕복의 알려진 예외와 출력 크기 상한(선형, 256배)은 `docs/decisions/2026-10-12-ktext-implementation-details.md`
- [x] S0-T8 `ksq-cli`(리뷰 11회, 결정 `2026-10-14-ksq-cli-output-path-scope.md`: `-o`는 일반 파일만). 픽스처 8곡 `assets/fixtures/`, 원본 텍스트 `tools/ksq-cli/fixtures-src/`
- [x] S0 출구 보고서 `docs/reports/stage-0-exit.md`(자동 게이트 통과, 사람 항목 대기)
- [x] 계이름 텍스트 주석을 `//`로(결정 Q1 개정, 2026-10-16 사람 지시). `#`는 음 뒤 올림표만. 헤더 영역의 `#`는 두 모드 오류, 트랙 안은 느슨 모드 경고. 리뷰 3회. `docs/reports/s0-t5-comments/`
- [x] S1-T7 `render`(keyscore-midi)와 재생 도구 `tools/ksq-play`(ksq-cli는 core+text 유지). 없는 이름 포트는 같은 이름의 가상 포트를 따로 만든다. 뮤트+솔로는 솔로가 이김(`2026-10-16-mute-solo-precedence.md`). 리뷰 2회
- [x] S1 출구 자동 게이트: 1시간판 NF-3 p99 0.104 ms, NF-4 0.0125–0.037 ms
- [x] S1-T6 SMF 입출력(리뷰 1 + 재리뷰 3 + 결정 `2026-10-16-s1-t6-review-limit.md` A의 확인 리뷰). 직접 구현(`midly`는 Unlicense). `SmfFile`(원시) → `Song`(PPQ 960, CC는 32비트 포인트 열) → 모델. 곡 길이 상한 `MAX_SONG_TICKS = u32::MAX`. core에 `TempoMap/MeterMap::from_changes`, `MeterMap::next_change_after`, 클립 겹침 이분 탐색. 공개 샘플 10곡 `crates/keyscore-midi/tests/data/smf/`. CC 레인 모델 보존은 S6-T3 이월, 가져오기 UX는 S2-T7 이월
- [x] S1-T5 메트로놈·미리듣기(리뷰 3회). 클릭은 스케줄 항목 `Next::Click`(이벤트 뒤, Clock 앞), 박 보고 `poll_beat`, 끄기 대기 표 64칸 + 노트별 `off_until`(넘긴 Note Off 뒤로 Note On을 미룸, 최대 lookahead). 기본 클릭 50 ms·벨로시티 127/100은 흔한 값
- [x] S1-T4 스케줄러 스레드(리뷰 4회, 주 세션 구현). `rtrb` 명령 큐, `mach_wait_until`(결정 `2026-10-16-scheduler-wait-syscall.md` A: 시각 대기는 예외), 기준점은 `host × numer × 960`으로 정확히, 보낸 구간 `sent_through`는 다음 미송신 항목에서 구함, 패닉은 원자 플래그 + `panic_after` 순번. NF-1 p99 약 1.09 ms, 10분판 NF-3 p99 0.077 ms·NF-4 0.011–0.046 ms. 한계는 `docs/reports/s1-t4/report.md` 끝 표
- [x] S1-T3 MIDI 1.0 인코딩(리뷰 3회, 경량 모델 구현). `Midi1Message`, `Group`, MIDI 2.0 변환 규칙(벨로시티 >>9·Note On 최소 1, CC >>25, 벤드 >>18)
- [x] S1-T2 OS 포트 열거·핫플러그·재연결(리뷰 2회). CoreMIDI 클라이언트는 전용 run loop 스레드(`midi_thread`)에서 만든다. 알림은 프로세스의 첫 클라이언트를 만든 스레드로 오기 때문이다. 사라진 목적지로의 송신은 CoreMIDI가 오류 없이 버린다
- [x] S1-T1 CoreMIDI 가상 출력 포트(리뷰 7회, 결정 `2026-10-15-virtual-port-limit-scope.md` 프로세스 단위 16개, `2026-10-15-port-name-visual-duplicates.md` 보이는 모양 기준 이름 중복). unsafe·FFI는 주 세션이 구현했다
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
- 리뷰마다 더 좁은 경계를 찾아 재리뷰가 9회까지 늘었다(S0-T5). 재리뷰 3회 뒤에는 결정 요청으로 넘기고, 이후 확인 리뷰는 직전 수정 범위로 좁힌다. 임의로 정한 수치(「64배」)를 완료 조건에 넣지 말고 측정으로 정한다.
- 테스트를 커밋한 뒤 clippy를 돌림: 경고가 뒤늦게 발견됐다. 게이트는 커밋 전에 모두 돌린다.
- 커널 경로 해석(링크, `/dev/fd`, `..`, 끝 `/`)을 도구가 흉내 내기(S0-T8 리뷰 2–8): 매 회차 새 구멍이 나왔다. 범위를 줄여(`-o`는 일반 파일만) 해결했다. 경계가 계속 나오면 일찍 범위 재정의를 묻는다.
- 유니코드 이름 규칙처럼 끝없는 모서리 영역은 확인 리뷰에 판정 기준(회귀·결정 위반·결함만 「수정」)을 명시해 좁힌다(S1-T1 리뷰 7).
- 확인 리뷰에서 같은 갈래 지적이 두 번 이상 나오면 패턴을 하나씩 막지 말고 원인을 고친다.
- 파서·변환에서 「정렬 삽입을 하나씩」은 악의적 입력에서 O(n²)이 된다(S1-T6 리뷰 1–3). 모아서 한 번 정렬하고, 반복 안의 전체 훑기를 피한다.
- 단계 파일 수치를 검증 없이 옮기기: S0-T3 「1시간 = 3,456,000틱」은 60 BPM 기준이었다(정정함).

## 남은 작업

- [ ] 원격 CI 첫 실행과 CI 러너 벤치 기준값: remote가 생기면(사람이 건너뜀)
- [ ] S1 출구 사람 항목: Surge XT·Dexed 청취, kill -9 뒤 포트 없음, REQ 9.2 문구 허락, 옛 `#` 주석 이행 규칙 여부, 서명
- [ ] REQ 9.2 문구에 「시각 대기 시스템 콜은 예외」 반영: 사람의 수정 허락 대기(결정 `2026-10-16-scheduler-wait-syscall.md`)
- [ ] 사람의 수동 확인 대기: Surge XT 수신(SP-3), 실제 키보드·IME(SP-1), 120 Hz 디스플레이(SP-2)

결정 대기(`rg -l "상태: 결정 대기" docs/decisions/`):
- `2026-10-11-ai-model-defaults.md` — S7 전
- `2026-10-12-ksq-newer-version-save.md` — S2-T7 전
- `2026-10-12-ksq-unknown-keys-on-value-change.md` — S2-T5 전
- `2026-10-12-quantize-grid-lengths.md` — S5 전

REQ 문구: 사람이 에이전트에게 수정을 허락한 범위(결정 문서의 「후속(사람)」)는 반영했다. 그 밖의 REQ 수정은 여전히 사람이 한다.

## 핵심 파일

- `CLAUDE.md` — 저장소 규칙. 커밋 첫 줄 `<crate>: …` / `plan: …`, 에이전트가 직접 커밋·병합한다
- `plan/01-protocol.md` — Task 루프와 적대적 리뷰 규칙
- `plan/stages/10-S0-foundation.md` — S0 Task와 「이월」 줄
- `plan/stages/11-S1-midi-engine.md` — S1 Task와 이월 줄
- `crates/keyscore-midi/src/scheduler.rs` — 스케줄러(`Scheduler` 공개 API, 실시간 상태 `Rt`)
- `docs/reports/s1-t4/report.md` — 스케줄러 리뷰 대응과 알려진 한계
- `docs/decisions/2026-10-12-quantize-and-transform-rules.md` — S0-T7 규칙(개정·부록 포함)
- `crates/keyscore-core/src/transform.rs` — S0-T7 연산(`plan`, `snap`, `relocate`)
- `crates/keyscore-core/src/edit.rs` — 공개 `Edit`, 내부 `Op`(역편집), `approx_bytes`
- `crates/keyscore-core/src/document.rs` — 편집의 유일한 진입점
- `crates/keyscore-core/src/ksq.rs` — `.ksq` 읽기·쓰기
- `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.py`, `scripts/bench-check.test.sh` — 추가 게이트

## 다음 에이전트에게

1. `docs/reports/stage-1-exit.md`의 사람 서명을 기다린다. 서명되면 S2(`plan/stages/12-S2-*.md`)를 시작한다. S2-T5·T7의 이월 줄과 결정 대기 문서(S2-T5 전, S2-T7 전)를 먼저 읽는다. MIDI 테스트 흔들림은 `scripts/midi-soak.sh N [쉬는 초]`로 본다.
2. Task 루프: 브랜치 `s1/t<번호>-<slug>` → `docs/reports/s1-t<번호>/plan.md` → 테스트 먼저(뼈대 `todo!()`로 실패 확인) → 구현 → 게이트 전부(커밋 전에) → `report.md` → 새 에이전트로 적대적 리뷰(`general-purpose`, 구현 맥락 없이, 양식 `plan/templates/adversarial-review.md`, 산출물만 남기고 커밋 금지, 기존 `.proptest-regressions` 삭제 금지) → 지적마다 커밋 하나 → 재리뷰(최대 3회) → `git merge --no-ff`.
3. 자동 게이트: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`. `core`·`text`에 외부 의존을 더하면 `scripts/check-deps.sh`의 허용 목록도 고친다.
4. REQ에 없는 결정은 임의로 정하지 말고 사람에게 묻거나 `docs/decisions/`에 결정 요청을 쓴다. 사람과의 대화는 한국어로 하고, 문서에 `**`를 쓰지 않는다.
5. 긴 MIDI 측정 중에는 다른 루프백 테스트를 돌리지 않는다(수치가 흔들린다). 그동안 다른 crate 작업은 `CARGO_TARGET_DIR=target/alt`로 빌드 잠금을 피한다.
