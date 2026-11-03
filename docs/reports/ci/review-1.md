# 적대적 리뷰: 원격 CI 첫 실행 실패 대응 (리뷰 1회차)

- 리뷰어 세션: 구현 컨텍스트를 물려받지 않은 별도 서브 에이전트 세션이다. 구현자의 보고서(`docs/reports/ci/first-run.md`)는 읽었지만 주장마다 코드와 원격 CI 로그(`gh run view --log`, 실행 41437033762·41437132455·41437639601·41437821878)로 다시 확인했다.
- 대상 커밋: 8ec4e60, 85b041a, 796f57c (`git diff 13087f8..HEAD`)
- 제약: 이 컨테이너는 Linux라 keyscore-app·keyscore-midi를 빌드·실행할 수 없다. Rust 시험은 돌리지 않았고, 코드 읽기와 셸·수치 모의 실행, 원격 로그 대조로 검증했다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 새 상한 `ticks(elapsed + lookahead) + 1`이 스케줄러가 돌려주는 일시정지 틱(`sent_through + 1`)의 진짜 위 끝인가. 시작 기준 시각이 `before_play`보다 앞설 수 있는가, 내림·`+1` 호스트 틱·`wake` 주기·`NoAnswer` 추정 경로가 끝을 넘기는가.
2. 템포 변화를 무시한 구현을 이 시험이 아직 잡는가. 흐른 시간이 길어질수록(느린 러너) 판별력이 얼마나 줄어드는가.
3. 시험 쪽 lookahead(`SchedulerConfig::default()`)가 엔진이 실제로 쓰는 설정과 같은가.
4. 새 도우미 함수와 시험이 `clippy::pedantic -D warnings`, `rustfmt`를 통과하는가.
5. GitHub Actions 기본 셸(`bash --noprofile --norc -eo pipefail`)에서 대기 루프·`$!`·`cat`이 단계를 깨뜨리지 않는가. 백그라운드 프로세스가 단계를 매달지 않고 끝나는가, 정리되는가.
6. Swift 스크립트: `MIDIClientCreate` 시그니처, `CFString` 브리징, 실패 시 종료 코드, 그리고 `RunLoop.main.run()`이 정말 계속 막히는가(입력 소스가 없으면 곧바로 돌아오는 API다).
7. 「ready」 로그 한 줄이 시험 내내 클라이언트가 살아 있었음을 보여 주는가. 다음 실패 때 원인을 가를 수 있는가.
8. 추가 클라이언트가 시험 동작을 바꾸는가(소스·목적지 열거, 이름 중복, 서버 유휴 종료에 기대는 시험, 핫플러그 알림).
9. `-50`의 원인 가설이 로그와 맞는가. 실행 1–3과 실행 4의 차이를 설명하는가. 다른 설명(같은 바이너리 안 다른 시험이 CoreMIDI를 먼저 건드림, CoreMIDI 스레드 막힘 등)은 없는가.
10. 보고서의 사실 주장(실행 목록, 시각, 틱 값)이 원격 기록과 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 상한의 건전성 | 없음(시험 자체가 대상) | 코드 경로로 확인. 아래 3절 #1 |
| 2. 템포 무시 구현 판별 | 시험의 주석에만 주장 | 수치 모의로 확인. 3절 #2 |
| 3. lookahead 일치 | 없음 | `engine.rs:801`이 `SchedulerConfig::default()`를 쓴다. 지금은 일치 |
| 4. fmt·clippy | CI 단계로 존재 | `rustfmt --check`만 여기서 확인. clippy는 macOS에서만 |
| 5. 셸 동작 | 없음 | 셸 모의 실행. 3절 #3 |
| 6·7. keepalive 생존 | 없음 | 시험 뒤 생존 확인이 워크플로에 없다. 지적 1 |
| 8. 추가 클라이언트 영향 | 없음 | grep으로 확인. 영향 없음 |
| 9. 원인 가설 | 없음 | 원격 로그 대조. 지적 2 |
| 10. 보고서 사실 | 없음 | 원격 기록과 어긋남. 지적 2 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 상한을 넘는 일시정지 틱 찾기(코드 경로). `Command::Stop`은 직전 `schedule`이 구한 `sent_through`(지평 `now + lookahead`, `tick_at`은 내림)에 1을 더한다(`scheduler.rs:893-895`, `1132`, `1175`, `1183-1213`). 기준 시각은 `max(release_time, scheduled_until, now + START_MARGIN 2 ms)`라 `before_play`보다 늦다(`scheduler.rs:954-959`). `NoAnswer` 추정은 `pause` 호출 전에 템포 지도로 구한다(`transport.rs:183-187`, `114-119`). `TempoMap::time_to_tick`은 내림이다(`tempo.rs:229-236`). `HostTime`은 `mach_absolute_time`, `Instant`는 `CLOCK_UPTIME_RAW`라 같은 시계다 | 못 깨뜨림. 최소 2 ms(약 2틱) 여유가 늘 있다 | 없음 |
| 2 | 템포 무시 구현이 통과하는 조건 찾기. 상한 공식과 「120 BPM 고정」 구현을 파이썬으로 모의 실행(바깥 지연 o, 안쪽 지연 0–0.5 s) | 못 깨뜨림. 바깥 지연(재생 직전→기준 시각, 정지 처리→`elapsed` 측정)이 약 0.49 s + 안쪽 지연을 넘어야 통과한다. 원격 실행의 안쪽 지연은 약 0.17 s(틱 1601–1604)였다 | 없음 |
| 3 | 워크플로 단계를 `bash --noprofile --norc -eo pipefail`로 모의 실행. `swift`를 (a) ready 뒤 곧바로 끝남 (b) `-50`으로 종료 (c) 명령 없음 (d) 계속 삶으로 바꿈 | (a)–(c) 모두 단계가 계속되고 종료 코드는 시험 결과를 따른다. 백그라운드 출력이 파일로 가 단계를 매달지 않는다. 다만 (a)와 (d)의 워크플로 로그가 `midi-keepalive: ready`로 똑같다. keepalive가 시험 도중 죽었는지 로그로 가를 수 없다(지적 1) | 없음 |
| 4 | 원격 로그로 보고서 주장 대조(`gh run view --log`) | 깨짐. 실행 2(b130832, 41437132455)는 끝났고 같은 시험이 `Tick(1603)`으로 실패했다. 보고서 16줄과 다르다. 실행 1·4는 같은 바이너리 순서(files_playback → import_export → inspector)와 비슷한 간격(마지막 MIDI 바이너리 종료 뒤 약 5–6 s에 `send_test_note`)이었는데 실행 1–3은 통과했다(지적 2) | 없음 |
| 5 | 가설 밖의 원인 찾기: inspector 바이너리의 다른 시험이 `send_test_note`보다 먼저 CoreMIDI를 건드리는가 | 못 깨뜨림. `init`은 CoreMIDI를 건드리지 않고(`engine.rs:3-5`), 앱의 `destinations()` 호출은 엔진이 있을 때만이다(`port_mapping.rs:147-151`, `ports.rs:99`). inspector에서 엔진을 설치하는 시험은 `send_test_note_reaches_track_port` 하나다 | 없음 |
| 6 | `rustfmt --edition 2024 --check crates/keyscore-app/tests/review_s2_t5a.rs`, 워크플로 YAML 파싱 | 못 깨뜨림(둘 다 통과) | 없음 |

이 컨테이너에서는 Rust 시험을 실행할 수 없다. 실행한 것은 #3 셸 모의, #2 수치 모의, #6 형식 검사, #4 원격 로그 조회다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 모델 불변식(클립 겹침, 노트 범위, 채널) | 해당 없음 | 제품 코드 변경 없음 |
| 일시정지 자리 = 넘기지 않은 첫 틱(S3-T8) | 이 변경으로 바뀌지 않음 | 시험 상한만 바뀜. 3절 #1에서 상한이 이 자리를 늘 덮음을 확인 |
| 프로세스 클라이언트를 해제하지 않음(`client.rs:190-194`) | 해당 없음 | 제품 코드 변경 없음 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| TRN-1(일시정지) | 충족 | 시험은 여전히 템포 지도를 따르지 않는 일시정지 자리를 잡는다(`review_s2_t5a.rs:279-296`, 3절 #2) |
| OUT-1(앱 시작 시 가상 포트 등록) | 충족(이 변경 범위에서) | 제품 코드 변경 없음. 다만 가설이 맞다면 사용자 Mac에서도 「마지막 MIDI 앱 종료 직후 앱 시작」이 10초 재시도를 넘겨 포트 없이 뜰 수 있다. CI keepalive는 그 경로를 가린다(제안 4) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(제품 코드 변경 없음).
- 제품 코드의 `unwrap`/`expect`: 변경 없음. 시험 도우미의 `u64::try_from(..).unwrap()`은 시험 코드라 해당 없음.
- 에러가 삼켜지는 곳: `ci.yml:52-58` keepalive 실패는 의도적으로 무시한다(보고서에 적힘). 그러나 시험 도중 keepalive가 죽는 경우는 어디에도 기록되지 않는다(지적 1). `midi-keepalive.swift:11-15`는 실패를 stderr와 종료 코드 1로 알린다. 문제없다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 지적 1 | 수정 | keepalive가 시험 내내 살아 있었는지 확인할 수 없다. (가) `RunLoop.main.run()`은 run loop에 입력 소스·타이머가 없으면 곧바로 돌아오는 API다(Foundation 문서). 알림 함수 없이(`nil`) 만든 클라이언트가 main run loop에 소스를 붙이는지는 이 저장소에서 확인된 적이 없다. 붙이지 않으면 스크립트는 「ready」를 찍고 곧바로 끝나 클라이언트가 사라지는데, 워크플로 로그는 살아 있을 때와 똑같이 `midi-keepalive: ready`다(3절 #3 (a)와 (d)). (나) 로그는 `cargo test` 전에 한 번만 찍는다. 보고서 「확인」절은 다음 실패 때 「midi-keepalive 로그 줄을 본다」고 하지만, 그 줄로는 「가설이 틀림」과 「keepalive가 없었음」을 가를 수 없다. 실패가 4회 중 1회인 간헐 현상이라 녹색 한 번도 가설을 확인하지 못한다. 고칠 길: 스크립트 끝을 조건 없이 막히는 형태(`while true { sleep(3600) }` 또는 `dispatchMain()`)로 바꾸고, `cargo test` 뒤 종료 코드를 보존한 채 `kill -0`으로 생존 여부를 한 줄 더 찍는다 | `.github/ci/midi-keepalive.swift:18`, `.github/workflows/ci.yml:52-59`, `docs/reports/ci/first-run.md:30` |
| 지적 2 | 수정 | 보고서의 사실과 원인 근거가 원격 기록과 어긋난다. (가) 「실행 2(b130832)는 끝난 기록이 없다」는 틀렸다. 실행 41437132455는 끝났고 같은 시험이 `Tick(1603)`으로 실패했다. 표에 넣어야 한다(실패 셋 모두 일시정지 시험, 1601·1603·1604). (나) 원인 2의 근거 「같은 실행의 앞선 바이너리들은 클라이언트를 만들었다」는 실행 1–3에도 그대로 성립한다. 같은 바이너리 순서와 비슷한 간격(files_playback 종료 뒤 약 5–6 s에 `send_test_note`가 클라이언트 생성, 실행 4는 11:48:56.8 → 약 11:49:02, 실행 1은 11:51:05.4 → 약 11:51:11)에서 실행 1–3은 통과했다. `review_s1_t1.rs:84`의 실측대로 유휴 종료가 약 3 s라면 실행 1–3에서도 서버는 내려갔다가 다시 떴다. 그러니 가설은 「서버 재기동이 가끔 10 s 넘게 실패한다」로 좁혀야 하고, 이 저장소가 지금까지 본 재기동 오류는 `-304`·`-2`였지 `-50`이 아니었다는 점(`client.rs:196-197`, `review_s1_t1.rs:91-93`)도 반대 근거로 적어야 한다. (다) 「앞 시험 끝 11:49:03 → 실패 11:49:12」는 9.2 s로 재시도 10 s보다 짧다. 시험이 병렬로 돌아 `send_test_note`는 앞 시험이 끝나기 전에 시작했다는 뜻이라, 이 시각 쌍은 근거가 되지 못한다. 커밋 85b041a 본문은 가설을 사실처럼 적었다(「MIDI 서버가 내려가 … 실패했다」) | `docs/reports/ci/first-run.md:16`, `docs/reports/ci/first-run.md:21`, 커밋 85b041a 본문 |
| 제안 3 | 제안 | `cargo test --workspace`는 첫 실패 바이너리에서 멈춘다. 원격 네 번 모두 keyscore-app 안에서 멈춰, keyscore-app의 `review_s2_t5a` 뒤 시험 파일(이름순으로 약 120개)과 keyscore-core·keyscore-midi·keyscore-text·tools의 시험은 러너에서 한 번도 돌지 않았다. 이번 두 대응이 맞아도 다음 실행에서 새 실패가 하나씩 드러날 수 있다. `--no-fail-fast`로 한 번에 전부 드러내거나, 최소한 보고서 「알려진 한계」에 이 사실을 적는다 | `.github/workflows/ci.yml:59`, `docs/reports/ci/first-run.md:35` |
| 제안 4 | 제안 | 가설이 맞다면 원인은 CI만의 사정이 아니다. 다른 MIDI 앱이 없는 사용자 Mac에서 마지막 MIDI 앱을 끈 직후 KeyScore를 열면 같은 10 s 재시도를 넘겨 엔진 없이(OUT-1 포트 없이) 뜰 수 있다. keepalive는 CI에서 그 경로를 가린다. 「앱 시작 동작을 CI 사정으로 바꾸지 않으려는 것」이라는 표현 대신, 제품 쪽 위험으로 HANDOFF나 결정 요청(`docs/decisions/`)에 남길 것을 권한다 | `docs/reports/ci/first-run.md:26` |
| 제안 5 | 제안 | 시험이 `SchedulerConfig::default().lookahead`를 직접 읽는다. 엔진이 다른 설정을 쓰게 바뀌면(`engine.rs:801`) 상한이 소리 없이 틀린다. 지금은 일치하므로 주석으로 「엔진이 기본 설정을 쓴다는 가정」을 적어 두는 정도면 된다 | `crates/keyscore-app/tests/review_s2_t5a.rs:290` |

확인했지만 문제가 아닌 것(지적 없음):

- 상한은 건전하다. 기준 시각이 `now + 2 ms` 뒤라 늘 2틱 안팎의 여유가 있고, 정지 처리 시점의 `sent_through`는 그 이전 바퀴의 지평(`now + lookahead`)에서 내림으로 구한 값이다. `NoAnswer` 추정도 `pause` 호출 전에 템포 지도로 구해 끝 안에 든다.
- 판별력: 템포 무시 구현은 바깥 지연이 약 0.49 s(+ 안쪽 지연)를 넘지 않는 한 실패한다.
- clippy pedantic: 새 함수는 비공개라 `must_use_candidate`·`missing_panics_doc`가 걸리지 않고, 캐스트가 없다. `rustfmt --check` 통과.
- 셸: `set -e`·`pipefail`에서 `grep … && break`, `kill -0 … || break`, `$(cat …)`이 단계를 끊지 않는다. 출력이 파일로 가 단계가 백그라운드 프로세스를 기다리지 않고, 작업 끝 「Cleaning up orphan processes」가 정리한다. 단계 종료 코드는 `cargo test`의 것이다.
- Swift: `MIDIClientCreate(_: CFString, _: MIDINotifyProc?, _: UnsafeMutableRawPointer?, _: UnsafeMutablePointer<MIDIClientRef>)` 호출 형태, `as CFString` 브리징, `status == noErr`(이종 정수 비교), `exit(1)`, `fflush(stdout)` 모두 맞다.
- 추가 클라이언트의 영향: keepalive는 엔드포인트를 만들지 않아 소스·목적지 열거, 이름 중복 검사, 핫플러그 알림에 끼지 않는다. 서버 유휴 종료를 다루는 `review_s1_t1.rs:76-124`는 자식 프로세스가 해제하지 않는 프로세스 클라이언트를 쥐고 있어 keepalive가 없어도 서버가 내려가지 않으므로 뜻이 바뀌지 않는다. 서버를 죽이는 시험(`killall`, `MIDIRestart`)은 없다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

시험 상한 변경(8ec4e60)은 그 자체로 병합 가능한 수준이다. 지적 1(keepalive 생존을 확인할 수 없어 간헐 실패의 원인을 가를 수 없음)과 지적 2(보고서 사실 오류와 반대 근거 누락)를 고친 뒤 다시 본다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [ ] 깨뜨리는 테스트를 실제로 실행했다 — Rust 시험은 이 컨테이너(Linux)에서 빌드할 수 없다. 대신 셸 모의 실행, 수치 모의, 원격 로그 대조를 실행했다
- [x] REQ ID를 하나씩 대조했다(TRN-1, OUT-1)
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(상한이 바뀌었을 뿐, 하한 1300과 시험 이름은 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 crate 의존성 없음. CI가 러너에 기본 설치된 `swift`를 쓴다)
- [x] 스타일 지적을 적지 않았다
