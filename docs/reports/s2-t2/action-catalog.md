# KeyScore Action 카탈로그 (S2-T2)

출처는 `docs/REQ.md`의 5.1–5.15(입력 모드와 키 표, 특히 5.6·5.7·5.8·5.8.1), 6.2·6.3·6.7·6.8, 7.2·7.3(키맵), 7.4(마우스), 7.5(macOS 표준 단축키), 7.6(기능 ↔ UI 대응표와 7개 메뉴)이다. 이름은 `namespace::Name`(namespace는 소문자, Name은 PascalCase)이고, 인자가 있으면 `Name(arg)`로 쓴다. 7.3의 JSON 예에 나온 이름(`cursor::Left`, `note::TransposeUp`, `mode::Step`, `view::Toggle`, `edit::Repeat`, `note::SetLength(sixteenth)`, `transport::Toggle`, `clip::Open`, `step::Rest`, `keymap::CancelCapture` 등)은 그대로 쓰고, 나머지는 같은 문체로 새로 붙였다.

표기 규칙은 다음과 같다.

- 키는 GPUI keystroke 문법이다(`cmd-s`, `shift-left`, `g g`는 시퀀스). 한 칸에 바인딩이 여러 개면 `; `로 그룹을 나누고, 그룹은 `키, 키 (컨텍스트)` 형식이다. 기호 키(`+` `>` `?` `:` `~` `{` `}`)는 REQ 표기 그대로 적었다. 백틱 키는 `grave`로 적었다.
- 컨텍스트는 GPUI 컨텍스트 문법이다. `Normal`은 피아노롤·배치 뷰 공통, `Normal > PianoRoll`·`Normal > Arrangement`는 뷰 한정이다. `Step > Piano`·`Step > Solfege`(키 레이아웃 (a)/(b)), `Step > Drum`, `Step > Pending`(음 키 뒤 옥타브 대기), `Step > Ghost`, `Sequence > PrePitched`·`Sequence > Loop`, `AI > Preview`, `Normal > Selection`, `Dragging`, `TrackList`, `Lane`, `CommandLine`, `Palette`, `Overlay`, `Dialog`은 REQ의 표현을 옮긴 가칭이며 구현 때 확정한다.
- macOS 표준 단축키(7.5)는 컨텍스트를 `(전역, 프리셋 불변)`으로 표시했다(KM-12).
- vim 열은 5.8.1이 추가하거나 덮어쓰는 키만 적는다. `-`는 vim 전용 키가 없다는 뜻이며 기본 프리셋 키를 그대로 상속한다(5.8.1 첫 문단 「5.8의 기본 프리셋 위에 별칭을 얹는다」). vim에서 기본 키가 덮이는 곳은 「키 충돌·모호한 곳」에 모았다.
- 같은 Action이 피아노롤과 배치 뷰에서 대상만 달리 적용되는 편집 키(`edit::Delete` `Cut` `Copy` `Paste` `Duplicate` `Repeat` `Split` `Join`)는 뷰별로 Action을 나누지 않고 하나로 두었다(5.10 「Normal 모드 공통 체계를 클립 단위로 적용」). 반면 이동·밀기·길이 조절처럼 뷰마다 뜻이 다른 키는 7.3 예(`note::Lengthen` / `clip::Lengthen`)에 따라 Action을 나눴다.
- 마우스 조작(7.4)은 같은 Action을 호출하므로(7.4 첫 문단) 별도 Action을 만들지 않았고, 드래그 취소만 `edit::CancelDrag`로 적었다.

## app

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `app::Quit` | 앱 종료(저장 안 된 변경은 macOS 시트로 확인) | - | `cmd-q` (전역, 프리셋 불변) | `:q` (명령줄/팔레트) | - (7개 메뉴에 항목 없음) | 7.5 |

## file

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `file::New` | 새 프로젝트 | - | `cmd-n` (전역, 프리셋 불변) | `:new` | 파일 > 새로 만들기 | 7.5, 7.6 |
| `file::Open` | 프로젝트 열기 | - | `cmd-o` (전역, 프리셋 불변) | `:e` | 파일 > 열기 | 7.5, 7.6 |
| `file::OpenRecent(path)` | 최근 파일 열기 | path | - | - | 파일 > 최근 파일 | 7.6 메뉴바 구성 |
| `file::Save` | 저장 | - | `cmd-s` (전역, 프리셋 불변) | `:w` | 파일 > 저장 | 7.5, 7.6, IO-1, IO-5 |
| `file::SaveAs` | 다른 이름으로 저장 | - | `cmd-shift-s` (전역, 프리셋 불변) | `:w name` | 파일 > 다른 이름 | 7.5, 7.6 |
| `file::ImportSmf` | SMF Type 0/1 가져오기 | - | - | - | 파일 > SMF 가져오기 | IO-2, 7.6 |
| `file::ExportSmf` | SMF 내보내기(클립 펼침, 7비트) | - | - | - | 파일 > 내보내기 (SMF) | IO-2, 7.6 |
| `file::ImportText` | 계이름 텍스트(MML) 가져오기 | - | - | - | 파일 > 계이름 텍스트 가져오기 | IO-3, 5.14, 7.6 |
| `file::ExportText` | 계이름 텍스트 내보내기 | - | - | - | 파일 > 계이름 텍스트 내보내기 | IO-3, 5.14, 7.6 |

## edit

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `edit::Undo` | 되돌리기(텍스트 필드 안에서는 필드 undo, KM-14) | - | `cmd-z` (전역, 프리셋 불변) | `u` (Normal) | 편집 > 되돌리기 | 7.5, 5.8.1, EDT-1, KM-14 |
| `edit::Redo` | 다시 실행 | - | `cmd-shift-z` (전역, 프리셋 불변) | `shift-u`, `ctrl-r` (Normal) | 편집 > 다시 실행 | 7.5, 5.8.1 |
| `edit::Cut` | 잘라내기(노트·클립·레인 포인트) | - | `cmd-x` (전역, 프리셋 불변); `shift-x` (Normal) | - | 편집 > 잘라내기 | 7.5, 5.8 |
| `edit::Copy` | 복사(내부 구조 + 시스템 클립보드 SMF/MML) | - | `cmd-c` (전역, 프리셋 불변) | `y y` (Normal) | 편집 > 복사 | 7.5, 5.8, KM-15 |
| `edit::Paste` | 커서 위치에 붙여넣기(SMF/MML 텍스트도 가져옴) | - | `cmd-v` (전역, 프리셋 불변) | - | 편집 > 붙여넣기 | 7.5, 5.10 |
| `edit::PasteAfter` | 커서 뒤에 붙여넣기 | - | - | `p` (Normal) | - | 5.8.1 |
| `edit::PasteBefore` | 커서 앞에 붙여넣기 | - | - | `shift-p` (Normal) | - | 5.8.1 |
| `edit::Duplicate` | 선택 길이만큼 바로 뒤에 복제 | - | `cmd-d` (전역, 프리셋 불변) | - | 편집 > 복제 | 7.5, 5.8 |
| `edit::Delete` | 선택 삭제(선택 없으면 커서 위 노트, 배치 뷰는 클립) | - | `x`, `delete`, `backspace` (Normal) | `x` (Normal) | - | 5.8, 5.10, 5.8.1 |
| `edit::DeleteBefore` | 커서 앞 노트 삭제 | - | - | `shift-x` (Normal) | - | 5.8.1 |
| `edit::DeleteToBarEnd` | 커서부터 마디 끝까지 삭제 | - | - | `shift-d` (Normal) | - | 5.8.1 |
| `edit::CopyBar` | 마디 전체 복사 | - | - | `shift-y` (Normal) | - | 5.8.1 |
| `edit::ChangeToBarEnd` | 커서부터 마디 끝까지 변경(삭제 후 Step) | - | - | `shift-c` (Normal) | - | 5.8.1 |
| `edit::Repeat` | 선택을 바로 뒤에 반복 복제(누를 때마다 하나 더) | - | `r` (Normal) | `y y p` (Normal) | 편집 > 반복 | 5.8, 7.3, 5.10, 7.6 |
| `edit::RepeatLast` | 마지막 편집 반복 | - | `.` (Normal) | `.` (Normal) | - | 5.8, 5.8.1 |
| `edit::Split` | 커서 위치에서 노트/클립 분할 | - | `/` (Normal) | `s` (Normal) | 편집 > 분할 | 5.8, 7.3, 5.10, 5.8.1 |
| `edit::Join` | 선택 노트 합치기(길이 연장), 배치 뷰는 인접 클립 | - | `cmd-j` (Normal) | `shift-j` (Normal) | 편집 > 합치기 | 5.8, 7.3, 5.10, 5.8.1 |
| `edit::Quantize` | 선택 퀀타이즈(현재 설정) | - | `q` (Normal) | `q` (Normal) | 편집 > 퀀타이즈 | 5.8, 5.2, EDT-2, 7.6 |
| `edit::QuantizeSettings` | 퀀타이즈 설정 시트(그리드·강도·스윙) | - | `shift-q` (Normal) | `shift-q` (Normal) | 편집 > 퀀타이즈 (설정 시트) | 5.8, 5.8.1, EDT-2, 7.6 |
| `edit::Legato` | 레가토 토글(끝을 다음 노트 시작에 붙임) | - | `~` (Normal) | `~` (Normal) | 편집 > 레가토 | 5.8, 5.8.1 |
| `edit::SetPitch` | 커서 노트 음높이 바꾸기(음 키 하나를 받음) | - | `e` (Normal > PianoRoll) | `r` (Normal) | - | 5.3, 5.8, 5.8.1 |
| `edit::Humanize(amount)` | 타이밍 무작위 ±N(우선순위 C) | amount | - | - | - | EDT-4 |
| `edit::CancelDrag` | 마우스 드래그 취소, 원위치 | - | `escape` (Dragging) | - | - | 7.4 |

## view

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `view::Toggle` | 피아노롤 ↔ 배치 뷰 | - | `v` (Normal) | `g a` (Normal) | 보기 > 배치 뷰 | 5.8, 7.3, 5.10, 5.8.1, 7.6 |
| `view::ToggleInspector` | 트랙 인스펙터 패널 토글 | - | `cmd-i` (Normal) | - | 보기 > 트랙 인스펙터 | 5.8, UI-12, 7.6 |
| `view::ToggleKeyboard` | 하단 건반 뷰 표시 토글 | - | - | - | 보기 > 건반 뷰 | 7.1, 7.6 |
| `view::ZoomIn` | 줌 인 | - | `cmd-=` (전역, 프리셋 불변) | `z l` (Normal) | 보기 > 줌 | 7.5, 5.8.1 |
| `view::ZoomOut` | 줌 아웃 | - | `cmd--` (전역, 프리셋 불변) | `z h` (Normal) | 보기 > 줌 | 7.5, 5.8.1 |
| `view::ZoomReset` | 줌 초기화 | - | `cmd-0` (전역, 프리셋 불변) | - | 보기 > 줌 | 7.5 |
| `view::ZoomToSelection` | 선택에 맞춰 줌 | - | `z` (Normal) | `shift-z` (Normal) | 보기 > 줌 | 5.8, 5.8.1 |
| `view::ZoomVerticalIn` | 세로 줌 인 | - | - | `z k` (Normal) | 보기 > 줌 | 5.8.1 (방향 추정) |
| `view::ZoomVerticalOut` | 세로 줌 아웃 | - | - | `z j` (Normal) | 보기 > 줌 | 5.8.1 (방향 추정) |
| `view::CenterCursor` | 커서를 화면 중앙으로 | - | - | `z z` (Normal) | - | 5.8.1 |
| `view::SetTheme(theme)` | 다크/라이트/사용자 테마 | theme | - | - | 보기 > 테마 | UI-7, 7.6 메뉴바 구성 |
| `view::ToggleHighContrast` | 고대비 모드 | - | - | - | - (UI 위치 미정) | UI-8 |
| `view::SetFontSize(size)` | 폰트 크기 조절 | size | - | - | - (UI 위치 미정) | UI-8 |

## mode

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `mode::Step` | 커서 위치에서 Step 모드 진입 | - | `enter`, `ctrl-tab` (Normal > PianoRoll) | `i` (Normal) | - | 5.1, 5.3, 5.5, 7.3, 5.8.1, 7.6 |
| `mode::StepAfterNote` | 커서 노트 뒤에서 Step 진입 | - | - | `a` (Normal) | - | 5.8.1 |
| `mode::StepAtBarStart` | 마디 처음에서 Step 진입 | - | - | `shift-i` (Normal) | - | 5.8.1 |
| `mode::StepAtBarEnd` | 마디 끝에서 Step 진입 | - | - | `shift-a` (Normal) | - | 5.8.1 |
| `mode::StepOverwrite` | 덮어쓰기 Step 모드(기존 노트를 밀지 않고 교체) | - | - | `shift-r` (Normal) | - | 5.8.1 |
| `mode::Sequence` | Sequence 모드(녹음, 카운트인 후 시작) | - | `cmd-r`, `ctrl-tab` (Normal) | - | 재생 > 녹음 | 5.2, 5.5, 7.6 |
| `mode::Play` | Play(건반 시뮬레이션) 모드 | - | `cmd-k`, `ctrl-tab` (Normal) | - | - (건반 뷰의 「연주」 버튼) | 5.4, 5.5, 7.6 |
| `mode::Lyric` | 가사 모드(커서 노트부터 음절 입력) | - | `w` (Normal) | - (대체 키 미정) | 편집 > 가사 입력 | 5.15, LYR-3, 7.6 |
| `mode::Controller` | Controller 모드(레인 전체 화면) | - | `shift-c` (Normal) | `g shift-c` (Normal) | - (레인의 확대 버튼) | 5.9, CTL-9 |
| `mode::Cycle` | 모드 순환 Normal → Step → Sequence → Play | - | `ctrl-tab` (전역) | - | - | 5.5, 5.11 |
| `mode::CycleBack` | 모드 역순환 | - | `ctrl-shift-tab` (전역) | - | - | 5.11 |
| `mode::Normal` | Normal로 복귀 | - | `escape` (Step, Sequence, Play, Lyric, Controller) | - | - | 5.1, 5.2, 5.4, 5.15, 5.9, 7.3 |
| ~~`mode::Back`~~ | 삭제(A13 결정, S2-T3): Esc는 피아노롤에서 배치 뷰로 나가지 않는다 | - | - | - | - | 5.3, 5.5, 5.8 |
| `mode::ShowMenu` | 상태줄 모드 표시를 눌러 모드 메뉴 열기 | - | - | - | - (상태줄 클릭) | 7.6 |

## cursor

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `cursor::Left` | 한 스텝 뒤로 | - | `left`, `h` (Normal) | `h` (Normal) | - | 5.7, 7.3 |
| `cursor::Right` | 한 스텝 앞으로 | - | `right`, `l` (Normal) | `l` (Normal) | - | 5.7 |
| `cursor::Down` | 반음 아래 | - | `down`, `j` (Normal) | `j` (Normal) | - | 5.7 |
| `cursor::Up` | 반음 위 | - | `up`, `k` (Normal) | `k` (Normal) | - | 5.7 |
| `cursor::PrevBar` | 한 마디 뒤로 | - | `ctrl-left`, `ctrl-h` (Normal) | - | - | 5.7, 5.10 |
| `cursor::NextBar` | 한 마디 앞으로 | - | `ctrl-right`, `ctrl-l` (Normal) | - | - | 5.7, 7.3, 5.10 |
| `cursor::OctaveDown` | 한 옥타브 아래 | - | `ctrl-down`, `ctrl-j`, `[` (Normal > PianoRoll) | `[` (Normal) | - | 5.7, 5.8 |
| `cursor::OctaveUp` | 한 옥타브 위 | - | `ctrl-up`, `ctrl-k`, `]` (Normal > PianoRoll) | `]` (Normal) | - | 5.7, 5.8 |
| `cursor::NextNote` | 다음 노트로 | - | `n` (Normal > PianoRoll) | `w` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `cursor::PrevNote` | 이전 노트로 | - | `p` (Normal > PianoRoll) | `b` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `cursor::NextClip` | 다음 클립으로 | - | `shift-n` (Normal) | `}` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `cursor::PrevClip` | 이전 클립으로 | - | `shift-p` (Normal) | `{` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `cursor::ClipStart` | 클립 처음 | - | `cmd-left` (Normal) | `g g` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `cursor::ClipEnd` | 클립 끝 | - | `cmd-right` (Normal) | `shift-g` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `cursor::FirstTrack` | 첫(맨 위) 트랙 | - | `cmd-up` (Normal); `ctrl-k` (Normal > Arrangement) | - | - | 5.7, 5.10 |
| `cursor::LastTrack` | 마지막(맨 아래) 트랙 | - | `cmd-down` (Normal); `ctrl-j` (Normal > Arrangement) | - | - | 5.7, 5.10 |
| `cursor::PrevClipBoundary` | 이전 클립 경계(배치 뷰) | - | `left`, `h` (Normal > Arrangement) | - | - | 5.10 |
| `cursor::NextClipBoundary` | 다음 클립 경계(배치 뷰) | - | `right`, `l` (Normal > Arrangement) | - | - | 5.10 |
| `cursor::TrackUp` | 위 트랙으로(배치 뷰) | - | `up`, `k` (Normal > Arrangement) | `(` (Normal) | - | 5.10, 5.8.1 |
| `cursor::TrackDown` | 아래 트랙으로(배치 뷰) | - | `down`, `j` (Normal > Arrangement) | `)` (Normal) | - | 5.10, 5.8.1 |
| `cursor::NextBeat` | 다음 박 시작 | - | - | `shift-w` (Normal) | - | 5.8.1 |
| `cursor::PrevBeat` | 이전 박 시작 | - | - | `shift-b` (Normal) | - | 5.8.1 |
| `cursor::NoteEnd` | 커서 노트의 끝 | - | - | `e` (Normal) | - | 5.8.1 |
| `cursor::LastNoteEndInBar` | 마디 안 마지막 노트의 끝 | - | - | `shift-e` (Normal) | - | 5.8.1 |
| `cursor::NextSamePitch` | 같은 음높이의 다음 노트 | - | - | `f` (Normal) | - | 5.8.1 |
| `cursor::PrevSamePitch` | 같은 음높이의 이전 노트 | - | - | `shift-f` (Normal) | - | 5.8.1 |
| `cursor::RepeatFind` | `f` 탐색 반복 | - | - | `;` (Normal) | - | 5.8.1 (모호, 아래 참조) |
| `cursor::RepeatFindReverse` | `f` 탐색 반대 방향 반복 | - | - | `,` (Normal) | - | 5.8.1 (모호, 아래 참조) |
| `cursor::SetMark(name)` | 마크 설정(`m a`) | name | - | `m` + 문자 (Normal) | - | 5.8.1 |
| `cursor::JumpMark(name)` | 마크로 점프(`'a`, `grave a`) | name | - | `'` + 문자, `grave` + 문자 (Normal) | - | 5.8.1 |

## select

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `select::ExtendLeft` | 선택을 뒤로 한 스텝 넓힘 | - | `shift-left`, `shift-h` (Normal > PianoRoll) | - (vim은 `shift-h`가 `note::Shorten`) | - | 5.7, 7.3 |
| `select::ExtendRight` | 선택을 앞으로 한 스텝 넓힘 | - | `shift-right`, `shift-l` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendDown` | 반음 아래까지 넓힘 | - | `shift-down`, `shift-j` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendUp` | 반음 위까지 넓힘 | - | `shift-up`, `shift-k` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendBarBack` | 한 마디 뒤까지 넓힘 | - | `ctrl-shift-left`, `ctrl-shift-h` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendBarForward` | 한 마디 앞까지 넓힘 | - | `ctrl-shift-right`, `ctrl-shift-l` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendOctaveDown` | 한 옥타브 아래까지 넓힘 | - | `ctrl-shift-down`, `ctrl-shift-j` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendOctaveUp` | 한 옥타브 위까지 넓힘 | - | `ctrl-shift-up`, `ctrl-shift-k` (Normal > PianoRoll) | - | - | 5.7 |
| `select::ExtendPrevClip` | 이전 클립까지 선택(배치 뷰) | - | `shift-left`, `shift-h` (Normal > Arrangement) | - | - | 5.10 |
| `select::ExtendNextClip` | 다음 클립까지 선택(배치 뷰) | - | `shift-right`, `shift-l` (Normal > Arrangement) | - | - | 5.10 |
| `select::ExtendTrackUp` | 위 트랙까지 선택(배치 뷰) | - | `shift-up`, `shift-k` (Normal > Arrangement) | - | - | 5.10 |
| `select::ExtendTrackDown` | 아래 트랙까지 선택(배치 뷰) | - | `shift-down`, `shift-j` (Normal > Arrangement) | - | - | 5.10 |
| `select::All` | 클립 안 전체(배치 뷰는 클립 전부) 선택 | - | `cmd-a` (전역, 프리셋 불변) | - | 편집 > 전체 선택 | 7.5, 5.7, 5.8 |
| `select::Bar` | 현재 마디 선택 | - | `b` (Normal > PianoRoll) | - | - | 5.3, 5.7, 5.8 |
| `select::Clip` | 클립 전체 선택 | - | `shift-b` (Normal > PianoRoll) | - | - | 5.7, 5.8 |
| `select::Clear` | 선택 해제 | - | `escape` (Normal > Selection) | - | - | 5.7, 5.3 |
| `select::VisualNote` | 노트 단위 비주얼 선택 | - | - | `v` (Normal) | - | 5.8.1, 7.4 |
| `select::VisualTime` | 시간 범위 선택(세로 전체) | - | - | `shift-v` (Normal) | - | 5.8.1 |
| `select::VisualBlock` | 블록 선택(음높이 × 시간) | - | - | `ctrl-v` (Normal) | - | 5.8.1 |

## note

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `note::TransposeUp` | 선택 노트 반음 올림(선택 없으면 커서 노트) | - | `alt-k`, `alt-up`, `+` (Normal > PianoRoll) | `=` (Normal) | - | 5.7, 5.8, 7.3, 5.8.1 |
| `note::TransposeDown` | 반음 내림 | - | `alt-j`, `alt-down`, `-` (Normal > PianoRoll) | `-` (Normal) | - | 5.7, 5.8, 5.8.1 |
| `note::NudgeEarlier` | 한 스텝 앞당김 | - | `alt-h`, `alt-left` (Normal > PianoRoll) | - | - | 5.7 |
| `note::NudgeLater` | 한 스텝 미룸 | - | `alt-l`, `alt-right` (Normal > PianoRoll) | - | - | 5.7 |
| `note::NudgeBarEarlier` | 한 마디 앞당김 | - | `alt-ctrl-h`, `alt-ctrl-left` (Normal > PianoRoll) | - | - | 5.7 |
| `note::NudgeBarLater` | 한 마디 미룸 | - | `alt-ctrl-l`, `alt-ctrl-right` (Normal > PianoRoll) | - | - | 5.7 |
| `note::Lengthen` | 끝을 한 스텝(현재 길이 단위) 늘림 | - | `alt-shift-l`, `alt-shift-right`, `>` (Normal > PianoRoll) | `shift-l`, `>` (Normal) | - | 5.7, 7.3, 5.8.1 |
| `note::Shorten` | 끝을 한 스텝 줄임 | - | `alt-shift-h`, `alt-shift-left`, `<` (Normal > PianoRoll) | `shift-h`, `<` (Normal) | - | 5.7, 5.8.1 |
| `note::OctaveUp` | 선택 노트 한 옥타브 올림 | - | `alt-shift-k`, `alt-shift-up` (Normal > PianoRoll) | `shift-k` (Normal) | - | 5.7, 5.8.1 |
| `note::OctaveDown` | 선택 노트 한 옥타브 내림 | - | `alt-shift-j`, `alt-shift-down` (Normal > PianoRoll) | `shift-j` (Normal) | - | 5.7, 5.8.1 |
| `note::SetLength(length)` | 선택 길이를 코드로 지정(`whole` `half` `quarter` `eighth` `sixteenth` `thirtysecond` `sixtyfourth`) | length | `1` whole, `2` half, `3` quarter, `4` eighth, `5` sixteenth, `6` thirtysecond, `7` sixtyfourth (Normal > PianoRoll) | - (vim Normal의 숫자는 카운트 접두) | - | 5.6, 5.7, 5.8, 7.3, 5.8.1 |
| `note::SetDottedLength(length)` | 선택 길이를 점음표로 지정 | length | `shift-1` … `shift-7` (Normal > PianoRoll) | - | - | 5.7, 5.8 |
| `note::ResetLength` | 선택 길이를 기본값(4분)으로 | - | `0` (Normal > PianoRoll) | - | - | 5.8 |
| `note::ShowInfo` | 커서 노트 정보 팝업(틱·길이·채널) | - | `i` (Normal) | `shift-k` (Normal) | - | 5.8, 5.8.1 |
| `note::Audition` | 커서 노트 듣기 | - | - (이동 시 자동, 설정) | `enter` (Normal) | - | 5.3, 5.8.1, PRE-1 |
| `note::TransposeBy(semitones)` | 선택 영역 조옮김(임의 값) | semitones | - | - | - (팔레트) | EDT-3 |
| `note::ShiftBy(ticks)` | 선택 영역 시간 이동(임의 값) | ticks | - | - | - (팔레트) | EDT-3 |
| `note::ScaleLength(ratio)` | 선택 영역 길이 스케일 | ratio | - | - | - (팔레트) | EDT-3 |

## clip

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `clip::Open` | 클립 안(피아노롤)으로 들어감 | - | `enter` (Normal > Arrangement) | - (vim `enter`는 미정) | - | 5.8, 5.10, 7.3 |
| `clip::MoveEarlier` | 선택 클립 한 마디 앞당김 | - | `alt-h`, `alt-left` (Normal > Arrangement) | - | - | 5.10 |
| `clip::MoveLater` | 선택 클립 한 마디 미룸 | - | `alt-l`, `alt-right` (Normal > Arrangement) | - | - | 5.10, 7.3 |
| `clip::MoveUp` | 위 트랙으로 옮김(채널 다르면 경고) | - | `alt-k`, `alt-up` (Normal > Arrangement) | - | - | 5.10 |
| `clip::MoveDown` | 아래 트랙으로 옮김 | - | `alt-j`, `alt-down` (Normal > Arrangement) | - | - | 5.10 |
| `clip::Lengthen` | 끝을 한 마디 늘림(비파괴 트림) | - | `alt-shift-l`, `alt-shift-right` (Normal > Arrangement) | - | - | 5.10, 7.3 |
| `clip::Shorten` | 끝을 한 마디 줄임 | - | `alt-shift-h`, `alt-shift-left` (Normal > Arrangement) | - | - | 5.10 |
| `clip::TrimStartEarlier` | 시작을 한 마디 앞으로 | - | `alt-ctrl-h`, `alt-ctrl-left` (Normal > Arrangement) | - | - | 5.10 |
| `clip::TrimStartLater` | 시작을 한 마디 뒤로 | - | `alt-ctrl-l`, `alt-ctrl-right` (Normal > Arrangement) | - | - | 5.10 |
| `clip::Rename(name)` | 클립 이름 지정 | name | - | - | - | 5.10 (UI 위치 미정) |
| `clip::SetColor(color)` | 클립 색 지정 | color | - | - | - | 5.10 (UI 위치 미정) |

## track

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `track::Next` | 다음 트랙으로(시간 위치 유지, 선택 유지) | - | `tab` (Normal) | `tab`, `)` (Normal) | - | 5.8, 5.11, 5.8.1 |
| `track::Prev` | 이전 트랙으로 | - | `shift-tab` (Normal) | `shift-tab`, `(` (Normal) | - | 5.8, 5.11, 5.8.1 |
| `track::New` | 새 트랙 추가(비어 있는 가장 낮은 채널) | - | `cmd-t` (전역, 프리셋 불변) | - | 트랙 > 새 트랙 | 5.8, 5.10, SEQ-7, 7.6 |
| `track::NewBelow` | 아래에 새 트랙 | - | - | `o` (Normal) | - | 5.8.1 |
| `track::NewAbove` | 위에 새 트랙 | - | - | `shift-o` (Normal) | - | 5.8.1 |
| `track::Delete` | 현재 트랙 삭제 | - | - | - | 트랙 > 삭제 | 7.6 메뉴바 구성 |
| `track::ToggleMute` | 현재 트랙 뮤트 토글 | - | `m` (Normal) | `shift-m` (Normal) | 트랙 > 뮤트 | 5.8, 5.8.1, 7.6 |
| `track::ToggleSolo` | 현재 트랙 솔로 토글 | - | `o` (Normal) | `shift-s` (Normal) | 트랙 > 솔로 | 5.8, 5.8.1, 7.6 |
| `track::MoveUp` | 트랙 순서를 위로 | - | - | - | 트랙 > 위로 | 7.6 메뉴바 구성 |
| `track::MoveDown` | 트랙 순서를 아래로 | - | - | - | 트랙 > 아래로 | 7.6 메뉴바 구성 |
| `track::FocusList` | 트랙 목록 패널로 포커스 | - | - | `g t` (Normal) | - | 5.8.1, KM-1 |
| `track::SetPort(port)` | 트랙 출력 포트 지정 | port | - | - | 트랙 > 포트 | SEQ-7, 7.6 |
| `track::SetChannel(channel)` | 트랙 채널 지정(1–16) | channel | - | - | 트랙 > 채널 | SEQ-7, 7.6 |
| `track::SetProgram(bank, program)` | 뱅크·프로그램 지정 | bank, program | - | - | - (인스펙터) | SEQ-7, UI-12, 7.6 |
| `track::SetDefaultVelocity(velocity)` | 트랙 기본 벨로시티 | velocity | - | - | - (인스펙터) | SEQ-7, UI-12, 6.2 |
| `track::SetLyricLang(lang)` | 가사 언어(ko·en·ja…) | lang | - | - | - (인스펙터) | 5.15, 7.6 |
| `track::Rename(name)` | 트랙 이름 | name | - | - | - (인스펙터) | SEQ-7, UI-12 |
| `track::SetColor(color)` | 트랙 색 | color | - | - | - (인스펙터) | SEQ-7, UI-12 |

## transport

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `transport::Toggle` | 재생/정지 | - | `space` (Normal, Play); `cmd-return` (전역, 프리셋 불변) | `ctrl-space` (Normal); `ctrl-.`는 패닉 | - | 5.5, 5.8, 7.3, 7.5, 7.6 |
| `transport::Play` | 재생 | - | - | - | 재생 > 재생 | 7.6, TRN-1 |
| `transport::Stop` | 정지 | - | - | - | 재생 > 정지 | 7.6, TRN-1 |
| `transport::Pause` | 일시정지 | - | - | - | - | TRN-1 |
| `transport::PlayFromCursor` | 커서(배치 뷰는 커서 클립)부터 재생 | - | `shift-space` (Normal) | `shift-space` (Normal) | 재생 > 커서부터 | 5.8, 5.10, 5.8.1, 7.6 |
| `transport::LoopSelection` | 선택 영역(클립) 루프 재생 | - | `shift-enter` (Normal) | `shift-enter` (Normal) | 재생 > 루프 | 5.8, 5.10, 5.8.1, 7.6 |
| `transport::ToggleLoop` | 루프 켜기/끄기 | - | - | - | 재생 > 루프 | 7.6, TRN-1 |
| `transport::SetLoopRange(start, end)` | 루프 구간 설정 | start, end | - | - | - | TRN-1 |
| `transport::GoTo(bar, beat)` | 마디·박 단위 위치 이동 | bar, beat | - | - | - | TRN-1 |
| `transport::ToggleMetronome` | 메트로놈 켜기/끄기 | - | - | - | 재생 > 메트로놈 | MET-1, 7.6 |
| `transport::SetCountIn(bars)` | 카운트인 0–4마디 | bars | - | - | - (설정) | TRN-2, 5.2 |
| `transport::SetTempo(bpm)` | 템포 입력 | bpm | `t` (Normal) | `t` (Normal) | - (트랜스포트 필드) | 5.8, 5.8.1, TRN-4, 7.6 |
| `transport::TapTempo` | 탭 템포 | - | `shift-t` (Normal) | `shift-t` (Normal) | - (트랜스포트 버튼) | 5.8, 5.8.1, 7.6 |
| `transport::SetTimeSignature(num, den)` | 박자표 | num, den | - | - | - (트랜스포트 필드) | TRN-4, 7.6 |
| `transport::SetKey(key)` | 조성 | key | - | - | - (트랜스포트 필드) | TRN-4, 7.6 |
| `transport::Panic` | 패닉(전 포트·채널 All Notes Off + All Sound Off + 서스테인 해제) | - | `cmd-.` (전역, 프리셋 불변) | `ctrl-.` (Normal) | 재생 > 패닉 | 5.5, 7.5, OUT-6, 7.6 |

## midi

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `midi::SelectOutputPort(port)` | 출력 포트 선택(트랜스포트 드롭다운) | port | - | - | - (트랜스포트 드롭다운, 팔레트 「포트 선택」) | OUT-3, 7.6 |
| `midi::AddVirtualPort` | 가상 포트 추가(최대 16) | - | - | - | - (설정 > MIDI 출력) | OUT-2, 7.6 |
| `midi::RenameVirtualPort(name)` | 가상 포트 이름 변경 | name | - | - | - (설정 > MIDI 출력) | OUT-2, 7.6 |
| `midi::SendTestNote` | 연결 테스트 노트 송신 | - | - | - | - (설정 > MIDI 출력, 인스펙터 「테스트」, 팔레트) | PRE-3, 7.6 |

## step

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `step::Note(key)` | 음 입력(현재 길이·옥타브, 커서 전진). 드럼 트랙은 GM 라벨 | key | `a`, `s`, `d`, `f`, `g`, `h`, `j`, `k`, `l`, `w`, `e`, `t`, `y`, `u`, `o`, `p` (Step > Piano); `c`, `d`, `e`, `f`, `g`, `a`, `b` (Solfege 레이아웃, Step > Solfege); `a`, `s`, `w`, `e`, `d`, `r`, `f`, `g`, `h`, `j`, `t`, `k`, `l`, `y`, `u`, `o` (Step > Drum) | - | - | 5.1, P3, 5.6, 5.13 |
| `step::Chord(key)` | 전진 없이 같은 위치에 음을 겹침 | key | `ctrl-` + 음 키 (Step) | - | - | 5.1 |
| `step::HalfNote(key)` | 현재 길이의 절반으로 그 음 하나만(스티키 아님) | key | `shift-` + 음 키 (Step) | `shift-` + 음 키 (Step) | - | 5.1, 5.6, 5.8.1 |
| `step::Triplet` | 셋잇단 접두(`n` + 음) | - | `n` (Step) | `n` (Step) | - | 5.1, 5.6 |
| `step::Accidental(kind)` | 변화표(`sharp` `flat`), 계이름 레이아웃 전용 | kind | `#`, `+` sharp; `b`, `-` flat (Step > Solfege) | - | - | 5.6, 5.1 |
| `step::SetLength(length)` | 길이 코드 지정(스티키) | length | `1`–`7` (Step) | `1`–`7` (Step) | - | 5.1, 5.6, 5.8.1 |
| `step::SetDottedLength(length)` | 점음표 길이(스티키) | length | `shift-1` … `shift-7` (Step) | `shift-1` … `shift-7` (Step) | - | 5.1, 5.6, 5.8.1 |
| `step::ResetLength` | 길이를 기본값(4분)으로 | - | `0` (Step) | - | - | 5.1, 5.6 |
| `step::SetOctave(octave)` | 음 키 직후 옥타브 숫자(0–9, 300 ms 대기) | octave | `0`–`9` (Step > Pending) | - | - | 5.6 |
| `step::OctaveDown` | 옥타브 −1 | - | `[`, `z` (Step > Piano); `[` (Step) | `[` (Step) | - | 5.1, 5.8.1 |
| `step::OctaveUp` | 옥타브 +1 | - | `]`, `x` (Step > Piano); `]` (Step) | `]` (Step) | - | 5.1, 5.8.1 |
| `step::OctaveOnce(delta)` | 입력 중인 음만 옥타브 ±1(스티키 아님) | delta | - | `{` (Step), `}` (Step) | - | 5.8.1 |
| `step::Rest` | 쉼표(현재 길이, 커서 전진) | - | `space` (Step) | `space` (Step) | - | 5.1, 5.6, 7.3, 5.8.1 |
| `step::HalfRest` | 현재 길이 절반의 쉼표 | - | - | `shift-space` (Step) | - | 5.8.1 |
| `step::Back` | 한 스텝 뒤로 가며 그 자리 노트 삭제 | - | `backspace` (Step) | `backspace` (Step) | - | 5.1, 7.3, 5.8.1 |
| `step::BackToBarStart` | 마디 처음까지 삭제 | - | - | `shift-backspace` (Step) | - | 5.8.1 |
| `step::Prev` | 한 스텝 뒤로(삭제 없음) | - | `shift-tab` (Step) | `shift-tab` (Step) | - | 5.11 |
| `step::Confirm` | 조합 확정 후 다음 스텝(조합 없으면 커서만 전진, 화음 확정) | - | `tab` (Step) | `tab` (Step) | - | 5.11, 7.3 |
| `step::Commit` | 조합 중인 입력 즉시 확정(전진 없음). 길이만 바꿀 때도 사용 | - | `enter` (Step) | `enter` (Step) | - | 5.6, 5.8.1 |
| `step::Tie` | 타이: 직전 노트를 현재 길이만큼 늘림 | - | `~` (Step) | `~` (Step) | - | 5.1, 5.8.1 |
| `step::TieLegato` | 입력 중인 음을 레가토로 | - | - | `shift-~` (Step) | - | 5.8.1 |
| `step::PreviewFromBar` | 입력을 마치고 마디 처음부터 들어보기 | - | - | `shift-enter` (Step) | - | 5.8.1 |
| `step::KitPageNext` | 드럼 킷 다음 페이지(GM 35–81) | - | `]` (Step > Drum) | - | - | 5.13 |
| `step::KitPagePrev` | 드럼 킷 이전 페이지 | - | `[` (Step > Drum) | - | - | 5.13 |

## sequence

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `sequence::Hit` | 박자에 맞춰 타격(미리 적기: 열의 다음 노트에 타이밍·길이 적용) | - | `space` (Sequence > PrePitched) | - | - | 5.2 |
| `sequence::Note(key)` | 직접 모드: 키 레이아웃으로 음과 타이밍 동시 입력 | key | `step::Note`와 같은 음 키 (Sequence > Piano, Sequence > Solfege) | - | - | 5.2 |
| `sequence::Skip` | 열의 다음 음 건너뛰기 | - | `tab` (Sequence > PrePitched) | - | - | 5.11 |
| `sequence::Rewind` | 열의 이전 음으로 되감기 | - | `shift-tab` (Sequence > PrePitched) | - | - | 5.11 |
| `sequence::ConfirmTake` | 이번 테이크 확정, 다음 루프 | - | `tab` (Sequence > Loop) | - | - | 5.11, 5.2 |
| `sequence::DiscardTake` | 이번 테이크 버리고 다시 | - | `shift-tab` (Sequence > Loop) | - | - | 5.11, 2장 표(Sequence 녹음 undo) |
| `sequence::SetLoopMode(mode)` | 루프 녹음 겹쳐 쓰기/덮어 쓰기 선택 | mode | - | - | - | 5.2 |

## play

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `play::Note(key)` | 누르는 동안 노트 송신(기록 없음) | key | `a`, `s`, `d`, `f`, `g`, `h`, `j`, `k`, `l`, `w`, `e`, `t`, `y`, `u`, `o`, `p` (Play > Piano); `c`, `d`, `e`, `f`, `g`, `a`, `b` (Play > Solfege) | - | - | 5.4, P3 |
| `play::Sustain` | 서스테인(CC64) 송신 | - | `shift` (Play) | - | - | 5.4 |
| `play::NextTrack` | 다음 트랙의 출력(포트·채널)으로 | - | `tab` (Play) | - | - | 5.4, 5.11 |
| `play::PrevTrack` | 이전 트랙의 출력으로 | - | `shift-tab` (Play) | - | - | 5.4, 5.11 |
| `play::SelectTrack(n)` | 트랙 1–8 직접 선택 | n | `f1` … `f8` (Play) | - | - | 5.4 |
| `play::ProgramPrev` | 프로그램 체인지 −1 송신 | - | `,` (Play) | - | - | 5.4 |
| `play::ProgramNext` | 프로그램 체인지 +1 송신 | - | `.` (Play) | - | - | 5.4 |
| `play::OctaveDown` | 옥타브 −1 | - | `[` (Play) | - | - | 5.4 |
| `play::OctaveUp` | 옥타브 +1 | - | `]` (Play) | - | - | 5.4 |

## lane

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `lane::Toggle` | 컨트롤러 레인 접기·펼치기 | - | `c` (Normal) | `g c` (Normal) | 보기 > 컨트롤러 레인 | 5.8, 5.9, CTL-1, 5.8.1, 7.6 |
| `lane::Next` | 다음 레인 | - | `tab` (Lane) | - | - | 5.9, 5.11 |
| `lane::Prev` | 이전 레인 | - | `shift-tab` (Lane) | - | - | 5.9, 5.11 |
| `lane::SelectKind(kind)` | 레인 종류 선택(벨로시티·CC·피치 벤드) | kind | - | - | - (레인 헤더 드롭다운) | 5.9, 7.6 |
| `lane::AddCC(number)` | CC 레인 추가(번호) | number | - | - | - (레인 헤더 드롭다운) | 5.9, CTL-3 |

## lyric

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `lyric::Next` | 다음 노트로 | - | `space`, `tab` (Lyric) | - | - | 5.15 |
| `lyric::Prev` | 이전 노트로 | - | `shift-tab` (Lyric) | - | - | 5.15 |
| `lyric::Melisma` | 멜리스마(`-`) 표시 후 다음 노트 | - | `-` (Lyric) | - | - | 5.15 |
| `lyric::Backspace` | 현재 노트 가사 지움, 비어 있으면 이전 노트로 | - | `backspace` (Lyric) | - | - | 5.15 |
| `lyric::Distribute` | 붙여넣은 문장을 음절 단위로 선택 노트에 분배 | - | `cmd-v` (Lyric) | - | 편집 > 가사 분배 | 5.15, LYR-4, 7.6 |
| `lyric::Suggest` | 가사 도우미: 다음 소절 가사 제안 요청(S) | - | `a` (Lyric) | - | - | LYR-8 |
| `lyric::AcceptSuggestion` | 제안된 가사 고스트 수락 | - | `tab` (Lyric) | - | - | LYR-8 |

## ai

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `ai::OpenPanel` | AI 패널 열기(커서·선택·현재 트랙이 대상) | - | `a` (Normal) | `:ai` (팔레트) | AI > 생성 | 5.8, 5.12, AI-2, 7.6 |
| `ai::Submit` | 프롬프트 전송 | - | - (키 미정) | - | - (패널 버튼) | 5.12, AI-2 |
| `ai::Accept` | 생성 결과 전체 수락(undo 한 단위) | - | `tab` (AI > Preview) | - | - | 5.5, 5.11, 5.12, AI-5 |
| `ai::Dismiss` | 결과 버림 / 요청 취소 | - | `escape` (AI) | - | - | 5.5, 5.12, AI-5, AI-10 |
| `ai::Regenerate` | 같은 프롬프트로 재생성 | - | `r` (AI > Preview) | - | - | 5.12, AI-5 |
| `ai::EditPrompt` | 프롬프트 수정 후 재생성 | - | `e` (AI > Preview) | - | - | 5.12, AI-5 |
| `ai::Replay` | 다시 듣기 | - | `enter` (AI > Preview) | - | - | 5.12, AI-5 |
| `ai::PrevCandidate` | 이전 후보(재생성 이력) | - | `shift-tab` (AI > Preview) | - | - | 5.11 |
| `ai::SetKind(kind)` | 생성 종류 선택(새로 만들기·이어쓰기·변주·반주·화성·가사/멜로디·필·고치기) | kind | - | - | - (패널) | 5.12, AI-2 |
| `ai::SetRole(role)` | 역할 선택(멜로디·코드·베이스·드럼·보컬) | role | - | - | - (패널) | 5.12 |
| `ai::ShowSent` | 「전송 내용 보기」 | - | - | - | - (패널) | 5.12, AI-2 |
| `ai::ShowResponse` | 응답 원문 보기 | - | - | - | - (패널) | AI-2, AI-6 |
| `ai::ToggleAssist` | 어시스트 켜기/끄기 | - | - | - | AI > 어시스트 | 5.12, AI-11, 7.6 (팔레트 `assist on/off`) |
| `ai::AcceptGhost` | 고스트를 다음 마디 끝까지 수락하고 커서 이동 | - | `tab` (Step > Ghost) | `tab` (Step > Ghost) | - | 5.11, 5.12, AI-11 |
| `ai::IgnoreGhost` | 고스트 무시하고 한 스텝 뒤로 | - | `shift-tab` (Step > Ghost) | `shift-tab` (Step > Ghost) | - | 5.11 |
| `ai::DismissGhost` | 고스트만 지우고 Step에 머묾 | - | `escape` (Step > Ghost) | `escape` (Step > Ghost) | - | 5.5, 5.12, AI-11 |
| `ai::OpenLab` | 프롬프트 실험실 | - | - | - | AI > 프롬프트 실험실 | 5.12, AI-16, 7.6 |
| `ai::OpenTemplateFolder` | 템플릿 폴더 열기 | - | - | - | AI > 템플릿 폴더 | 5.12, AI-14, 7.6 |
| `ai::RevertTemplate` | 수정한 과제 템플릿을 원본으로 | - | - | - | - (패널 되돌리기 버튼) | 5.12, AI-15 |
| `ai::RunGrid` | 실험실 격자 실행(픽스처 × 템플릿 × 모델) | - | - | - | - (실험실 탭) | AI-16 |
| `ai::StarResult` | 실험실 듣기 비교 별표 | - | - | - | - (실험실 탭) | 5.12, AI-16 |
| `ai::SetDefaultModel(purpose, model)` | 이 모델을 어시스트/생성 기본으로 | purpose, model | - | - | - (실험실 탭) | AI-19 |
| `ai::SaveFixture` | 사용자 프로젝트 범위를 픽스처로 저장 | - | - | - | - (실험실 탭) | AI-18 |
| `ai::LoadFixture` | 픽스처 불러오기 | - | - | - | - (실험실 탭) | AI-18 |

## search

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `search::Open` | 검색(`c#4`, 트랙 이름, 가사) | - | `cmd-f` (전역, 프리셋 불변) | `/` (Normal) | - | 5.8, 7.5, 5.8.1 |
| `search::Next` | 다음 검색 결과 | - | - | `n` (Normal) | - | 5.8.1 |
| `search::Prev` | 이전 검색 결과 | - | - | `shift-n` (Normal) | - | 5.8.1 |

## operator

vim 프리셋 전용(5.8 「vim 프리셋에서만 산다」). 텍스트 객체는 REQ에 예(`dac` `yib` `vin`)만 있고 전체 목록이 없다.

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `operator::Delete` | 삭제 오퍼레이터(`d w`, `d a c`) | - | - | `d` (Normal) | - | 5.8.1 |
| `operator::Yank` | 복사 오퍼레이터(`y i b`) | - | - | `y` (Normal) | - | 5.8.1 |
| `operator::Change` | 변경 오퍼레이터(삭제 후 Step 모드) | - | - | `c` (Normal) | - | 5.8.1 |
| `operator::Count(digit)` | 숫자 카운트 접두(`3 l`) | digit | - | `1`–`9` (Normal) | - | 5.7, KM-5, 5.8.1 |
| `object::AroundClip` | 텍스트 객체 `a c` (예시로만 등장) | - | - | `a c` (Normal > Operator) | - | 5.8.1 (예시) |
| `object::InnerBar` | 텍스트 객체 `i b` (예시로만 등장) | - | - | `i b` (Normal > Operator) | - | 5.8.1 (예시) |
| `object::InnerNote` | 텍스트 객체 `i n` (예시로만 등장) | - | - | `i n` (Normal > Operator) | - | 5.8.1 (예시) |

## keymap

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `keymap::Edit` | 키맵 편집 화면(설정 > 키맵) 열기 | - | `cmd-,` (전역, 프리셋 불변) | `:set` | - (설정, 팔레트 「키맵 편집」) | KM-3, 7.5, 7.6 |
| `keymap::StartCapture` | 항목에서 키 직접 눌러 바인딩 지정 시작 | - | `enter` (Keymap > Editing) | - | - | KM-3 |
| `keymap::CancelCapture` | 키 캡처 취소 | - | `escape` (Keymap > Editing) | - | - | KM-8, 7.3 |
| `keymap::ClearBinding` | 바인딩 해제 | - | `backspace` (Keymap > Editing) | - | - | KM-3 |
| `keymap::RestoreDefault` | 프리셋 기본값 복원 | - | `r` (Keymap > Editing) | - | - | KM-3 |
| `keymap::SelectPreset(preset)` | 프리셋 `default` / `vim` 선택 | preset | - | - | - (설정 > 키맵) | KM-2, 7.6 |
| `keymap::Export` | 키맵 JSON 내보내기 | - | - | - | - (설정 > 키맵) | KM-9 |
| `keymap::Import` | 키맵 JSON 가져오기(충돌 검사 후 적용) | - | - | - | - (설정 > 키맵) | KM-9, KM-4 |
| `keymap::SwapModifiers` | 수식키 교환 옵션(`Alt` ↔ `Cmd` 등) | - | - | - | - (설정 > 키맵) | KM-11 |

## palette

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `palette::Open` | 명령 팔레트 열기(모든 Action 검색·실행) | - | `cmd-shift-p`, `ctrl-shift-p` (전역, 프리셋 불변); `:` (Normal) | `:` (Normal) | - | 5.8, 7.2 UI-2, 7.5 |
| `palette::Complete` | 자동완성 확정 | - | `tab` (Palette) | - | - | 5.11 |
| `palette::CompletePrev` | 이전 후보 | - | `shift-tab` (Palette) | - | - | 5.11 |

## help

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `help::KeymapOverlay` | 현재 모드의 키맵 오버레이 | - | `?` (Normal) | `?` (Normal) | 도움말 > 키맵 보기 | 5.8, 5.8.1, UI-6, KM-10, 7.6 |
| `help::NextPage` | 오버레이 다음 페이지 | - | `tab` (Overlay) | - | - | 5.11 |
| `help::PrevPage` | 오버레이 이전 페이지 | - | `shift-tab` (Overlay) | - | - | 5.11 |
| `help::ReplayOnboarding` | 온보딩 다시 보기 | - | - | - | 도움말 > 온보딩 다시 보기 | 7.7, 7.6 |
| `help::OpenDocs` | 문서 열기 | - | - | - | 도움말 > 문서 | 7.6 메뉴바 구성 |

## settings

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `settings::Open(section)` | 설정 열기(`metronome` `midi_output` `input` `ai` `keymap`) | section(선택) | `cmd-,` (전역, 프리셋 불변) | `:set` | - (7개 메뉴에 항목 없음) | 7.5, 7.6 |
| `settings::Calibrate` | 레이턴시 보정 마법사(설정 > 입력) | - | - | - | - (팔레트 「보정 마법사」) | 5.2, UI-10, 7.6 |
| `settings::SetKeyLayout(layout)` | 키 레이아웃 건반형/계이름형 | layout | - | - | - (설정 > 입력) | P3, UI-10, 7.6 |

## window

| Action | 설명 | 인자 | default 키(컨텍스트) | vim 키(컨텍스트) | 메뉴 | REQ 근거 |
| --- | --- | --- | --- | --- | --- | --- |
| `window::Close` | 창 닫기(저장 안 된 변경은 시트로 확인) | - | `cmd-w` (전역, 프리셋 불변) | `:q` | - (7개 메뉴에 항목 없음) | 7.5 |
| `window::FocusNext` | 다음 필드(키맵 편집·설정·대화상자) | - | `tab` (Dialog) | - | - | 5.11 |
| `window::FocusPrev` | 이전 필드 | - | `shift-tab` (Dialog) | - | - | 5.11 |

## 키 충돌·모호한 곳

분류는 「충돌」(같은 컨텍스트에서 같은 키가 둘 이상의 뜻)과 「모호」(키·Action이 언급은 되나 정의가 불충분)이다. 인용은 `docs/REQ.md`의 절 번호와 원문이다. 번호는 결정 요청(`docs/decisions/`) 때 그대로 참조할 수 있게 붙였다.

### A. 같은 컨텍스트에서 같은 키가 두 뜻 (충돌)

1. vim `J` `K`가 두 뜻. 5.8.1 `j` `k` 행은 「`Shift` (대문자)」 칸에 「선택 노트 한 옥타브 내림/올림」을 배정하는데, 같은 표의 `J` 행은 「선택 노트 합치기(길이 연장)」, `K` 행은 「커서 노트 정보 팝업(틱·길이·채널)」이다. vim Normal의 `shift-j` `shift-k`가 `note::OctaveDown/Up`과 `edit::Join` / `note::ShowInfo`에 동시에 걸린다. 카탈로그에는 5.8.1 표대로 둘 다 적었으므로 결정이 필요하다(KM-4 위반 상태).
2. vim 프리셋 `g` 접두의 뷰 전환·정보 키가 REQ 안에서 갈린다. 5.8.1 표는 「`ga` 배치 뷰 ↔ 피아노롤」, 용어 정의(3장)도 「피아노롤과 `ga`로 오간다」인데, 같은 절 끝 「충돌 메모」는 「그 기능은 `gv`·`gc`·`gi`·`K`·`yyp`·`w`/`b`로 간다」라고 적어 `gv`(뷰)와 `gi`(정보)를 말한다. 표에는 `gi`도 `gv`도 없고 정보는 `K`다. 카탈로그는 `g a`를 채택했다.
3. 컨트롤러 레인 키가 기본 프리셋과 6.7에서 다르다. 5.8·5.9는 `c`(레인)·`Shift+c`(Controller 모드)이고 `g`는 「비어 있는 글자」(5.8 「`d` `f` `g` `s` `u` `y`」)다. 그러나 CTL-1은 「`gc` 토글」, CTL-9는 「Controller 모드(`gC`)」라고 적었다. `gc`/`gC`는 vim 프리셋에서만 성립한다(5.8.1은 `gc`만 언급, `gC`는 어디에도 없음). 카탈로그는 기본 `c`/`shift-c`, vim `g c`/`g shift-c`로 적었다.
4. Sequence 모드의 `Tab` / `Shift+Tab`이 두 뜻. 5.11 표는 「Sequence 모드 · 미리 적기: `Tab` 열의 다음 음 건너뛰기 / `Shift+Tab` 열의 이전 음으로 되감기」와 「Sequence 모드 · 루프 녹음: `Tab` 이번 테이크를 확정하고 다음 루프 / `Shift+Tab` 이번 테이크를 버리고 다시」를 둔다. 미리 적기 상태에서 루프 녹음을 하면 두 컨텍스트가 겹친다. 또 2장 표(Sequence 녹음 undo)는 「`Shift+Tab`(테이크 버리기)과 `Cmd+Z`는 같은 결과」라고만 해 미리 적기 중 `Shift+Tab`(되감기)과 모순된다. 카탈로그는 `Sequence > PrePitched`와 `Sequence > Loop`로 나눴으나 동시 활성 때의 우선순위가 정의되지 않았다.
5. 가사 모드의 `Tab`이 두 뜻. 5.15는 「`Space`·`Tab`은 다음 노트」, LYR-8은 「`Tab`으로 받는다」(가사 도우미 고스트 수락). 고스트가 있을 때만 수락으로 보는 해석이 자연스러우나 REQ에 명시가 없다.
6. 가사 모드의 `a` 키. LYR-8은 「가사 모드에서 `a`를 누르면 …다음 소절 가사만 제안」인데, 5.15는 가사 모드를 「문자 입력 모드(KM-7의 물리 키 매칭을 끄고 IME를 그대로 받는다)」로 정의한다. `a`는 영어 가사의 글자이기도 하므로 글자 입력과 명령이 충돌한다. 같은 이유로 `Space`(다음 노트)와 `-`(멜리스마)도 영어 가사(`lov-ing you`)의 공백·하이픈과 겹친다.
7. AI 패널의 `r` `e` `Enter`. 5.12·AI-5는 「`r` 재생성, `e` 프롬프트 수정, `Enter` 다시 듣기」인데 같은 패널에서 프롬프트 한 줄을 입력받는다. 텍스트 입력 포커스와 미리보기 포커스를 구분하는 컨텍스트(`AI > Preview` 가칭)가 REQ에 없고, 프롬프트 전송 키(`Enter`로 보이나 미정)도 없다.
8. vim `Enter`. 5.8.1 `Enter` 행은 「커서 노트 듣기 / `Shift+Enter` 선택 영역 루프 재생」이다. 반면 5.5·5.8의 `Enter`는 모드 진입(피아노롤에서 Step, 배치 뷰에서 클립 안으로)이고 vim은 Step 진입을 `i`에 준다. 배치 뷰에서 vim 사용자가 클립을 여는 키가 정의돼 있지 않다.
9. vim의 숫자 키. 5.7 「vim 프리셋에서만 숫자 카운트 접두(`3l`)를 쓰며, 기본 프리셋에서 숫자는 길이 코드다」. 그렇다면 vim Normal의 `1`–`7`·`0`(선택 길이 지정, 5.8)은 카운트에 먹히는데 대체 키가 5.8.1에 없다. 같은 이유로 `0`(길이 기본값 복귀)도 대체가 없다.
10. vim에서 덮이는 기본 기능 중 대체 키가 없는 것. 5.8.1 충돌 메모는 `v` `c` `a` `i` `r` `n` `p`의 이동처만 적는다. 그 밖에 덮이는 기본 키 중 이동처가 없는 것: `w` 가사 모드(`w`=다음 노트), `a` AI 패널(`a`=노트 뒤 Step, 진입은 `:ai`만 가능), `b` 현재 마디 선택과 `shift-b` 클립 전체 선택(`b`/`B`=이전 노트/박), `e` 음높이 바꾸기(→ `r`로 이동, 메모에 없음), `/` 분할(→ `s`로 이동, 메모에 없음), `z` 선택에 맞춰 줌(→ `Z`).
11. vim `;` `,`. 5.8.1 `f` 행은 「vim `f` `F`, `;` `,`로 반복」이라 하나, 기호 표의 `;` 행은 비어 있고(Shift 칸에 `:` 명령줄) `,`는 `.` `,` 행에 소문자 칸 뜻이 없다(`.`만 정의, `>` `<`는 Shift 조합). 카탈로그는 `;` `,`를 `cursor::RepeatFind` / `RepeatFindReverse`로 추정 배정했다.
12. 명령줄 존재 여부. 5.8 「`Cmd+Shift+P` / `:` … `:`도 같은 팔레트를 연다. 별도의 명령줄은 없다」인데, 5.8.1 「`;` | `:` 명령줄」, 5.11 「명령줄 `:` · 명령 팔레트」, KM-1 컨텍스트 「포커스 패널(트랙 목록/레인/명령줄)」, 5.12 「`:ai`」, 7.5 vim 대응 열의 `:new` `:e` `:w` `:q` `:set`, 12.2의 `:test`가 명령줄을 전제한다. 팔레트와 명령줄이 같은 것인지, 7.5의 ex 명령을 실제로 파싱하는지 불명확하다.
13. 종료 Esc의 의미가 절마다 다르다. P2(4장)와 5.11은 「`Esc`는 어느 모드에서든 Normal로 돌아간다」, 7.3 예는 `Step`의 `"escape": "mode::Normal"`. 반면 5.5·5.8은 「한 단계씩 나간다: AI 고스트 → 선택 → 모드 → 피아노롤 → 배치 뷰」, 5.12는 「`Esc`는 고스트가 있으면 고스트만 지우고 Step 모드에 머문다. 한 번 더 누르면 Normal」, 5.3은 「선택 해제 → 배치 뷰 순」(모드 단계 없음). 카탈로그는 `mode::Normal`(모드에서 복귀), `mode::Back`(Normal에서 한 단계), `select::Clear`, `ai::DismissGhost`로 나눠 배정했다.

### B. 같은 동작에 키가 둘 이상이거나 REQ 안에서 불일치 (모호)

14. `[` `]`와 `Ctrl+j/k`. 5.8 「`[` / `]` | 커서 옥타브 아래/위」와 5.7 「`Ctrl` 큰 커서 이동: `j` 한 옥타브 아래 / `k` 한 옥타브 위」가 같은 동작으로 보인다. 별칭인지 다른 단위인지(커서 이동 vs 입력 옥타브 설정) 불명확하다. 카탈로그는 둘 다 `cursor::OctaveDown/Up`에 배정했다.
15. `Space` 재생. 5.5 「어느 모드에서든 `Space`(Normal·Play) 또는 `Cmd+Return`은 재생/정지」인데 Play 모드는 키를 눌러 음을 내는 모드이고 Sequence에서는 `Space`가 타격 키(5.2), Step에서는 쉼표(5.1)다. vim Normal의 `Space`는 5.8.1에서 `(Ctrl+Space 재생/정지)`로 단독 `Space`가 비어 있다. 7.6 첫 행은 「`Space`, `Cmd+R`」만 적고 `Cmd+Return`을 빠뜨린다.
16. `Cmd+V`와 vim `p`. 7.5는 「`Cmd+V` 붙여넣기 | vim 대응 `p`」, 「붙여넣기 위치는 키보드 커서」인데 5.8.1은 `p`=「커서 뒤에 붙여넣기」, `P`=「커서 앞」이다. 위치 규칙이 다르다(`edit::Paste` / `PasteAfter` / `PasteBefore`로 분리).
17. 7.5 vim 대응 열과 5.8.1의 불일치. `Cmd+X`→「`x`(선택 시)」이지만 5.8.1의 `x`는 「커서 노트 삭제」(삭제, 잘라내기 아님). `Cmd+C`→`y`이지만 5.8.1의 `y`는 복사 오퍼레이터(모션 필요)이고 단독 동작이 정의되지 않았다. `Cmd+D`→「`yyp`류」, 5.8.1 충돌 메모의 `r` 이동처도 `yyp`로 같지만 `y y`(줄 복사에 해당하는 동작)가 5.8.1에 정의돼 있지 않다. `Cmd+A`→`ggVG`도 `V`가 「시간 범위 선택」이라 의미가 다르다.
18. `Ctrl+Shift+P`. 7.5는 「`Cmd+Shift+P` … `Ctrl+Shift+P`도 유지」라 하나 5.8 표와 UI-2에는 없다.
19. 줌 키 방향. 7.5 vim 대응 열 「`zl` `zh`」(줌 인/아웃), 5.8.1 「`z` 줌 접두(`zh` `zl` 가로, `zj` `zk` 세로, `zz` 커서 중앙)」. `zj` `zk`의 인/아웃 방향이 적혀 있지 않다(카탈로그는 `zk`=인, `zj`=아웃으로 추정).
20. vim의 `(` `)`. 5.8.1 「`9` `0` 자리의 `(` `)` | `(` `)` 위/아래 트랙으로 커서 (`Tab`의 별칭)」. `Tab`이 다음 트랙이므로 `)`=다음, `(`=이전으로 읽었으나, 「위/아래」 순서와 「Tab의 별칭」(다음 한쪽만) 표현이 어긋난다.
21. 마크 키. 5.8.1 `m` 행 「마크 설정(`ma`, `'a`로 복귀)」과 `` ` `` 행 「마크로 점프」. `'`와 백틱이 같은 동작인지 불명확하고, `'` 행은 표에 없다. 마크 이름이 전역인지 클립 단위인지 정의 없음.
22. vim의 Shift+방향키·Alt·Ctrl 조합. 5.8.1은 「`Shift`가 선택 넓히기 대신 노트 길이·옥타브를 맡는다」고 하나 화살표 키(5.7 「`hjkl`과 화살표 키는 모든 조합에서 같은 뜻」)에도 적용되는지, `Ctrl`·`Alt` 체계는 그대로인지 명시가 없다. 카탈로그는 기본 키를 상속한다고 가정했다.
23. 텍스트 객체. 5.8.1 「오퍼레이터(`d` `y` `c`)와 텍스트 객체(`dac` `yib` `vin`)」, 5.8.1 `d` 행 「`dw`, `dac`」. 나온 예(`ac` `ib` `in`)가 전부이며 객체 목록·모션(`dw`의 `w`=다음 노트까지)·`vin`의 `in` 의미가 정의되지 않았다. `object::*` 3개는 예시에서만 도출했다.
24. 수식키 단독 바인딩. 5.4 「서스테인 `Shift`(CC64 송신)」는 수식키 단독을 키로 쓰는데 GPUI keystroke 문법으로 수식키만 바인딩할 수 있는지 REQ에 근거가 없다(`play::Sustain` 키를 `shift`로 적음). 같은 모드에서 `Shift+음`이 무엇인지도 5.4에 없다.
25. 기호 키의 표기. `+` `>` `<` `?` `:` `~` `{` `}` `(` `)`는 `Shift`가 필요한 키인데 REQ는 5.7·5.8에서 `shift-` 접두 없이 기호로 쓰고, `Shift+3`(점4분)처럼 숫자는 `Shift`로 쓴다. KM-7(물리 키 매칭)에서 이들을 어떤 keystroke 문자열로 쓸지(`shift-=`인지 `+`인지) 정해야 한다. 또 `Alt`+글자는 macOS에서 특수문자를 내는 문제를 KM-11이 언급하나 기본 프리셋은 Alt를 그대로 쓴다.
26. 5.6 숫자 의미의 시간 의존. 같은 `0`–`9`가 길이 코드(앞)와 옥타브(음 키 뒤)로 쓰이고 「음 키가 들어온 뒤 옥타브 숫자를 기다리는 시간은 기본 300 ms」로 구분한다. `0`은 길이 복귀인데 음 뒤의 `0`은 옥타브 0이다. 타이밍 의존 규칙이라 컨텍스트 `Step > Pending`을 가칭으로 도입했다.
27. 계이름 레이아웃의 글자 겹침. 5.1 「`c d e f g a b` `c+` `c-` `o5` `l8` `r`」 MML 입력은 글자 키 `b`(음 b vs 내림표), `o` `l` `r`(옥타브·길이·쉼표 토큰)를 모두 쓰는데, 5.6은 「`n`은 두 레이아웃 모두에서 비어 있는 키」라고만 하고 `o` `l` `r`의 역할과 `l8`의 `8`(5.6 길이 코드 `1`–`7` 밖)을 정의하지 않는다. 숫자 줄 규칙(5.6)과 MML 숫자 규칙(5.14 「음 뒤의 숫자가 길이」)이 다르다는 점은 REQ가 인정했다.
28. 드럼 트랙의 키. 5.13 표의 건반 레이아웃 키 `R`(ph)은 일반 건반 레이아웃(P3: `W E T Y U O P`)에 없고, 반대로 `P`는 드럼에 쓰이지 않는다. 드럼에서 `[` `]`는 옥타브 대신 킷 페이지를 넘기는데 `Z`/`X`(5.1의 옥타브 별칭)의 역할은 적혀 있지 않다. 계이름 레이아웃에서 드럼 라벨(`bd` `sd` …)을 글자 키로 칠 때 라벨 확정 규칙(`b` `d` 뒤 글자)이 없다.
29. Sequence 모드의 `q`. 5.2 「`q`로 다른 설정을 수동 재적용」은 테이크가 끝난 뒤 Normal에서의 `q`(5.8)와 같은 `edit::Quantize`인지 Sequence 컨텍스트 전용 키인지 불명확하다. 또 5.2 「아무 키(기본 `Space`)」는 타격 키를 바꿀 수 있다는 뜻이나 설정 UI가 없다.
30. 모드 순환의 범위. `Ctrl+Tab`은 Normal → Step → Sequence → Play만 순환한다(5.5, 5.11). AI·Lyric·Controller 모드는 순환에 없고, Step에서 `Cmd+R`·`Cmd+K`를 직접 눌렀을 때(5.5 다이어그램은 Normal에서만 표시) 동작도 불명확하다.
31. 배치 뷰의 `Ctrl+Shift` · 예약 칸. 5.10 표에는 `Ctrl+Shift` 행이 없어 배치 뷰의 큰 단위 선택 넓히기가 정의되지 않았고, 「(예약)」 칸(`Alt+Shift+j/k`, `Alt+Ctrl+j/k`)은 Action이 없다. 또 같은 표 「`Ctrl+h/l`이 마디 단위」(비고)와 `Ctrl` 행 「한 마디 뒤/앞」이 중복이고, `Ctrl+j/k`(맨 위/맨 아래 트랙)가 5.7의 `Cmd+↑/↓`(첫/마지막 트랙)와 같은 동작이다.
32. Tab의 포커스 기준. 5.11은 Normal에서 `Tab`=다음 트랙, 컨트롤러 레인에서 `Tab`=다음 레인(5.9도 「`Tab`/`Shift+Tab` 또는 헤더 클릭으로 전환」)이라 한다. 레인이 펼쳐져 있되 포커스가 피아노롤에 있을 때 `Tab`이 어느 쪽인지가 컨텍스트 `Lane` 포커스 규칙(KM-1 「포커스 패널」)에 달려 있으나 포커스 이동 방법이 정의돼 있지 않다.
33. 7.3 예와 5.10의 컨텍스트. 7.3 예는 `Normal`에 `note::SetLength(sixteenth)`(`5`)·`note::Lengthen`(`alt-shift-l`)을 두고 `Normal > Arrangement`에서 `clip::Lengthen`으로 덮는다. 배치 뷰에서 `5`(길이 코드)·`alt-k`(노트 올리기 대신 클립 이동)처럼 `Normal`에 걸린 노트 전용 키가 Arrangement에서 무엇을 하는지(무시인지 클립 대상인지)는 5.10에 일부만 적혀 있다. 카탈로그는 노트 관련 키를 `Normal > PianoRoll`에 걸었다.
34. 반복 계열 이름. `r`=「선택을 바로 뒤에 반복 복제」(`edit::Repeat`, 7.3)이고 `.`=「마지막 편집 반복」(5.8)이다. 7.3은 `edit::Repeat`만 주므로 `.`의 Action 이름은 카탈로그가 `edit::RepeatLast`로 새로 붙였다.
35. 메뉴 항목 대응. KM-13은 메뉴바를 「파일·편집·보기·트랙·재생·도움말」 6개로 쓰고, 7.1·7.6은 AI를 더한 7개다. 또 7.6 「메뉴 항목은 Action과 1:1」인데 「줌」「테마」「포트」「채널」「최근 파일」「배치 뷰」(보기)·「녹음」(재생)은 항목 하나에 Action이 여럿이거나(줌 인/아웃/초기화) 다른 Action과 겹친다(배치 뷰=`view::Toggle`, 녹음=`mode::Sequence`). `Cmd+,` `Cmd+N/O/S` `Cmd+W` `Cmd+Q`와 설정·종료·창 닫기는 7개 메뉴에 항목이 없다(macOS 앱 메뉴로 추정).
36. 팔레트 명령 이름. 7.6은 「팔레트 명령은 메뉴 항목 이름과 같다」(한글: 「어시스트 켜기/끄기」「프롬프트 실험실」)인데 5.12·AI-11은 `assist on/off`, 5.12·AI-2는 `:ai`, 12.2는 `:test`처럼 영문 명령도 쓴다. 팔레트 검색이 Action 이름(`ai::ToggleAssist`)·한글 라벨·영문 별칭 중 무엇을 기준으로 하는지(UI-2 「모든 Action을 이름으로 검색」) 정해야 한다.
37. 팔레트·키맵 편집의 `Enter` / `Esc`. 5.11은 팔레트에서 `Tab`(자동완성 확정)·`Shift+Tab`(이전 후보)만 정의하고 실행·닫기 키는 적지 않았다(`palette::Run` `Close` 미등록). 키맵 편집은 KM-3·KM-8로 `Enter`/`Backspace`/`r`/`Esc`가 정의되나, 이 `r`·`Backspace`가 항목 검색 입력과 충돌하는지(검색창 포커스 vs 목록 포커스)는 불명확하다.
38. 텍스트 필드의 undo. KM-14 「`Cmd+Z`/`Cmd+Shift+Z`는 텍스트 입력 중에는 그 텍스트 필드의 undo를 따른다」. 가사 모드(IME 입력)에서 `Cmd+Z`가 노트 단위 undo(LYR-3 「되돌리기는 노트 하나 단위」)인지 필드 undo인지 겹친다.
39. 레인 키. 5.9 「키보드는 보조: 레인 열기·전환·줌·되돌리기·모드 전환만 키로 한다」에서 「줌」이 `view::Zoom*`인지 레인 전용 줌인지 불명확하다. 또 5.9 레인 위 마우스 조작은 Action이 아니라 포인터 제스처로만 정의돼 있어 P1과 7.2 UI-9의 「키보드로 못 하는 편집은 컨트롤러 커브뿐」과 맞춘다.
40. 가사 분배·입력의 위치. 7.6은 「가사 모드: 메뉴 편집 > 가사 입력」「가사 분배: 메뉴 편집 > 가사 분배, `Cmd+V`」로 두었으나 7.6 말미 메뉴바 구성의 편집 메뉴 열거(「되돌리기·…·퀀타이즈·레가토」)에는 두 항목이 없다.

## 통계

namespace별 Action 수는 다음과 같다(`operator` 표에는 `object::*` 3개를 포함했다).

| namespace | 개수 |
| --- | --- |
| app | 1 |
| file | 9 |
| edit | 23 |
| view | 13 |
| mode | 14 |
| cursor | 30 |
| select | 19 |
| note | 18 |
| clip | 11 |
| track | 18 |
| transport | 16 |
| midi | 4 |
| step | 24 |
| sequence | 7 |
| play | 9 |
| lane | 5 |
| lyric | 7 |
| ai | 24 |
| search | 3 |
| operator (+ object 3) | 7 |
| keymap | 9 |
| palette | 3 |
| help | 5 |
| settings | 3 |
| window | 3 |
| 합계 | 285 (표 25개, object를 별도 namespace로 세면 26개) |

바인딩 수는 표의 키 칸을 기계적으로 세었다(스크립트: 백틱 토큰을 (컨텍스트, 키)로 분해해 중복 제거).

- default 키가 하나 이상 있는 Action: 178개. 서로 다른 default 바인딩((컨텍스트, 키) 쌍): 284개. 컨텍스트를 무시한 서로 다른 키: 153개.
- vim 열에 키가 있는 Action: 121개. 서로 다른 vim 바인딩((컨텍스트, 키) 쌍): 127개(ex 명령 `:new` `:e` `:w` `:q` `:set` `:ai`와 `m` + 문자 같은 합성 표기 포함).
- 오차 주의: `ctrl-` + 음 키, `shift-` + 음 키, `1`–`7`, `f1` … `f8`처럼 범위로 적은 칸은 토큰 단위로만 셌으므로 실제 키 수는 이보다 많다. 음 키(`step::Note`)는 레이아웃별 컨텍스트에 따라 중복 집계되었다. 키가 없는 Action(메뉴·인스펙터·팔레트로만 호출)은 default 107개, 메뉴 항목과 1:1로 연결된 Action은 표의 메뉴 열을 참조한다.
- 충돌·모호 항목: 40건(A. 충돌 13건, B. 모호 27건).
