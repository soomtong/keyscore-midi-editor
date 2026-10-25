# Task 계획: S3-T3a 삭제·클립보드·반복

- 브랜치: `s3/t3a-delete-clipboard-repeat`
- 관련 REQ: 5.8 선택·편집 표(`x` `Delete` `Backspace` `Shift+x` `Cmd+X/C/V/D` `r`), 5.8.1(vim `p` `P` `X` `D` `Y` `y y` `y y p`), 7.5(표 `Cmd+X/C/V/D`와 「클립보드 규칙」), KM-15, 5.10(입력이 클립 끝을 넘으면 마디 단위로 클립이 늘어남, 클립 밖 입력은 새 클립), 5.3(앞뒤 클립의 노트는 편집되지 않는다), 5.7(되돌리기 묶음), IO-2·IO-3(같은 변환)
- 선행 Task: S3-T2(선택·`apply_coalesced`·`seal`)
- 관련 스파이크: 없음
- 이월 받음: S3-T2 리뷰 1 제안 8(편집 뒤 선택, undo·redo 뒤 선택 되살리기)
- 나눔: 단계 파일 S3-T3 「나눔(2026-10-26)」. 이 Task는 그 가운데 T3a다.

## 끝의 정의 (내 말로)

피아노롤(`Normal > PianoRoll`)에서 노트를 지우고, 자르고, 복사하고, 붙이고, 복제·반복할 수 있다. 복사한 노트는 앱 안(모든 창)에서는 구조 그대로, 시스템 클립보드에는 SMF Type 0 바이트와 계이름 텍스트로 함께 올라간다. 다른 앱이 올린 SMF·텍스트도 `Cmd+V`로 들어온다. 편집 뒤와 undo·redo 뒤 선택이 「방금 바뀐 노트」를 가리킨다.

용어: 「대상」은 선택이 있으면 선택, 없으면 커서 위 노트 하나(`select::note_under`, S3-T2와 같다). 「현재 클립」은 현재 트랙에서 커서 틱을 품은 클립이다. 「한 스텝」은 `PianoRollState::step_ticks`(지금 960, S3-T4가 Step 길이와 묶는다).

1. 대상 고르기 규칙(모든 동작 공통)
   - 선택이 있고 선택의 (트랙, 클립)이 현재 클립이 아니면, 문서를 바꾸는 동작(삭제·잘라내기·복제·반복)은 거부하고 상태줄에 알린다(S3-T2 `Alt` 조작과 같은 규칙, 5.3). 복사(`Cmd+C`)는 문서를 바꾸지 않으므로 선택의 클립에서 그대로 복사한다(선택 → 커서 이동 → 복사의 흐름을 막지 않는다).
   - 선택은 값 집합(`BTreeSet<Note>`)이다. 클립에 같은 값의 노트가 여럿 있으면(화면에서 구별되지 않는다) 그 값의 노트 전부가 대상이다. 삭제는 모두 지우고 복사는 모두 복사한다.
   - 대상이 없으면(선택 없음 + 커서 위 노트 없음) 상태줄 한 줄, 문서·클립보드는 그대로.
2. 삭제 `edit::Delete`(`x` `delete` `backspace`): 대상 노트를 `Edit::RemoveNote`로 지운다(undo 한 단위). 뒤에 선택과 기준 칸을 비운다. 커서는 그대로.
   - vim `shift-x` `edit::DeleteBefore`: 커서 앞 노트 하나(`n`/`p`의 순서에서 커서 칸 바로 앞, `select::prev_note`)를 지운다. 선택과 무관하다. 그 노트가 현재 클립 밖(다른 클립)이면 거부한다.
   - vim `shift-d` `edit::DeleteToBarEnd`: 현재 클립에서 시작이 `[커서 틱, 커서가 든 마디의 끝)`인 노트 전부(음높이 무관)를 지운다.
   - `X`·`D`는 선택과 무관한 동작이라 선택을 비우지 않는다. 지운 노트는 문서 관찰자가 선택에서 뺀다(`retain_existing`).
3. 복사 `edit::Copy`(`cmd-c`, vim `y y`)·잘라내기 `edit::Cut`(`cmd-x`, `shift-x`): 대상 노트를 클립보드 내용(`ClipNotes`)으로 만든다. 내용은 노트 값(길이·음높이·벨로시티·피치 오프셋·가사·모르는 키)을 그대로 두고 시작만 「가장 이른 대상 노트의 시작」 기준으로 바꾼 것이다. 숨김 꼬리 노트도 전체 길이로 담는다. 잘라내기는 복사한 뒤 삭제(2)와 같다(undo 한 단위). 선택은 복사에서는 그대로, 잘라내기에서는 비운다.
   - vim `shift-y` `edit::CopyBar`: 현재 클립에서 커서가 든 마디에 걸치는 노트(`select::bar_notes`, `b`와 같은 기준)를 복사한다. 선택은 바꾸지 않는다.
   - 시스템 클립보드에 쓰기(KM-15, 7.5): 한 번에 지우고(`clearContents`) 두 형식을 함께 쓴다.
     - `public.utf8-plain-text`: 계이름 텍스트(`keyscore_text::project_to_text`). 노트를 담은 임시 프로젝트(트랙 하나: 원래 트랙의 이름·채널·프로그램·뱅크, 클립 하나: 틱 0부터 `span` 마디 단위)를 내보낸다. 템포·박자표는 원래 곡의 복사 시작 자리 값 하나씩이다. 텍스트 내보내기가 실패하거나 `Loss::Mismatch`면 텍스트는 쓰지 않는다(SMF와 내부 구조는 쓴다).
     - `public.midi-audio`: 같은 임시 프로젝트의 SMF Type 0 바이트(`keyscore_midi::smf_export(…, Type0)`).
     - 다 쓴 뒤 그 클립보드의 `changeCount`를 내부 클립보드와 함께 기억한다.
   - 앱 내부 클립보드는 앱 전역 하나(`Global`)다. 그래서 다른 창에서 붙여넣을 수 있다.
4. 붙여넣기 `edit::Paste`(`cmd-v`)·`edit::PasteBefore`(vim `shift-p`)·`edit::PasteAfter`(vim `p`)
   - 가져올 내용: 시스템 클립보드의 `changeCount`가 기억한 값과 같으면 내부 구조. 다르면(다른 앱이 바꿈, 앱을 다시 띄움) 시스템 클립보드에서 `public.midi-audio`가 있으면 SMF로(`SmfFile::read` → `smf_import`, IO-2와 같은 변환), 없으면 문자열을 계이름 텍스트로(`text_to_project(…, Lenient)`, IO-3과 같은 변환) 읽는다. 다른 앱에서 복사한 MML 조각에는 `@track` 머리가 없으므로, 그대로 읽어 실패하고 `@track`이 없으면 `@track Clip ch1` 블록 하나로 다시 읽는다. 가져온 프로젝트의 모든 트랙·클립의 노트를 절대 틱으로 모아 가장 이른 시작 기준으로 바꾼다(트랙 구분·헤더·템포는 버린다. 붙여넣기는 노트를 현재 클립에 넣는 동작이다). 읽기 실패·노트 0개면 「붙여넣을 노트가 없다」(이유 포함)이고 문서는 그대로다.
   - 자리: `Paste`와 `PasteBefore`는 내용의 틱 0이 커서 틱에, `PasteAfter`는 커서 틱 + 한 스텝에 온다(vim `p`는 커서 뒤, `P`는 커서 자리). 음높이는 바꾸지 않는다(커서 음높이로 옮기지 않는다).
   - 넣을 클립: 자리 틱을 품은 현재 트랙의 클립. 노트 끝이 클립 끝을 넘으면 클립을 마디 단위로 늘린다(S0-T7 규칙과 같은 `bar_aligned_length`). 늘린 클립이 다음 클립과 겹치면 거부한다.
   - 자리 틱이 클립 밖(틈)이면 새 클립을 만든다(5.10 「클립 밖에서 입력을 시작하면 마디 단위로 새 클립」의 읽기): 시작은 자리 틱이 든 마디의 시작, 끝은 마지막 노트 끝을 품는 마디의 끝. 다만 앞 클립이 그 마디 안 자리 틱 앞에서 끝나면 시작은 그 끝, 다음 클립이 노트 끝과 마디 끝 사이에서 시작하면 끝은 그 시작이다(리뷰 1 지적 2: 마디 중간에서 끝나는 클립 옆의 틈). 노트가 다른 클립과 겹치면 거부한다.
   - 배치 뷰에서는 클립 단위 편집(S4)이라 이 Task의 처리기는 아무것도 바꾸지 않고 「배치 뷰의 클립 편집은 아직 없다」를 알린다(삭제·복사·잘라내기·복제·반복도 같다).
   - 뒤: 선택은 붙인 노트, 기준 칸은 비운다. 커서는 그대로.
5. 복제 `edit::Duplicate`(`cmd-d`)·반복 `edit::Repeat`(`r`, vim `y y p`): 대상 노트를 같은 클립 안에 `offset`만큼 뒤로 복사한다. `offset`은 대상의 `span`(가장 늦은 끝 − 가장 이른 시작)을 한 스텝의 배수로 올린 값(최소 한 스텝)이다. 스타카토(960 칸의 900틱 노트)를 반복해도 격자에서 밀리지 않게 하기 위해서다. 클립 끝을 넘으면 4와 같이 늘리고, 겹치면 거부한다.
   - 뒤: 선택은 새로 만든 사본, 기준 칸은 비운다. 그래서 다시 누르면 사본의 뒤에 하나 더 붙는다(「누를 때마다 하나 더」). 선택 없이 커서 노트에서 시작해도 사본이 선택이 된다.
   - 되돌리기 묶음: 같은 키를 연달아 누른 복제·반복은 undo 한 단위다(5.7 「같은 조작을 연속으로 누른 것」. S3-T2의 `apply_coalesced` + `observe_keystrokes`의 `seal` 규칙). 삭제·붙여넣기·잘라내기는 누를 때마다 한 단위다(vim `x x`를 하나씩 되돌리는 관례).
   - 복제·반복은 클립보드를 바꾸지 않는다.
6. 편집 뒤와 undo·redo 뒤 선택(이월 S3-T2 리뷰 1 제안 8)
   - 편집 처리기가 스스로 정한다: 삭제·잘라내기는 비움, 붙여넣기는 붙인 노트, 복제·반복은 사본, 복사는 그대로.
   - undo·redo 뒤: 현재 트랙의 클립마다 실행 전·후 노트를 비교해 새로 나타난 노트(다중집합 차)를 구한다. 나타난 노트가 모두 한 클립에 있으면 선택을 그 노트로 바꾼다(삭제를 되돌리면 되살아난 노트, `Alt+k`를 되돌리면 원래 노트). 나타난 노트가 없으면 지금처럼 클립에 없는 노트만 뺀다(`retain_existing`, 붙여넣기를 되돌리면 비워진다). 여러 클립에 나타나면 `retain_existing`만 한다. 기준 칸은 선택을 바꾼 경우 비운다. 이 비교는 현재 트랙만 본다(다른 트랙의 변화는 화면에 없다).
   - core는 바꾸지 않는다(이월 줄의 「undo가 편집을 돌려줘야 한다」 대신 앱이 앞뒤를 비교한다. 역편집 `Op`을 공개하지 않기 위해서다).
7. 시험 격리: 시스템 클립보드는 `ClipboardTarget`(앱 전역)으로 고른다. 기본은 `NSPasteboard::generalPasteboard`, 시험은 `keyscore_app::clipboard::use_named_pasteboard(cx, 이름)`으로 프로세스마다 고유한 이름의 pasteboard를 쓴다. 시험이 사람의 클립보드를 덮지 않는다.

단계 파일 원문과 다른 점: 단계 파일의 S3-T3 범위 가운데 클립보드·삭제·반복만 맡는다(나눔). vim `p` `P` `X` `D` `Y`는 단계 파일에 없지만 같은 클립보드·삭제 경로라 함께 단다(Action은 S2-T2에 있고 처리기만 없다). `y y`는 이미 `edit::Copy`다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-core/src/transform.rs
/// 노트들을 클립에 넣는 편집. 끝이 클립을 넘으면 먼저 마디 단위로 늘린다(`ExtendClip`, 다음 클립과 겹치면 거부).
/// 노트 시작은 클립 기준이다.
pub fn insert_notes(project: &Project, track: TrackId, clip: ClipId, notes: &[Note]) -> Result<Vec<Edit>, ModelError>;
/// 틈에 노트를 넣을 새 클립 편집(`InsertClip`). 시작은 `at`이 든 마디 시작, 끝은 마지막 노트 끝을 품는 마디의 끝.
/// 노트 시작은 절대 틱이다. 다음 클립과 겹치면 거부.
pub fn clip_for_notes(project: &Project, track: TrackId, notes: &[Note], at: Tick) -> Result<Edit, ModelError>;

// crates/keyscore-app/src/clipboard.rs
pub struct ClipNotes { notes: Vec<Note>, span: u64 }          // 시작은 가장 이른 노트 기준, 정렬됨, 비지 않음
impl ClipNotes {
    pub fn new(notes: Vec<Note>) -> Option<Self>;               // 절대·상대 무관, 가장 이른 시작을 0으로
    pub fn notes(&self) -> &[Note]; pub fn span(&self) -> u64;
    pub fn shifted(&self, by: u64) -> Result<Vec<Note>, ModelError>;
    pub fn to_smf(&self, source: &Project, track: &Track, at: Tick) -> Option<Vec<u8>>;
    pub fn to_text(&self, source: &Project, track: &Track, at: Tick) -> Option<String>;
    pub fn from_smf(bytes: &[u8]) -> Result<Self, String>;
    pub fn from_text(text: &str) -> Result<Self, String>;
}
pub const TEXT_TYPE: &str = "public.utf8-plain-text";
pub const MIDI_TYPE: &str = "public.midi-audio";
pub struct SystemContents { pub change_count: isize, pub midi: Option<Vec<u8>>, pub text: Option<String> }
pub fn write_system(cx: &App, text: Option<&str>, midi: Option<&[u8]>) -> isize;   // changeCount
pub fn read_system(cx: &App) -> SystemContents;
pub fn store(cx: &mut App, notes: ClipNotes, text: Option<&str>, midi: Option<&[u8]>); // 복사: 내부 + 시스템
pub fn paste_contents(cx: &App) -> Result<ClipNotes, String>;                     // 내부 → SMF → 텍스트
pub fn use_named_pasteboard(cx: &mut App, name: &str);        // 시험용
pub fn internal(cx: &App) -> Option<ClipNotes>;                // 시험용 읽기

// crates/keyscore-app/src/pianoroll/edit.rs (순수 → Edit)
pub struct Placed { pub edits: Vec<Edit>, pub notes: Vec<Note> /* 클립 기준 시작 */, pub clip: PlacedClip }
pub enum PlacedClip { Existing(ClipId), New { start: Tick } }
pub fn plan_paste(project: &Project, track: TrackId, at: Tick, content: &ClipNotes) -> Result<Placed, ModelError>;
pub fn duplicate_offset(notes: &[Note], step: u64) -> u64;
pub fn plan_duplicate(project: &Project, track: TrackId, clip: ClipId, notes: &[Note], step: u64) -> Result<Placed, ModelError>;
pub fn plan_delete(track: TrackId, clip: ClipId, notes: &[Note]) -> Vec<Edit>;
pub fn targets(clip: &Clip, values: &BTreeSet<Note>) -> Vec<Note>;   // 값마다 클립의 같은 노트 전부
pub fn appeared(before: &Track, after: &Track) -> Option<(ClipId, BTreeSet<Note>)>; // undo·redo 뒤 선택(6)

// 되돌리기 묶음 판정: selection::is_note_op_action → is_coalescing_action(edit::Duplicate·Repeat 추가)
// 처리기(crate 안): src/note_edit.rs — edit::Delete/DeleteBefore/DeleteToBarEnd/Cut/Copy/CopyBar/Paste/PasteBefore/
// PasteAfter/Duplicate/Repeat. 되돌리기 묶음 키는 ops::COALESCE_BASE와 겹치지 않는 자리.
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `insert_notes_extends_clip_by_bars_and_rejects_overlap` | 단위(core) | 끝 안이면 `InsertNote`만, 넘으면 `ExtendClip`(마디 단위, 박자표 변화 뒤 마디) 먼저, 다음 클립과 겹치면 `ClipOverlap`, 적용하면 불변식 유지 |
| 2 | `clip_for_notes_spans_bars_and_rejects_overlap` | 단위(core) | 마디 시작·끝, 다음 클립과 겹치면 거부, 노트 시작은 새 클립 기준 |
| 3 | `insert_notes_preserves_invariants`(core) | 속성 | 임의 노트·클립 길이로 `Ok`면 적용이 성공하고 노트가 모두 클립 안 |
| 4 | `clip_notes_normalizes_to_earliest_start_and_span` | 단위 | 상대 시작, `span`, 꼬리 노트의 전체 길이, 빈 목록 `None` |
| 5 | `clip_notes_smf_and_text_round_trip` | 단위 | `to_smf`→`from_smf`, `to_text`→`from_text`가 같은 (시작, 길이, 음높이) 목록. 템포가 다른 곡도 틱은 같다 |
| 6 | `from_text_rejects_non_notation_and_empty` | 단위 | 노트 없는 텍스트·문법 오류는 `Err` |
| 7 | `plan_paste_into_clip_gap_and_extension` | 단위 | 클립 안 자리, 끝 넘김 연장, 틈이면 새 클립(마디 정렬), 다음 클립과 겹치면 `Err` |
| 8 | `plan_duplicate_offset_is_span_rounded_up_to_step` | 단위 | 스타카토 900틱 셋 → 2880, 한 스텝보다 짧은 노트 하나 → 한 스텝, 연장·겹침 거부 |
| 9 | `appeared_picks_new_notes_in_one_clip` | 단위 | 다중집합 차, 두 클립에 나타나면 `None`, 사라지기만 하면 `None` |
| 10 | `delete_x_delete_backspace_remove_targets` | UI | 선택 삭제(같은 값 둘 포함), 선택 없이 커서 노트, 대상 없음 알림, 선택이 다른 클립이면 거부, 뒤에 선택·`Selection` 컨텍스트 비움 |
| 11 | `cut_copy_paste_in_piano_roll` | UI | `cmd-c` → 커서 이동 → `cmd-v`는 커서 틱에 붙고 선택이 붙인 노트, `cmd-x`는 지우고 붙일 수 있음, `shift-x`는 `cmd-x`와 같음, 끝 넘김은 클립 연장, 틈은 새 클립, 겹침 거부 |
| 12 | `copy_puts_smf_and_text_on_system_clipboard` | UI(게이트) | 이름 붙은 pasteboard에 `public.midi-audio`(SMF Type 0, `SmfFile::read` 성공·노트 수 같음)와 `public.utf8-plain-text`(계이름 텍스트, `parse` 성공)가 둘 다 있다 |
| 13 | `paste_in_other_window_uses_internal_structure` | UI(게이트) | 창 A에서 복사, 창 B에서 붙여넣기 → 가사·피치 오프셋까지 같은 노트 |
| 14 | `paste_reads_external_smf_then_text` | UI | 다른 앱처럼 pasteboard에 SMF만 / 텍스트만 쓴 뒤 `cmd-v` → 그 노트, `changeCount`가 같으면 내부 구조가 앞선다 |
| 15 | `duplicate_and_repeat_append_copies_and_coalesce` | UI | `cmd-d`·`r` 사본 자리와 선택, `r r r` → 사본 셋 + `cmd-z` 한 번에 원복, `r` `l` `r`은 두 단위 |
| 16 | `vim_p_shift_p_shift_x_shift_d_shift_y` | UI | vim 프리셋: `p` 커서+한 스텝, `shift-p` 커서 자리, `shift-x` 커서 앞 노트 삭제, `shift-d` 마디 끝까지 삭제, `shift-y` 마디 복사, `y y p` 반복 |
| 17 | `undo_redo_restore_selection_to_changed_notes` | UI | 삭제 → `cmd-z` 뒤 선택 = 되살아난 노트, `alt-k` → `cmd-z` 뒤 선택 = 원래 노트, 붙여넣기 → `cmd-z` 뒤 선택 비움, `cmd-shift-z` 뒤 선택 = 다시 붙인 노트 |
| 18 | `arrangement_view_edit_keys_do_not_change_document` | UI | 배치 뷰에서 `x` `cmd-v` `r`은 문서를 바꾸지 않고 알린다 |
| 19 | 기존 게이트 | 기존 | Action 수 변화 없음(처리기만), 명령 표·팔레트·메뉴 시험 그대로 |

## 불변식

- 모델 불변식은 `core`가 지킨다. 앱은 `Edit`만 만들고 `ProjectDocument::apply*`로 적용한다. 실패하면 문서는 그대로다(전부 아니면 전무).
- 선택의 노트는 늘 그 (트랙, 클립)의 실제 값이거나 다음 문서 변경 때 걸러진다(S3-T2). 편집 처리기가 정한 선택도 적용 뒤 문서에서 찾은 값이다.
- 시스템 클립보드 쓰기는 메인 스레드에서 한 번에(`clearContents` 뒤 두 형식) 한다. 실시간 경로와 무관하다.
- `unsafe` 없이 `objc2-app-kit`의 안전한 메서드만 쓴다(정적 `NSPasteboardType*` 상수 대신 UTI 문자열).

## 범위 밖

- 배치 뷰의 클립 단위 삭제·복사·붙여넣기(겹치면 뒤로 밀기)·복제·반복 → S4
- 레인 포인트 잘라내기·복사 → S6
- `/` `Cmd+J` 길이 코드 `e` `q` `~` `.` → S3-T3b. `edit::RepeatLast`는 이 Task의 동작도 반복하게 S3-T3b가 만든다
- vim 오퍼레이터(`d` `y` `c` + 텍스트 객체), `edit::ChangeToBarEnd`(Step 모드 필요) → S3-T3c·S3-T4
- 붙여넣기의 커서 이동, 음높이를 커서로 옮겨 붙이기
- 큰 SMF를 클립보드에서 읽을 때의 백그라운드 처리(파일 가져오기와 달리 메인 스레드. 붙여넣기 내용은 보통 작다)

## 리스크·결정 요청

- 틈에 붙이면 새 클립을 만드는 것은 5.10의 「클립 밖에서 Step·Sequence 입력을 시작하면 새 클립」을 붙여넣기에 넓혀 읽은 것이다. 리뷰가 다르게 읽으면(거부) 계획을 고친다.
- 복제·반복의 간격을 한 스텝 배수로 올리는 것, 복제·반복만 되돌리기 묶음에 넣는 것은 REQ에 없는 세부다. 이유는 끝의 정의 5에 적었다.
- 외부 텍스트를 계이름 텍스트로 읽으므로 우연히 문법에 맞는 글(「c d e」)은 노트가 된다. 문법 오류나 노트 0개면 거부한다.
- 시스템 클립보드는 GPUI가 문자열·이미지만 쓸 수 있어 `objc2-app-kit`(이미 의존 트리에 있음, MIT/Apache-2.0)으로 직접 쓴다. 쓰고 나면 GPUI의 `read_from_clipboard`도 같은 문자열을 본다. `cargo deny`와 `scripts/check-deps.sh`(app은 대상 아님)로 확인한다.
