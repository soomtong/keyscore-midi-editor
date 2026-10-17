//! S2-T3 모드 FSM(순수 상태 기계). 전이 표는 `docs/reports/s2-t3/plan.md`.

use std::collections::HashSet;

use keyscore_app::mode::{Mode, ModeEvent, ModeState, View};

/// 사건만으로 상태를 만든다(필드가 비공개라서). 만든 상태가 뜻한 대로인지 확인한다.
fn st(view: View, mode: Mode, selection: bool, ghost: bool) -> ModeState {
    let mut s = ModeState::INITIAL;
    if view == View::PianoRoll {
        s = s.apply(ModeEvent::OpenClip);
    }
    if mode != Mode::Normal {
        s = s.apply(ModeEvent::Enter(mode));
    }
    if selection {
        s = s.apply(ModeEvent::Select);
    }
    if ghost {
        s = s.apply(ModeEvent::ShowGhost);
    }
    assert_eq!(
        (s.view(), s.mode(), s.has_selection(), s.has_ghost()),
        (view, mode, selection, ghost),
        "만들 수 없는 상태"
    );
    s
}

use Mode::{Ai, Controller, Lyric, Normal, Play, Sequence, Step};
use ModeEvent as E;
use View::{Arrangement as Arr, PianoRoll as Pr};

// 전이 표 한 장을 한눈에 보이게 나누지 않는다.
#[allow(clippy::too_many_lines)]
#[rustfmt::skip]
#[test]
fn transition_table_matches_req_5_5() {
    let rows: Vec<(ModeState, ModeEvent, ModeState)> = vec![
        // 배치 뷰 ⇄ 피아노롤(5.10). 뷰가 바뀌면 선택을 비운다
        (ModeState::INITIAL, E::OpenClip, st(Pr, Normal, false, false)),
        (st(Arr, Normal, true, false), E::OpenClip, st(Pr, Normal, false, false)),
        (st(Pr, Normal, false, false), E::OpenClip, st(Pr, Normal, false, false)),
        (st(Arr, Play, false, false), E::OpenClip, st(Arr, Play, false, false)),
        (st(Arr, Normal, false, false), E::ToggleView, st(Pr, Normal, false, false)),
        (st(Pr, Normal, true, false), E::ToggleView, st(Arr, Normal, false, false)),
        (st(Pr, Step, false, false), E::ToggleView, st(Pr, Step, false, false)),
        (st(Arr, Ai, false, false), E::ToggleView, st(Arr, Ai, false, false)),
        // 모드 진입. 모드만 바뀌고 뷰·선택은 그대로(5.11)
        (st(Arr, Normal, false, false), E::Enter(Step), st(Arr, Step, false, false)),
        (st(Pr, Normal, true, false), E::Enter(Step), st(Pr, Step, true, false)),
        (st(Arr, Normal, true, false), E::Enter(Step), st(Arr, Step, true, false)),
        (st(Arr, Normal, false, false), E::Enter(Play), st(Arr, Play, false, false)),
        (st(Arr, Normal, true, false), E::Enter(Ai), st(Arr, Ai, true, false)),
        (st(Pr, Normal, false, false), E::Enter(Sequence), st(Pr, Sequence, false, false)),
        (st(Arr, Normal, false, false), E::Enter(Sequence), st(Arr, Sequence, false, false)),
        (st(Arr, Normal, false, false), E::Enter(Controller), st(Arr, Controller, false, false)),
        (st(Arr, Normal, false, false), E::Enter(Lyric), st(Arr, Lyric, false, false)),
        (st(Pr, Step, false, true), E::Enter(Play), st(Pr, Play, false, false)),
        (st(Pr, Step, true, true), E::Enter(Step), st(Pr, Step, true, true)),
        (st(Arr, Play, false, false), E::Enter(Normal), st(Arr, Normal, false, false)),
        // Normal로(선택·뷰 유지, 고스트는 Step과 함께 사라짐)
        (st(Pr, Step, true, true), E::ToNormal, st(Pr, Normal, true, false)),
        (st(Arr, Ai, false, false), E::ToNormal, st(Arr, Normal, false, false)),
        (st(Arr, Sequence, false, false), E::ToNormal, st(Arr, Normal, false, false)),
        (ModeState::INITIAL, E::ToNormal, ModeState::INITIAL),
        // AI 패널의 수락·닫기는 AI 모드에서만 Normal로(5.5)
        (st(Arr, Ai, true, false), E::LeaveAi, st(Arr, Normal, true, false)),
        (st(Arr, Play, false, false), E::LeaveAi, st(Arr, Play, false, false)),
        (st(Pr, Step, false, true), E::LeaveAi, st(Pr, Step, false, true)),
        // Esc 한 단계: 고스트 → 선택 → 모드 → 피아노롤 → 배치 뷰
        (st(Pr, Step, true, true), E::Back, st(Pr, Step, true, false)),
        (st(Pr, Step, true, false), E::Back, st(Pr, Step, false, false)),
        (st(Pr, Step, false, false), E::Back, st(Pr, Normal, false, false)),
        (st(Arr, Sequence, false, false), E::Back, st(Arr, Normal, false, false)),
        (st(Pr, Normal, true, false), E::Back, st(Pr, Normal, false, false)),
        (st(Pr, Normal, false, false), E::Back, st(Arr, Normal, false, false)),
        (st(Arr, Normal, true, false), E::Back, st(Arr, Normal, false, false)),
        (ModeState::INITIAL, E::Back, ModeState::INITIAL),
        (st(Arr, Play, true, false), E::Back, st(Arr, Play, false, false)),
        (st(Arr, Play, false, false), E::Back, st(Arr, Normal, false, false)),
        (st(Arr, Ai, false, false), E::Back, st(Arr, Normal, false, false)),
        (st(Pr, Lyric, false, false), E::Back, st(Pr, Normal, false, false)),
        // 선택·고스트
        (st(Pr, Normal, true, false), E::ClearSelection, st(Pr, Normal, false, false)),
        (st(Pr, Step, true, true), E::ClearSelection, st(Pr, Step, false, true)),
        (st(Pr, Step, false, true), E::DismissGhost, st(Pr, Step, false, false)),
        (ModeState::INITIAL, E::DismissGhost, ModeState::INITIAL),
        (ModeState::INITIAL, E::Select, st(Arr, Normal, true, false)),
        (st(Pr, Step, false, false), E::ShowGhost, st(Pr, Step, false, true)),
        (st(Arr, Step, false, false), E::ShowGhost, st(Arr, Step, false, true)),
        (st(Pr, Normal, false, false), E::ShowGhost, st(Pr, Normal, false, false)),
        (st(Arr, Play, false, false), E::ShowGhost, st(Arr, Play, false, false)),
    ];
    for (from, event, to) in rows {
        assert_eq!(from.apply(event), to, "{from:?} --{event:?}-->");
    }
}

#[rustfmt::skip]
#[test]
fn cycle_visits_normal_step_sequence_play() {
    let rows = [
        (st(Arr, Normal, false, false), E::Cycle, st(Arr, Step, false, false)),
        (st(Arr, Normal, true, false), E::Cycle, st(Arr, Step, true, false)),
        (st(Pr, Step, true, true), E::Cycle, st(Pr, Sequence, true, false)),
        (st(Pr, Sequence, false, false), E::Cycle, st(Pr, Play, false, false)),
        (st(Pr, Play, false, false), E::Cycle, st(Pr, Normal, false, false)),
        (st(Arr, Play, false, false), E::Cycle, st(Arr, Normal, false, false)),
        // 순환 밖 모드는 Normal처럼 센다
        (st(Arr, Ai, false, false), E::Cycle, st(Arr, Step, false, false)),
        (st(Arr, Controller, false, false), E::CycleBack, st(Arr, Play, false, false)),
        (st(Pr, Lyric, false, false), E::Cycle, st(Pr, Step, false, false)),
        (st(Arr, Normal, false, false), E::CycleBack, st(Arr, Play, false, false)),
        (st(Pr, Play, false, false), E::CycleBack, st(Pr, Sequence, false, false)),
        (st(Pr, Sequence, false, false), E::CycleBack, st(Pr, Step, false, false)),
        (st(Pr, Step, false, true), E::CycleBack, st(Pr, Normal, false, false)),
    ];
    for (from, event, to) in rows {
        assert_eq!(from.apply(event), to, "{from:?} --{event:?}-->");
    }
    // 한 바퀴: 모드 차례가 맞고, 다 돌면 처음 상태 그대로(뷰·선택 포함, 리뷰 1 제안 4)
    for start in ModeState::all().filter(|s| s.mode() == Normal) {
        let mut s = start;
        let mut seen = Vec::new();
        for _ in 0..4 {
            s = s.apply(E::Cycle);
            seen.push(s.mode());
            assert_eq!((s.view(), s.has_selection()), (start.view(), start.has_selection()));
        }
        assert_eq!(seen, [Step, Sequence, Play, Normal], "{start:?}");
        assert_eq!(s, start);
        let mut back = Vec::new();
        for _ in 0..4 {
            s = s.apply(E::CycleBack);
            back.push(s.mode());
        }
        assert_eq!(back, [Play, Sequence, Step, Normal], "{start:?}");
        assert_eq!(s, start);
    }
}

fn invariant(s: ModeState) -> bool {
    !s.has_ghost() || s.mode() == Step
}

#[test]
fn every_state_upholds_invariants_after_any_event() {
    let all: HashSet<ModeState> = ModeState::all().collect();
    // 뷰 2 × 모드 7 × 선택 2 = 28, Step의 고스트 +4
    assert_eq!(all.len(), 32);
    assert!(all.contains(&ModeState::INITIAL));
    assert!(all.iter().all(|s| invariant(*s)), "{all:?}");
    assert_eq!(ModeEvent::all().len(), 11 + Mode::ALL.len());
    for s in &all {
        for e in ModeEvent::all() {
            let next = s.apply(e);
            assert!(all.contains(&next), "{s:?} --{e:?}--> {next:?}");
        }
    }
    // 모든 상태가 시작 상태에서 닿는다
    let mut reached = HashSet::from([ModeState::INITIAL]);
    let mut frontier = vec![ModeState::INITIAL];
    while let Some(s) = frontier.pop() {
        for e in ModeEvent::all() {
            if reached.insert(s.apply(e)) {
                frontier.push(s.apply(e));
            }
        }
    }
    assert_eq!(reached, all);
}

#[test]
fn back_repeated_converges_to_arrangement_without_selection() {
    for start in ModeState::all() {
        let mut s = start;
        for _ in 0..4 {
            s = s.apply(E::Back);
        }
        assert_eq!(s, ModeState::INITIAL, "{start:?}");
        assert_eq!(s.apply(E::Back), ModeState::INITIAL);
    }
}

/// A13 선택지별 `escape` 바인딩이 상태(=키 컨텍스트)마다 부르는 사건.
fn escape(policy: char, s: ModeState) -> ModeEvent {
    match (policy, s.mode()) {
        // AI 컨텍스트의 escape는 프리셋이 이미 ai::Dismiss로 건다
        (_, Ai) => E::LeaveAi,
        ('a', _) | (_, Normal) => E::Back,
        ('c', Step) if s.has_ghost() => E::DismissGhost,
        _ => E::ToNormal,
    }
}

#[test]
fn escape_policies_converge_from_every_state() {
    for policy in ['a', 'b', 'c'] {
        for start in ModeState::all() {
            let mut s = start;
            for _ in 0..5 {
                s = s.apply(escape(policy, s));
            }
            assert_eq!(s, ModeState::INITIAL, "{policy}: {start:?}");
        }
    }
}

#[test]
fn context_path_per_state() {
    let rows: [(ModeState, &[&str]); 12] = [
        (ModeState::INITIAL, &["Normal", "Arrangement"]),
        (
            st(Arr, Normal, true, false),
            &["Normal", "Arrangement", "Selection"],
        ),
        (st(Pr, Normal, false, false), &["Normal", "PianoRoll"]),
        (
            st(Pr, Normal, true, false),
            &["Normal", "PianoRoll", "Selection"],
        ),
        (st(Pr, Step, false, false), &["Step"]),
        (st(Pr, Step, true, false), &["Step"]),
        (st(Pr, Step, true, true), &["Step", "Ghost"]),
        (st(Pr, Sequence, false, false), &["Sequence"]),
        (st(Arr, Play, true, false), &["Play"]),
        (st(Arr, Ai, false, false), &["AI"]),
        (st(Pr, Lyric, false, false), &["Lyric"]),
        (st(Arr, Controller, false, false), &["Controller"]),
    ];
    for (s, path) in rows {
        assert_eq!(s.context_path(), path, "{s:?}");
    }
}

#[test]
fn labels_name_every_mode() {
    let labels: Vec<_> = Mode::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(
        labels,
        [
            "NORMAL",
            "STEP",
            "SEQUENCE",
            "PLAY",
            "AI",
            "LYRIC",
            "CONTROLLER"
        ]
    );
}
