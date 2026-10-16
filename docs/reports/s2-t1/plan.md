# Task 계획: S2-T1 GPUI 앱과 창 모델

- 브랜치: `s2/t1-app-window`
- 관련 REQ: 7.1(화면 구성), 7.2 UI-7(다크·라이트), UI-12(인스펙터 접기, `Cmd+I`), 9.1, 9.4(gpui-kit 컴포넌트), SEQ-7(트랙 색)
- 선행 Task: S1 전체(앱은 아직 MIDI를 쓰지 않는다), S0-T4(`Document`)
- 관련 스파이크: SP-1·SP-2(gpui-kit `=0.7.1`, `gpui-pre =0.3.8` 빌드 확인)

## 끝의 정의 (내 말로)

- `keyscore-app`이 gpui-kit(`=0.7.1`, GPUI는 gpui-kit이 고정한 `gpui-pre =0.3.8`)을 의존하고, 실행하면 창이 뜬다.
- 창 모델
  - 창 하나 = 프로젝트 하나다. 여러 창을 열 수 있다(「새 창」 Action).
  - 창의 루트 뷰 `ProjectWindow`가 문서 엔티티 `Entity<ProjectDocument>`를 가진다.
  - `ProjectDocument`는 `core::Document`(undo 엔진)를 감싼다. 밖에는 읽기(`project() -> &Project`)와 `apply(Edit)`, `undo`, `redo`만 내준다. `&mut Project`를 내주는 경로가 없다(리뷰 초점).
- 레이아웃(7.1)
  - 위에서부터: 트랜스포트 자리(빈 툴바), 가운데 행 [트랙 목록 | 메인 빈 영역 | 인스펙터], 하단 영역(레인·건반 자리), 상태줄.
  - 메뉴바는 macOS 시스템 메뉴다. 이 Task에서는 파일(새 창, 닫기), 보기(패널 토글, 테마), 앱(종료)만 둔다. 메뉴 7개 전체는 S2-T4다.
  - 트랙 목록·인스펙터는 `h_resizable`, 하단 영역은 `v_resizable`이다. 셋 다 접을 수 있다. Action은 `ToggleTrackList`, `ToggleInspector`(`Cmd+I`), `ToggleBottomPanel`이다.
  - 각 영역은 빈 자리표시자다. 내용은 S2-T5 이후 채운다.
- 테마
  - 다크·라이트·시스템을 따른다. `ThemeMode` 설정과 `SetTheme` Action이 있다.
  - 12색 트랙 팔레트 토큰 `track_color(index, cx) -> Hsla`
    - gpui-kit 테마의 색 12개(`red`, `green`, `blue`, `yellow`, `magenta`, `cyan`과 각 `_light`)에 대응한다.
    - 그래서 다크·라이트를 바꾸면 함께 바뀐다. 앱 코드에 원시 색값을 쓰지 않는다.
- 설정 저장
  - 패널 열림·크기, 테마 모드를 앱 설정 JSON에 저장하고 다음 실행에서 복원한다. 위치는 `~/Library/Application Support/KeyScore/settings.json`이다. 테스트는 경로를 주입한다.
  - 모르는 키는 보존하고, 없는 키는 기본값을 쓴다. 쓰기는 임시 파일 + rename이다.
  - 저장 시점: 패널 토글·크기 조절·테마 변경 뒤 디바운스(1 s)와 종료 때.

## 공개 인터페이스 초안

~~~rust
// keyscore-app (바이너리 + lib: 테스트가 lib을 쓴다)
pub mod settings {
    pub struct Settings { /* layout: LayoutSettings, theme: ThemeChoice, extra: Map */ }
    pub struct LayoutSettings { /* track_list_open/width, inspector_open/width, bottom_open/height */ }
    pub enum ThemeChoice { System, Light, Dark }
    impl Settings {
        pub fn load(path: &Path) -> (Settings, Vec<String>); // 없거나 깨진 파일은 기본값 + 경고
        pub fn save(&self, path: &Path) -> io::Result<()>;
    }
}
pub mod document { pub struct ProjectDocument { /* core::Document */ } }
pub mod window { pub struct ProjectWindow { /* document, layout state */ } }
pub mod palette { pub fn track_color(color: TrackColor, cx: &App) -> Hsla; }
pub mod actions { /* actions!(keyscore, [NewWindow, CloseWindow, Quit, ToggleTrackList, ToggleInspector, ToggleBottomPanel]); SetTheme(ThemeChoice) */ }
pub fn init(cx: &mut App, settings_path: PathBuf);
pub fn open_project_window(project: Project, cx: &mut App) -> WindowHandle<Root>;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `settings_round_trip_keeps_unknown_keys` | 단위 | 저장 → 읽기 같음, 모르는 키 보존 |
| 2 | `settings_missing_or_broken_file_falls_back_to_defaults` | 단위 | 없는 파일, 깨진 JSON, 범위 밖 크기 → 기본값과 경고 |
| 3 | `opening_window_shows_all_layout_regions` | UI | 창을 열면 트랜스포트·트랙 목록·메인·인스펙터·하단·상태줄 자리가 모두 있다(`test_support` id) |
| 4 | `toggle_actions_collapse_and_restore_panels` | UI | 세 토글 Action이 패널을 접고 펴며, 설정에 반영된다 |
| 5 | `panel_layout_is_restored_from_settings` | UI | 저장된 크기·열림으로 새 창이 열린다 |
| 6 | `theme_choice_switches_mode_and_palette` | UI | 다크·라이트 전환 시 테마 모드와 `track_color` 값이 바뀐다. 12색이 서로 다르다 |
| 7 | `each_window_owns_its_own_document` | UI | 창 둘의 문서가 독립이다. 한 창에서 편집·undo해도 다른 창은 그대로 |
| 8 | `document_changes_only_through_edits` | 단위 | `ProjectDocument`는 `apply`·`undo`·`redo`로만 바뀌고 undo 수가 맞다 |

## 불변식

- 모델 변경은 `ProjectDocument::apply`(core `Edit`)로만 한다. `Project`의 가변 참조를 밖에 내지 않는다.
- 앱 코드에 원시 색값(`rgb`, `hsla` 리터럴)이 없다. 테마 토큰만 쓴다.
- 설정 파일 쓰기는 원자적이다(임시 파일 + rename).

## 범위 밖

- 키맵 엔진(S2-T2), 모드 FSM(S2-T3), 메뉴 7개와 명령 팔레트(S2-T4), 트랜스포트·트랙 목록·인스펙터 내용(S2-T5), 설정 화면(S2-T6), 파일 열기·저장(S2-T7).
- 사용자 테마 JSON(UI-7, C), Ely 팔레트(9.4)도 범위 밖이다.

## 리스크·결정 요청

- 레이아웃은 `Resizable`로 짓는다. 9.4는 `Dock`도 들지만 `DockArea`는 패널을 탭으로 감싸는 재배치용 구조다. 7.1은 고정 배치라 쓰지 않는다. 구현 세부라 결정 요청은 없다.
- 12색 팔레트를 테마 토큰에 대응시키는 것은 흔한 방식을 고른 것이다. UI-8(색각 이상 대응 팔레트)은 S8에서 본다.
- GPUI 빌드가 무거워 게이트 시간이 늘어난다(첫 빌드 수 분).
- gpui-kit의 전이 의존 라이선스를 `cargo deny`로 확인한다. 허용 목록 밖이 나오면 결정 요청을 쓴다.
- `NOTICE`에 gpui-kit·GPUI(Apache-2.0) 고지를 더한다.
