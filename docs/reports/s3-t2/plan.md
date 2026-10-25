# Task 계획: S3-T2 커서·선택·수식키 체계

- 브랜치: `s3/t2-cursor-selection-modifiers`
- 관련 REQ: 5.3(선택 → 동작, 이동 시 미리듣기, `Esc`), 5.7(표 전체, 선택, 노트 단위 이동, 되돌리기 묶음), 5.8(`n` `p` `Shift+n` `Shift+p` `Cmd+화살표` `[` `]` `b` `Shift+b` `Cmd+A` `:` 팔레트), 5.8.1(vim `g a` `enter` `g b` `g shift-b` `;` `,`, 마크), 5.11(`Shift+Space` 커서부터 재생은 5.8), 7.5(`Cmd+A`, `:w` `:q` `:e` `:new` `:set`), PRE-1, UI-11, KM-5, EDT-1
- 선행 Task: S3-T1(피아노롤 Element·커서·뷰포트)
- 관련 스파이크: 없음
- 이월 받음: S2-T3 A13(`operator::Cancel`, 시퀀스 대기 중 `Esc`), 키맵 결정 A2·A8·A10·A11·A12(`docs/decisions/2026-10-17-keymap-conflicts.md`), S2-T4 vim 마크 우클릭 메뉴(`2026-10-17-vim-marks-visible-ui.md` a), S3-T1 리뷰 1 제안 3(`Ctrl+h` 읽기), S2-T5a 리뷰 1 제안 4(곡 끝 동작)

## 끝의 정의 (내 말로)

피아노롤(`Normal > PianoRoll`)에서 REQ 5.7 표의 모든 칸이 키 하나로 동작하고, 선택이 모델 안의 실제 노트 집합이 되며, 커서가 노트 위로 가면 그 트랙의 포트·채널로 미리듣기가 나간다. 구체적으로:

1. 선택(`pianoroll::Selection`): (트랙, 클립, 노트 집합)을 가진다. 노트는 그 클립의 실제 `Note` 값이다. `Shift+이동`의 기준 칸은 선택이 비어도 남아야 해서(빈 칸을 지나 노트까지 넓히기) `PianoRollState::anchor`가 따로 든다(구현 중 변경).
   - `Shift+이동`(`select::ExtendLeft/Right/Up/Down`, `ExtendBarBack/Forward`, `ExtendOctaveDown/Up`): 커서를 수식키 없는 이동과 같은 규칙으로 옮기고(`ExtendBarBack`은 `Ctrl+h`와 같은 읽기: 마디 중간이면 그 마디 시작, 시작이면 앞 마디 시작. S3-T1 리뷰 1 제안 3을 여기서 고정한다), 기준 칸 ↔ 새 커서 칸이 만드는 직사각형(시간은 두 칸을 포함하는 `[min, max + 한 스텝)`, 음높이는 두 칸을 포함하는 범위)에 걸치는 현재 클립의 노트를 선택으로 삼는다. 「걸친다」는 노트 구간 `[start, end)`가 시간 범위와 겹치고 음높이가 범위 안인 것이다(7.4 러버밴드와 같은 기준). 기준 칸이 없으면(선택이 비었거나 `b`·`Shift+b`·`Cmd+A`로 만든 선택) 옮기기 전 커서 칸이 기준이 된다.
   - `b` `select::Bar`: 현재 클립에서 커서가 든 마디 `[마디 시작, 다음 마디 시작)`에 걸치는 노트. `Shift+b` `select::Clip`: 현재 클립의 모든 노트. `Cmd+A` `select::All`: 피아노롤에서는 `Clip`과 같다(배치 뷰의 클립 전체 선택은 S4). 셋 다 기준 칸을 지운다.
   - `Esc` `select::Clear`: 선택을 비운다. 선택이 비면 모드 FSM에 `ClearSelection`, 생기면 `Select`를 넣어 `Normal > PianoRoll > Selection` 컨텍스트가 실제 선택과 같이 움직인다. 뷰를 바꾸면(FSM이 선택을 비움) 실제 선택도 비운다.
   - 문서가 바뀌면(undo·redo·다른 편집) 선택에서 클립에 더 없는 노트를 뺀다. 비면 `ClearSelection`.
   - 장면은 선택의 클립이 현재 클립일 때만 그 노트를 `selected`로 그린다.
2. `Alt` 조작(`note::NudgeEarlier/Later`, `TransposeDown/Up`, `Alt+Ctrl` `NudgeBarEarlier/Later`)과 `Alt+Shift` 조작(`note::Shorten/Lengthen`, `OctaveDown/Up`): 대상은 선택이고, 선택이 비었으면 커서 위의 노트 하나(커서 틱을 품고 커서 음높이인 현재 클립의 노트. 여럿이면 가장 이른 것)다. 한 스텝은 `PianoRollState::step_ticks`(지금 4분음표, S3-T4가 Step 길이와 묶는다), 한 마디는 박자표 맵의 그 자리 마디 길이(절대 틱 기준으로 「시작이 든 마디의 길이」)다.
   - 계산은 `keyscore_core::transform::map_notes`(이 Task가 `plan`을 공개한 것)로 한다. 그래서 S0-T7 규칙을 그대로 따른다: 클립 끝을 넘으면 마디 단위로 클립을 늘리고(다음 클립과 겹치면 거부), 클립 시작 앞으로 나가면 거부, 꼬리 노트는 시작만 클립 안이면 된다. 음높이가 0–127을 벗어나면 거부한다(일괄 연산의 옥타브 접기는 쓰지 않는다. 반음 올림이 열한 반음 내림으로 보이면 안 되기 때문이다). 줄이기는 길이가 한 스텝 이하인 노트는 그대로 둔다. 거부·변화 없음은 상태줄 한 줄이고 문서는 바뀌지 않는다(전부 아니면 전무).
   - 적용 뒤 선택은 바뀐 노트들로 갱신된다. 선택 없이 커서 노트에 적용했으면 커서가 그 노트를 따라간다(틱·음높이). 연타가 같은 노트에 계속 적용되게 하기 위해서다.
   - 되돌리기 묶음(5.7): 같은 조작의 연속은 undo 한 단위다. `ProjectDocument::apply_coalesced(edits, key)`가 `core::Document::apply(…, Some(CoalesceKey))`를 쓰고, 조작마다 다른 키를 준다. 「다른 키가 끼면 끊긴다」는 창이 `observe_keystrokes`로 모든 키 입력을 보고, 그 키가 같은 조작의 Action이 아니면(커서 이동, 선택, 바인딩 없는 키, 모두) 문서를 `seal`하는 것으로 한다. 마우스 클릭도 `seal`한다. 메뉴·팔레트로 같은 조작을 실행한 것은 키 입력이 아니라 묶임을 끊지 않는다(팔레트에 친 글자는 키 입력이라 끊는다).
3. 노트·클립 단위 이동: `n`/`p` `cursor::NextNote/PrevNote`는 현재 트랙의 모든 클립 노트를 (절대 시작, 음높이) 순으로 놓고 커서 칸 다음·이전 노트로 커서를 옮긴다(화음도 한 음씩 지난다). `Shift+n`/`Shift+p` `cursor::NextClip/PrevClip`은 커서 틱 뒤·앞에서 시작하는 클립의 시작으로(음높이 유지). `Cmd+←`/`Cmd+→` `cursor::ClipStart/ClipEnd`는 현재 클립의 시작 / 마지막 스텝 칸(`start + floor((len − 1) / step) × step`). `Cmd+↑`/`Cmd+↓`는 S2-T5b의 `track_first/last`가 이미 처리한다(커서 틱 유지). `[` `]`는 S3-T1. 없으면(다음 노트·클립이 없음) 커서는 그대로다.
   - vim `f`/`shift-f` `cursor::NextSamePitch/PrevSamePitch`(커서 음높이의 다음·이전 노트)와 A11의 `;` `cursor::RepeatFind`(마지막 `f`/`F` 방향 반복), `,` `cursor::RepeatFindReverse`(반대 방향). `f`를 쓴 적이 없으면 아무 일도 없다.
4. 미리듣기(PRE-1): 커서가 옮겨진 뒤(키·클릭·`n`·`p`·마크 점프, 모두) 커서 위에 노트가 있고 설정 「이동 시 미리듣기」가 켜져 있으면 그 노트의 음높이·벨로시티를 현재 트랙의 포트·채널로 `Scheduler::preview`(길이는 설정, 기본 200 ms)한다. 경로는 S2-T5c의 테스트 노트(`inspector::midi_send_test_note`)와 같은 규칙을 함수로 묶어 공유한다: 이 창이 재생 중이면 그 스냅샷의 포트, 다른 창이 재생 중이면 기본 포트만, 정지 중이면 이 프로젝트 스냅샷을 넣어 포트를 연다. 실패는 미리듣기에서는 조용히 넘긴다(이동마다 상태줄을 덮지 않기 위해). `note::Audition`(vim `enter`, 팔레트)은 설정과 무관하게 커서 노트를 들려주고 실패를 알린다.
   - 설정 `settings.json`에 `preview` 객체 `{ "on_move": true, "length_ms": 200 }`. 설정 > 입력에 「이동 시 미리듣기」 토글과 「미리듣기 길이」 선택 상자. Action `settings::TogglePreviewOnMove`, `settings::SetPreviewLength(ms)`(팔레트 변형 100·200·300·500).
5. vim 프리셋(결정 A2·A8·A10·A11·A12, `// 결정 대기` 주석 제거): `g a`=`view::Toggle`, `Normal > PianoRoll`의 `enter`=`note::Audition`(배치 뷰 `enter`=`clip::Open` 상속), `g b`=`select::Bar`, `g shift-b`=`select::Clip`, `;`=`cursor::RepeatFind`, `,`=`cursor::RepeatFindReverse`. `Normal > Operator`의 `escape`=`operator::Cancel`(신규: 카운트를 비운다. 오퍼레이터 대기 상태는 S3-T3가 만든다).
6. 시퀀스 대기 중 `Esc`(A13): `g` 뒤 `escape`는 시퀀스만 취소한다. GPUI 기본은 접두를 재생한 뒤 `escape`를 디스패치하므로(선택이 있으면 `select::Clear`가 실행된다), 키맵 엔진의 `intercept_keystrokes`에서 「대기 중 + 수식키 없는 `escape`」면 전파를 멈추고, 대기를 비우기 위해 바인딩이 없는 키 하나(`f20`)를 `Window::defer`로 디스패치한다(GPUI의 시퀀스 시간 초과와 같은 결과: 접두 단독 바인딩이 있으면 그것이 실행되고 대기와 카운트가 비워진다). 프로젝트 창에서만 한다.
7. 명령줄(A12): `:`는 팔레트를 `:` 접두가 채워진 채 연다(`palette::OpenCommandLine`, 신규. `cmd-shift-p`는 그대로 `palette::Open`). 질의가 `:`로 시작하면 ex 명령으로 읽는다(`commands::ex::parse`): `:w`=`file::Save`, `:w 이름`=`file::SaveAs`(그 이름을 제안), `:q`=`window::Close`, `:e`=`file::Open`, `:e 경로`=그 파일 열기, `:new`=`file::New`, `:set`=`settings::Open`, `:set 구역`=그 구역. 팔레트는 ex 줄 하나(실행 가능)만 보이고 `Enter`로 실행한다. 모르는 명령·인자는 실행 불가 줄로 이유를 보인다. `:ai`(S7), `:test`(S8)는 모르는 명령이다.
8. vim 마크(UI-11): `cursor::SetMark(a–z)`는 (트랙, 틱, 음높이)를 창에 기억하고 `cursor::JumpMark`는 그 트랙·칸으로 커서를 옮긴다(트랙이 없어지면 알림). 피아노롤 우클릭 메뉴 「마크 설정 ▸ a–z」「마크로 이동 ▸ a–z」(`PopupMenu::submenu`). 배치 뷰의 우클릭 메뉴는 배치 뷰 Element가 생기는 S4-T1에서 같은 메뉴를 단다.
9. `Shift+Space` `transport::PlayFromCursor`는 피아노롤 뷰에서 피아노롤 커서 틱부터 재생한다(S2-T5a가 「커서가 생기기 전까지 처음부터」로 둔 자리). 배치 뷰 커서는 S4.
10. REQ 문구(결정 문서 「후속」, 개발 단계 수정 승인): 5.8 `:` 행의 「별도의 명령줄은 없다」를 「`:` 접두는 ex 명령」으로, 5.8.1 `g` 행에 `gb` `gB`, `;` 행에 `f` 반복 정의, 충돌 메모의 `gv`를 `ga`로.

단계 파일 원문과 다른 점: 「`Esc` 선택 해제」는 S2-T3가 이미 바인딩했고 여기서 실제 선택을 비운다. 미리듣기 설정 UI(설정 > 입력)와 `palette::OpenCommandLine`은 단계 파일에 없지만 REQ 5.3·UI-11·A12를 채우려면 필요해 넣었다. `Cmd+↑/↓`는 S2-T5b 처리기를 그대로 쓴다(추가 구현 없음, 시험만). 곡 끝 동작은 REQ에 없어 결정 요청(`docs/decisions/2026-10-25-playback-end-of-song.md`)으로 넘기고 이 Task는 구현하지 않는다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-core/src/transform.rs
/// 고른 노트마다 `f`가 준 노트로 바꾸는 편집 묶음(클립마다 `ReplaceNotes` 하나). 클립 늘리기·거부 규칙은 quantize 등과 같다.
pub fn map_notes(project: &Project, notes: &[NoteRef],
                 f: impl FnMut(&NoteRef, &Clip, &Note) -> Result<Note, ModelError>) -> Result<Vec<Edit>, ModelError>;
// crates/keyscore-core/src/note.rs: 앱이 노트 변형을 만들 수 있게 공개(규칙 변경 없음, 각각 자기 검사를 한다)
impl Note { pub fn with_start(&self, Tick) -> Result<Note>; pub fn with_pitch(&self, Pitch) -> Result<Note>; pub fn with_length(&self, u32) -> Result<Note>; }

// crates/keyscore-app/src/document.rs
impl ProjectDocument {
    pub fn apply_coalesced(&mut self, edits: Vec<Edit>, key: u64) -> Result<(), ModelError>; // 같은 키 연속은 undo 한 단위
    pub fn seal(&mut self);                                                                    // 다음 apply를 새 단위로
}

// crates/keyscore-app/src/pianoroll/mod.rs
pub struct Selection { track: Option<TrackId>, clip: Option<ClipId>, notes: BTreeSet<Note> }
impl Selection {
    pub fn is_empty(&self) -> bool; pub fn len(&self) -> usize;
    pub fn clip(&self) -> Option<ClipId>; pub fn track(&self) -> Option<TrackId>;
    pub fn contains(&self, clip: ClipId, note: &Note) -> bool; pub fn notes(&self) -> impl Iterator<Item = &Note>;
    pub fn set(&mut self, track: TrackId, clip: ClipId, notes: BTreeSet<Note>);   // 비면 clear
    pub fn clear(&mut self);
    pub fn retain_existing(&mut self, clip: Option<&Clip>) -> bool; // 클립에 없는 노트를 뺀다. 바뀌었으면 참
}
pub struct Mark { pub track: TrackId, pub cursor: Cursor }
pub enum FindDirection { Forward, Backward }
impl PianoRollState {
    pub fn selection(&self) -> Selection; pub fn set_selection(&self, s: Selection);
    pub fn anchor(&self) -> Option<Cursor>; pub fn set_anchor(&self, Option<Cursor>);   // Shift+이동 기준 칸
    pub fn set_mark(&self, name: char, mark: Mark); pub fn mark(&self, name: char) -> Option<Mark>;
    pub fn last_find(&self) -> Option<FindDirection>; pub fn set_last_find(&self, d: FindDirection);
}

// crates/keyscore-app/src/pianoroll/select.rs (순수)
pub fn region_notes(clip: &Clip, a: Cursor, b: Cursor, step: u64) -> BTreeSet<Note>;        // 커서는 절대 틱
pub fn bar_notes(clip: &Clip, meter: &MeterMap, at: Tick) -> BTreeSet<Note>;
pub fn note_under(clip: &Clip, cursor: Cursor) -> Option<Note>;
pub fn next_note(track: &Track, cursor: Cursor) -> Option<(ClipId, Cursor)>;  pub fn prev_note(..) -> ..;
pub fn next_same_pitch(track: &Track, cursor: Cursor) -> Option<Cursor>;      pub fn prev_same_pitch(..) -> ..;
pub fn next_clip_start(track: &Track, tick: Tick) -> Option<Tick>;            pub fn prev_clip_start(..) -> ..;
pub fn clip_last_step(clip: &Clip, step: u64) -> Tick;

// crates/keyscore-app/src/pianoroll/ops.rs (순수 → Edit)
pub enum NoteOp { NudgeEarlier, NudgeLater, NudgeBarEarlier, NudgeBarLater, TransposeUp, TransposeDown,
                  OctaveUp, OctaveDown, Lengthen, Shorten }
impl NoteOp { pub const ALL: [NoteOp; 10]; pub fn coalesce_key(self) -> u64; pub fn label(self) -> &'static str;
              pub fn moves_time(self) -> bool; pub fn moves_pitch(self) -> bool; }
pub struct Planned { pub edits: Vec<Edit>, pub moved: Vec<(Note, Note)> }
pub fn plan_note_op(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], op: NoteOp,
                    step: u64) -> Result<Option<Planned>, OpError>;   // Ok(None) = 바뀌는 노트 없음

// crates/keyscore-app/src/commands/ex.rs (순수)
pub enum ExCommand { Write(Option<String>), Quit, Edit(Option<String>), New, Set(Option<SettingsSection>) }
pub fn parse(query: &str) -> Result<ExCommand, ExError>;   // ":w name" 등. 접두 ':' 포함 문자열
pub fn is_command_line(query: &str) -> bool;               // 앞 공백 뒤 ':'
impl ExCommand { pub fn label(&self) -> String; }           // 팔레트 줄에 보이는 설명
// command_palette: PaletteEvent::Ex(ExCommand), CommandPalette::set_query_text(text, window, cx)
// preview.rs(crate 안): ProjectWindow::sound_note(track, pitch, velocity, length, cx) -> NoteSend — PRE-1·PRE-3 공통 경로
// selection.rs(crate 안): 처리기. ProjectWindow::open_popup_with(kind, at, window, cx, build(menu, window, cx)) 하위 메뉴용

// Action(신규 4개, 301개): operator::Cancel, palette::OpenCommandLine, settings::TogglePreviewOnMove, settings::SetPreviewLength(Int)
// 설정: settings.preview { on_move: bool, length_ms: u32(1..=10000) }

// ProjectWindow
pub fn selection(&self) -> Selection;             // 시험용 읽기
pub fn context_menu_open(&self) -> bool;          // 우클릭 메뉴가 떠 있는가
// PianoRollElement
pub fn on_right_click(self, handler: impl Fn(Cursor, Point<Pixels>, &mut Window, &mut App) + 'static) -> Self;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `region_notes_picks_notes_overlapping_cells_between_anchor_and_cursor` | 단위 | 기준↔커서 직사각형(시간 `[min, max+step)`, 음높이 포함 범위)에 걸치는 노트. 긴 노트는 겹침으로 든다, 범위 밖 음높이는 빠진다 |
| 2 | `next_prev_note_walk_track_notes_in_start_pitch_order` | 단위 | 화음은 음높이 순으로 한 음씩, 클립 경계를 넘어 다음 클립 노트로, 끝에서 `None` |
| 3 | `bar_notes_and_clip_last_step` | 단위 | 마디 걸침, 박자표 변화 뒤 마디, 길이가 스텝 배수가 아닌 클립의 마지막 칸 |
| 4 | `plan_note_op_rules` | 단위 | 밀기·조옮김·옥타브·늘림·줄임의 `ReplaceNotes`. 클립 끝 넘김은 마디 단위 연장, 다음 클립과 겹치면 `Err`, 시작 앞은 `Err`, 음높이 범위 밖은 `Err`, 한 스텝 이하 줄임은 `Ok(None)` |
| 5 | `ex_parse_maps_w_q_e_new_set_and_rejects_unknown` | 단위 | `:w` `:w a.ksq` `:q` `:e` `:e p` `:new` `:set` `:set keymap` `:wq`(모름) `:x`(모름) 공백 처리 |
| 6 | `modifier_table_5_7_every_cell` | UI(테이블) | 5.7 표 7행 × 4열 × {hjkl, 화살표}: 키 → 커서·선택·노트 변화 기대값. `Alt+Ctrl+j/k`는 (예약)이라 아무 변화 없음 확인 |
| 7 | `alt_operations_without_selection_apply_to_cursor_note` | UI | 선택 없이 `alt-k`·`alt-shift-l`가 커서 노트 하나만 바꾸고 커서가 따라간다(게이트) |
| 8 | `repeated_same_operation_is_one_undo_and_other_key_breaks_group` | UI | `alt-shift-l` ×5 → `cmd-z` 한 번에 원복. `alt-shift-l`, `l`, `alt-shift-l` → undo 2회 필요. `alt-shift-l`, `alt-shift-h`는 다른 묶음(리뷰 초점) |
| 9 | `n_p_shift_n_shift_p_cmd_arrows_move_cursor` | UI | 다음·이전 노트, 다음·이전 클립, 클립 처음·끝, 첫·마지막 트랙(틱 유지) |
| 10 | `b_shift_b_cmd_a_select_and_escape_clears` | UI | 선택 집합과 컨텍스트 경로(`Selection`), `escape` 한 번에 비움, 두 번째 `escape`는 no-op, `v`로 뷰를 바꾸면 비움 |
| 11 | `cursor_move_onto_note_sends_preview_on_track_port_channel` | UI + CoreMIDI 루프백 | 커서가 노트 위로 가면 트랙 채널로 Note On/Off(기본 200 ms), 빈 칸은 없음, 설정 끄면 없음, `note::Audition`은 설정 무관 |
| 12 | `vim_preset_keys_from_decisions_a2_a8_a10_a11` | UI | vim: `g a` 뷰 전환, 피아노롤 `enter` 미리듣기(모드 그대로), 배치 뷰 `enter` 클립 열기, `g b`·`g shift-b`, `f`·`shift-f`·`;`·`,`, `// 결정 대기` 주석이 vim.json에 없음 |
| 13 | `escape_during_pending_sequence_only_cancels_sequence` | UI | vim, 선택 있음: `g` `escape` → 선택 유지·대기 없음·카운트 없음. 이어 `g g`는 시퀀스로 동작(클립 처음). 기본 프리셋 `g`(바인딩 없음)에는 영향 없음 |
| 14 | `operator_cancel_clears_count` | UI | vim `3` 뒤 `operator::Cancel` 실행 → `pending_count` 없음 |
| 15 | `colon_opens_palette_with_ex_prefix_and_runs_commands` | UI | `:` → 팔레트 질의 `:`, `set` + `enter` → 설정 창, `new` → 새 창, 모르는 명령은 실행 불가 줄, `cmd-shift-p`는 빈 질의 |
| 16 | `marks_set_and_jump_and_context_menu_opens` | UI | `cursor::SetMark(a)` 뒤 이동·트랙 변경 → `JumpMark(a)`로 복귀, 없는 마크는 알림, 피아노롤 우클릭이 팝업을 띄움 |
| 17 | `selection_follows_document_changes` | UI | 밀기 뒤 선택 = 새 노트, `cmd-z` 뒤 선택은 클립에 있는 노트만(없어지면 비움·컨텍스트 복귀) |
| 18 | `play_from_cursor_starts_at_piano_roll_cursor` | UI + CoreMIDI | 커서를 2마디로 두고 `shift-space` → 재생 위치 ≥ 2마디 |
| 19 | `preview_settings_round_trip_and_defaults` | 단위 | `preview` 객체 읽기·쓰기, 범위 밖·잘못된 값은 기본값 + 경고, 모르는 키 보존 |
| 20 | `scene_marks_selected_only_in_selected_clip` | 단위(scene) | 선택 노트는 현재 클립일 때만 `selected`, 이웃 클립이 현재면 아니다 |
| 21 | `action_counts_match_palette_and_menus`(기존) | 기존 게이트 | 새 Action 4개가 표·팔레트·`ALL_ACTION_NAMES`에 모두 있다 |
| 22 | `map_notes_matches_shift_and_transpose`(core) | 속성 | `map_notes`로 만든 이동이 `shift`와 같은 편집을 낸다(proptest, 공개 래퍼가 규칙을 바꾸지 않음) |

## 불변식

- 모델 불변식은 `core`가 지킨다. 앱은 `Edit`만 만들고 `ProjectDocument::apply*`로 적용한다. 실패하면 문서는 그대로다.
- 선택의 노트는 늘 그 (트랙, 클립)의 실제 `Note` 값이거나 다음 문서 변경 때 걸러진다. 선택이 비어 있으면 모드 FSM도 선택 없음이다(`sync_selection_mode`가 한 곳에서 맞춘다).
- 커서 틱 ≥ 0, 음높이 0–127(S3-T1). 조작은 `Pitch::new` 실패를 거부로 바꾼다.
- 실시간 경로: 미리듣기는 `Scheduler::preview`(명령 큐)만 쓴다. 스케줄러 스레드 코드는 건드리지 않는다.
- 되돌리기 묶음은 `core::History`의 `CoalesceKey` 규칙(같은 키 연속만 묶임, `seal`로 끊음)에 기댄다. 앱은 키와 `seal` 시점만 정한다.

## 범위 밖

- 편집 동작(`x` `Delete` 복사·붙여넣기 `r` `/` `Cmd+J` 길이 코드 `e` `q` `~` `.`)과 `edit::*` 처리기 → S3-T3
- `select::VisualNote/VisualTime/VisualBlock`(vim `v` `V` `Ctrl+v`) 처리기 → S3-T3(선택 모드는 편집 오퍼레이터와 함께)
- `Alt+Ctrl+j/k`(5.7 「예약」): 바인딩 없음. 5.11 `Tab`의 「마지막으로 편집한 음높이」 → S3-T4(편집이 생기면)
- 배치 뷰의 선택·커서·마크 메뉴·`Cmd+A` → S4-T1. `select::All`은 배치 뷰에서 아무 일도 하지 않는다
- 마우스 러버밴드·`Shift+클릭`·`Cmd+클릭` → S3-T6(같은 `Selection`을 쓴다)
- 곡 끝 동작·루프 구간 → 결정 요청(`2026-10-25-playback-end-of-song.md`), 구현은 결정 뒤
- `:ai` `:test` ex 명령 → S7·S8. `:w 이름`은 그 이름을 제안하는 「다른 이름으로 저장」 시트다(바로 쓰지 않는다)
- `transport::LoopSelection`(`Shift+Enter`) → S5(루프 구간)

## 리스크·결정 요청

- 5.7 「선택 넓히기」의 정확한 기하(직사각형, 겹침 기준)는 REQ에 없는 세부다. 7.4 러버밴드(「시간×음높이 사각형 안의 노트」)와 같은 기준으로 읽었다. 리뷰어가 다르게 읽으면 계획을 고친다.
- 선택 없이 커서 노트에 적용할 때 커서가 노트를 따라가는 것은 REQ에 없는 세부다. 따라가지 않으면 `Alt+k` 연타가 두 번째부터 노트를 잃어 5.7 「되돌리기 묶음(연타)」이 성립하지 않는다.
- `Esc` 시퀀스 취소는 GPUI 내부(`clear_pending_keystrokes`가 `pub(crate)`)를 쓸 수 없어 바인딩 없는 키 디스패치로 비운다. GPUI 스냅샷이 바뀌면 다시 본다.
- 곡 끝 동작: 결정 요청 `docs/decisions/2026-10-25-playback-end-of-song.md`. 이 Task는 그 결정에 의존하지 않는다.
- `core`에 공개 함수 하나(`transform::map_notes`)를 더한다. 규칙을 바꾸지 않는 래퍼이고 시험 22가 `shift`와 같음을 확인한다. `scripts/check-deps.sh` 허용 목록은 바뀌지 않는다(의존성 추가 없음).
