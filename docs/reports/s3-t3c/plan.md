# Task 계획: S3-T3c vim 오퍼레이터와 비주얼 선택

- 브랜치: `s3/t3c-vim-operators`
- 관련 REQ: 5.8.1(`d` `y` `c` 오퍼레이터, `D` `C`, 텍스트 객체 `dac` `yib` `vin`, `v` `V` `Ctrl+v`, 모션 `w` `b` `W` `B` `e` `E` `f` `F` `;` `,` `g g` `G` `{` `}`, 숫자 카운트 접두), 5.7(vim 카운트 `3l`), KM-5, P8, 7.4(마우스 선택은 `v` 비주얼과 같은 선택 상태)
- 선행 Task: S3-T2(커서·선택·기준 칸), S3-T3a(삭제·복사·`edit_targets`), S3-T3b(`.`의 `LastEdit`)
- 관련 결정: `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`(1 A 읽기 순서, 2 A 노트만, 3 A vim 대응, 4 A 모션 카운트 포함)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

vim 프리셋의 피아노롤 Normal에서 `d` `y` `c` 오퍼레이터가 모션·텍스트 객체·카운트와 결합하고, `v` `V` `Ctrl+v`로 비주얼 선택을 하며, 이동 키가 카운트를 받는다. 규칙은 아래가 전부이고 시험이 고정한다. 기본 프리셋 동작은 바뀌지 않는다.

### 1. 용어

- 칸: 커서 자리 (절대 틱, 음높이).
- 읽기 순서 키: 노트의 (절대 시작 틱, 음높이). 칸의 읽기 순서 자리는 그 칸 위 노트(`note_under`: 칸의 틱을 품고 음높이가 같은 노트)가 있으면 그 노트의 키, 없으면 칸 자체다. 노트 중간의 칸은 그 노트의 시작 자리로 친다.
- 현재 클립: 커서 틱을 품은 현재 트랙의 클립. 오퍼레이터는 이 클립의 노트에만 작용한다(5.3 「앞뒤 클립의 노트는 편집되지 않는다」). 커서가 클립 밖이면 오퍼레이터는 알리고 끝난다.

### 2. 오퍼레이터 대기

- `d`(`operator::Delete`), `y`(`operator::Yank`), `c`(`operator::Change`)는 피아노롤 Normal에서만 뜻이 있다. 배치 뷰에서는 「피아노롤에서 쓴다」고 알리고 대기에 들지 않는다.
- 선택이 있으면(노트가 하나 이상이거나 비주얼 중) 대기 없이 선택에 바로 작용한다(결정 3 A, 7절). 아니면 대기에 든다. 대기 중에는 창의 키 컨텍스트 경로 끝에 `Operator`가 붙어(`Normal > PianoRoll > Operator`) vim.json의 `Normal > Operator` 바인딩이 산다. 상태줄 모드 표시는 `NORMAL d`(카운트가 있으면 `NORMAL 3d`)다.
- 오퍼레이터 앞의 카운트는 대기에 담는다(`3 d`). 대기 뒤 모션의 카운트와 곱한다(`3 d 2 w` = 6). 곱은 9999에서 자른다(키맵의 카운트 상한과 같다).
- 같은 오퍼레이터를 다시 누르면(`d d`, `c c`, 대기 시간 뒤의 `y`… `y`) 편집 대상(선택, 없으면 커서 노트, S3-T3a `edit_targets`)에 작용한다. 빠르게 친 `y y`는 지금처럼 `edit::Copy`, `y y p`는 `edit::Repeat`다(키맵 그대로). 다른 오퍼레이터를 누르면 대기를 취소한다.
- 대기 중 허용되는 키 입력: 카운트 숫자, `Esc`(`operator::Cancel`), 오퍼레이터 세 개, 텍스트 객체(`i n` `i b` `a c`와 그 접두 `i` `a`), 4절의 모션, 그리고 대기가 `y`일 때만 `y`로 시작하는 `edit::Copy`·`edit::Repeat`. 그 밖의 키(바인딩이 없는 키, 멀티키 시퀀스가 위 Action이 아닌 것으로 끝나는 경우 포함)는 그 키의 동작을 하지 않고 대기만 취소한다(vim과 같다). 상태줄이 「오퍼레이터를 취소했다」를 알린다. `j` `k`(`cursor::Up/Down`)와 옥타브 이동도 결정 1 A에 따라 같은 취소다.
- 대기 중 `i` `a` 단독은 아무 일도 하지 않는다(대기 유지). Normal의 `i`(Step 모드)·`a`(커서 노트 뒤 Step)가 대기 중에 실행되지 않게 `Normal > Operator`에서 `null`로 가린다. 단독 바인딩이 없으니 GPUI는 시간 초과를 걸지 않고 다음 키를 기다린다.
- 텍스트 객체 시퀀스 대기 중(`d i` 뒤)의 `Esc`는 A13 결정(시퀀스 대기 중 `Esc`는 시퀀스만 취소)대로 시퀀스만 취소하고 오퍼레이터 대기는 남긴다. 다음 `Esc`가 오퍼레이터를 취소한다.
- 대기는 모드·뷰·트랙이 바뀌거나, 피아노롤을 클릭(왼쪽·오른쪽)하거나, 오퍼레이터가 한 번 작용하면 끝난다.
- `y`의 접두 처리(구현 세부): Normal의 `y`는 `y y`·`y y p`의 접두라 대기 시간(1 s) 동안 GPUI가 붙들고 있다. 그 사이 다른 키(`i`, `w`, 숫자)가 오면 GPUI는 `y`를 재생한 뒤 그 키를 같은 (낡은) 키 컨텍스트로 맞춰 `i`가 Step 모드로 간다. 그래서 창이 키를 가로채: 대기 입력이 `y`로 시작하고 새 키로 이어지는 바인딩이 없으면 그 키를 삼키고, 대기 입력을 비워(`y` = `operator::Yank` 실행) 다시 그린 뒤 그 키를 다시 보낸다. 대기 입력이 `y`일 때의 `Esc`는 `y`를 실행하지 않고 취소한다.

### 3. 오퍼레이터의 작용

| 오퍼레이터 | 노트가 있을 때 | 노트가 없을 때 | 커서 | undo | `.` |
| --- | --- | --- | --- | --- | --- |
| `d` 삭제 | 지운다. 선택·비주얼을 비운다 | 「지울 노트가 없다」 | 모션: 범위의 앞쪽 칸(앞으로 가는 모션은 그대로, 뒤로 가는 모션은 목적지). 객체·선택: 그대로 | 한 단위 | 같은 대상으로 다시 |
| `y` 복사 | 클립보드로(S3-T3a `copy_targets`, 내부 구조 + 시스템 SMF·텍스트). 문서는 그대로 | 「복사할 노트가 없다」 | `d`와 같다 | 없음 | 대상 아님(마지막 편집을 바꾸지 않는다) |
| `c` 변경 | 지운 뒤 Step 모드로 | 지우지 않고 Step 모드로 | 모션: 범위 앞쪽 칸의 틱. 객체·선택: 대상 가운데 가장 이른 노트 시작 틱(없으면 그대로). 음높이는 그대로 | 삭제가 한 단위 | 같은 대상으로 지우고 Step |

- `y`로 비주얼을 끝내면 선택도 비운다(vim). 비주얼이 아닌 선택(`g b` 등)은 `y` 뒤에도 남는다(`Cmd+C`와 같다).
- `D`(`edit::DeleteToBarEnd`)는 S3-T3a 그대로다. `C`(`edit::ChangeToBarEnd`)는 `D`와 같은 노트(현재 클립, 시작이 [커서 틱, 커서 마디 끝))를 지우고 커서 자리에서 Step 모드로 간다. `.`은 같은 Action을 다시 보낸다.
- 오퍼레이터의 삭제가 거부되면(문서 오류) 알리고 Step으로 가지 않는다.

### 4. 모션 (결정 1 A)

모션은 아래 Action의 처리기다. 오퍼레이터 대기가 없으면 커서를 옮기고(카운트만큼), 있으면 커서 칸 → 목적지 칸 범위에 오퍼레이터를 적용한다.

| Action (vim 키) | 목적지 한 번 | 범위 |
| --- | --- | --- |
| `cursor::Left` `Right` (`h` `l`, 화살표) | 한 스텝 뒤·앞(S3-T1) | 배타 |
| `cursor::PrevBar` `NextBar` (`Ctrl+h` `Ctrl+l`) | 마디(S3-T1 읽기) | 배타 |
| `cursor::NextNote` `PrevNote` (`w` `b`) | 읽기 순서 다음·이전 노트(S3-T2) | 배타 |
| `cursor::NextBeat` (`W`) | 커서 틱 뒤 첫 박 시작(박자표 변화 지점도 박 시작) | 배타 |
| `cursor::PrevBeat` (`B`) | 박 중간이면 그 박 시작, 박 시작이면 앞 박 시작 | 배타 |
| `cursor::NoteEnd` (`e`) | 커서 노트의 마지막 칸. 커서 노트가 없거나 이미 그 칸 이후면 읽기 순서 다음 노트의 마지막 칸. 마지막 칸 = 노트 시작 + ⌊(길이 − 1) / 스텝⌋ × 스텝 | 포함 |
| `cursor::LastNoteEndInBar` (`E`) | 커서 마디에서 시작하는 현재 클립 노트 가운데 읽기 순서 마지막 노트의 마지막 칸. 그 칸이 커서 칸보다 뒤가 아니면 다음 마디들에서 같은 것을 찾는다(현재 클립 끝까지) | 포함 |
| `cursor::NextSamePitch` `PrevSamePitch` (`f` `F`) | S3-T2 | 포함 |
| `cursor::RepeatFind` `RepeatFindReverse` (`;` `,`) | S3-T2 | 포함 |
| `cursor::NextClip` `PrevClip` (`}` `{`) | S3-T2 | 배타 |
| `cursor::ClipStart` (`g g`) | 클립 처음 | 클립 처음부터 커서 칸까지 포함 |
| `cursor::ClipEnd` (`G`) | 클립 마지막 스텝 칸 | 커서 칸부터 클립 끝까지 |
| `cursor::Up` `Down` `OctaveUp` `OctaveDown` (`k` `j` `[` `]`) | S3-T1 | 오퍼레이터와 결합하지 않음(대기 취소) |

- 범위의 두 끝은 1절의 읽기 순서 자리다. 배타: [앞 자리, 뒤 자리), 포함: [앞 자리, 뒤 자리]. 앞으로 가는 모션은 커서 칸이 앞, 뒤로 가는 모션은 목적지가 앞이다. 그래서 `d w`·`d l`은 커서 노트를 지우고 `d b`·`d h`는 커서 노트를 남긴다(vim의 exclusive와 같다).
- 읽기 순서는 음높이가 같은 틱의 둘째 열쇠다. 예: D4(960) 위에서 `d h`의 범위는 [(0, 62), (960, 62))이고 C4(0, 60)는 (0, 60) < (0, 62)라 범위 밖이다. 이것이 결정 1 A의 글자 그대로의 결과이며 시험이 기록한다.
- 카운트: 키 입력의 카운트(`take_count`)만큼 한 번 목적지를 되풀이한다. 더 갈 수 없으면 거기서 멈춘다. `g g` `G`는 카운트를 받되 쓰지 않는다. 목적지가 없으면(다음 노트 없음, `f`가 못 찾음) 커서는 그대로이고, 오퍼레이터 대기 중이면 「대상이 없다」를 알리고 대기를 끝낸다.
- 카운트는 키 입력에만 붙는다(S2-T2 결정 C). 메뉴·팔레트로 부른 모션은 한 번이다.
- 이 Task가 처리기를 새로 다는 모션: `cursor::NextBeat` `PrevBeat` `NoteEnd` `LastNoteEndInBar`(S2-T2부터 키만 있었다). 배치 뷰의 이동(`cursor::PrevClipBoundary` 등)은 카운트를 받지 않는다(S4).

### 5. 텍스트 객체 (결정 2 A)

| Action (키) | 대상 |
| --- | --- |
| `object::InnerNote` (`i n`) | 커서 노트(`note_under`) |
| `object::InnerBar` (`i b`) | 커서 마디에 걸친 현재 클립 노트(`g b`와 같은 `bar_notes`) |
| `object::AroundClip` (`a c`) | 현재 클립의 노트 전부(`g B`와 같은 집합). 클립 블록은 그대로 |

- 오퍼레이터 대기 중이면 그 노트에 오퍼레이터를 적용한다. 객체 앞의 카운트(`d 2 i n`)는 쓰지 않고 버린다.
- 비주얼 중이면 선택을 그 노트로 바꾼다(`v i n`). 비주얼은 이어지고 기준 칸은 그대로다.
- 둘 다 아니면(팔레트 실행) 「오퍼레이터(d y c)나 비주얼(v) 뒤에 쓴다」고 알린다.

### 6. 비주얼 선택 (결정 3 A)

- `v`(`select::VisualNote`), `V`(`select::VisualTime`), `Ctrl+v`(`select::VisualBlock`)는 피아노롤 Normal에서만 뜻이 있다(배치 뷰는 알림).
- 비주얼이 아닐 때 누르면 기준 칸 = 커서 칸으로 비주얼을 시작한다. 같은 종류를 다시 누르면 선택과 기준 칸을 비우고 끝낸다. 다른 종류를 누르면 종류만 바꾸고 다시 계산한다.
- 비주얼 중 모든 커서 이동(4절의 모션, `j` `k` `[` `]` 포함, 피아노롤 클릭 포함)은 기준 칸을 지우지 않고 선택을 다시 계산한다. 선택은 늘 기준 칸과 커서 칸만으로 정해진다(S3-T2의 `Shift` 넓히기와 달리 좁아질 수 있다).
  - `v`: 읽기 순서 자리가 [min, max](양끝 포함)인 노트. 두 끝은 1절의 읽기 순서 자리.
  - `V`: 시간 범위 [min 틱, max 틱 + 스텝)에 걸치는 모든 음높이의 노트(`Shift` 넓히기와 같은 걸침 기준).
  - `Ctrl+v`: 기준 칸·커서 칸 직사각형(`region_notes`, `Shift` 넓히기와 같다).
  - 클립: 기준 칸을 품은 클립, 없으면 커서 칸을 품은 클립(S3-T2 넓히기 규칙의 앞 두 단계). 둘 다 없으면 선택은 빈다(기준 칸은 남는다).
- 비주얼 중 `d` `y` `c`는 대기 없이 선택에 작용하고 비주얼을 끝낸다(3절). 다른 편집(`x`, `Alt` 조작, `q` …)은 S3-T2·T3a·T3b대로 선택에 작용하고 비주얼은 이어진다.
- 비주얼이 끝나는 때: `Esc`(`select::Clear`), 같은 비주얼 키, `d` `y` `c`, 선택을 비우는 편집(삭제·잘라내기), `g b`·`g B`·`Cmd+A`(기준 칸을 지우는 선택), `Shift` 넓히기(기준 칸은 남기고 비주얼만 끝, S3-T2 규칙으로 넘어감), Normal을 떠나는 모드 전환, 뷰 전환.
- 비주얼 중에는 창의 키 컨텍스트 경로 끝에 `Visual`이 붙는다(`Normal > PianoRoll > Selection > Visual`). vim.json `Normal > Visual`에 텍스트 객체와 `i` `a` `null`을 둔다. 상태줄 모드 표시는 `VISUAL` / `V-LINE` / `V-BLOCK`이다.
- 비주얼은 S3-T2의 기준 칸(`PianoRollState::anchor`)을 쓴다. 불변식: 비주얼이면 기준 칸이 있다(`set_anchor(None)`이 비주얼을 끝낸다).

### 7. 선택이 있을 때의 `d` `y` `c`

비주얼이 아니어도 선택에 노트가 있으면 `d` `y` `c`는 바로 그 노트에 작용한다(결정 3 A 「`g b`로 고른 선택도 같다」). 대상 규칙은 S3-T3a `edit_targets`다(선택이 현재 클립 밖이면 `d` `c`는 거부, `y`는 허용).

### 8. `.` 반복 (결정 4 A)

- `LastEdit::Operator { op, target }`을 더한다. `target`은 모션(모션 종류와 쓴 카운트), 객체, 선택(편집 대상) 셋이다. `.`은 지금 커서·선택에서 같은 대상으로 같은 오퍼레이터를 다시 한다(대기를 거치지 않는다). `c`는 Step 진입까지 한다.
- `y`는 `LastEdit`를 바꾸지 않는다. 실패한 오퍼레이터(대상 없음, 거부)도 바꾸지 않는다. 노트가 없는 `c`(빈 칸의 `c c` 등)는 Step 진입은 했으므로 마지막 편집이 된다(리뷰 1 제안 6).
- `;` `,`의 모션은 `.`에 해석된 방향(`f`/`F`)으로 남는다.

### 9. 단계 파일 원문과 다른 점

- 단계 파일의 T3c 범위에 없는 것을 더 한다: 이동 카운트(결정 4 A), 처리기가 없던 모션 넷(`W` `B` `e` `E`, 오퍼레이터의 모션이라 필요), `C`(`c` 행의 `Shift`).

## 공개 인터페이스 초안

~~~rust
// src/pianoroll/vim.rs (순수)
pub enum Operator { Delete, Yank, Change }
pub enum Motion { Left, Right, PrevBar, NextBar, NextNote, PrevNote, NextBeat, PrevBeat, NoteEnd,
                  LastNoteEndInBar, NextSamePitch, PrevSamePitch, NextClip, PrevClip, ClipStart, ClipEnd,
                  Up, Down, OctaveUp, OctaveDown }
pub enum Reach { Exclusive, Inclusive, FromClipStart, ToClipEnd, Pitch }
pub enum TextObject { InnerNote, InnerBar, AroundClip }
pub enum VisualKind { Note, Time, Block }
impl Motion { pub fn reach(self) -> Reach; }
pub fn motion_target(track: &Track, meter: &MeterMap, step: u64, from: Cursor, m: Motion) -> Option<Cursor>;
pub fn reading_key(clip: &Clip, cell: Cursor) -> (u64, u8);
pub fn motion_notes(clip: &Clip, from: Cursor, to: Cursor, reach: Reach) -> BTreeSet<Note>;
pub fn object_notes(clip: &Clip, meter: &MeterMap, cursor: Cursor, o: TextObject) -> BTreeSet<Note>;
pub fn visual_notes(clip: &Clip, anchor: Cursor, cursor: Cursor, step: u64, kind: VisualKind) -> BTreeSet<Note>;

// PianoRollState
pub fn visual(&self) -> Option<VisualKind>;
pub fn set_visual(&self, kind: Option<VisualKind>); // 기준 칸이 없으면 무시
pub fn moved(&self, delta: CursorMove, meter: &MeterMap) -> Cursor; // move_cursor의 순수 판

// ProjectWindow
pub fn pending_operator(&self) -> Option<(Operator, Option<u32>)>;
pub fn context_path(&self) -> Vec<&'static str>; // 모드 경로 + Operator/Visual

// note_ops::LastEdit
LastEdit::Operator { op: Operator, target: OperatorTarget }
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `operator_times_target_table` | UI | 게이트: `d` `y` `c` × (`w` `b` `l` `h` `e` `E` `f` `F` `W` `B` `g g` `G` `}` `i n` `i b` `a c` 겹친 오퍼레이터) 표. 남은 노트·클립보드 노트·모드·커서 |
| 2 | `reading_order_ranges_pure` | 단위 | `motion_notes` 배타·포함·화음·노트 중간 칸 |
| 3 | `motion_targets_pure` | 단위 | `W` `B`(박자표 변화 포함) `e` `E` 목적지 |
| 4 | `visual_notes_pure` | 단위 | `v` `V` `Ctrl+v` 집합 |
| 5 | `count_moves_cursor` | UI | `3 l`, `2 w`, `9 9 w`(끝에서 멈춤), `3 g g`(카운트 무시) |
| 6 | `operator_count_multiplies` | UI | `2 d w`, `d 2 w`, `2 d 2 w` |
| 7 | `pitch_motion_cancels_operator` | UI | `d j` `d k` `d ]`: 아무것도 안 지우고 커서 그대로, 대기 끝 |
| 8 | `other_key_cancels_operator_without_running` | UI | `d x`, `d u`, `d g a`, `d i x`, `d ctrl-space`: 그 동작 없음, 대기 끝, 알림 |
| 9 | `escape_cancels_operator` | UI | `d escape w`는 이동만 |
| 10 | `yank_prefix_then_object_or_motion` | UI | `y i b`, `y w`, `3 y w`가 Normal의 `i`·이동으로 새지 않는다 |
| 11 | `yank_prefix_escape_cancels` | UI | `y escape`: 대기 없음, 복사 없음 |
| 12 | `operator_with_selection_acts_now` | UI | `g b d`, `g b y`(선택 남음), `g b c` |
| 13 | `operator_in_arrangement_says_piano_roll` | UI | 배치 뷰 `d`: 대기 없음, 알림 |
| 14 | `status_shows_pending_operator_and_visual` | UI | `NORMAL d`, `NORMAL 3d`, `VISUAL` `V-LINE` `V-BLOCK` |
| 15 | `visual_modes_select_by_kind` | UI | `v w w`, `V l`, `ctrl-v k l`, 좁아짐 |
| 16 | `visual_toggle_switch_and_escape` | UI | `v v` 끝, `v shift-v` 종류 바꿈, `v escape` |
| 17 | `visual_objects_replace_selection` | UI | `v i n`, `v i b`, `v a c`, Step으로 새지 않음 |
| 18 | `visual_operators_act_and_end` | UI | `v w d`, `v w y`(선택 비움), `v w c`(Step, 커서) |
| 19 | `visual_ends_on_extend_select_and_view` | UI | `Shift` 넓히기·`g b`·`g a`가 비주얼을 끝낸다 |
| 20 | `visual_click_extends` | UI | 비주얼 중 클릭이 선택을 다시 계산 |
| 21 | `dot_repeats_operator` | UI | `d w` 뒤 `.`, `c w` 뒤 `escape .`(Step), `y w` 뒤 `.`은 그 전 편집 |
| 22 | `change_to_bar_end` | UI | `C`: 지우고 Step, `.` |
| 23 | `operator_is_one_undo_unit` | UI | `d 3 w` 한 단위, `c a c` 한 단위 |
| 24 | `click_cancels_operator` | UI | 대기 중 클릭 |
| 25 | `default_preset_unchanged` | UI | 기본 프리셋 `v`는 뷰 전환, `d`·`y`는 바인딩 없음 |
| 26 | `palette_object_without_operator_says` | UI | 팔레트 `object::InnerNote` |

## 불변식

- 문서는 `ProjectDocument::apply`(S3-T3a `apply_edit`)로만 바뀐다. 오퍼레이터 삭제는 S3-T3a `plan_delete`를 쓴다. 클립 블록은 바뀌지 않는다.
- 비주얼이면 기준 칸이 있다(`PianoRollState`가 지킨다). 오퍼레이터 대기와 비주얼은 동시에 없다(선택·비주얼이 있으면 대기에 들지 않는다).
- 키 컨텍스트 경로의 `Operator`·`Visual`은 Normal + 피아노롤일 때만 붙는다.
- 순수 계산(`pianoroll::vim`)은 GPUI와 무관하다. 모션 반복은 진행이 없으면 멈춘다(카운트 9999의 상한).

## 범위 밖

- 배치 뷰의 오퍼레이터·비주얼·이동 카운트(S4).
- 검색 모션 `n` `N`(S3-T7), 마크 점프를 모션으로(`d ' a`)는 하지 않는다(대기 취소).
- 비주얼 블록의 블록 붙여넣기 같은 vim 고유 동작, `gv`(직전 비주얼 다시).
- 다른 키(`x` 등)에 카운트(`3 x`). 이 Task의 카운트는 모션과 오퍼레이터만.
- `c`가 들어간 Step 모드의 입력(S3-T4). `.`이 `c` 뒤의 입력을 되풀이하는 것은 S3-T4 뒤.

## 리스크·결정 요청

- 결정 `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`(결정됨). 위 「끝의 정의」의 작은 규칙(겹친 오퍼레이터, 대기 중 다른 키는 동작 없이 취소, 커서 자리, `y`의 선택 처리)은 vim 관습을 따른 읽기다. 사람이 다르게 보면 고친다.
- `y` 접두 키 재전송은 GPUI의 대기 입력 처리에 기대는 구현이다. 시험 10·11이 고정한다.
- 읽기 순서의 음높이 열쇠 때문에 `d h`·`d B`가 같은 틱의 낮은 음을 남기는 결과는 결정 1 A의 글자 그대로다(시험 1이 기록).
- undo·redo는 되살린 노트를 선택한다(S3-T3a). 선택이 있으면 `d` `y` `c`는 바로 작용하므로(결정 3 A) `u` 바로 뒤의 `d w`는 모션을 기다리지 않고 그 선택을 지운다. 두 결정이 겹친 결과이며 시험 23이 기록한다. 사람이 다르게 보면 vim 프리셋에서 undo 뒤 선택을 만들지 않는 쪽을 결정 요청으로 올린다.
