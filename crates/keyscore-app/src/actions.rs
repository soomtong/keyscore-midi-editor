//! 앱 Action. S2-T2(키맵 엔진)가 Action 정의를 한 곳에 모으기 전까지 S2-T1이 쓰는 것만 둔다.

gpui_kit::actions!(
    keyscore,
    [
        /// 새 창(빈 프로젝트)을 연다.
        NewWindow,
        /// 지금 창을 닫는다.
        CloseWindow,
        /// 앱을 끝낸다.
        Quit,
        /// 트랙 목록 패널을 접거나 편다.
        ToggleTrackList,
        /// 트랙 인스펙터를 접거나 편다(UI-12, `Cmd+I`).
        ToggleInspector,
        /// 하단 영역(레인·건반 뷰)을 접거나 편다.
        ToggleBottomPanel,
        /// 테마를 시스템 외관에 맞춘다.
        ThemeSystem,
        /// 라이트 테마.
        ThemeLight,
        /// 다크 테마.
        ThemeDark,
    ]
);
