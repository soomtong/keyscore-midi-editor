# HANDOFF: KeyScore S2 진행 중 — S2-T1~T5d·T6a~T6c 병합, S2-T6d 다음

## 목표

`plan/` 계획(README → 00-conventions → 01-protocol → 02-spikes → stages)에 따라 KeyScore(Rust + GPUI, macOS 키보드 중심 MIDI 시퀀서)를 단계별로 구현한다. Task마다 「계획 → 테스트 먼저(실패 확인) → 구현 → 자동 게이트 → 보고 → 새 에이전트의 적대적 리뷰 → 수정 → `--no-ff` 병합」 루프를 돈다. 규칙은 `CLAUDE.md`와 `plan/01-protocol.md`가 기준이다.

## 현재 브랜치

`main` — S0 완료·서명(2026-10-15), S1-T1~T7과 계이름 텍스트 주석 `//` 변경 병합(테스트 818개 통과). S1 출구 서명(YoungTaek, 2026-10-16). S2-T1~T4·S2-T5a~T5d·S2-T6a~T6c 병합(테스트 1467개). S2-T6은 T6a(설정 창·키맵 편집기)·T6b(`?` 오버레이)·T6c(입력·메트로놈·AI 탭)·T6d(MIDI 출력 탭)·T6e(이월 줄)로 나눴다. 다음은 S2-T6d.

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
- [x] S2-T6c 설정 > 입력·메트로놈·AI(리뷰 2회, 2회차 「병합 가능」, 병합 3162dbf). `settings.json`에 `input.key_layout`·`metronome{port,channel,accent,beat,length_ms}`(필드별 검사, 모르는 키 보존). `src/settings_tabs.rs`: 선택 상자(검색, Tab 정지점, `enter`/`space`로 열기), 노트는 GM 타악기 이름(`gm::percussion_label`), AI는 비활성 자리. 전역 처리기 `settings::SetKeyLayout`(저장만, 해석은 S3)·`settings::Calibrate`(입력 구역 + 알림, 마법사는 S5-T2). 엔진 `prepare(project, live, metronome_port)`가 메트로놈 포트를 트랙 포트 규칙으로 열고 `Prepared.metronome`, 없으면 클릭만 끄고 알림(`metronome_port_missing`). 재생 중 교체는 클릭 끄기 → swap → set_metronome → 켜기. 설정 변경은 `engine::metronome_changed`로 재생 창에 바로 반영. 메트로놈 이름 포트도 포트 감시 대상(모든 창). 결정: undo 상한 A(`2026-10-19-undo-history-memory-limit.md`, REQ EDT-1 문구 반영, 구현은 T6e)
- [x] S2-T6b `?` 키맵 오버레이(리뷰 3회, 3회차 「병합 가능」, 병합 e3a9fe0). `src/overlay.rs`: 지금 모드 경로에서 엔진이 실제로 고르는 바인딩만(`bindings_for_input` 첫 승자), 명령 표 변형마다 한 줄, 변형 없는 인자는 키맵 원문(`effective_with`)에서 라벨, `Kbd`로 키, (키맵 판, 모드 경로) 캐시, 쪽 넘김 `Tab`, `Esc`(`help::CloseOverlay`)·`?`·바깥 클릭(`occlude`)으로 닫기, 팔레트·팝업과 하나만
- [x] S2-T6a 설정 창과 키맵 편집기(리뷰 3회 + 결정 `2026-10-18-s2-t6a-review-limit.md` A의 좁은 확인 1회, 병합 31d9182). `src/settings_window.rs`(앱에 하나인 별도 창, 컨텍스트 `Settings > Keymap > Editing`, 시트 `Dialog`, 구역 이동 `ctrl-tab`, 키맵 밖 구역은 T6c 자리표시자), `src/keymap/user.rs`(실효 바인딩 `effective_with`(수식키 교환 반영, `null`·`zed::Unbind` GPUI 규칙), 충돌은 한 경로의 관련 컨텍스트(같음·바깥·안쪽·전역)의 같은 키·접두, `UserKeymap`: 편집 메서드는 실제 키를 받고 파일에 쓸 때 교환, 지우기는 `null` 대신 `["zed::Unbind", 이름]`, 같은 자리 여러 항목은 k번째 절). `keymap::save_user`(내용 기준 `.bak`, 원자적 쓰기, 곧바로 apply), `set_swap_alt_cmd`는 표준 지정 해제를 새 교환 상태로 고쳐 씀. press-to-bind는 앱 키 가로채기(`intercept_keystrokes`)로 받고 1 s에 확정. 새 Action 7개(`keymap::SelectNext/SelectPrev/FocusSearch`, `settings::NextSection/PrevSection`, `dialog::Confirm/Cancel`). 키맵 구역 키 `alt-1/alt-2/alt-m/cmd-shift-e/cmd-shift-i`. `window::Close`는 `defer`로 닫는다
- [x] S2-T5d 포트 감시와 재연결(리뷰 3회, 3회차 「병합 가능」, 병합 23cdc56). `src/ports.rs`: 전역 `PortMonitor`(백그라운드 `PortWatcher::start`, 100 ms `tick`: 이벤트 반영 → 새 이름 `want` → `Lost`/`Reconnect` 처리, 1 s마다 가상 포트 `is_registered`, 실패는 처음 한 번만 알리고 10 s 간격 재시도). 쓰는 창: 「사라졌다」(재생 중이면 그 포트 빼고 다시 준비)·「다시 연결했다」(재생 중이면 다시 준비 + 그 목적지 음색 한 번)·「같은 이름의 다른 포트로 바꿨다」. 쓰지 않는 창: 핫플러그 묶음 줄(OUT-3). 엔진: 같은 이름은 고유 ID 최소(`best_match`, `Reconnector`와 같은 규칙), 연 포트는 `(PortInfo, Arc<OsOutput>)`로 재생 시작 때 정체 확인·안 쓰면 놓음, `recover_virtual_port`(빈 스냅샷 → `Arc::get_mut` → `reregister`). 상태줄 「포트 없음」. 열린 창 목록 `window::project_windows`. 테스트에서 CoreMIDI 알림은 목록보다 늦다: `wait_seen`으로 기다린 뒤 창을 연다
- [x] S2-T5c 트랙 인스펙터와 상태줄(리뷰 3회 + 결정 `2026-10-18-s2-t5c-review-limit.md` A의 좁은 확인 1회, 병합 004ed73). `src/fields.rs`(창의 한 줄 입력란 5개 공통 규칙, 창 비활성화의 Blur는 취소 아님), `src/inspector.rs`(이름·색·포트·채널·뱅크·프로그램 `Select`(GM 검색)·기본 벨로시티·가사 언어·M/S·테스트 노트, `track::Rename/SetColor/SetPort/SetProgram/SetDefaultVelocity/SetLyricLang`, `midi::SendTestNote`), `src/gm.rs`, 상태줄(모드·「트랙 · 포트 chN」·키맵 오류 KM-6 또는 마지막 메시지). 모드 키 컨텍스트 겹은 가운데 주 뷰만 감싼다. 겹 바깥(트랜스포트 줄·패널·상태줄)을 누르면 `focus_document_on_press`가 문서로 포커스를 돌린다(입력란을 누른 경우만 예외). `SetProgram` program은 GM 번호 1–128(0 없음), bank 음수 없음
- [x] S2-T5b 트랜스포트 바와 트랙 목록(리뷰 3회 + 결정 `2026-10-17-s2-t5b-review-limit.md` A의 좁은 확인 1회, 병합 a6bc6c1). `src/transport_bar.rs`(재생·정지·패닉 `⏹!`·녹음(꺼짐)·템포/박자표 입력란·조성 메뉴·위치·메트로놈·탭·`AI✦`(꺼짐)·포트 메뉴), `src/track_list.rs`(트랙 Action과 목록, SEQ-8 경고), `src/tracks.rs`(순수 계산). 트랜스포트 바는 모드 키 컨텍스트 겹 바깥(루트 첫 줄)에 그린다. 입력란 규칙: 「고친 필드」 = 지금 글자 ≠ 앱이 마지막에 넣은 글자, 같은 값 확정은 편집을 만들지 않음, Esc·포커스 잃음은 취소, 버튼은 누른 뒤 문서로 포커스. `edit::Undo/Redo`는 포커스 경로에 `Input` 컨텍스트가 있으면 `input::Undo/Redo`로 보낸다(키·메뉴 공통, KM-14). 시험 창은 `activate_window()`해야 Focus/Blur 사건이 난다
- [x] S2-T5a MIDI 엔진 연결과 트랜스포트 Action(리뷰 3회 + 결정 `2026-10-17-s2-t5a-review-limit.md` A의 좁은 확인 2회). core: 포트·조성 값이 바뀌면 모르는 키를 지우고 undo가 되살린다(결정 B, 내부 `Op::SetPort`·`Op::SetKey{extra}`). app `src/engine.rs`: 전역 `Engine`(첫 CoreMIDI 호출 `MidiClient::new` → 가상 포트 → 스케줄러, `run`이 창보다 먼저 `install`, `init`은 CoreMIDI 안 건드림), `prepare`(트랙 → 포트, 없는 이름 포트는 소리 없이 `PortProblem`, 들리지 않는 트랙의 뱅크·프로그램을 먼저 틱 0에 넣어 chase, `Setup`은 트랙별 비교). `src/transport.rs`: Play/Stop/Toggle/Pause/PlayFromCursor/Panic/ToggleMetronome, 위치 기준점 `Anchor`(시각·틱·템포), 재생 중 편집은 `cx.observe(document)` → 교체 직전 `collect_garbage` → swap → 바뀐 트랙 프로그램만 send_now, 한 번에 한 창. 루프백 테스트는 같은 프로세스 `coremidi` 수신기(dev-dep)
- [x] S2-T4 명령 팔레트·메뉴바(리뷰 3회 + 결정 `2026-10-17-s2-t4-review-limit.md` A의 좁은 확인 2회). `src/commands/{mod.rs,table.rs}` 명령 표(Action 287개마다 이름·자리·변형; 자리는 `Menu{메뉴,묶음,순서}` 또는 `Elsewhere{Surface, 설명}`, 메뉴 80 + 메뉴 밖 207), `src/dispatch.rs` `run_action`(카운트 비우고 디스패치, 결정 C), `src/command_palette.rs`(gpui-component `Command`, 거르기·순위는 `commands::filter_entries`가 직접, 실행 가능한 줄 먼저, 바깥 클릭으로 닫힘, 팝업은 하나만). 메뉴는 명령 표로 자동 생성되고 키맵을 걸 때마다 다시 만든다. GPUI가 구울 키 등가물이 Cmd·Ctrl 없는 키이거나 어느 모드에서도 그 명령이 아니면 `menu::RunCommand`로 감싸고 제목 꼬리에 실제로 누를 키(`effective_keys`, 모든 모드 경로)를 보인다. 결정: vim 마크는 피아노롤 우클릭 메뉴(`2026-10-17-vim-marks-visible-ui.md`, S3 이월). 새 Action `palette::Close`(A13)
- [x] S2-T3 모드 FSM(리뷰 3회 + 좁은 확인 1회). `src/mode.rs`(순수 FSM, 상태 28개, 사건 `Enter`/`Cycle`/`ToNormal`/`LeaveAi`/`ClearSelection`/`DismissGhost` 등), 창은 컨텍스트 경로만큼 `div`를 겹치고 가장 안쪽이 포커스를 잡는다, 상태줄 모드 표시·모드 메뉴(컨텍스트 겹 바깥에 그림). 키맵 엔진은 설치 전 gpui-component 바인딩을 바탕 레이어(`keymap::base_bindings`)로 보존. 결정: A13 Esc = 통상의 취소(사람, 해석 표는 `2026-10-17-keymap-conflicts.md` A13, 조합 중은 확정 후 Normal, 레이아웃 패널은 닫지 않음), 모드 진입 뷰(`2026-10-17-mode-entry-view.md`: Lyric·Controller만 피아노롤로). `mode::Back` 삭제(Action 286개). REQ P2·5.2·5.3·5.5·5.6·5.8·5.10·5.11·UI-2 문구 수정
- [x] S2-T2 키맵 엔진(리뷰 3회, 3회차 「범위 재정의」 → 결정 `2026-10-17-count-and-non-key-dispatch.md` C). Action 287개(`src/actions.rs`, 카탈로그 `docs/reports/s2-t2/action-catalog.md`), 프리셋 `assets/keymaps/{standard,default,vim}.json`, 레이어(표준→기본→vim→사용자), 같은 레이어 안 짧은 시퀀스 먼저, `keymap.json` 1 s 폴링, 카운트는 키 디스패치에만. 결정: 시퀀스 1 s 고정(A), REQ 키 충돌 13건은 결정 대기(`2026-10-17-keymap-conflicts.md`, 프리셋에서 비움). GPUI macOS는 Shift+숫자를 기호 키(`#`)로 보낸다
- [x] S2-T1 GPUI 앱과 창 모델(리뷰 3회). gpui-kit `=0.7.1`(결정 `2026-10-16-gpui-kit-licenses-and-advisories.md` A: 0BSD·CC0 허용, 권고 넷 무시). 창 = 프로젝트, `ProjectDocument`가 undo 엔진만 거침, 펴진 패널만 Resizable 자식, 설정 `settings.json`(모르는 키 보존, `.bak`). core `.ksq` 쓰기는 모르는 키를 직접 정렬(워크스페이스에서 `serde_json` `preserve_order`가 켜짐)
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
- `serde_json::Map` 순서에 기대지 않는다. 워크스페이스에 gpui-kit이 있으면 feature 통합으로 `preserve_order`가 켜진다(S2-T1).
- 파서·변환에서 「정렬 삽입을 하나씩」은 악의적 입력에서 O(n²)이 된다(S1-T6 리뷰 1–3). 모아서 한 번 정렬하고, 반복 안의 전체 훑기를 피한다.
- REQ에 없는 전이 규칙을 계획의 「세부」로 넣기(S2-T3 「노트 입력 모드는 피아노롤로」): 리뷰가 차단으로 막았다. 규칙을 넣을 때도 뺄 때도 REQ 문장을 근거로 결정 문서를 쓴다.
- 메뉴 키 표시를 GPUI 규칙(`find_or_first`)만으로 고르기(S2-T4 리뷰 3·4): 가려진 키를 보였다. 표시는 「실제로 그 명령이 나오는 키」로 고르고 키 등가물은 그것과 맞을 때만 굽는다.
- 재생 시작 때 트랙마다 `send_now`로 프로그램 보내기(S2-T5a 리뷰 1): 명령 큐(256)가 넘쳤다. 스케줄러 chase를 쓴다. 스냅샷 교체 전 `collect_garbage`를 부르지 않으면 교체 약 260번 뒤 큐가 막힌다(리뷰 2).
- 사람 결정의 해석을 REQ 의미로 옮길 때 원문과 다르게 읽힐 수 있는 행은 사람에게 확인한다(S2-T3 리뷰 3).
- 단계 파일 수치를 검증 없이 옮기기: S0-T3 「1시간 = 3,456,000틱」은 60 BPM 기준이었다(정정함).

## 남은 작업

- [ ] 원격 CI 첫 실행과 CI 러너 벤치 기준값: remote가 생기면(사람이 건너뜀)
- [ ] 후속: `crates/keyscore-midi/tests/scheduler.rs`의 20초 지터 측정이 같은 바이너리의 루프백 테스트와 겹치면 가끔 0.5 ms를 넘는다. 타이밍 측정 테스트를 서로 직렬로 묶는다.
- [ ] 결정 대기: `2026-10-17-keymap-conflicts.md`의 A1–A12·B24·B25(S3–S6 전). A13은 결정됨
- [x] REQ 9.2 문구(시각 대기 예외) 반영, 옛 `#` 주석은 레거시 지원 모드(`ParseOptions::legacy_hash_comments`, `txt2ksq --legacy`)
- 개발 단계 REQ 문구 수정은 사람이 모두 승인(2026-10-17, `MEMORY.md`). 결정을 REQ에 옮길 때 묻지 않고 커밋 본문에 근거를 적는다
- [ ] 사람의 수동 확인 대기: Surge XT 수신(SP-3), 실제 키보드·IME(SP-1), 120 Hz 디스플레이(SP-2)

결정 대기(`rg -l "상태: 결정 대기" docs/decisions/`):
- `2026-10-11-ai-model-defaults.md` — S7 전
- `2026-10-12-ksq-newer-version-save.md` — S2-T7 전
- `2026-10-12-quantize-grid-lengths.md` — S5 전

REQ 문구: 사람이 에이전트에게 수정을 허락한 범위(결정 문서의 「후속(사람)」)는 반영했다. 그 밖의 REQ 수정은 여전히 사람이 한다.

## 핵심 파일

- `CLAUDE.md` — 저장소 규칙. 커밋 첫 줄 `<crate>: …` / `plan: …`, 에이전트가 직접 커밋·병합한다
- `plan/01-protocol.md` — Task 루프와 적대적 리뷰 규칙
- `plan/stages/10-S0-foundation.md` — S0 Task와 「이월」 줄
- `plan/stages/11-S1-midi-engine.md` — S1 Task와 이월 줄
- `plan/stages/12-S2-app-shell.md` — S2 Task와 이월 줄
- `crates/keyscore-app/src/{mode.rs,window.rs,keymap/mod.rs}` — 모드 FSM, 창·키 컨텍스트, 키맵 엔진(바탕 레이어)
- `crates/keyscore-app/src/{commands/,dispatch.rs,command_palette.rs}` — 명령 표·메뉴·팔레트·실행 함수
- `crates/keyscore-app/src/{engine.rs,transport.rs}` — MIDI 엔진과 창 트랜스포트
- `crates/keyscore-midi/src/scheduler.rs` — 스케줄러(`Scheduler` 공개 API, 실시간 상태 `Rt`)
- `docs/reports/s1-t4/report.md` — 스케줄러 리뷰 대응과 알려진 한계
- `docs/decisions/2026-10-12-quantize-and-transform-rules.md` — S0-T7 규칙(개정·부록 포함)
- `crates/keyscore-core/src/transform.rs` — S0-T7 연산(`plan`, `snap`, `relocate`)
- `crates/keyscore-core/src/edit.rs` — 공개 `Edit`, 내부 `Op`(역편집), `approx_bytes`
- `crates/keyscore-core/src/document.rs` — 편집의 유일한 진입점
- `crates/keyscore-core/src/ksq.rs` — `.ksq` 읽기·쓰기
- `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.py`, `scripts/bench-check.test.sh` — 추가 게이트

## 다음 에이전트에게

1. S2-T6d(설정 > MIDI 출력)를 시작한다: 가상 포트 추가·이름 `midi::AddVirtualPort/RenameVirtualPort`(OUT-2, 최대 16, 엔진은 지금 가상 포트 하나 `Engine.main`뿐이라 포트 모델·트랙 포트 지정 방식을 계획에서 정한다), 포트별 MIDI Clock(OUT-5, 스케줄러 `set_clock(port, on)` 있음, 스냅샷마다 포트 번호가 바뀌므로 교체마다 다시 보냄), 테스트 노트(PRE-3, 인스펙터 `midi_send_test_note` 재사용), 리스너 표시(PRE-2, 확인 가능할 때). 단계 파일 S2-T6 절의 이월(S2-T6c 리뷰 2 제안 1: 교체 묶음의 실패 처리)을 함께 본다. 설정 창의 MIDI 출력 구역 자리표시자(`settings_window.rs`)를 채운다. 그다음 S2-T6e(undo 상한 알림, `alt-cmd-i`, 패널 너비, 창 최소 크기, 오버레이 인자 라벨). S2 출구 수동 확인 목록: MIDI 서버 재시작 감지(S2-T5d), 팔레트 첫 Esc(A13), 메뉴 키 등가물, IME, `keyscore-text` 시간 측정 테스트가 전체 실행 중 가끔 실패.
2. Task 루프: 브랜치 `s2/t<번호>-<slug>` → `docs/reports/s2-t<번호>/plan.md` → 테스트 먼저(뼈대 `todo!()`로 실패 확인) → 구현 → 게이트 전부(커밋 전에) → `report.md` → 새 에이전트로 적대적 리뷰(`general-purpose`, 구현 맥락 없이, 양식 `plan/templates/adversarial-review.md`, 산출물만 남기고 커밋 금지, 기존 `.proptest-regressions` 삭제 금지) → 지적마다 커밋 하나 → 재리뷰(최대 3회) → `git merge --no-ff`.
3. 자동 게이트: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `scripts/check-deps.sh`, `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`. `core`·`text`에 외부 의존을 더하면 `scripts/check-deps.sh`의 허용 목록도 고친다.
4. REQ에 없는 결정은 임의로 정하지 말고 사람에게 묻거나 `docs/decisions/`에 결정 요청을 쓴다. 사람과의 대화는 한국어로 하고, 문서에 `**`를 쓰지 않는다.
5. 긴 MIDI 측정 중에는 다른 루프백 테스트를 돌리지 않는다(수치가 흔들린다). 그동안 다른 crate 작업은 `CARGO_TARGET_DIR=target/alt`로 빌드 잠금을 피한다.
