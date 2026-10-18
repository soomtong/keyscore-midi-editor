//! 앱 Action 전체. 카탈로그(`docs/reports/s2-t2/action-catalog.md`)의 이름이 곧 GPUI Action 이름이다.
//!
//! 처리기가 없는 Action은 디스패치돼도 아무 일도 하지 않는다. 각 단계가 처리기를 단다.
//! 인자가 있는 Action의 키맵 표기 `ns::Name(arg)`는 JSON 문자열 하나(`"arg"`)로 바뀌어 들어오므로,
//! 인자 하나짜리는 튜플 구조체이고 인자 둘 이상은 이름 있는 필드 구조체(GPUI 표기 `["ns::Name", {...}]`)다.

// Action 파생 매크로가 unsafe 메서드(등록 코드)를 만들어 이 lint가 오탐한다. 직접 쓴 unsafe는 없다.
#![allow(clippy::unsafe_derive_deserialize)]

use serde::{Deserialize, Deserializer};

/// 인자 타입. 키맵 JSON에서 문자열로 오는 값을 받기 위한 것들이다.
pub mod args {
    use super::{Deserialize, Deserializer};

    /// 정수 인자. 키맵의 `ns::Name(3)` 표기가 만드는 문자열 `"3"`과 JSON 숫자 `3`을 모두 받는다.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, schemars::JsonSchema)]
    #[schemars(with = "String")]
    pub struct Int(pub i64);

    /// 실수 인자(템포, 길이 배율). 문자열 `"120.5"`와 JSON 숫자를 모두 받는다.
    #[derive(Clone, Copy, PartialEq, Debug, schemars::JsonSchema)]
    #[schemars(with = "String")]
    pub struct Real(pub f64);

    /// 생략할 수 있는 인자. GPUI는 인자 없는 바인딩을 빈 객체 `{}`로 넘기므로 `{}`와 `null`을 `None`으로 읽는다.
    #[derive(Clone, Copy, PartialEq, Debug, schemars::JsonSchema)]
    #[schemars(with = "Option<String>")]
    pub struct Opt<T>(pub Option<T>);

    impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Opt<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let value = serde_json::Value::deserialize(d)?;
            match &value {
                serde_json::Value::Null => Ok(Self(None)),
                serde_json::Value::Object(map) if map.is_empty() => Ok(Self(None)),
                _ => T::deserialize(value)
                    .map(|v| Self(Some(v)))
                    .map_err(serde::de::Error::custom),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw<N> {
        Num(N),
        Text(String),
    }

    impl<'de> Deserialize<'de> for Int {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            match Raw::<i64>::deserialize(d)? {
                Raw::Num(n) => Ok(Self(n)),
                Raw::Text(s) => s
                    .trim()
                    .parse()
                    .map(Self)
                    .map_err(|_| serde::de::Error::custom(format!("정수가 아니다: {s:?}"))),
            }
        }
    }

    impl<'de> Deserialize<'de> for Real {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let value = match Raw::<f64>::deserialize(d)? {
                Raw::Num(n) => n,
                Raw::Text(s) => s
                    .trim()
                    .parse()
                    .map_err(|_| serde::de::Error::custom(format!("숫자가 아니다: {s:?}")))?,
            };
            if value.is_finite() {
                Ok(Self(value))
            } else {
                Err(serde::de::Error::custom("유한한 수여야 한다"))
            }
        }
    }

    /// 테마 선택(UI-7). 사용자 테마 이름은 아직 받지 않는다.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ThemeKind {
        /// 시스템 외관을 따른다.
        System,
        /// 라이트 테마.
        Light,
        /// 다크 테마.
        Dark,
    }

    /// 길이 코드(REQ 5.6). 점음표는 이 코드를 `SetDottedLength`에 넘겨 표현한다.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum LengthCode {
        /// 온음표(`1`).
        Whole,
        /// 2분음표(`2`).
        Half,
        /// 4분음표(`3`).
        Quarter,
        /// 8분음표(`4`).
        Eighth,
        /// 16분음표(`5`).
        Sixteenth,
        /// 32분음표(`6`). 키맵 표기는 `thirtysecond`.
        ThirtySecond,
        /// 64분음표(`7`). 키맵 표기는 `sixtyfourth`.
        SixtyFourth,
    }

    /// 변화표 종류(계이름 레이아웃 전용, REQ 5.6).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum AccidentalKind {
        /// 올림표.
        Sharp,
        /// 내림표.
        Flat,
    }

    /// 루프 녹음에서 기존 노트를 다루는 방식(REQ 5.2).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum LoopMode {
        /// 기존 노트 위에 겹쳐 쓴다.
        Overdub,
        /// 기존 노트를 지우고 덮어 쓴다.
        Replace,
    }

    /// 컨트롤러 레인 종류(REQ 5.9). CC 번호는 `lane::AddCC`로 따로 정한다.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum LaneKind {
        /// 벨로시티.
        Velocity,
        /// 컨트롤 체인지.
        Cc,
        /// 피치 벤드.
        PitchBend,
    }

    /// AI 생성 종류(REQ 5.12).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum AiKind {
        /// 새로 만들기.
        New,
        /// 이어쓰기.
        Continue,
        /// 변주.
        Variation,
        /// 반주.
        Accompaniment,
        /// 화성.
        Harmony,
        /// 가사/멜로디.
        LyricMelody,
        /// 필.
        Fill,
        /// 고치기.
        Fix,
    }

    /// AI 생성 역할(REQ 5.12).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum AiRole {
        /// 멜로디.
        Melody,
        /// 코드.
        Chord,
        /// 베이스.
        Bass,
        /// 드럼.
        Drum,
        /// 보컬.
        Vocal,
    }

    /// 기본 모델을 지정하는 용도(AI-19).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum AiPurpose {
        /// 어시스트(고스트 제안).
        Assist,
        /// 생성(AI 패널).
        Generate,
    }

    /// 키맵 프리셋 이름(KM-2).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum PresetName {
        /// 기본 프리셋.
        Default,
        /// vim 프리셋.
        Vim,
    }

    /// 설정 화면의 구역(REQ 7.6).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum SettingsSection {
        /// 메트로놈.
        Metronome,
        /// MIDI 출력.
        MidiOutput,
        /// 입력.
        Input,
        /// AI.
        Ai,
        /// 키맵.
        Keymap,
    }

    /// 음 키 레이아웃(P3): 건반형 또는 계이름형.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum KeyLayout {
        /// 건반형 레이아웃.
        Piano,
        /// 계이름형 레이아웃.
        Solfege,
    }
}

use args::{
    AccidentalKind, AiKind, AiPurpose, AiRole, Int, KeyLayout, LaneKind, LengthCode, LoopMode, Opt,
    PresetName, Real, SettingsSection, ThemeKind,
};

/// `app::` Action.
pub mod app {
    gpui_kit::actions!(
        app,
        [
            /// 앱 종료(저장 안 된 변경은 macOS 시트로 확인)
            Quit,
        ]
    );
}

/// `file::` Action.
pub mod file {
    gpui_kit::actions!(
        file,
        [
            /// 새 프로젝트
            New,
            /// 프로젝트 열기
            Open,
            /// 저장
            Save,
            /// 다른 이름으로 저장
            SaveAs,
            /// SMF Type 0/1 가져오기
            ImportSmf,
            /// SMF 내보내기(클립 펼침, 7비트)
            ExportSmf,
            /// 계이름 텍스트(MML) 가져오기
            ImportText,
            /// 계이름 텍스트 내보내기
            ExportText,
        ]
    );

    /// 최근 파일 열기
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = file)]
    #[serde(deny_unknown_fields)]
    pub struct OpenRecent(pub String);
}

/// `edit::` Action.
pub mod edit {
    use super::Int;

    gpui_kit::actions!(
        edit,
        [
            /// 되돌리기(텍스트 필드 안에서는 필드 undo, KM-14)
            Undo,
            /// 다시 실행
            Redo,
            /// 잘라내기(노트·클립·레인 포인트)
            Cut,
            /// 복사(내부 구조 + 시스템 클립보드 SMF/MML)
            Copy,
            /// 커서 위치에 붙여넣기(SMF/MML 텍스트도 가져옴)
            Paste,
            /// 커서 뒤에 붙여넣기
            PasteAfter,
            /// 커서 앞에 붙여넣기
            PasteBefore,
            /// 선택 길이만큼 바로 뒤에 복제
            Duplicate,
            /// 선택 삭제(선택 없으면 커서 위 노트, 배치 뷰는 클립)
            Delete,
            /// 커서 앞 노트 삭제
            DeleteBefore,
            /// 커서부터 마디 끝까지 삭제
            DeleteToBarEnd,
            /// 마디 전체 복사
            CopyBar,
            /// 커서부터 마디 끝까지 변경(삭제 후 Step)
            ChangeToBarEnd,
            /// 선택을 바로 뒤에 반복 복제(누를 때마다 하나 더)
            Repeat,
            /// 마지막 편집 반복
            RepeatLast,
            /// 커서 위치에서 노트/클립 분할
            Split,
            /// 선택 노트 합치기(길이 연장), 배치 뷰는 인접 클립
            Join,
            /// 선택 퀀타이즈(현재 설정)
            Quantize,
            /// 퀀타이즈 설정 시트(그리드·강도·스윙)
            QuantizeSettings,
            /// 레가토 토글(끝을 다음 노트 시작에 붙임)
            Legato,
            /// 커서 노트 음높이 바꾸기(음 키 하나를 받음)
            SetPitch,
            /// 마우스 드래그 취소, 원위치
            CancelDrag,
        ]
    );

    /// 타이밍 무작위 ±N(우선순위 C)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = edit)]
    #[serde(deny_unknown_fields)]
    pub struct Humanize(pub Int);
}

/// `view::` Action.
pub mod view {
    use super::{Int, ThemeKind};

    gpui_kit::actions!(
        view,
        [
            /// 피아노롤 ↔ 배치 뷰
            Toggle,
            /// 트랙 인스펙터 패널 토글
            ToggleInspector,
            /// 하단 건반 뷰 표시 토글
            ToggleKeyboard,
            /// 줌 인
            ZoomIn,
            /// 줌 아웃
            ZoomOut,
            /// 줌 초기화
            ZoomReset,
            /// 선택에 맞춰 줌
            ZoomToSelection,
            /// 세로 줌 인
            ZoomVerticalIn,
            /// 세로 줌 아웃
            ZoomVerticalOut,
            /// 커서를 화면 중앙으로
            CenterCursor,
            /// 고대비 모드
            ToggleHighContrast,
            /// 트랙 목록 패널을 접거나 편다.
            ToggleTrackList,
            /// 하단 영역(레인·건반 뷰)을 접거나 편다.
            ToggleBottomPanel,
        ]
    );

    /// 다크/라이트/사용자 테마
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = view)]
    #[serde(deny_unknown_fields)]
    pub struct SetTheme(pub ThemeKind);

    /// 폰트 크기 조절
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = view)]
    #[serde(deny_unknown_fields)]
    pub struct SetFontSize(pub Int);
}

/// `mode::` Action.
pub mod mode {
    gpui_kit::actions!(
        mode,
        [
            /// 커서 위치에서 Step 모드 진입
            Step,
            /// 커서 노트 뒤에서 Step 진입
            StepAfterNote,
            /// 마디 처음에서 Step 진입
            StepAtBarStart,
            /// 마디 끝에서 Step 진입
            StepAtBarEnd,
            /// 덮어쓰기 Step 모드(기존 노트를 밀지 않고 교체)
            StepOverwrite,
            /// Sequence 모드(녹음, 카운트인 후 시작)
            Sequence,
            /// Play(건반 시뮬레이션) 모드
            Play,
            /// 가사 모드(커서 노트부터 음절 입력)
            Lyric,
            /// Controller 모드(레인 전체 화면)
            Controller,
            /// 모드 순환 Normal → Step → Sequence → Play
            Cycle,
            /// 모드 역순환
            CycleBack,
            /// Normal로 복귀
            Normal,
            /// 상태줄 모드 표시를 눌러 모드 메뉴 열기
            ShowMenu,
        ]
    );
}

/// `cursor::` Action.
pub mod cursor {
    gpui_kit::actions!(
        cursor,
        [
            /// 한 스텝 뒤로
            Left,
            /// 한 스텝 앞으로
            Right,
            /// 반음 아래
            Down,
            /// 반음 위
            Up,
            /// 한 마디 뒤로
            PrevBar,
            /// 한 마디 앞으로
            NextBar,
            /// 한 옥타브 아래
            OctaveDown,
            /// 한 옥타브 위
            OctaveUp,
            /// 다음 노트로
            NextNote,
            /// 이전 노트로
            PrevNote,
            /// 다음 클립으로
            NextClip,
            /// 이전 클립으로
            PrevClip,
            /// 클립 처음
            ClipStart,
            /// 클립 끝
            ClipEnd,
            /// 첫(맨 위) 트랙
            FirstTrack,
            /// 마지막(맨 아래) 트랙
            LastTrack,
            /// 이전 클립 경계(배치 뷰)
            PrevClipBoundary,
            /// 다음 클립 경계(배치 뷰)
            NextClipBoundary,
            /// 위 트랙으로(배치 뷰)
            TrackUp,
            /// 아래 트랙으로(배치 뷰)
            TrackDown,
            /// 다음 박 시작
            NextBeat,
            /// 이전 박 시작
            PrevBeat,
            /// 커서 노트의 끝
            NoteEnd,
            /// 마디 안 마지막 노트의 끝
            LastNoteEndInBar,
            /// 같은 음높이의 다음 노트
            NextSamePitch,
            /// 같은 음높이의 이전 노트
            PrevSamePitch,
            /// `f` 탐색 반복
            RepeatFind,
            /// `f` 탐색 반대 방향 반복
            RepeatFindReverse,
        ]
    );

    /// 마크 설정(`m a`)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = cursor)]
    #[serde(deny_unknown_fields)]
    pub struct SetMark(pub String);

    /// 마크로 점프(`'a`, `grave a`)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = cursor)]
    #[serde(deny_unknown_fields)]
    pub struct JumpMark(pub String);
}

/// `select::` Action.
pub mod select {
    gpui_kit::actions!(
        select,
        [
            /// 선택을 뒤로 한 스텝 넓힘
            ExtendLeft,
            /// 선택을 앞으로 한 스텝 넓힘
            ExtendRight,
            /// 반음 아래까지 넓힘
            ExtendDown,
            /// 반음 위까지 넓힘
            ExtendUp,
            /// 한 마디 뒤까지 넓힘
            ExtendBarBack,
            /// 한 마디 앞까지 넓힘
            ExtendBarForward,
            /// 한 옥타브 아래까지 넓힘
            ExtendOctaveDown,
            /// 한 옥타브 위까지 넓힘
            ExtendOctaveUp,
            /// 이전 클립까지 선택(배치 뷰)
            ExtendPrevClip,
            /// 다음 클립까지 선택(배치 뷰)
            ExtendNextClip,
            /// 위 트랙까지 선택(배치 뷰)
            ExtendTrackUp,
            /// 아래 트랙까지 선택(배치 뷰)
            ExtendTrackDown,
            /// 클립 안 전체(배치 뷰는 클립 전부) 선택
            All,
            /// 현재 마디 선택
            Bar,
            /// 클립 전체 선택
            Clip,
            /// 선택 해제
            Clear,
            /// 노트 단위 비주얼 선택
            VisualNote,
            /// 시간 범위 선택(세로 전체)
            VisualTime,
            /// 블록 선택(음높이 × 시간)
            VisualBlock,
        ]
    );
}

/// `note::` Action.
pub mod note {
    use super::{Int, LengthCode, Real};

    gpui_kit::actions!(
        note,
        [
            /// 선택 노트 반음 올림(선택 없으면 커서 노트)
            TransposeUp,
            /// 반음 내림
            TransposeDown,
            /// 한 스텝 앞당김
            NudgeEarlier,
            /// 한 스텝 미룸
            NudgeLater,
            /// 한 마디 앞당김
            NudgeBarEarlier,
            /// 한 마디 미룸
            NudgeBarLater,
            /// 끝을 한 스텝(현재 길이 단위) 늘림
            Lengthen,
            /// 끝을 한 스텝 줄임
            Shorten,
            /// 선택 노트 한 옥타브 올림
            OctaveUp,
            /// 선택 노트 한 옥타브 내림
            OctaveDown,
            /// 선택 길이를 기본값(4분)으로
            ResetLength,
            /// 커서 노트 정보 팝업(틱·길이·채널)
            ShowInfo,
            /// 커서 노트 듣기
            Audition,
        ]
    );

    /// 선택 길이를 코드로 지정(`whole` `half` `quarter` `eighth` `sixteenth` `thirtysecond` `sixtyfourth`)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = note)]
    #[serde(deny_unknown_fields)]
    pub struct SetLength(pub LengthCode);

    /// 선택 길이를 점음표로 지정
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = note)]
    #[serde(deny_unknown_fields)]
    pub struct SetDottedLength(pub LengthCode);

    /// 선택 영역 조옮김(임의 값)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = note)]
    #[serde(deny_unknown_fields)]
    pub struct TransposeBy(pub Int);

    /// 선택 영역 시간 이동(임의 값)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = note)]
    #[serde(deny_unknown_fields)]
    pub struct ShiftBy(pub Int);

    /// 선택 영역 길이 스케일
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = note)]
    #[serde(deny_unknown_fields)]
    pub struct ScaleLength(pub Real);
}

/// `clip::` Action.
pub mod clip {
    gpui_kit::actions!(
        clip,
        [
            /// 클립 안(피아노롤)으로 들어감
            Open,
            /// 선택 클립 한 마디 앞당김
            MoveEarlier,
            /// 선택 클립 한 마디 미룸
            MoveLater,
            /// 위 트랙으로 옮김(채널 다르면 경고)
            MoveUp,
            /// 아래 트랙으로 옮김
            MoveDown,
            /// 끝을 한 마디 늘림(비파괴 트림)
            Lengthen,
            /// 끝을 한 마디 줄임
            Shorten,
            /// 시작을 한 마디 앞으로
            TrimStartEarlier,
            /// 시작을 한 마디 뒤로
            TrimStartLater,
        ]
    );

    /// 클립 이름 지정
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = clip)]
    #[serde(deny_unknown_fields)]
    pub struct Rename(pub String);

    /// 클립 색 지정
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = clip)]
    #[serde(deny_unknown_fields)]
    pub struct SetColor(pub String);
}

/// `track::` Action.
pub mod track {
    use super::Int;

    gpui_kit::actions!(
        track,
        [
            /// 다음 트랙으로(시간 위치 유지, 선택 유지)
            Next,
            /// 이전 트랙으로
            Prev,
            /// 새 트랙 추가(비어 있는 가장 낮은 채널)
            New,
            /// 아래에 새 트랙
            NewBelow,
            /// 위에 새 트랙
            NewAbove,
            /// 현재 트랙 삭제
            Delete,
            /// 현재 트랙 뮤트 토글
            ToggleMute,
            /// 현재 트랙 솔로 토글
            ToggleSolo,
            /// 트랙 순서를 위로
            MoveUp,
            /// 트랙 순서를 아래로
            MoveDown,
            /// 트랙 목록 패널로 포커스
            FocusList,
        ]
    );

    /// 트랙 출력 포트 지정
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct SetPort(pub String);

    /// 트랙 채널 지정(1–16)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct SetChannel(pub Int);

    /// 뱅크·프로그램 지정
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct SetProgram {
        /// `bank` 인자.
        pub bank: Int,
        /// `program` 인자.
        pub program: Int,
    }

    /// 트랙 기본 벨로시티
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct SetDefaultVelocity(pub Int);

    /// 가사 언어(ko·en·ja…)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct SetLyricLang(pub String);

    /// 트랙 이름
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct Rename(pub String);

    /// 트랙 색
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = track)]
    #[serde(deny_unknown_fields)]
    pub struct SetColor(pub String);
}

/// `transport::` Action.
pub mod transport {
    use super::{Int, Opt, Real};

    gpui_kit::actions!(
        transport,
        [
            /// 재생/정지
            Toggle,
            /// 재생
            Play,
            /// 정지
            Stop,
            /// 일시정지
            Pause,
            /// 커서(배치 뷰는 커서 클립)부터 재생
            PlayFromCursor,
            /// 선택 영역(클립) 루프 재생
            LoopSelection,
            /// 루프 켜기/끄기
            ToggleLoop,
            /// 메트로놈 켜기/끄기
            ToggleMetronome,
            /// 탭 템포
            TapTempo,
            /// 패닉(전 포트·채널 All Notes Off + All Sound Off + 서스테인 해제)
            Panic,
        ]
    );

    /// 루프 구간 설정
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = transport)]
    #[serde(deny_unknown_fields)]
    pub struct SetLoopRange {
        /// `start` 인자.
        pub start: Int,
        /// `end` 인자.
        pub end: Int,
    }

    /// 마디·박 단위 위치 이동
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = transport)]
    #[serde(deny_unknown_fields)]
    pub struct GoTo {
        /// `bar` 인자.
        pub bar: Int,
        /// `beat` 인자.
        pub beat: Int,
    }

    /// 카운트인 0–4마디
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = transport)]
    #[serde(deny_unknown_fields)]
    pub struct SetCountIn(pub Int);

    /// 템포 입력
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = transport)]
    #[serde(deny_unknown_fields)]
    pub struct SetTempo(pub Opt<Real>);

    /// 박자표
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = transport)]
    #[serde(deny_unknown_fields)]
    pub struct SetTimeSignature {
        /// `num` 인자.
        pub num: Int,
        /// `den` 인자.
        pub den: Int,
    }

    /// 조성
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = transport)]
    #[serde(deny_unknown_fields)]
    pub struct SetKey(pub String);
}

/// `midi::` Action.
pub mod midi {
    gpui_kit::actions!(
        midi,
        [
            /// 가상 포트 추가(최대 16)
            AddVirtualPort,
            /// 연결 테스트 노트 송신
            SendTestNote,
        ]
    );

    /// 출력 포트 선택(트랜스포트 드롭다운)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = midi)]
    #[serde(deny_unknown_fields)]
    pub struct SelectOutputPort(pub String);

    /// 가상 포트 이름 변경
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = midi)]
    #[serde(deny_unknown_fields)]
    pub struct RenameVirtualPort(pub String);
}

/// `step::` Action.
pub mod step {
    use super::{AccidentalKind, Int, LengthCode};

    gpui_kit::actions!(
        step,
        [
            /// 셋잇단 접두(`n` + 음)
            Triplet,
            /// 길이를 기본값(4분)으로
            ResetLength,
            /// 옥타브 −1
            OctaveDown,
            /// 옥타브 +1
            OctaveUp,
            /// 쉼표(현재 길이, 커서 전진)
            Rest,
            /// 현재 길이 절반의 쉼표
            HalfRest,
            /// 한 스텝 뒤로 가며 그 자리 노트 삭제
            Back,
            /// 마디 처음까지 삭제
            BackToBarStart,
            /// 한 스텝 뒤로(삭제 없음)
            Prev,
            /// 조합 확정 후 다음 스텝(조합 없으면 커서만 전진, 화음 확정)
            Confirm,
            /// 조합 중인 입력 즉시 확정(전진 없음). 길이만 바꿀 때도 사용
            Commit,
            /// 타이: 직전 노트를 현재 길이만큼 늘림
            Tie,
            /// 입력 중인 음을 레가토로
            TieLegato,
            /// 입력을 마치고 마디 처음부터 들어보기
            PreviewFromBar,
            /// 드럼 킷 다음 페이지(GM 35–81)
            KitPageNext,
            /// 드럼 킷 이전 페이지
            KitPagePrev,
        ]
    );

    /// 음 입력(현재 길이·옥타브, 커서 전진). 드럼 트랙은 GM 라벨
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct Note(pub String);

    /// 전진 없이 같은 위치에 음을 겹침
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct Chord(pub String);

    /// 현재 길이의 절반으로 그 음 하나만(스티키 아님)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct HalfNote(pub String);

    /// 변화표(`sharp` `flat`), 계이름 레이아웃 전용
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct Accidental(pub AccidentalKind);

    /// 길이 코드 지정(스티키)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct SetLength(pub LengthCode);

    /// 점음표 길이(스티키)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct SetDottedLength(pub LengthCode);

    /// 음 키 직후 옥타브 숫자(0–9, 300 ms 대기)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct SetOctave(pub Int);

    /// 입력 중인 음만 옥타브 ±1(스티키 아님)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = step)]
    #[serde(deny_unknown_fields)]
    pub struct OctaveOnce(pub Int);
}

/// `sequence::` Action.
pub mod sequence {
    use super::LoopMode;

    gpui_kit::actions!(
        sequence,
        [
            /// 박자에 맞춰 타격(미리 적기: 열의 다음 노트에 타이밍·길이 적용)
            Hit,
            /// 열의 다음 음 건너뛰기
            Skip,
            /// 열의 이전 음으로 되감기
            Rewind,
            /// 이번 테이크 확정, 다음 루프
            ConfirmTake,
            /// 이번 테이크 버리고 다시
            DiscardTake,
        ]
    );

    /// 직접 모드: 키 레이아웃으로 음과 타이밍 동시 입력
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = sequence)]
    #[serde(deny_unknown_fields)]
    pub struct Note(pub String);

    /// 루프 녹음 겹쳐 쓰기/덮어 쓰기 선택
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = sequence)]
    #[serde(deny_unknown_fields)]
    pub struct SetLoopMode(pub LoopMode);
}

/// `play::` Action.
pub mod play {
    use super::Int;

    gpui_kit::actions!(
        play,
        [
            /// 서스테인(CC64) 송신
            Sustain,
            /// 다음 트랙의 출력(포트·채널)으로
            NextTrack,
            /// 이전 트랙의 출력으로
            PrevTrack,
            /// 프로그램 체인지 −1 송신
            ProgramPrev,
            /// 프로그램 체인지 +1 송신
            ProgramNext,
            /// 옥타브 −1
            OctaveDown,
            /// 옥타브 +1
            OctaveUp,
        ]
    );

    /// 누르는 동안 노트 송신(기록 없음)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = play)]
    #[serde(deny_unknown_fields)]
    pub struct Note(pub String);

    /// 트랙 1–8 직접 선택
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = play)]
    #[serde(deny_unknown_fields)]
    pub struct SelectTrack(pub Int);
}

/// `lane::` Action.
pub mod lane {
    use super::{Int, LaneKind};

    gpui_kit::actions!(
        lane,
        [
            /// 컨트롤러 레인 접기·펼치기
            Toggle,
            /// 다음 레인
            Next,
            /// 이전 레인
            Prev,
        ]
    );

    /// 레인 종류 선택(벨로시티·CC·피치 벤드)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = lane)]
    #[serde(deny_unknown_fields)]
    pub struct SelectKind(pub LaneKind);

    /// CC 레인 추가(번호)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = lane)]
    #[serde(deny_unknown_fields)]
    pub struct AddCC(pub Int);
}

/// `lyric::` Action.
pub mod lyric {
    gpui_kit::actions!(
        lyric,
        [
            /// 다음 노트로
            Next,
            /// 이전 노트로
            Prev,
            /// 멜리스마(`-`) 표시 후 다음 노트
            Melisma,
            /// 현재 노트 가사 지움, 비어 있으면 이전 노트로
            Backspace,
            /// 붙여넣은 문장을 음절 단위로 선택 노트에 분배
            Distribute,
            /// 가사 도우미: 다음 소절 가사 제안 요청(S)
            Suggest,
            /// 제안된 가사 고스트 수락
            AcceptSuggestion,
        ]
    );
}

/// `ai::` Action.
pub mod ai {
    use super::{AiKind, AiPurpose, AiRole};

    gpui_kit::actions!(
        ai,
        [
            /// AI 패널 열기(커서·선택·현재 트랙이 대상)
            OpenPanel,
            /// 프롬프트 전송
            Submit,
            /// 생성 결과 전체 수락(undo 한 단위)
            Accept,
            /// 결과 버림 / 요청 취소
            Dismiss,
            /// 같은 프롬프트로 재생성
            Regenerate,
            /// 프롬프트 수정 후 재생성
            EditPrompt,
            /// 다시 듣기
            Replay,
            /// 이전 후보(재생성 이력)
            PrevCandidate,
            /// 「전송 내용 보기」
            ShowSent,
            /// 응답 원문 보기
            ShowResponse,
            /// 어시스트 켜기/끄기
            ToggleAssist,
            /// 고스트를 다음 마디 끝까지 수락하고 커서 이동
            AcceptGhost,
            /// 고스트 무시하고 한 스텝 뒤로
            IgnoreGhost,
            /// 고스트만 지우고 Step에 머묾
            DismissGhost,
            /// 프롬프트 실험실
            OpenLab,
            /// 템플릿 폴더 열기
            OpenTemplateFolder,
            /// 수정한 과제 템플릿을 원본으로
            RevertTemplate,
            /// 실험실 격자 실행(픽스처 × 템플릿 × 모델)
            RunGrid,
            /// 실험실 듣기 비교 별표
            StarResult,
            /// 사용자 프로젝트 범위를 픽스처로 저장
            SaveFixture,
            /// 픽스처 불러오기
            LoadFixture,
        ]
    );

    /// 생성 종류 선택(새로 만들기·이어쓰기·변주·반주·화성·가사/멜로디·필·고치기)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = ai)]
    #[serde(deny_unknown_fields)]
    pub struct SetKind(pub AiKind);

    /// 역할 선택(멜로디·코드·베이스·드럼·보컬)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = ai)]
    #[serde(deny_unknown_fields)]
    pub struct SetRole(pub AiRole);

    /// 이 모델을 어시스트/생성 기본으로
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = ai)]
    #[serde(deny_unknown_fields)]
    pub struct SetDefaultModel {
        /// `purpose` 인자.
        pub purpose: AiPurpose,
        /// `model` 인자.
        pub model: String,
    }
}

/// `search::` Action.
pub mod search {
    gpui_kit::actions!(
        search,
        [
            /// 검색(`c#4`, 트랙 이름, 가사)
            Open,
            /// 다음 검색 결과
            Next,
            /// 이전 검색 결과
            Prev,
        ]
    );
}

/// `operator::` Action.
pub mod operator {
    use super::Int;

    gpui_kit::actions!(
        operator,
        [
            /// 삭제 오퍼레이터(`d w`, `d a c`)
            Delete,
            /// 복사 오퍼레이터(`y i b`)
            Yank,
            /// 변경 오퍼레이터(삭제 후 Step 모드)
            Change,
        ]
    );

    /// 숫자 카운트 접두(`3 l`)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = operator)]
    #[serde(deny_unknown_fields)]
    pub struct Count(pub Int);
}

/// `object::` Action.
pub mod object {
    gpui_kit::actions!(
        object,
        [
            /// 텍스트 객체 `a c` (예시로만 등장)
            AroundClip,
            /// 텍스트 객체 `i b` (예시로만 등장)
            InnerBar,
            /// 텍스트 객체 `i n` (예시로만 등장)
            InnerNote,
        ]
    );
}

/// `keymap::` Action.
pub mod keymap {
    use super::PresetName;

    gpui_kit::actions!(
        keymap,
        [
            /// 키맵 편집 화면(설정 > 키맵) 열기
            Edit,
            /// 항목에서 키 직접 눌러 바인딩 지정 시작
            StartCapture,
            /// 키 캡처 취소
            CancelCapture,
            /// 바인딩 해제
            ClearBinding,
            /// 프리셋 기본값 복원
            RestoreDefault,
            /// 키맵 JSON 내보내기
            Export,
            /// 키맵 JSON 가져오기(충돌 검사 후 적용)
            Import,
            /// 수식키 교환 옵션(`Alt` ↔ `Cmd` 등)
            SwapModifiers,
            /// 키맵 편집 목록의 다음 항목(S2-T6a)
            SelectNext,
            /// 키맵 편집 목록의 이전 항목(S2-T6a)
            SelectPrev,
            /// 키맵 편집 검색란으로(S2-T6a)
            FocusSearch,
        ]
    );

    /// 프리셋 `default` / `vim` 선택
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = keymap)]
    #[serde(deny_unknown_fields)]
    pub struct SelectPreset(pub PresetName);
}

/// `palette::` Action.
pub mod palette {
    gpui_kit::actions!(
        palette,
        [
            /// 명령 팔레트 열기(모든 Action 검색·실행)
            Open,
            /// 자동완성 확정
            Complete,
            /// 이전 후보
            CompletePrev,
            /// 팔레트 닫기(A13: Esc = 통상의 취소)
            Close,
        ]
    );
}

/// `help::` Action.
pub mod help {
    gpui_kit::actions!(
        help,
        [
            /// 현재 모드의 키맵 오버레이
            KeymapOverlay,
            /// 오버레이 다음 페이지
            NextPage,
            /// 오버레이 이전 페이지
            PrevPage,
            /// 온보딩 다시 보기
            ReplayOnboarding,
            /// 문서 열기
            OpenDocs,
        ]
    );
}

/// `settings::` Action.
pub mod settings {
    use super::{KeyLayout, Opt, SettingsSection};

    gpui_kit::actions!(
        settings,
        [
            /// 레이턴시 보정 마법사(설정 > 입력)
            Calibrate,
            /// 설정 다음 구역(S2-T6a)
            NextSection,
            /// 설정 이전 구역(S2-T6a)
            PrevSection,
        ]
    );

    /// 설정 열기(`metronome` `midi_output` `input` `ai` `keymap`)
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = settings)]
    #[serde(deny_unknown_fields)]
    pub struct Open(pub Opt<SettingsSection>);

    /// 키 레이아웃 건반형/계이름형
    #[derive(
        Clone, PartialEq, Debug, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action,
    )]
    #[action(namespace = settings)]
    #[serde(deny_unknown_fields)]
    pub struct SetKeyLayout(pub KeyLayout);
}

/// `dialog::` Action(대화상자·시트, A13).
pub mod dialog {
    gpui_kit::actions!(
        dialog,
        [
            /// 대화상자 확인(기본 버튼)
            Confirm,
            /// 대화상자 취소(A13: Esc = 통상의 취소)
            Cancel,
        ]
    );
}

/// `window::` Action.
pub mod window {
    gpui_kit::actions!(
        window,
        [
            /// 창 닫기(저장 안 된 변경은 시트로 확인)
            Close,
            /// 다음 필드(키맵 편집·설정·대화상자)
            FocusNext,
            /// 이전 필드
            FocusPrev,
        ]
    );
}

/// 모든 Action 이름(정렬). 카탈로그 285개, S2-T1 패널 토글 2개(`view::ToggleTrackList`, `view::ToggleBottomPanel`),
/// 그 뒤 Task가 더한 것(`palette::Close`, S2-T6a의 `keymap::SelectNext`·`SelectPrev`·`FocusSearch`,
/// `settings::NextSection`·`PrevSection`, `dialog::Confirm`·`Cancel`)이다. `mode::Back`은 S2-T3에서 뺐다.
pub const ALL_ACTION_NAMES: &[&str] = &[
    "ai::Accept",
    "ai::AcceptGhost",
    "ai::Dismiss",
    "ai::DismissGhost",
    "ai::EditPrompt",
    "ai::IgnoreGhost",
    "ai::LoadFixture",
    "ai::OpenLab",
    "ai::OpenPanel",
    "ai::OpenTemplateFolder",
    "ai::PrevCandidate",
    "ai::Regenerate",
    "ai::Replay",
    "ai::RevertTemplate",
    "ai::RunGrid",
    "ai::SaveFixture",
    "ai::SetDefaultModel",
    "ai::SetKind",
    "ai::SetRole",
    "ai::ShowResponse",
    "ai::ShowSent",
    "ai::StarResult",
    "ai::Submit",
    "ai::ToggleAssist",
    "app::Quit",
    "clip::Lengthen",
    "clip::MoveDown",
    "clip::MoveEarlier",
    "clip::MoveLater",
    "clip::MoveUp",
    "clip::Open",
    "clip::Rename",
    "clip::SetColor",
    "clip::Shorten",
    "clip::TrimStartEarlier",
    "clip::TrimStartLater",
    "cursor::ClipEnd",
    "cursor::ClipStart",
    "cursor::Down",
    "cursor::FirstTrack",
    "cursor::JumpMark",
    "cursor::LastNoteEndInBar",
    "cursor::LastTrack",
    "cursor::Left",
    "cursor::NextBar",
    "cursor::NextBeat",
    "cursor::NextClip",
    "cursor::NextClipBoundary",
    "cursor::NextNote",
    "cursor::NextSamePitch",
    "cursor::NoteEnd",
    "cursor::OctaveDown",
    "cursor::OctaveUp",
    "cursor::PrevBar",
    "cursor::PrevBeat",
    "cursor::PrevClip",
    "cursor::PrevClipBoundary",
    "cursor::PrevNote",
    "cursor::PrevSamePitch",
    "cursor::RepeatFind",
    "cursor::RepeatFindReverse",
    "cursor::Right",
    "cursor::SetMark",
    "cursor::TrackDown",
    "cursor::TrackUp",
    "cursor::Up",
    "dialog::Cancel",
    "dialog::Confirm",
    "edit::CancelDrag",
    "edit::ChangeToBarEnd",
    "edit::Copy",
    "edit::CopyBar",
    "edit::Cut",
    "edit::Delete",
    "edit::DeleteBefore",
    "edit::DeleteToBarEnd",
    "edit::Duplicate",
    "edit::Humanize",
    "edit::Join",
    "edit::Legato",
    "edit::Paste",
    "edit::PasteAfter",
    "edit::PasteBefore",
    "edit::Quantize",
    "edit::QuantizeSettings",
    "edit::Redo",
    "edit::Repeat",
    "edit::RepeatLast",
    "edit::SetPitch",
    "edit::Split",
    "edit::Undo",
    "file::ExportSmf",
    "file::ExportText",
    "file::ImportSmf",
    "file::ImportText",
    "file::New",
    "file::Open",
    "file::OpenRecent",
    "file::Save",
    "file::SaveAs",
    "help::KeymapOverlay",
    "help::NextPage",
    "help::OpenDocs",
    "help::PrevPage",
    "help::ReplayOnboarding",
    "keymap::CancelCapture",
    "keymap::ClearBinding",
    "keymap::Edit",
    "keymap::Export",
    "keymap::FocusSearch",
    "keymap::Import",
    "keymap::RestoreDefault",
    "keymap::SelectNext",
    "keymap::SelectPreset",
    "keymap::SelectPrev",
    "keymap::StartCapture",
    "keymap::SwapModifiers",
    "lane::AddCC",
    "lane::Next",
    "lane::Prev",
    "lane::SelectKind",
    "lane::Toggle",
    "lyric::AcceptSuggestion",
    "lyric::Backspace",
    "lyric::Distribute",
    "lyric::Melisma",
    "lyric::Next",
    "lyric::Prev",
    "lyric::Suggest",
    "midi::AddVirtualPort",
    "midi::RenameVirtualPort",
    "midi::SelectOutputPort",
    "midi::SendTestNote",
    "mode::Controller",
    "mode::Cycle",
    "mode::CycleBack",
    "mode::Lyric",
    "mode::Normal",
    "mode::Play",
    "mode::Sequence",
    "mode::ShowMenu",
    "mode::Step",
    "mode::StepAfterNote",
    "mode::StepAtBarEnd",
    "mode::StepAtBarStart",
    "mode::StepOverwrite",
    "note::Audition",
    "note::Lengthen",
    "note::NudgeBarEarlier",
    "note::NudgeBarLater",
    "note::NudgeEarlier",
    "note::NudgeLater",
    "note::OctaveDown",
    "note::OctaveUp",
    "note::ResetLength",
    "note::ScaleLength",
    "note::SetDottedLength",
    "note::SetLength",
    "note::ShiftBy",
    "note::Shorten",
    "note::ShowInfo",
    "note::TransposeBy",
    "note::TransposeDown",
    "note::TransposeUp",
    "object::AroundClip",
    "object::InnerBar",
    "object::InnerNote",
    "operator::Change",
    "operator::Count",
    "operator::Delete",
    "operator::Yank",
    "palette::Close",
    "palette::Complete",
    "palette::CompletePrev",
    "palette::Open",
    "play::NextTrack",
    "play::Note",
    "play::OctaveDown",
    "play::OctaveUp",
    "play::PrevTrack",
    "play::ProgramNext",
    "play::ProgramPrev",
    "play::SelectTrack",
    "play::Sustain",
    "search::Next",
    "search::Open",
    "search::Prev",
    "select::All",
    "select::Bar",
    "select::Clear",
    "select::Clip",
    "select::ExtendBarBack",
    "select::ExtendBarForward",
    "select::ExtendDown",
    "select::ExtendLeft",
    "select::ExtendNextClip",
    "select::ExtendOctaveDown",
    "select::ExtendOctaveUp",
    "select::ExtendPrevClip",
    "select::ExtendRight",
    "select::ExtendTrackDown",
    "select::ExtendTrackUp",
    "select::ExtendUp",
    "select::VisualBlock",
    "select::VisualNote",
    "select::VisualTime",
    "sequence::ConfirmTake",
    "sequence::DiscardTake",
    "sequence::Hit",
    "sequence::Note",
    "sequence::Rewind",
    "sequence::SetLoopMode",
    "sequence::Skip",
    "settings::Calibrate",
    "settings::NextSection",
    "settings::Open",
    "settings::PrevSection",
    "settings::SetKeyLayout",
    "step::Accidental",
    "step::Back",
    "step::BackToBarStart",
    "step::Chord",
    "step::Commit",
    "step::Confirm",
    "step::HalfNote",
    "step::HalfRest",
    "step::KitPageNext",
    "step::KitPagePrev",
    "step::Note",
    "step::OctaveDown",
    "step::OctaveOnce",
    "step::OctaveUp",
    "step::Prev",
    "step::PreviewFromBar",
    "step::ResetLength",
    "step::Rest",
    "step::SetDottedLength",
    "step::SetLength",
    "step::SetOctave",
    "step::Tie",
    "step::TieLegato",
    "step::Triplet",
    "track::Delete",
    "track::FocusList",
    "track::MoveDown",
    "track::MoveUp",
    "track::New",
    "track::NewAbove",
    "track::NewBelow",
    "track::Next",
    "track::Prev",
    "track::Rename",
    "track::SetChannel",
    "track::SetColor",
    "track::SetDefaultVelocity",
    "track::SetLyricLang",
    "track::SetPort",
    "track::SetProgram",
    "track::ToggleMute",
    "track::ToggleSolo",
    "transport::GoTo",
    "transport::LoopSelection",
    "transport::Panic",
    "transport::Pause",
    "transport::Play",
    "transport::PlayFromCursor",
    "transport::SetCountIn",
    "transport::SetKey",
    "transport::SetLoopRange",
    "transport::SetTempo",
    "transport::SetTimeSignature",
    "transport::Stop",
    "transport::TapTempo",
    "transport::Toggle",
    "transport::ToggleLoop",
    "transport::ToggleMetronome",
    "view::CenterCursor",
    "view::SetFontSize",
    "view::SetTheme",
    "view::Toggle",
    "view::ToggleBottomPanel",
    "view::ToggleHighContrast",
    "view::ToggleInspector",
    "view::ToggleKeyboard",
    "view::ToggleTrackList",
    "view::ZoomIn",
    "view::ZoomOut",
    "view::ZoomReset",
    "view::ZoomToSelection",
    "view::ZoomVerticalIn",
    "view::ZoomVerticalOut",
    "window::Close",
    "window::FocusNext",
    "window::FocusPrev",
];
