# 적대적 리뷰: 원격 CI 첫 실행 실패 대응 (리뷰 2회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 서브 에이전트 세션이다. 리뷰 1(`review-1.md`)과 보고서(`first-run.md`)의 주장을 코드, 셸 모의 실행, 원격 CI 로그로 다시 확인했다.
- 대상 커밋: d19b27b (`git diff 13087f8..HEAD`, 리뷰 1 뒤 수정은 c86a67f, ba00d66, c20dec4, 45f3712, d19b27b)
- 제약: 이 컨테이너는 Linux라 keyscore-app·keyscore-midi를 빌드할 수 없다. 대신 이 브랜치의 PR #1에서 돈 원격 CI 두 번을 증거로 썼다.
  - 실행 41442088403: 커밋 796f57c(리뷰 1 전, `RunLoop.main.run()` 판, `--no-fail-fast` 없음)
  - 실행 41442250458: 커밋 d19b27b(이 리뷰 대상, HEAD)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 리뷰 1 지적 1: 시험 단계 종료 코드가 `cargo test`의 것으로 보존되는가(`/bin/bash -e`에서 `|| status=$?`, `exit "$status"`).
2. keepalive 생존 기록이 「시험 전에 죽음」「시험 도중 죽음」「끝까지 삶」「늦게 뜸」을 서로 가를 수 있는가.
3. `while true { sleep(3600) }`가 Swift 스크립트(즉시 실행 모드)로 컴파일·실행되는가. 경고가 오류가 되는 설정이 있는가. `$!`의 pid가 실제로 클라이언트를 쥔 프로세스(또는 그것을 기다리는 부모)인가.
4. `--no-fail-fast`가 단계 종료 코드와 실행 시간에 주는 영향. 그리고 그것이 드러내는 새 실패를 보고서와 HANDOFF가 감당하는가.
5. 리뷰 1 지적 2: 보고서의 사실(실행 표, 원인 가설과 반대 근거)이 원격 기록과 맞는가. 같은 가설이 다른 파일(Swift 주석, 시험 주석, 커밋 본문)에 사실처럼 남아 있지 않은가.
6. keepalive가 다른 시험의 뜻을 바꾸는가(특히 서버 유휴 종료에 기대는 회귀 시험).
7. HANDOFF 새 두 줄이 사실과 맞고, 다음 세션이 그대로 따라 해도 저장소 규칙(squash 금지 등)을 어기지 않는가.
8. `review_s2_t5a.rs`의 주석 추가(c20dec4)가 컴파일·형식에 영향이 없는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 종료 코드 보존 | 없음(워크플로) | 셸 모의와 원격 로그로 확인. 3절 #1, #3 |
| 2. 생존 기록의 판별력 | 없음 | 셸 모의. 3절 #1. 제안 4 |
| 3. Swift 스크립트 동작 | 없음 | 원격 로그로 확인. 3절 #2, #3 |
| 4. `--no-fail-fast`의 결과 | CI 자체 | 원격 실행 41442250458에서 14개 대상 실패. 지적 1 |
| 5. 사실 정확성 | 없음 | 원격 로그 대조. 지적 2 |
| 6. 다른 시험의 뜻 | `review_s1_t1.rs:94-124`가 있으나 keepalive 아래에서는 늘 통과 | 제안 3 |
| 7. HANDOFF | 없음 | 저장소 설정 조회. 제안 5 |
| 8. 시험 형식 | CI fmt·clippy 단계 | 원격 두 실행 모두 fmt·clippy 통과, `rustfmt --check` 통과 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `ci.yml`의 test 단계 `run`을 그대로 뽑아 원격과 같은 `/bin/bash -e`로 모의 실행했다. 가짜 `swift`·`cargo`로 여섯 경우: (가) ready 뒤 계속 삶 + cargo 101 (나) ready 뒤 곧바로 끝남 + 101 (다) `MIDIClientCreate -50`으로 종료 (라) 컴파일 오류 문구에 `already`가 들어 있음 (마) `swift` 없음 (바) ready가 대기 상한보다 늦음(상한을 2 s로 줄이고 5 s 뒤 ready) | 종료 코드는 모두 cargo의 것(101/0)으로 보존. (가) `alive after tests` (나) `gone before tests ended`로 리뷰 1 지적 1 (가)의 두 경우를 이제 가른다. 다만 (라) `grep -q ready`가 `already`에 걸려 대기 고리가 곧바로 빠지고 (바) 첫 줄이 `midi-keepalive: `(빈 값)인데 끝 줄은 `alive after tests`라, 시험 앞부분을 keepalive 없이 돈 실행이 「살아 있었음」처럼 읽힌다. (다)(마)는 시작조차 못 했는데 끝 줄이 「시험이 끝나기 전에 사라짐」이라 적힌다(제안 4) | 없음 |
| 2 | 원격 실행 41442088403(796f57c, `RunLoop.main.run()` 판) 로그로 리뷰 1 지적 1 (가)의 전제를 확인했다 | 전제가 틀렸다. `midi-keepalive: ready`는 단계 시작 9.6 s 뒤(12:10:50)에 찍혔고, 작업 끝 12:23:51에 러너가 `Terminate orphan process: pid (6456) (swift-frontend)`를 찍었다. `RunLoop.main.run()`은 13분 동안 돌아오지 않았다. 그런데 c86a67f 본문과 `midi-keepalive.swift:18` 주석은 「곧바로 돌아온다」를 사실로 적는다(지적 2) | 없음 |
| 3 | 원격 실행 41442250458(HEAD) 로그로 수정 결과를 확인했다 | 단계 자체는 의도대로다. `ready`가 7.4 s 뒤(12:11:43), 시험 뒤 `midi-keepalive: alive after tests`, 단계는 `exit code 101`로 실패를 보존했다. 작업 끝에 `swift-frontend`(pid 6337)가 고아로 정리됐다. 그러나 `error: 14 targets failed`. 이 PR이 고친 두 시험(`pause_position_follows_tempo_change_in_song`, `send_test_note_reaches_track_port`)은 두 실행 모두 통과했지만, 나머지 14개 대상이 실패해 CI는 녹색이 될 수 없다(지적 1) | 없음 |
| 4 | 같은 종류의 실패가 고친 시험 바로 옆에 남는지: `review_s2_t5a_r2::pause_after_several_tempo_edits_matches_elapsed_time`(고정 상한 2300틱) | 깨짐. 796f57c 실행에서 `Tick(2834)`, HEAD 실행에서 `Tick(3038)`. 두 번 연속이고 넘친 폭이 500–700틱(240 BPM에서 약 0.14–0.19 s)이라, 리뷰 1 때 고친 1–4틱 넘침과 크기가 다르다. 시간 여유 부족인지 결함인지 이 PR은 다루지 않는다 | 없음 |
| 5 | keepalive 아래에서 서버 유휴 종료 회귀 시험이 뜻을 갖는지(`review_s1_t1.rs:94-124`, 코드 경로) | 깨짐(약화). 이 시험은 「다른 MIDI 클라이언트가 떠 있으면 재현되지 않고 통과한다」고 스스로 적는다(`review_s1_t1.rs:93-94`). CI는 다른 MIDI 앱이 없는 유일한 자동 환경인데, keepalive가 그 조건을 없앴다. HEAD 실행에서 `client_new_after_last_client_dropped_and_server_idle_succeeds ... ok`. 누군가 `MIDIClientDispose`를 되살려도 CI가 잡지 못한다(제안 3) | 없음 |
| 6 | 리뷰 1 지적 2 (가)의 기록을 원격으로 대조(`gh run view --log`, 41437033762·41437132455·41437639601·41437821878) | 못 깨뜨림. 실행 1–4의 커밋과 `Tick(1601)`·`Tick(1603)`·`Tick(1604)`·`-50`이 보고서 표(`first-run.md:12-15`)와 맞다 | 없음 |
| 7 | 저장소 병합 설정 조회(`gh api repos/soomtong/keyscore-midi-editor`) | `allow_squash_merge: true`. HANDOFF 141줄은 병합 방식을 적지 않는다(제안 5) | 없음 |
| 8 | `rustfmt --edition 2024 --check crates/keyscore-app/tests/review_s2_t5a.rs` | 못 깨뜨림(통과). 원격 두 실행의 clippy 단계도 통과 | 없음 |

Rust 시험은 이 컨테이너에서 실행할 수 없다. 실행한 것은 #1 셸 모의, #8 형식 검사, #2·#3·#4·#6 원격 로그 조회, #7 설정 조회다. #3·#4의 원격 실행이 이 브랜치의 실제 시험 실행이다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 모델 불변식(클립 겹침, 노트 범위, 채널) | 해당 없음 | 제품 코드 변경 없음 |
| 일시정지 자리 = 넘기지 않은 첫 틱(S3-T8) | 이 변경으로 바뀌지 않음 | 시험 상한과 주석만 바뀜(리뷰 1 3절 #1에서 상한 건전성 확인) |
| 프로세스 클라이언트를 해제하지 않음(`client.rs:190-194`) | 제품 코드는 그대로지만, 이 불변식을 지키는 회귀 시험이 CI에서 무력해짐 | 3절 #5, 제안 3 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1(일시정지) | 충족(고친 시험 범위) | `review_s2_t5a.rs:279-297`, 원격 두 실행에서 통과. 다만 같은 TRN-1 시험 `review_s2_t5a_r2.rs:238-263`이 러너에서 연달아 실패한다(3절 #4) |
| OUT-1(앱 시작 시 가상 포트 등록) | 충족(이 변경 범위에서) | 제품 코드 변경 없음. `inspector::send_test_note_reaches_track_port`가 keepalive 아래 두 번 통과 |
| NF-3(재생 지터 p99 ≤ 0.5 ms), REQ 1063줄(CI는 macOS 러너에서 시험) | 미충족(러너에서) | HEAD 실행 `scheduler.rs:258` `playback_jitter_p99_within_half_ms_short`가 p99 4.478 ms로 실패. 이 변경의 회귀가 아니라 `--no-fail-fast`로 처음 드러난 것이다. 러너에서 이 단언을 어떻게 다룰지는 REQ에 없는 결정이다(지적 1) |

「부분 충족」은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(제품 코드 변경 없음).
- 제품 코드의 `unwrap`/`expect`: 변경 없음. 시험 도우미 `ticks_at_120_then_60`의 `unwrap`은 시험 코드다. `midi-keepalive.swift:13`의 `data(using: .utf8)!`는 ASCII 문자열이라 실패하지 않는다.
- 에러가 삼켜지는 곳: keepalive가 시험 도중 죽으면 그 이유(stderr)는 `$RUNNER_TEMP/midi-keepalive.log`에만 남고 워크플로 로그에 나오지 않는다. 로그 파일은 업로드되지 않아 작업이 끝나면 사라진다(제안 4).

## 7. 지적 사항

리뷰 1 지적·제안의 해결 확인:

| 리뷰 1 # | 주장된 대응 | 확인 결과 |
| --- | --- | --- |
| 지적 1 | sleep 고리, 시험 뒤 생존 줄, 종료 코드 보존 | 해결. HEAD 실행에서 `alive after tests`와 `exit code 101` 확인. 다만 이 지적의 (가) 전제(`RunLoop.main.run()`이 곧바로 돌아올 수 있음)는 796f57c 실행으로 틀렸음이 드러났다. 고친 코드는 무해하지만 주석과 커밋 본문이 틀린 전제를 사실로 적었다(아래 지적 2) |
| 지적 2 | 실행 2 기록, 가설 좁힘, 반대 근거 | 보고서에서는 해결. Swift 스크립트 머리 주석과 시험 주석에는 고치기 전 서술이 남았다(아래 지적 2) |
| 제안 3 | `--no-fail-fast` | 반영. 그 결과 14개 대상 실패가 드러났다(아래 지적 1) |
| 제안 4 | 「알려진 한계」와 HANDOFF 142줄 | 반영. 142줄의 분기 조건에 논리 문제가 있다(제안 5) |
| 제안 5 | 가정 주석 | 반영(`review_s2_t5a.rs:290`) |

이번 회차 지적:

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 지적 1 | 수정(범위 재정의 사유) | 이 PR의 전제 「두 실패를 고치면 원격 CI가 녹색이 된다」가 이 PR 자신의 CI로 깨졌다. HEAD 실행 41442250458에서 `--no-fail-fast`가 14개 대상 실패를 드러냈다. 세 갈래다. (가) 벽시계·부하 의존 시험 8개: `review_s2_t5a_r2`(`Tick(3038)`, 796f57c 실행에서도 `Tick(2834)`), `review_s2_t5a_r3`(메트로놈 켜짐 없음), `review_s3_t8_r2`(첫 박 `[40, 41]`), `transport_bar`(탭 템포 84.4), `review_s1_t4_r2`(`None` unwrap), `review_s1_t5`(클릭 순서), `review_s3_t8_r3`(루프 끝 경계), `scheduler`(NF-3 지터 p99 4.478 ms). (나) CoreMIDI 환경 2개: `review_s2_t6c`(「감시가 시작되지 않았다」 둘), `review_s1_t1_r3`(`MIDISourceCreateWithProtocol` `-307`). (다) 파일 시스템·권한 4개(`ksq-cli`의 `cli`, `review_s0_t8_r6`·`r9`·`r11`: 러너의 `/dev` 쓰기가 `Permission denied`로 나와 문구 분기가 어긋남, 문자 장치 판정 등). 그런데 HANDOFF 141줄은 「PR의 CI 결과 … 녹색이면 병합한다」, 보고서 `first-run.md:38`은 「대응 2가 가설대로가 아니면 inspector 실패가 다시 나온다」만 예상한다. 다음 세션이 HANDOFF를 따르면 이 PR은 영원히 병합 조건을 채우지 못하거나, 시험을 하나씩 고치는 회차가 14번 이어진다. `#[ignore]`·삭제는 금지(CLAUDE.md)이고, NF-3을 공유 러너에서 단언할지, 타이밍 시험을 별도 작업(야간·직렬)으로 뺄지, `/dev` 권한 같은 환경 가정을 어떻게 다룰지는 REQ에 없는 결정이라 `docs/decisions/`에 결정 요청이 필요하다. 고칠 길: 이 PR의 범위를 「keepalive + 고친 두 시험 + `--no-fail-fast` + 실패 목록 기록」으로 다시 정해 보고서 「관찰」에 HEAD 실행의 14개 목록과 세 갈래를 적고, HANDOFF 141줄의 「녹색이면 병합」을 범위에 맞게 고치고, 나머지는 결정 요청과 후속 Task로 넘긴다 | `HANDOFF.md:141`, `docs/reports/ci/first-run.md:8-17`, `docs/reports/ci/first-run.md:38`, `docs/reports/ci/first-run.md:43` |
| 지적 2 | 수정 | 리뷰 1 지적 2(가설을 사실처럼 적음)가 보고서 밖에 남았고, 새로 틀린 사실이 하나 더해졌다. (가) `midi-keepalive.swift:3-6` 머리 주석은 여전히 「바이너리가 끝날 때마다 서버가 내려가고 … 원격 CI 첫 실행에서 이 기다림이 10초 재시도를 넘겨 -50으로 실패했다」를 사실로 적는다. 보고서는 「원인은 확인하지 못했다」, 실행 1–3은 같은 조건에서 통과했다고 고쳤으므로 서로 어긋난다. 또 `-50`은 「첫 실행」이 아니라 네 번째 실행이다. (나) `midi-keepalive.swift:18`과 커밋 c86a67f 본문은 「`RunLoop.main.run()`은 run loop에 입력 소스가 없으면 곧바로 돌아온다」를 사실로 적는데, 실행 41442088403에서 그 판의 `swift-frontend`는 ready 뒤 13분 동안 살아 있다가 작업 정리에서 죽었다(3절 #2). 리뷰 1도 「확인된 적이 없다」고만 했다. 주석은 「왜」를 적는 자리라 틀린 「왜」는 다음 사람이 `sleep` 고리를 지우거나 반대로 다른 CoreMIDI 도구에 같은 오해를 옮기게 한다. (다) `review_s2_t5a.rs:287-288` 주석은 「1601·1604」로 실행 2의 1603이 빠졌다. 고칠 길: Swift 머리 주석을 보고서의 좁힌 가설(「재기동이 가끔 10초 넘게 실패한다는 가설」)로, 18줄을 「곧바로 돌아올 수 있어(리뷰 1 지적 1) 조건 없이 막는다. 실측으로는 돌아오지 않았다(실행 41442088403)」로 고치고, 시험 주석에 1603을 넣는다. 커밋 본문은 고쳐 쓰지 않고 보고서 「리뷰 대응」에 바로잡음을 적는다 | `.github/ci/midi-keepalive.swift:3-6`, `.github/ci/midi-keepalive.swift:18`, `crates/keyscore-app/tests/review_s2_t5a.rs:287-288`, 커밋 c86a67f 본문 |
| 제안 3 | 제안 | keepalive가 CI에서 서버 유휴 종료 회귀 시험을 무력화한다. `review_s1_t1.rs:94-124`는 S1-T1 리뷰 1의 차단 지적(마지막 클라이언트 해제 → OUT-1 영구 불가)을 지키는 시험인데, 다른 MIDI 클라이언트가 있으면 늘 통과한다고 스스로 적는다(93-94줄). 사람 Mac에는 보통 DAW가 떠 있어 원래 무력했고, CI가 그 조건이 없는 유일한 자동 환경이었다. 실패 시나리오: 누군가 `ClientInner`에 `MIDIClientDispose`를 되살려도 keepalive가 서버를 붙잡아 HEAD 실행처럼 `ok`가 나온다. 리뷰 1이 「뜻이 바뀌지 않는다」고 본 것은 지금 코드가 통과한다는 뜻이었지 회귀를 잡는다는 뜻이 아니었다. 최소한 보고서 「알려진 한계」에 적고, 가능하면 `keyscore-midi --test review_s1_t1`만 keepalive 없이 따로 돌리는 길을 검토한다 | `.github/workflows/ci.yml:52-63`, `crates/keyscore-midi/tests/review_s1_t1.rs:91-124`, `docs/reports/ci/first-run.md:40-43` |
| 제안 4 | 제안 | 생존 기록의 판별력이 아직 모자란 경우가 셋 있다(3절 #1 모의). (가) `grep -q ready`가 `already`(Swift 진단에 흔한 단어)에 걸린다. 진단이 ready보다 먼저 찍히면 대기 고리가 빠져 시험이 keepalive 없이 시작한다. `grep -qx ready`로 줄 전체를 비교한다. (나) 120 s 안에 ready가 없다가 시험 도중 뜨면 첫 줄은 빈 값인데 끝 줄은 `alive after tests`라, 앞부분 바이너리가 keepalive 없이 돈 실행이 살아 있었던 것처럼 읽힌다. 끝 줄에 「시험 전 ready 여부」를 함께 찍는다. (다) 시험 도중 죽으면 그 stderr가 워크플로 로그에 나오지 않는다. `gone`일 때 로그 파일을 한 번 더 `cat`한다. 또 시작조차 못 한 경우의 끝 줄 「gone before tests ended」는 「시험 도중 죽음」으로 오해된다 | `.github/workflows/ci.yml:55`, `.github/workflows/ci.yml:59`, `.github/workflows/ci.yml:63` |
| 제안 5 | 제안 | HANDOFF 두 줄. (가) 141줄 「녹색이면 병합한다」는 방법을 적지 않는다. 원격 저장소는 squash 병합을 허용한다(`allow_squash_merge: true`). CLAUDE.md는 squash 병합을 금지하므로 `gh pr merge --merge`(또는 로컬 `git merge --no-ff`)라고 적는다. (나) 142줄은 「CI에서 keepalive가 살아 있는데도 `-50`이 다시 나면 제품 쪽 Task(재시도 시간, 늦은 포트 재등록)를 연다」인데, keepalive가 살아 있는데 `-50`이 나면 「서버 재기동」 가설이 틀렸다는 뜻이다. 그때 열 것은 재기동 대비가 아니라 `-50`의 원인 조사다. 분기를 「가설 확인(사람 Mac 재현) → 제품 Task」와 「가설 반증(keepalive 생존 + `-50`) → 원인 조사」로 나눈다 | `HANDOFF.md:141`, `HANDOFF.md:142` |

확인했지만 문제가 아닌 것(지적 없음):

- Swift 스크립트: `while true { sleep(3600) }`는 즉시 실행 모드의 최상위 코드로 컴파일·실행된다. 원격 HEAD 실행에서 7.4 s 뒤 ready, 시험 28분 뒤에도 생존. 가져온 C 함수 `sleep`의 반환값은 버려도 경고가 없고, `swift file.swift`에는 경고를 오류로 바꾸는 설정이 없다.
- `$!`: `swift` 드라이버 사슬(`/usr/bin/swift` → 툴체인 → `swift-frontend -interpret`)이 exec로 이어지든 부모가 기다리든, `$!`의 생존은 클라이언트를 쥔 프로세스의 생존과 같이 간다. 원격 로그의 고아 정리가 `swift-frontend` 하나만 찍은 것도 이와 맞다. pid 재사용은 시험 단계의 프로세스 수(수천)가 macOS pid 범위(약 10만)에 못 미쳐 현실적이지 않다.
- 셸: 원격 셸은 `/bin/bash -e {0}`(macOS 기본 bash 3.2)이다. `|| status=$?`, `&& break`, `kill -0 … || break`, `$(seq 1 120)` 모두 3.2에서 쓸 수 있는 구문이고 `-e`로 끊기지 않는다. 백그라운드 명령의 표준 입력은 `/dev/null`, 출력은 파일이라 단계가 매달리지 않는다(원격에서 단계가 정상 종료).
- `--no-fail-fast`는 실패가 있으면 101로 끝나 종료 코드 의미가 같다. 시험 단계는 28분 42초(컴파일 8분 32초 포함)로 작업 제한 시간에 한참 못 미친다.
- 시험 주석 추가(c20dec4)는 형식·clippy에 영향이 없다(원격 fmt·clippy 통과).

## 8. 판정

- [ ] 병합 가능
- [ ] 수정 후 재리뷰
- [x] 범위 재정의 (이유: 이 PR이 고친 두 시험과 keepalive는 원격에서 의도대로 동작함이 확인됐다. 그러나 「원격 CI를 녹색으로」라는 목표는 이 PR 자신의 실행에서 14개 대상 실패로 깨졌고, 그 대부분(공유 러너에서의 NF-3 지터 단언, 벽시계 타이밍 시험의 취급, 러너 파일 시스템 권한 가정)은 REQ에 없는 결정이 필요하다. 범위를 「keepalive와 첫 두 실패의 대응, 실패 목록 기록」으로 다시 정해 지적 1·2를 반영하고, 나머지는 결정 요청과 후속 Task로 넘긴 뒤 다시 리뷰한다)

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [ ] 깨뜨리는 테스트를 실제로 실행했다 — 이 컨테이너(Linux)에서 Rust 시험을 빌드할 수 없다. 대신 셸 모의 실행을 했고, 이 브랜치의 원격 CI 실행 두 번(41442088403, 41442250458)의 로그를 시험 실행 증거로 썼다
- [x] REQ ID를 하나씩 대조했다(TRN-1, OUT-1, NF-3)
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`.github/`, 시험 하나, `docs/reports/ci/`, `HANDOFF.md`)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 crate 의존성 없음)
- [x] 스타일 지적을 적지 않았다
