# 단계 출구: S2 앱 뼈대(GPUI, 키맵, 모드, 명령 팔레트, 설정, 저장·복구)

- 기준 커밋: `main` `1949395`(S2-T7e 병합 96aa5aa 뒤 HANDOFF 갱신)
- 작성: 계획자 에이전트, 2026-10-24
- 서명: YoungTaek, 2026-10-24(주요 키보드 동작 확인. 세세한 점검은 추후)

## Task 결과

| Task | 내용 | 리뷰 | 보고서 |
| --- | --- | --- | --- |
| S2-T1 | GPUI 앱과 창 모델(gpui-kit 0.7.1, 창 = 프로젝트, 설정 파일) | 3회, 병합 가능 | `docs/reports/s2-t1/` |
| S2-T2 | 키맵 엔진(Action 287개, 프리셋 3개, 레이어, 시퀀스, 카운트) | 3회(3회차 범위 재정의 → 결정 C), 병합 가능 | `docs/reports/s2-t2/` |
| S2-T3 | 모드 FSM과 Enter/Esc/Tab 규약(상태 28개, 컨텍스트 겹) | 3회와 좁은 확인 1회, 병합 가능 | `docs/reports/s2-t3/` |
| S2-T4 | 명령 팔레트와 메뉴바(명령 표가 유일한 원천, `run_action`) | 3회와 결정 A의 좁은 확인 2회, 병합 가능 | `docs/reports/s2-t4/` |
| S2-T5a | MIDI 엔진 연결과 트랜스포트 Action | 3회와 결정 A의 좁은 확인 2회, 병합 가능 | `docs/reports/s2-t5a/` |
| S2-T5b | 트랜스포트 바와 트랙 목록 | 3회와 결정 A의 좁은 확인 1회, 병합 가능 | `docs/reports/s2-t5b/` |
| S2-T5c | 트랙 인스펙터와 상태줄 | 3회와 결정 A의 좁은 확인 1회, 병합 가능 | `docs/reports/s2-t5c/` |
| S2-T5d | 포트 감시와 재연결 | 3회, 병합 가능 | `docs/reports/s2-t5d/` |
| S2-T6a | 설정 창과 키맵 편집기 | 3회와 결정 A의 좁은 확인 1회, 병합 가능 | `docs/reports/s2-t6a/` |
| S2-T6b | `?` 키맵 오버레이 | 3회, 병합 가능 | `docs/reports/s2-t6b/` |
| S2-T6c | 설정 > 입력·메트로놈·AI | 2회, 병합 가능 | `docs/reports/s2-t6c/` |
| S2-T6d | 설정 > MIDI 출력의 가상 포트 | 3회와 결정 A의 좁은 확인 1회, 병합 가능 | `docs/reports/s2-t6d/` |
| S2-T6e | 포트별 MIDI Clock·테스트 노트·포트 상태 | 3회, 병합 가능 | `docs/reports/s2-t6e/` |
| S2-T6f | S2-T1~T6 이월 줄 정리(undo 상한 알림, 패널 너비, 창 최소 크기 등) | 3회, 병합 가능 | `docs/reports/s2-t6f/` |
| S2-T7a | 새로 만들기·열기·저장·닫기·끝내기·최근 파일 | 3회, 병합 가능 | `docs/reports/s2-t7a/` |
| S2-T7b | 자동 저장본과 크래시 후 복구 | 3회와 결정 A의 좁은 확인 1회, 병합 가능 | `docs/reports/s2-t7b/` |
| S2-T7c | SMF·계이름 텍스트 가져오기·내보내기 | 2회, 병합 가능 | `docs/reports/s2-t7c/` |
| S2-T7d | OUT-8 없는 출력 포트의 대체 매핑 제안 | 3회, 병합 가능 | `docs/reports/s2-t7d/` |
| S2-T7e | S2-T7b 리뷰 4·S2-T7c 리뷰 2·S2-T7d 리뷰 3 이월 | 2회, 병합 가능 | `docs/reports/s2-t7e/` |

## 자동 게이트 (계획자가 `1949395`에서 실행, 2026-10-24)

- [x] `cargo fmt --all -- --check`: 통과
- [x] `cargo clippy --workspace --all-targets -- -D warnings`: 경고 0
- [x] `cargo test --workspace --no-fail-fast`: 1875 passed, 1 failed, 2 ignored(1시간판 측정용으로 원래 무시하는 것). 실패 하나는 재생 시간 측정 시험 `review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits`(일시정지 자리 Tick 6206이 들은 박 6240보다 앞, 경과 3.22 s)이고, 그 바이너리만 다시 돌리면 13 passed로 통과했다. 전체 실행 중 다른 바이너리의 CoreMIDI 루프백과 겹쳐 생기는 지터로 본다(HANDOFF 「시도했으나 실패한 접근」에 같은 기록). 아래 「전체 시험 메모」.
- [x] `cargo deny check`: advisories·bans·licenses·sources ok
- [x] `scripts/check-deps.sh`: ok
- [x] `scripts/check-deps.test.sh`: 16 passed
- [x] `scripts/bench-check.test.sh`: 8 passed
- [x] UI-11: 모든 Action에 메뉴 또는 패널 UI가 있다. `crates/keyscore-app/tests/commands.rs`의 `every_action_has_exactly_one_command_spec`(등록된 Action마다 명령 표 한 줄, 메뉴 밖 자리는 화면 면이고 「키」가 아님)과 `action_counts_match_palette_and_menus`(등록 수 = 팔레트 줄 수 = 메뉴 + 메뉴 밖)가 통과했다. 팔레트에서만 되는 기능은 없다(`Place::Elsewhere`의 `Surface`는 트랜스포트·트랙 목록·인스펙터·설정·오버레이 같은 화면 면만).
- [x] S2-T7 게이트: 저장 중 kill 뒤 원본 보존(`crates/keyscore-app/tests/save_kill.rs`, SIGKILL 20회), 자동 저장본은 프로젝트 폴더가 아니라 설정 파일 옆 `autosave/`에만 생긴다(`autosave.rs` 시험).

## 사람이 확인할 항목

앱 실행은 저장소 루트에서 다음과 같다. 설정은 `~/Library/Application Support/KeyScore/settings.json`, 자동 저장본은 같은 폴더의 `autosave/`다.

~~~
cargo run -p keyscore-app --release
~~~

- [x] (사람: 확인, 2026-10-24) 키보드만으로 한 바퀴(출구 게이트 「수동」): 마우스를 쓰지 않고 다음을 한다.
  1. `cmd-n` 새 프로젝트(이미 빈 창이면 그대로).
  2. `cmd-t` 트랙 추가. 트랙 목록에 「트랙 1」이 보인다.
  3. 포트 선택: `cmd-shift-p` 팔레트에서 「포트」를 쳐 `track::SetPort`를 고르거나, 인스펙터 패널의 포트 선택 상자를 Tab으로 짚어 `enter`로 연 뒤 화살표·`enter`로 고른다. 상태줄에 「트랙 1 · <포트> ch1」이 보인다.
  4. `cmd-enter` 재생, 다시 `cmd-enter` 정지. 트랜스포트 바 위치가 움직인다(노트가 없어 소리는 없다. 메트로놈을 켜면 클릭이 들린다).
  5. `cmd-s` 저장. 이름 시트에서 경로를 치고 `enter`. 창 제목이 파일 이름으로 바뀐다.
  6. `cmd-q` 종료. 바뀐 창이 없으니 시트 없이 끝난다.
- [ ] (사람: 추후 점검, S3을 막지 않음) 메뉴 키 등가물(출구 게이트 「수동(S2-T4 이월)」): 팔레트 입력란과 한글 입력 중에 글자가 메뉴로 빠지지 않는지 본다.
  1. `cmd-shift-p`로 팔레트를 열고 영문·한글·`option-글자`(예: `option-a` → å)를 친다. 글자가 입력란에 그대로 들어가고 메뉴 명령이 실행되지 않는다.
  2. 인스펙터의 트랙 이름 입력란에서 한글을 조합하는 중에(예: 「ㅎㅏㄴ」) 메뉴 명령이 끼어들지 않는다. `enter`로 확정, `esc`로 취소된다.
  3. 메뉴를 열어 Cmd·Ctrl 조합만 키 등가물로 보이고, 맨 키(`space`, `v` 등)는 제목 꼬리에 글자로만 표시되는지 본다(`menu::RunCommand`로 감싼 항목).
- [ ] (사람: 추후 점검, S3을 막지 않음) 팔레트의 첫 Esc(결정 A13): 팔레트에 글자를 친 상태에서 `esc` 한 번에 팔레트가 닫히고 모드는 그대로다.
- [ ] (사람: 추후 점검, S3을 막지 않음) 크래시 뒤 복구(S2-T7b): 트랙을 추가하고 30초 기다린 뒤(자동 저장) 터미널에서 `kill -9 <keyscore-app pid>`. 다시 실행하면 복구 시트가 뜬다. [복구]는 「(복구됨)」 창을 열고, [버리기]는 저장본을 지운다. `autosave/`에 남는 파일이 없다.
- [ ] (사람: 추후 점검, S3을 막지 않음) SMF·텍스트 가져오기·내보내기(S2-T7c): 파일 메뉴에서 `crates/keyscore-midi/tests/data/smf/`의 SMF 하나를 가져오면 새 창이 열리고, 상태줄에 손실 요약이 보인다. `assets/fixtures/`의 `.ksq`를 열어 SMF·텍스트로 내보내고, 텍스트(`tools/ksq-cli/fixtures-src/*.txt`)를 지금 창에 가져오면 재생 위치에 트랙이 생기고 `cmd-z` 한 번에 되돌아간다.
- [ ] (사람: 추후 점검, S3을 막지 않음) 다른 Mac 파일의 포트 매핑 시트(S2-T7d): 이 기계에 없는 포트 이름을 가진 `.ksq`(픽스처의 `port`를 손으로 바꾼 사본)를 열면 대체 매핑 시트가 뜬다. 받아들이면 같은 이름의 앱 가상 포트가 생기고 트랙이 그 포트를 쓴다. 같은 이름의 장치가 나타나면(예: 신디의 가상 입력 이름을 그 이름으로) 대체 포트가 사라지고 트랙이 장치로 돌아간다.
- [ ] (사람: 추후 점검, S3을 막지 않음) 포트 핫플러그와 MIDI 서버 재시작(S2-T5d·T6d 이월): 재생 중 USB 장치를 뽑았다 꽂으면 상태줄에 「사라졌다」·「다시 연결했다」가 보이고 재생이 이어진다. Audio MIDI 설정 등으로 MIDI 서버를 다시 띄우면(에이전트는 하지 않는다) 기본 포트와 추가 가상 포트가 다시 등록되고 상태줄에 알린다. 자동 시험으로는 일으킬 수 없다.
- [ ] (사람: 추후 점검, S3을 막지 않음) IME(SP-1 이월): 한글 입력 소스에서 Normal 모드의 글자 키(예: `v`, `i`)가 모드 바인딩으로 들어가고 입력란 밖에서 조합창이 뜨지 않는다.
- [ ] (사람: 추후 점검, S3을 막지 않음) 시간 측정 시험의 가끔 실패: `keyscore-text`의 시간 상한 시험과 `keyscore-app`의 재생 시간 측정 시험(`review_s2_t5a_r2`)은 기계가 바쁘면 실패할 수 있다. 전체 시험에서 실패하면 그 바이너리만 다시 돌려 통과를 확인한다(아래 「전체 시험 메모」). 이번 출구 실행에서 후자가 한 번 실패하고 단독 실행에서 통과했다. 받아들일지, 타이밍 시험을 직렬로 묶는 후속(HANDOFF 「남은 작업」)을 S3 전에 할지 정한다.

### 전체 시험 메모

- 명령: `cargo test --workspace --no-fail-fast > /tmp/s2exit/test-full.log 2>&1`. `--no-fail-fast`가 없으면 첫 실패 바이너리에서 멈춘다.
- 실패 찾기: `rg -n 'FAILED|panicked' /tmp/s2exit/test-full.log`.
- 단독 재실행: `cargo test -p keyscore-app --test review_s2_t5a_r2`.

## 결정 요약 (S2에서 정한 것)

- 사람이 정함
  - `2026-10-16-gpui-kit-licenses-and-advisories.md`: A, 0BSD·CC0 허용, 권고 넷 무시
  - `2026-10-17-count-and-non-key-dispatch.md`: C, 카운트는 키 디스패치에만, 팔레트·메뉴·버튼은 실행 함수가 카운트를 비움
  - `2026-10-17-key-sequence-timeout.md`: A, 시퀀스 1 s 고정
  - `2026-10-17-keymap-conflicts.md`: A13(팔레트 Esc = 통상의 취소) 결정됨. A1–A12·B24·B25는 결정 대기(S3–S6 전)
  - `2026-10-17-req-wording-approval.md`: 개발 단계 REQ 문구 수정은 사람이 모두 승인
  - `2026-10-17-vim-marks-visible-ui.md`: vim 마크는 피아노롤 우클릭 메뉴(S3 이월)
  - `2026-10-12-ksq-unknown-keys-on-value-change.md`: B, 포트·조성 값이 바뀌면 모르는 키를 지우고 undo가 되살림
  - `2026-10-12-ksq-newer-version-save.md`: A, 상위 `format_version` 파일은 읽기 전용
  - `2026-10-19-undo-history-memory-limit.md`: A, undo 상한과 알림(REQ EDT-1 문구 반영)
  - `2026-10-19-virtual-port-rename-tracks.md`: 가상 포트 이름을 바꾸면 열린 프로젝트의 트랙도 고침
  - `2026-10-21-s2-t7-save-policy-and-split.md`: T7 나누기와 저장 정책
  - `2026-10-22-s2-t7c-import-export.md`: SMF는 새 창, 텍스트는 지금 프로젝트의 재생 위치, 손실은 결과 요약
  - `2026-10-22-out8-port-mapping-sheet.md`: 열 때·복구할 때 시트, 같은 이름의 앱 가상 포트, 장치가 돌아오면 대체 포트를 지움
  - 리뷰 상한(재리뷰 3회 뒤 좁은 확인 1회): `2026-10-17-s2-t4`, `s2-t5a`, `s2-t5b`, `2026-10-18-s2-t5c`, `s2-t6a`, `2026-10-19-s2-t6d`, `2026-10-22-s2-t7b` 모두 A
- 에이전트가 REQ 문장을 근거로 정함(사람이 바꿀 수 있음)
  - `2026-10-17-mode-entry-view.md`: Lyric·Controller 모드만 피아노롤로 전환
- 결정 대기(S2를 막지 않음)
  - `2026-10-11-ai-model-defaults.md`: S7 전
  - `2026-10-12-quantize-grid-lengths.md`: S5 전
  - `2026-10-17-keymap-conflicts.md` A1–A12·B24·B25: S3–S6 전

## 알려진 한계와 이월

S2 단계 파일의 이월 줄 가운데 처리하지 않고 남긴 것이다. 출구 전에 사람이 「지금 한다」로 돌리지 않으면 아래 적힌 단계로 넘긴다.

단계 파일의 이월 줄 31개를 보고서·코드·시험 이름으로 대조했다. 26개는 S2-T5b·T5c·T5d·T6e·T6f·T7a·T7b·T7c·T7e가 처리했고(각 줄에 처리한 Task를 적음), 아래 다섯이 남는다.

- (S2-T7a 리뷰 3, 남음) FAT32 inode 재사용: 파일 정체 `FileId`는 장치·inode만 보고 `st_birthtime`은 보지 않는다. 원래 경로의 파일이 지워진 뒤 다른 경로의 새 파일이 inode를 재사용하면 옛 창으로 묶일 수 있다(코드만 보고 한 추론, FAT 디스크 이미지 재현 시험은 없음). `.bak`와 하드 링크·최근 목록 캐시는 S2-T7b가 처리했다. 제안: S8(배포·외부 볼륨 확인) 전에 `st_birthtime`을 정체에 더한다.
- (S1-T6 리뷰 4 제안 1, 부분) 가져오기 진행 표시: SMF 가져오기는 백그라운드에서 돌고 상태줄에 「가져오는 중」을 띄우지만 진행률은 없다. 텍스트 가져오기는 메인 스레드에서 돈다(지금 문서에 undo 하나로 붙이기 때문). 제안: 큰 파일에서 거슬리면 S6-T3(CC 레인 가져오기)에서 함께 다룬다.
- (S2-T7e 리뷰 1 제안 2, 남음) [복구]·[버리기]가 다른 실행에서 겹칠 때 [버리기]의 메타 읽기와 내용 열기 사이에 [복구] 쪽 첫 쓰기가 끝나면 새 저장본을 지우는 좁은 틈. 같은 사용자 계정에서 두 실행이 같은 순간 복구 시트를 다루어야 한다. 제안: 알려진 한계로 두고, S8 전에 「메타·내용 모두 잠근 뒤 지우기」로 닫는다.
- (S2-T7e 리뷰 1 제안 5, 남음) `rename_virtual_port`로 설정 `virtual_ports`에 남은(시작 때 만들지 못한) 이름으로 바꾸면 설정에 같은 이름이 둘 된다(`check_new_name`은 목적지만, `set_name`은 살아 있는 포트만 본다). 제안: S3 시작 전 짧은 Task 또는 S8 전에 `check_new_name`이 설정 목록도 보게 한다.
- (S2-T5d·S1-T2, 수동) MIDI 서버 재시작 감지와 재등록, `suggest_mapping`은 앱이 부르지 않는다(S2-T7d가 `port_mapping::plan`으로 대신). 위 「사람이 확인할 항목」의 핫플러그 줄로 확인한다.

그 밖의 알려진 한계(각 Task 보고서에 적힌 것):

- S2-T5d: 같은 이름 포트 여럿에서 쓰던 포트가 재생 중 빠지면 메시지 목록에 「없어 소리 내지 않는다」가 중간에 남는다. 잃음 뒤 같은 틱의 재연결을 한 번의 다시 준비로 줄이지 않았다.
- S2-T6d: 가상 포트 16개 상한은 프로세스 단위(S1-T1 결정). 대체 매핑 포트도 이 상한을 나눠 쓴다(`port_mapping::plan`의 `Main`).
- S2-T7b: 자동 저장 간격 30 s, 첫 변경부터. 저장본 키는 창마다 고유(pid-나노초-순번)이고 원래 경로는 메타에만 있다. 세션 잠금 파일이 없거나 잡히면 끝난 세션으로 본다.
- S2-T7c: SMF Type 0 합치기에서 같은 채널·음높이 겹침의 짝이 바뀐다(MIDI 1.0 한계). CC·벤드·프레셔 레인은 S6-T3 이월.
- 결정 대기 `2026-10-17-keymap-conflicts.md` A1–A12·B24·B25: 해당 키는 프리셋에서 비워 두었다. S3 피아노롤 키를 붙이기 전에 정해야 한다.
- 원격 CI와 CI 러너 벤치 기준값: remote가 없어 S0부터 건너뛴다.
- 이월 줄을 더한 단계 파일: S3(vim 마크 우클릭 메뉴), S6(T3 CC 레인), S8(FAT32 정체, 복구 경합 틈, `virtual_ports` 중복 이름).

## 서명

S3을 시작해도 좋다면 아래에 이름과 날짜를 적는다.

- 확인자: YoungTaek
- 날짜: 2026-10-24
