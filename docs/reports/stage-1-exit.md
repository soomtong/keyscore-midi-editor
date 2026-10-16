# 단계 출구: S1 MIDI 엔진(가상 포트, 스케줄러, SMF)

- 기준 커밋: `main` `4b97155`(S1-T7 병합, 계이름 텍스트 주석 `//` 변경 병합)
- 작성: 계획자 에이전트, 2026-10-16
- 서명: YoungTaek, 2026-10-16

## Task 결과

| Task | 내용 | 리뷰 | 보고서 |
| --- | --- | --- | --- |
| S1-T1 | CoreMIDI 가상 출력 포트(프로세스 단위 16개, 보이는 모양 기준 이름 중복) | 7회, 병합 가능 | `docs/reports/s1-t1/` |
| S1-T2 | OS 포트 열거·핫플러그·재연결, 대체 매핑 제안 | 2회, 병합 가능 | `docs/reports/s1-t2/` |
| S1-T3 | MIDI 1.0 인코딩과 MIDI 2.0 → 1.0 다운스케일 | 3회, 병합 가능 | `docs/reports/s1-t3/` |
| S1-T4 | 스케줄러 스레드(정수 시각, chase, Clock, 패닉, 교체) | 4회, 병합 가능 | `docs/reports/s1-t4/` |
| S1-T5 | 메트로놈(박 격자, 강제 켬, 박 보고)과 미리듣기 | 3회, 병합 가능 | `docs/reports/s1-t5/` |
| S1-T6 | SMF 읽기·쓰기(`SmfFile` → `Song` → 모델), 공개 샘플 10곡 | 1+3회와 결정 A의 확인 1회, 병합 가능 | `docs/reports/s1-t6/` |
| S1-T7 | 모델 → 이벤트 `render`, 재생 도구 `ksq-play` | 2회, 병합 가능 | `docs/reports/s1-t7/` |
| (S0-T5 변경) | 계이름 텍스트 주석을 `//`로(결정 Q1 개정) | 3회, 병합 가능 | `docs/reports/s0-t5-comments/` |

## 자동 게이트 (계획자가 `4b97155`에서 실행)

- [x] `cargo fmt --all -- --check`: 통과
- [x] `cargo clippy --workspace --all-targets -- -D warnings`: 경고 0
- [x] `cargo test --workspace --no-fail-fast`: 818 passed, 0 failed, 2 ignored(1시간판 측정용으로 원래 무시하는 것)
- [x] `cargo deny check`: advisories·bans·licenses·sources ok
- [x] `scripts/check-deps.sh`: ok(`ksq-play` 포함. `ksq-cli`에는 CoreMIDI가 들어오지 않는다)
- [x] `scripts/check-deps.test.sh`: 16 passed
- [x] `scripts/bench-check.test.sh`: 8 passed
- [x] 루프백 NF-1·NF-3·NF-4(가상 포트 → 같은 프로세스의 독립 수신기)

  | 항목 | 기준 | 짧은 판(20초) | 10분판 | 1시간판(2026-10-16, 노트 28,800개) |
  | --- | --- | --- | --- | --- |
  | NF-1 명령 → 도착 p99 | ≤ 3 ms | 약 1.08–1.11 ms | | |
  | NF-3 간격 편차 p99 | ≤ 0.5 ms | 0.035–0.139 ms | 0.077 ms | 0.104 ms(표준편차 0.027–0.034 ms) |
  | NF-4 드리프트 | 1시간 ≤ 1 ms | 0.004–0.065 ms | 0.011–0.046 ms | 0.0125–0.037 ms(타임스탬프 기준 0) |

  - 1시간판은 원격 CI가 없어 야간 CI 대신 개발 기계에서 한 번 돌렸다.
  - 명령: `KS_LONG_TESTS=hour cargo test -p keyscore-midi --test scheduler playback_ -- --nocapture --test-threads=1`, 7200 s.
- [x] 스케줄러 할당 0
  - `assert_no_alloc` 전역 할당자 아래 테스트가 통과했다: `scheduler_alloc`, `metronome_alloc`, 리뷰 할당 테스트.
  - 디버그 빌드에서 루프에 할당을 넣은 변이는 SIGABRT로 실패함을 확인했다.

## 사람이 확인할 항목

- [x] (사람: 서명으로 승인, 청취 결과는 따로 기록되지 않음) 수동 청취: Surge XT와 Dexed 두 신디에서 픽스처를 재생한다. 절차는 `docs/reports/s1-t7/report.md` 「수동 확인 절차」다. 요약하면:
  ~~~
  cargo run -p ksq-play --release -- assets/fixtures/04-band.ksq --wait 10
  ~~~
  - 10초 안에 신디의 MIDI 입력에서 「KeyScore Out」을 고른다.
  - 끝까지 들리고 프로그램이 스스로 끝나는지 본다.
  - 다시 실행해 재생 중 `p` + Enter로 소리가 모두 멈추고 걸린 노트가 없는지 본다. `q` + Enter로 끝낸다.
  - `--metronome`으로 박마다 클릭(채널 10, 37/42)이 들리는지 본다.
- [x] (사람: 서명으로 승인, 결과는 따로 기록되지 않음) 비정상 종료 뒤 포트: 재생 중 다른 터미널에서 `kill -9 <ksq-play pid>`를 보낸 뒤, 신디의 입력 목록에서 「KeyScore Out」이 사라지는지 본다.
- [ ] (답 대기, S2를 막지 않음) REQ 9.2 문구: 결정 `docs/decisions/2026-10-16-scheduler-wait-syscall.md`(A: 시각 대기 `mach_wait_until`은 실시간 루프 규칙의 예외)를 REQ 9.2 문구에 반영할지. 허락하시면 에이전트가 고친다.
- [ ] (답 대기, S2를 막지 않음) 계이름 텍스트 이행 규칙: 옛 `#` 주석 줄을 통째로 건너뛰는 규칙을 둘지. 지금은 두지 않았다(`docs/reports/s0-t5-comments/report.md` 「리뷰어에게」 1).

## 결정 요약 (S1에서 정한 것)

- 사람이 정함
  - `2026-10-15-virtual-port-limit-scope.md`: 가상 포트 16개는 프로세스 단위
  - `2026-10-15-port-name-visual-duplicates.md`: 이름 중복은 보이는 모양 기준
  - `2026-10-16-scheduler-wait-syscall.md`: A, 시각 대기는 예외(REQ 9.2 문구 반영 대기)
  - `2026-10-16-s1-t6-review-limit.md`: A, 좁은 확인 리뷰 1회 뒤 병합
  - `2026-10-11-ktext-grammar-ambiguities.md` Q1 개정: 주석은 `//`만
- 에이전트가 흔한 규칙으로 정함(사람이 바꿀 수 있음)
  - `2026-10-16-mute-solo-precedence.md`: 솔로가 이긴다
  - 클릭 길이 50 ms, 벨로시티 127/100
  - SMF 곡 길이 상한 `MAX_SONG_TICKS = u32::MAX`, 가사 붙이기 규칙
- 구조: 재생 도구를 `tools/ksq-play`(core + midi)로 분리했다. `ksq-cli`는 core + text 그대로다.
- 결정 대기(S1을 막지 않음, S0에서 넘어옴)
  - `2026-10-11-ai-model-defaults.md`: S7 전
  - `2026-10-12-ksq-newer-version-save.md`: S2-T7 전
  - `2026-10-12-ksq-unknown-keys-on-value-change.md`: S2-T5 전
  - `2026-10-12-quantize-grid-lengths.md`: S5 전

## 알려진 한계와 이월

- 스케줄러(S1-T4 보고서 끝 표)
  - 한 창에 항목이 4096개를 넘는 극단 경우의 Clock 하나 누락과 최대 10 ms 지연
  - `collect_garbage`를 부르지 않는 호출자의 패닉 순서
  - 루프 되감기 chase 없음
  - RPN 널 패턴(S6)
- 메트로놈·미리듣기: 이미 넘긴 Note Off와 겹치는 다시 켜기는 최대 10 ms 늦다. 끄기 대기 표 64칸을 넘으면 가장 이른 노트가 일찍 끊긴다.
- SMF
  - CC·벤드·프레셔는 `Song`까지 보존한다. 모델 레인은 S6-T3 이월이다.
  - Type 0 합치기에서 같은 채널·음높이 겹침의 짝이 바뀐다(MIDI 1.0 한계).
  - 가져오기 결과 요약·백그라운드 처리는 S2-T7 이월이다.
- `ksq-play`
  - 공백만 다른 포트 이름, 대체 이름 충돌은 종료 코드 1로 끝난다.
  - 피치 오프셋은 보내지 않는다.
- 이월 줄을 더한 단계 파일
  - S2(T5·T7)
  - S6(T1·T3)
  - 11-S1(S1-T7 구현 메모)

## 서명

S2를 시작해도 좋다면 아래에 이름과 날짜를 적는다.

- 확인자: YoungTaek
- 날짜: 2026-10-16
