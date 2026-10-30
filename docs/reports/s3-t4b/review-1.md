# 적대적 리뷰: S3-T4b vim Step과 MML 호환 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 84309e225e89c19e40c3532f21c5740970ca6010

## 1. 코드만 읽고 적은 「내가 검사할 것」

`tests/step_vim_mml.rs`를 열기 전에 `step.rs`, `step_input.rs`, `window.rs`, `settings_*.rs`, 키맵, core `splice`만 읽고 적었다.

1. 덮어쓰기 음: [커서, 커서 + 길이)에서 시작하는 노트만 지우고, 그 뒤 노트는 제자리. 범위 앞에서 시작해 걸쳐 들어오는 노트는 그대로. 한 번의 `apply`(undo 한 단위)인지.
2. 덮어쓰기 화음: `Ctrl+음`(첫 화음 음)만 범위를 지우고 다음 화음 음(`push = 0`)은 아무것도 지우지 않는지. 절반 길이 화음 음이 섞여도 범위가 첫 음 길이인지.
3. 덮어쓰기 쉼표·절반 쉼표(`Shift+Space`)·`n` 셋잇단 쉼표: 범위 노트 지움, 클립 끝을 넘으면 클립 연장, 틈에서는 편집 없음.
4. 덮어쓰기 타이: 직전 노트를 늘리고 [커서, 커서 + 길이)에서 시작하는 노트만 지우며 뒤 노트를 밀지 않는지. 늘린 노트가 클립 끝을 넘으면 클립이 늘어나는지.
5. 덮어쓰기 `Backspace`·`Shift+Backspace`: 지우기만 하고 당기지 않는지, 자취(`trail`)를 따라 한 스텝씩 돌아가는지, undo 한 번이 그 지움만 되살리는지.
6. `Shift+Backspace`(삽입): 마디 중간 → 마디 시작, 마디 시작 → 앞 마디 시작, 뒤 노트 당김, 조합·MML 대기 버림, 화음 닫은 뒤 지움, `drop_after`가 커서 앞 스텝 자취를 남겨 이어지는 `Backspace`가 맞게 돌아가는지. 지우기가 거부될 때 자취를 먼저 버리는 순서.
7. MML 대기 확정·폐기 경로: 음 키, 쉼표(`r`, `Space`), `Tab`, `Enter`, `Esc`, `Ctrl+Tab`, undo·redo, `Backspace`(버림), `Shift+Backspace`(버림), `[` `]`, `n`, 길이 숫자 아닌 키, `{` `}`(확정하지 않음?), 대기 시간 경과(만료 없음). 조합과 MML 대기가 동시에 있지 않은지.
8. MML 레이아웃 경계: 계이름형에서 MML 대기 중 레이아웃을 건반형으로 바꾸면 컨텍스트가 `Step > Piano > Mml`이 되는데 바인딩이 없다. 숫자가 `SetLength`로 가서 대기를 「틀린 값」으로 확정하는지. 팔레트로 건반형에서 `step::MmlOctave`를 부르면 같은 문제.
9. MML 값: `l16`, `l4.`, `l1` 뒤 `6`, `l3`·`l0`·`l`·`l.` 알림, 세 자리 넘는 숫자, `o` 뒤 숫자 아닌 키, `o9`, `n l8 c`에서 셋잇단 접두가 남는지와 표시.
10. 진입 처리기: `mode::Step` 등이 `step_handlers`로 옮겨진 뒤에도 메뉴·팔레트에서 Step에 들어가는지. FSM 검사(`apply(Enter(Step)) != Step`)는 `with_mode`가 늘 바꾸므로 실제로는 막는 일이 없다. 결정 1절의 「AI 패널」 예와 맞는지. 배치 뷰에서 `I` `A`가 커서를 옮기는지.
11. 진입 위치: `a`가 커서 노트 끝(음높이가 다른 노트 위에서는 그대로), `A`가 마디 시작에서 그 마디 끝, `I`가 마디 시작에서 그대로. Step 안에서 팔레트로 다시 고를 때 조합 확정과 덮어쓰기 전환.
12. 덮어쓰기 끄기: `Esc`, `Ctrl+Tab` 순환, Step 안에서 `mode::Step` 재선택, vim `c` 오퍼레이터(`apply_mode_event` 직접 호출)에서 꺼지는지.
13. `TieLegato`: 직전 스텝이 아니면 알림, 바뀔 것이 없으면 알림, 레가토 뒤 `~` 타이가 이상한 길이를 만들지 않는지(`last.end`를 그대로 두므로).
14. `OctaveOnce` `{` `}`: 조합 음만 옥타브 ±1, 0·9 경계 알림, 스티키 옥타브 그대로, 대기 다시 잼. MML 대기 중이면?
15. `PreviewFromBar`: 확정 → Normal → 마디 시작부터. 코드로만 판단(재생 시험은 돌리지 않음).
16. 설정: `SetOctaveWait` 음수·2001 무시, 0 허용, 목록 밖 값 덧붙임, `SetStepDefaultLength`가 `0`의 복귀 값을 바꾸고 열린 창의 지금 길이는 그대로.
17. vim 프리셋 키 `{` `}` `shift-space` `shift-backspace` `shift-enter`가 실제 키 입력으로 Step에서 해당 Action에 닿는지(기본 프리셋 `[` `]`·`space`·`backspace`·`enter`에 먹히지 않는지).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 덮어쓰기 음 범위·undo 한 단위 | 있음 | `vim_shift_r_overwrites_without_pushing`. 걸쳐 들어오는 노트는 시험 없음(알려진 한계로 보고) |
| 2. 덮어쓰기 화음 | 있음 | `overwrite_chord_removes_range_once`. 절반 길이 화음 음 섞임은 없음 |
| 3. 덮어쓰기 쉼표·절반·셋잇단 쉼표 | 부분 | 보통 쉼표만(`overwrite_rest_tie_and_backspace_do_not_shift`). 셋잇단 쉼표는 리뷰 시험으로 더함 |
| 4. 덮어쓰기 타이·클립 연장 | 부분 | 클립 안의 타이만. 클립 끝을 넘는 타이는 리뷰 시험으로 더함 |
| 5. 덮어쓰기 `Backspace`·`Shift+Backspace` | 부분 | `Backspace` 한 번만. 덮어쓰기 `Shift+Backspace`와 자취를 따라 여러 번 돌아가기는 없음 |
| 6. `Shift+Backspace`(삽입) | 있음 | `shift_backspace_deletes_to_bar_start_and_pulls`. 열린 화음, 자취 보존은 없음 |
| 7. MML 대기 확정·폐기 경로 | 부분 | 음 키, `Enter`, `Backspace`, `Esc`, 대기 경과만. `[`·쉼표·`Tab`·undo·`Ctrl+Tab`은 없음 |
| 8. MML과 레이아웃 경계 | 부분 | 건반형 `o` `l`이 음 키인 것만(`mml_keys_only_in_solfege_layout`). 대기 중 레이아웃 변경·건반형 팔레트 호출은 없음 |
| 9. MML 값 | 있음 | `mml_length_*` 두 개. `n l8 c`는 없음 |
| 10. 진입 처리기·FSM 검사 | 없음 | AI 패널·다른 모드에서 진입하는 시험 없음 |
| 11. 진입 위치 | 있음 | 시험 1–3 |
| 12. 덮어쓰기 끄기 | 부분 | `Esc`, `Ctrl+Tab`. Step 안 팔레트 재선택은 없음 |
| 13. `TieLegato` | 부분 | 알림 두 가지는 있음. 레가토 뒤 `~`는 없음 |
| 14. `OctaveOnce` | 있음 | `braces_shift_composing_note_octave_once` |
| 15. `PreviewFromBar` | 있음 | CoreMIDI 시험이라 이 리뷰에서는 돌리지 않고 코드로 읽음 |
| 16. 설정 | 있음 | 시험 20·21. 0 ms는 없음 |
| 17. vim 키 충돌 | 있음 | 시험 22와 `press`로 실제 키를 치는 시험들 |

## 3. 깨뜨리기 시도

새 시험 파일: `crates/keyscore-app/tests/review_s3_t4b.rs`. 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t4b` → 11개 중 6 통과, 5 실패.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 덮어쓰기 `R g h` 뒤 `Shift+Backspace`: 당기지 않음, undo 한 단위, undo가 그 지움만 되살림 | 못 깨뜨림 | `review_overwrite_shift_backspace_does_not_pull` |
| 2 | 클립 끝 직전 덮어쓰기 음 뒤 `~`로 클립 끝을 넘김 | 못 깨뜨림(클립이 마디 단위로 늘고 밀지 않음) | `review_overwrite_tie_past_clip_end_extends_clip` |
| 3 | 덮어쓰기 절반 음 + 셋잇단 쉼표 뒤 `Backspace` 두 번 | 못 깨뜨림(자취 길이 480·640을 그대로 돌아가고 당기지 않음) | `review_overwrite_backspace_follows_trail_without_pull` |
| 4 | Step 안에서 팔레트 `mode::Step`·`mode::StepOverwrite` 재선택 | 못 깨뜨림(조합 확정, 덮어쓰기 전환) | `review_palette_reentry_toggles_overwrite_and_commits` |
| 5 | 화음이 열린 채 `Shift+Backspace` | 못 깨뜨림 | `review_shift_backspace_with_open_chord_removes_chord` |
| 6 | MML 대기 중 `[`, 쉼표, `Tab`, undo, `Ctrl+Tab` | 못 깨뜨림(모두 먼저 확정) | `review_mml_wait_settles_on_every_step_action` |
| 7 | `TieLegato` 뒤 `~`: 레가토로 노트가 2880까지 늘어 커서(960)에서 끝나지 않는데 타이가 받아들여져 노트를 3840으로 늘리고 E를 밀며 커서가 노트 한가운데(1920)로 감 | 깨짐 | `review_tie_after_legato_is_refused` |
| 8 | 계이름형에서 `l 1` 대기 중 레이아웃을 건반형으로: 컨텍스트 `Step > Piano > Mml`, 상태줄 `l1_`가 남음 | 깨짐 | `review_mml_wait_does_not_survive_layout_switch` |
| 9 | 건반형에서 팔레트 `step::MmlOctave`: 바인딩 없는 `Step > Piano > Mml` 대기가 열리고, 이어 친 `5`는 「o 뒤에 옥타브 숫자」 알림과 함께 길이를 16분으로 바꿈 | 깨짐 | `review_mml_palette_in_piano_layout_does_not_open_dead_wait` |
| 10 | `n l 8 c`: 셋잇단은 적용되나(320) 대기 중 상태줄은 `l8_`로 `n`이 사라짐 | 깨짐(표시) | `review_triplet_prefix_survives_mml_length` |
| 11 | AI 패널에서 팔레트 `mode::StepAtBarEnd`: 결정 1절 예와 달리 Step으로 들어가고 커서가 3840으로 옮겨짐. `step_enter`의 FSM 검사는 `with_mode`가 늘 바꾸므로 죽은 코드 | 깨짐(결정 문서와 코드 불일치) | `review_entry_from_ai_panel_neither_moves_nor_enters` |

기존 `step_vim_mml`은 CoreMIDI 시험 하나를 빼고 돌려 21개 모두 통과했다(`-- --skip preview_from_bar`).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 찾음 | 덮어쓰기도 core `splice`(다음 클립과 겹치면 `ClipOverlap`)와 `clip_for_notes`를 쓴다 |
| 노트는 클립 안(시작 기준) | 못 찾음 | 덮어쓰기 타이가 클립 끝을 넘으면 `splice`가 클립을 늘린다(시도 2) |
| 틱 ≥ 0 | 못 찾음 | `bar_back`·`step_back_from`은 0에서 멈추고, 당기기는 `NoteBeforeClipStart`로 거부 |
| 채널 1–16, 음높이 0–127, 옥타브 0–9 | 못 찾음 | `OctaveOnce`·`MmlDigit`은 `OCTAVE_MAX` 검사, 음높이는 `pitch_of`·`Composing::pitch` |
| 확정 하나 = `apply` 하나 = undo 한 단위 | 못 찾음 | 덮어쓰기 음·쉼표·타이·`Shift+Backspace`(시도 1) |
| 조합과 MML 대기는 동시에 없다 | 못 찾음 | `step_key`·`step_mml_open`이 모두 `step_commit`(맨 앞 `step_settle_mml`)을 먼저 부른다 |
| 덮어쓰기는 Step 안에서만 참 | 못 찾음 | `apply_mode_event`가 Step을 나갈 때 끈다. `c` 오퍼레이터는 Normal에서 들어오므로 이미 거짓 |
| MML 대기는 계이름형에서만(결정 4절) | 깨짐 | 시도 8·9 |
| 타이는 직전 노트가 커서에서 끝날 때만(결정 2026-10-29 5절) | 깨짐 | 시도 7 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.1 vim `i`로 Step | 충족 | `crates/keyscore-app/src/step_input.rs:1103`(`enter_step`) |
| 5.1 계이름 MML `o5` `l8` `r` `c+` `c-` | 충족 | `assets/keymaps/default.json:297-318`, `step_input.rs:972-1051`. REQ 예의 `g2` `c2`는 키 입력 문법(REQ 2절 표 「텍스트와 키 입력의 숫자」)과 충돌해 결정 4절이 `l2 g`로 읽는다. 지적 8 |
| 5.6 기본 길이 설정 | 충족 | `settings_window.rs:182-185`, `settings_tabs.rs:558-567` |
| 5.6 옥타브 대기 300 ms 설정 가능 | 충족 | `settings_window.rs:186-195`, `settings_tabs.rs:568-577` |
| 5.8.1 `I` `a` `A` 진입 | 충족 | `step_input.rs:1084-1140` |
| 5.8.1 `R` 덮어쓰기(밀지 않고 교체) | 충족 | `step_input.rs:292-297`(음), `620-627`(쉼표), `832-836`(타이), `720-724`(지우기) |
| 5.8.1 Step 표 `Shift+Space` | 충족 | `step_input.rs:574-576` |
| 5.8.1 Step 표 `Shift+Backspace` | 충족 | `step_input.rs:673-692` |
| 5.8.1 Step 표 `Shift+Enter` | 충족(「입력을 마치고」를 Step 나가기로 읽은 결정 3절) | `step_input.rs:953-968` |
| 5.8.1 Step 표 `[` `]`의 Shift(`{` `}`) | 충족 | `step_input.rs:871-899`, `assets/keymaps/vim.json:215-216` |
| 5.8.1 Step 표 `~`의 Shift(레가토) | 충족(키는 B25 결정대로 비움, 팔레트) | `step_input.rs:901-950`. 다만 레가토 뒤 타이가 깨진다(지적 1) |
| 5.11 Step `Tab` 확정하고 다음으로 | 충족 | `step_input.rs:763-782`(MML 대기도 확정, 시도 6) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `keyscore-midi`·엔진을 건드리지 않는다. `transport.rs`는 `play_from`의 가시성만 바꿨다(`transport.rs:332`).
- 제품 코드의 `unwrap`/`expect`: 새로 더한 것 없음(`git diff main...HEAD -- 'crates/*/src/*'`에서 확인).
- 에러가 삼켜지는 곳
  - `step_mml_digit`·`SetOctaveWait`·`SetStepDefaultLength`의 범위 밖 값은 조용히 무시한다(팔레트 Action이라 결정 5절의 「무시」와 맞다).
  - `step_back_to_bar_start`는 `drop_after`로 자취를 먼저 버리고 `step_delete_span`이 거부되면 커서는 그대로인데 자취만 사라진다(`step_input.rs:686-691`). 지적 7.
  - `step_octave_once`는 MML 대기 중이면 「입력 중인 음이 없다」만 알리고 대기를 확정도 폐기도 하지 않는다(`step_input.rs:875-878`). 지적 6.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `TieLegato` 뒤 `~`가 받아들여진다. 레가토가 노트 끝을 늘렸는데 `last.end`는 그대로라 「직전 노트가 커서에서 끝날 때만」(결정 2026-10-29 5절) 검사를 통과한다. 결과로 노트를 더 늘리고 뒤 노트를 밀며 커서가 노트 한가운데로 간다. 레가토로 끝이 바뀌면 `last`를 비우거나 타이 검사가 실제 노트 끝을 보게 한다. 시험 `review_tie_after_legato_is_refused` | `crates/keyscore-app/src/step_input.rs:935-949`, `793-797` |
| 2 | 수정 | 건반형에서 팔레트 `step::MmlOctave`·`MmlLength`가 MML 대기를 연다. `Step > Piano > Mml`에는 바인딩이 없어 숫자가 길이 코드로 가고, 대기는 「틀린 값」 알림과 함께 확정된다(`5` → 「o 뒤에 옥타브 숫자」 + 길이 16분). 결정 4절은 MML을 계이름형 문법으로 정했다. `step_mml_open`에서 레이아웃을 검사하고 알린다. 시험 `review_mml_palette_in_piano_layout_does_not_open_dead_wait` | `step_input.rs:972-984` |
| 3 | 수정 | MML 대기 중 레이아웃을 건반형으로 바꾸면 대기가 `Step > Piano > Mml`에 남는다(상태줄 `l1_`). 지적 2와 같은 뿌리. 레이아웃을 읽는 `refresh_step_layout`에서 바뀌면 대기를 확정하거나 버린다. 시험 `review_mml_wait_does_not_survive_layout_switch` | `step_input.rs:128-130` |
| 4 | 수정 | `step_enter`의 FSM 검사(`apply(Enter(Step)).mode() != Step`)는 `ModeState::with_mode`가 어떤 모드에서든 Step으로 바꾸므로 늘 거짓이다(죽은 코드). 결정 1절의 「예: AI 패널」과 달리 AI 패널에서 `I`·`A` 등이 커서를 옮기고 Step으로 들어간다. 코드를 결정에 맞추든(AI 모드에서 막기) 결정 문구를 실제 FSM에 맞추든 한쪽으로 정한다. 결정을 고치는 쪽이면 시험 `review_entry_from_ai_panel_neither_moves_nor_enters`의 기대도 함께 고친다 | `step_input.rs:1064-1066`, `crates/keyscore-app/src/mode.rs:215`, `docs/decisions/2026-10-30-s3-t4b-vim-step-mml.md:19` |
| 5 | 제안 | `n` 뒤 `l`·`o`를 치면 `step_mml_open`이 `prefix`를 비워 상태줄에서 `n`이 사라지지만 셋잇단은 다음 음에 그대로 적용된다. `step_length`처럼 `n`을 표시에 남긴다(S3-T4a 리뷰 1 제안 6과 같은 이유). 시험 `review_triplet_prefix_survives_mml_length` | `step_input.rs:977` |
| 6 | 제안 | MML 대기 중 `{` `}`는 대기를 확정하지 않고 「입력 중인 음이 없다」만 알린다. 결정 4절 「다른 Step 동작이 확정한다」와 어긋난다(대기는 남아 실제 피해는 작다) | `step_input.rs:871-878` |
| 7 | 제안 | `Shift+Backspace`가 지우기 전에 자취를 버린다. 지우기가 거부되면 커서·문서는 그대로인데 자취만 사라져 다음 `Backspace`가 한 스텝 길이로 돌아간다. 지운 뒤에 버린다 | `step_input.rs:686-691` |
| 8 | 제안 | REQ 5.1의 예 `o4 l4 c c g g a a g2 …`를 키 입력으로 그대로 치면 `g2`가 G2(옥타브 2)가 된다. 결정 4절과 보고서가 이를 적었지만 REQ 문구는 그대로다. REQ 예가 텍스트 가져오기(IO-3) 예임을 밝히는 문구 수정 또는 결정 요청이 필요하다(REQ는 사람이 고친다) | `docs/REQ.md:136-139` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 사항은 없다. 수정 1–4를 고치고 해당 리뷰 시험 4개가 통과하면 다시 본다. 제안 5를 거절하면 그 시험(`review_triplet_prefix_survives_mml_length`)은 기대를 고치고 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`transport.rs`의 가시성 한 줄과 카탈로그 줄 추가는 이 Task가 쓰는 것)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
