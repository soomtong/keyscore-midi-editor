//! 피아노롤의 선택·수식키 조작·노트 단위 이동·마크(S3-T2, REQ 5.3, 5.7, 5.8, 5.8.1, PRE-1, UI-11).
//!
//! 순수 계산은 `pianoroll::select`(선택 영역·탐색)와 `pianoroll::ops`(편집 계산)에 있고, 여기는 창 처리기다.
//! 선택은 `PianoRollState`가 들고 모드 FSM의 `Selection` 컨텍스트는 `sync_selection_mode`가 한곳에서 맞춘다.

use std::collections::BTreeSet;

use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::{Action, Context, Div, InteractiveElement as _, Pixels, Point, Stateful, Window};
use keyscore_core::{ClipId, Note, Tick, Track, TrackId};

use crate::actions::{cursor, note, operator, select};
use crate::app_state::AppSettings;
use crate::dispatch::run_action;
use crate::mode::{ModeEvent, View};
use crate::pianoroll::ops::{NoteOp, plan_note_op};
use crate::pianoroll::select as sel;
use crate::pianoroll::{Cursor, CursorMove, FindDirection, Mark, Selection};
use crate::window::{PopupKind, ProjectWindow};

/// 마크 이름(vim `m a` … `m z`).
pub(crate) const MARK_NAMES: &str = "abcdefghijklmnopqrstuvwxyz";

/// 음이름(`C4` = 60).
fn pitch_name(pitch: keyscore_core::Pitch) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let p = pitch.get();
    format!("{}{}", NAMES[usize::from(p % 12)], i16::from(p / 12) - 1)
}

/// 5.7 `Alt` 행의 Action인가(되돌리기 묶음을 끊지 않는 키 입력).
pub(crate) fn is_note_op_action(action: &dyn Action) -> bool {
    let a = action.as_any();
    a.is::<note::NudgeEarlier>()
        || a.is::<note::NudgeLater>()
        || a.is::<note::NudgeBarEarlier>()
        || a.is::<note::NudgeBarLater>()
        || a.is::<note::TransposeUp>()
        || a.is::<note::TransposeDown>()
        || a.is::<note::OctaveUp>()
        || a.is::<note::OctaveDown>()
        || a.is::<note::Lengthen>()
        || a.is::<note::Shorten>()
}

impl ProjectWindow {
    // ---------- 공통 ----------

    /// 현재 트랙을 읽어 `f`를 돌린다.
    fn with_track<R>(&self, cx: &Context<Self>, f: impl FnOnce(&Track) -> R) -> Option<R> {
        let doc = self.document.read(cx);
        let track = doc.project().track(self.current_track()?)?;
        Some(f(track))
    }

    /// 커서 틱을 품은 현재 트랙의 클립.
    fn clip_under_cursor(&self, cx: &Context<Self>) -> Option<(TrackId, ClipId)> {
        let track_id = self.current_track()?;
        let doc = self.document.read(cx);
        let track = doc.project().track(track_id)?;
        let clip = self.pianoroll.current_clip(Some(track))?;
        Some((track_id, clip))
    }

    /// `tick`을 품은 현재 트랙의 클립.
    fn clip_at(&self, tick: Tick, cx: &Context<Self>) -> Option<(TrackId, ClipId)> {
        let track_id = self.current_track()?;
        self.with_track(cx, |track| {
            track
                .clips()
                .find(|(_, c)| c.start() <= tick && tick.get() < c.end())
                .map(|(id, _)| (track_id, id))
        })
        .flatten()
    }

    /// 커서가 옮겨진 뒤 할 일: 뷰포트가 따라가고, 설정이 켜져 있으면 커서 노트를 들려준다. `keep_anchor`가
    /// 거짓이면(수식키 없는 이동) 다음 `Shift+이동`의 기준 칸을 지운다.
    pub(crate) fn cursor_moved(&mut self, keep_anchor: bool, cx: &mut Context<Self>) {
        if !keep_anchor {
            self.pianoroll.set_anchor(None);
        }
        self.pianoroll.ensure_cursor_visible();
        self.preview_cursor_note(cx);
        cx.notify();
    }

    /// 커서를 `to`로 옮기고 뒤처리를 한다.
    fn jump_cursor(&mut self, to: Cursor, cx: &mut Context<Self>) {
        self.pianoroll.set_cursor(to);
        self.cursor_moved(false, cx);
    }

    /// 되돌리기 묶음을 끊는다(다른 키·클릭이 끼었을 때).
    pub(crate) fn seal_history(&mut self, cx: &mut Context<Self>) {
        self.document.update(cx, |doc, _| doc.seal());
    }

    /// 커서 위의 노트(현재 클립).
    fn note_under_cursor(&self, cx: &Context<Self>) -> Option<(TrackId, ClipId, Note)> {
        let (track_id, clip_id) = self.clip_under_cursor(cx)?;
        let cursor = self.pianoroll.cursor();
        let doc = self.document.read(cx);
        let clip = doc.project().track(track_id)?.clip(clip_id)?;
        sel::note_under(clip, cursor).map(|n| (track_id, clip_id, n))
    }

    // ---------- 선택 ----------

    /// 선택 유무를 모드 FSM에 맞춘다(`Normal > PianoRoll > Selection` 컨텍스트, `Esc`=`select::Clear`).
    pub(crate) fn sync_selection_mode(&mut self, cx: &mut Context<Self>) {
        let has = !self.pianoroll.selection().is_empty();
        if has != self.mode_state().has_selection() {
            self.apply_selection_event(if has {
                ModeEvent::Select
            } else {
                ModeEvent::ClearSelection
            });
            cx.notify();
        }
    }

    /// 문서가 바뀐 뒤: 선택에서 클립에 더 없는 노트를 뺀다.
    pub(crate) fn selection_document_changed(&mut self, cx: &mut Context<Self>) {
        let mut selection = self.pianoroll.selection();
        if !selection.is_empty() {
            let changed = {
                let doc = self.document.read(cx);
                let clip = selection
                    .track()
                    .and_then(|t| doc.project().track(t))
                    .zip(selection.clip())
                    .and_then(|(track, id)| track.clip(id));
                selection.retain_existing(clip)
            };
            if changed {
                self.pianoroll.set_selection(selection);
                cx.notify();
            }
        }
        self.sync_selection_mode(cx);
    }

    /// 선택을 `notes`로 바꾸고 FSM을 맞춘다.
    fn replace_selection(
        &mut self,
        track: TrackId,
        clip: ClipId,
        notes: BTreeSet<Note>,
        cx: &mut Context<Self>,
    ) {
        let mut selection = Selection::default();
        selection.set(track, clip, notes);
        self.set_selection(selection, cx);
    }

    /// `Shift+이동`: 커서를 옮기고 기준 칸 ↔ 커서 칸 직사각형의 노트를 선택한다(5.7 「선택 넓히기」).
    fn extend(&mut self, mv: CursorMove, cx: &mut Context<Self>) {
        let before = self.pianoroll.cursor();
        let meter = self.document.read(cx).project().meter_map().clone();
        self.pianoroll.move_cursor(mv, &meter);
        let after = self.pianoroll.cursor();
        let anchor = self.pianoroll.anchor().unwrap_or(before);
        self.pianoroll.set_anchor(Some(anchor));
        let step = self.pianoroll.step_ticks();
        // 직사각형이 걸치는 클립: 기준 칸이 든 클립, 기준 칸이 클립 사이 틈이면 커서가 든 클립(리뷰 1 지적 2)
        let picked = self
            .clip_at(anchor.tick, cx)
            .or_else(|| self.clip_at(after.tick, cx))
            .and_then(|(track_id, clip_id)| {
                let doc = self.document.read(cx);
                let clip = doc.project().track(track_id)?.clip(clip_id)?;
                Some((
                    track_id,
                    clip_id,
                    sel::region_notes(clip, anchor, after, step),
                ))
            });
        match picked {
            Some((track_id, clip_id, notes)) => {
                self.replace_selection(track_id, clip_id, notes, cx);
            }
            None => self.set_selection(Selection::default(), cx),
        }
        self.cursor_moved(true, cx);
    }

    fn select_extend_left(
        &mut self,
        _: &select::ExtendLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::StepBack, cx);
    }

    fn select_extend_right(
        &mut self,
        _: &select::ExtendRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::StepForward, cx);
    }

    fn select_extend_up(&mut self, _: &select::ExtendUp, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(CursorMove::SemitoneUp, cx);
    }

    fn select_extend_down(
        &mut self,
        _: &select::ExtendDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::SemitoneDown, cx);
    }

    fn select_extend_bar_back(
        &mut self,
        _: &select::ExtendBarBack,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::BarBack, cx);
    }

    fn select_extend_bar_forward(
        &mut self,
        _: &select::ExtendBarForward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::BarForward, cx);
    }

    fn select_extend_octave_up(
        &mut self,
        _: &select::ExtendOctaveUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::OctaveUp, cx);
    }

    fn select_extend_octave_down(
        &mut self,
        _: &select::ExtendOctaveDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend(CursorMove::OctaveDown, cx);
    }

    /// 현재 클립의 노트 가운데 `pick`이 고른 것을 선택한다. 기준 칸은 지운다.
    fn select_in_current_clip(
        &mut self,
        pick: impl FnOnce(&keyscore_core::Clip, &keyscore_core::MeterMap) -> BTreeSet<Note>,
        cx: &mut Context<Self>,
    ) {
        self.pianoroll.set_anchor(None);
        let picked = self.clip_under_cursor(cx).and_then(|(track_id, clip_id)| {
            let doc = self.document.read(cx);
            let project = doc.project();
            let clip = project.track(track_id)?.clip(clip_id)?;
            Some((track_id, clip_id, pick(clip, project.meter_map())))
        });
        match picked {
            Some((track_id, clip_id, notes)) => {
                self.replace_selection(track_id, clip_id, notes, cx);
            }
            None => self.set_selection(Selection::default(), cx),
        }
    }

    /// `b`: 커서가 든 마디의 노트(5.3, 5.7).
    fn select_bar(&mut self, _: &select::Bar, _: &mut Window, cx: &mut Context<Self>) {
        let at = self.pianoroll.cursor().tick;
        self.select_in_current_clip(|clip, meter| sel::bar_notes(clip, meter, at), cx);
    }

    /// `Shift+b`: 클립 전체(5.7).
    fn select_clip(&mut self, _: &select::Clip, _: &mut Window, cx: &mut Context<Self>) {
        self.select_in_current_clip(|clip, _| clip.notes().iter().cloned().collect(), cx);
    }

    /// `Cmd+A`: 피아노롤에서는 클립 안 전체(7.5). 배치 뷰의 클립 전체 선택은 S4.
    ///
    /// 입력란에 포커스가 있으면 글자 전체 선택이다. 표준 키(`cmd-a`)가 입력란의 바인딩보다 우선하므로 여기서
    /// 전파를 이어 입력란 바인딩이 실행되게 한다(KM-14의 undo와 같은 태도).
    fn select_all(&mut self, _: &select::All, window: &mut Window, cx: &mut Context<Self>) {
        if Self::text_input_focused(window) || self.mode_state().view() != View::PianoRoll {
            cx.propagate();
            return;
        }
        self.select_in_current_clip(|clip, _| clip.notes().iter().cloned().collect(), cx);
    }

    /// `Esc`(`Normal > Selection`): 선택 해제. 커서·뷰는 그대로다(A13).
    fn select_clear(&mut self, _: &select::Clear, _: &mut Window, cx: &mut Context<Self>) {
        self.pianoroll.set_anchor(None);
        self.set_selection(Selection::default(), cx);
    }

    // ---------- Alt 조작 ----------

    /// 선택(없으면 커서 노트)에 `op`를 적용한다. 같은 조작의 연속은 undo 한 단위다(5.7).
    ///
    /// 선택이 가리키는 (트랙, 클립)이 커서의 현재 클립이 아니면 거부한다. 그 노트는 흐리게 그려지거나 화면에
    /// 없고 5.3은 「앞뒤 클립의 노트는 … 편집은 되지 않는다」다(리뷰 1 지적 1). 선택 자체는 남는다(5.11의
    /// 「선택을 유지한 채 다음 트랙」 흐름, `Esc`로 비운다).
    fn apply_note_op(&mut self, op: NoteOp, cx: &mut Context<Self>) {
        let selection = self.pianoroll.selection();
        let (track_id, clip_id, notes, on_cursor_note) = if selection.is_empty() {
            let Some((t, c, n)) = self.note_under_cursor(cx) else {
                self.say("커서 위에 노트가 없다(선택하거나 노트 위로 간다)", cx);
                return;
            };
            (t, c, vec![n], true)
        } else {
            let (Some(t), Some(c)) = (selection.track(), selection.clip()) else {
                return;
            };
            if self.clip_under_cursor(cx) != Some((t, c)) {
                self.say(
                    format!(
                        "{}: 선택이 현재 클립에 없다(그 클립으로 돌아가거나 Esc로 선택을 비운다)",
                        op.label()
                    ),
                    cx,
                );
                return;
            }
            (t, c, selection.notes().cloned().collect(), false)
        };
        let step = self.pianoroll.step_ticks();
        let planned = {
            let doc = self.document.read(cx);
            plan_note_op(doc.project(), track_id, clip_id, &notes, op, step)
        };
        let planned = match planned {
            Ok(Some(p)) => p,
            Ok(None) => {
                self.say(
                    format!("{}: 바뀌는 노트가 없다(한 스텝 이하)", op.label()),
                    cx,
                );
                return;
            }
            Err(e) => {
                self.say(format!("{}: {e}", op.label()), cx);
                return;
            }
        };
        let key = op.coalesce_key();
        let result = self.document.update(cx, |doc, cx| {
            let r = doc.apply_coalesced(planned.edits, key);
            if r.is_ok() {
                cx.notify();
            }
            r
        });
        if let Err(e) = result {
            self.say(format!("{}: {e}", op.label()), cx);
            return;
        }
        if on_cursor_note {
            if let Some((from, to)) = planned.moved.first() {
                self.follow_cursor_note(track_id, clip_id, from, to, cx);
            }
        } else {
            if let Some((from, to)) = planned.moved.first() {
                self.shift_anchor(from, to);
            }
            let notes: BTreeSet<Note> = planned.moved.into_iter().map(|(_, to)| to).collect();
            self.replace_selection(track_id, clip_id, notes, cx);
        }
        cx.notify();
    }

    /// 선택 없이 커서 노트에 조작한 뒤: 커서가 노트를 따라간다(연타가 같은 노트에 계속 적용되도록). 선택은 생기지
    /// 않는다. 길이를 줄여 커서가 노트 밖에 남으면 노트 안(끝 직전)으로 당긴다(리뷰 1 지적 3).
    fn follow_cursor_note(
        &mut self,
        track_id: TrackId,
        clip_id: ClipId,
        from: &Note,
        to: &Note,
        cx: &mut Context<Self>,
    ) {
        let clip_start = {
            let doc = self.document.read(cx);
            doc.project()
                .track(track_id)
                .and_then(|t| t.clip(clip_id))
                .map_or(0, |c| c.start().get())
        };
        let cursor = self.pianoroll.cursor();
        let delta = i128::from(to.start().get()) - i128::from(from.start().get());
        let moved_tick = i128::from(cursor.tick.get()) + delta;
        let abs_start = i128::from(clip_start) + i128::from(to.start().get());
        let abs_last = i128::from(clip_start) + i128::from(to.end()) - 1;
        let tick = moved_tick.clamp(abs_start, abs_last.max(abs_start));
        let new_cursor = Cursor {
            tick: Tick::new(u64::try_from(tick.max(0)).unwrap_or(u64::MAX)),
            pitch: to.pitch(),
        };
        if new_cursor != cursor {
            self.pianoroll.set_cursor(new_cursor);
            self.cursor_moved(false, cx);
        }
    }

    /// 선택에 조작한 뒤: 기준 칸도 노트와 함께 옮긴다. 그대로 두면 다음 `Shift+이동`이 낡은 직사각형으로 다시
    /// 계산해 방금 옮긴 노트를 선택에서 떨어뜨린다(리뷰 1 지적 4).
    fn shift_anchor(&mut self, from: &Note, to: &Note) {
        let Some(anchor) = self.pianoroll.anchor() else {
            return;
        };
        let dt = i128::from(to.start().get()) - i128::from(from.start().get());
        let dp = i16::from(to.pitch().get()) - i16::from(from.pitch().get());
        let tick = (i128::from(anchor.tick.get()) + dt).max(0);
        let pitch = (i16::from(anchor.pitch.get()) + dp).clamp(0, 127);
        let moved = Cursor {
            tick: Tick::new(u64::try_from(tick).unwrap_or(u64::MAX)),
            pitch: u8::try_from(pitch)
                .ok()
                .and_then(|p| keyscore_core::Pitch::new(p).ok())
                .unwrap_or(anchor.pitch),
        };
        self.pianoroll.set_anchor(Some(moved));
    }

    fn note_nudge_earlier(
        &mut self,
        _: &note::NudgeEarlier,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_note_op(NoteOp::NudgeEarlier, cx);
    }

    fn note_nudge_later(&mut self, _: &note::NudgeLater, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_note_op(NoteOp::NudgeLater, cx);
    }

    fn note_nudge_bar_earlier(
        &mut self,
        _: &note::NudgeBarEarlier,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_note_op(NoteOp::NudgeBarEarlier, cx);
    }

    fn note_nudge_bar_later(
        &mut self,
        _: &note::NudgeBarLater,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_note_op(NoteOp::NudgeBarLater, cx);
    }

    fn note_transpose_up(&mut self, _: &note::TransposeUp, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_note_op(NoteOp::TransposeUp, cx);
    }

    fn note_transpose_down(
        &mut self,
        _: &note::TransposeDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_note_op(NoteOp::TransposeDown, cx);
    }

    fn note_octave_up(&mut self, _: &note::OctaveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_note_op(NoteOp::OctaveUp, cx);
    }

    fn note_octave_down(&mut self, _: &note::OctaveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_note_op(NoteOp::OctaveDown, cx);
    }

    fn note_lengthen(&mut self, _: &note::Lengthen, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_note_op(NoteOp::Lengthen, cx);
    }

    fn note_shorten(&mut self, _: &note::Shorten, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_note_op(NoteOp::Shorten, cx);
    }

    // ---------- 노트·클립 단위 이동 ----------

    fn cursor_next_note(&mut self, _: &cursor::NextNote, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.pianoroll.cursor();
        if let Some(Some((_, to))) = self.with_track(cx, |t| sel::next_note(t, c)) {
            self.jump_cursor(to, cx);
        }
    }

    fn cursor_prev_note(&mut self, _: &cursor::PrevNote, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.pianoroll.cursor();
        if let Some(Some((_, to))) = self.with_track(cx, |t| sel::prev_note(t, c)) {
            self.jump_cursor(to, cx);
        }
    }

    fn cursor_next_clip(&mut self, _: &cursor::NextClip, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.pianoroll.cursor();
        if let Some(Some(tick)) = self.with_track(cx, |t| sel::next_clip_start(t, c.tick)) {
            self.jump_cursor(Cursor { tick, ..c }, cx);
        }
    }

    fn cursor_prev_clip(&mut self, _: &cursor::PrevClip, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.pianoroll.cursor();
        if let Some(Some(tick)) = self.with_track(cx, |t| sel::prev_clip_start(t, c.tick)) {
            self.jump_cursor(Cursor { tick, ..c }, cx);
        }
    }

    fn cursor_clip_start(&mut self, _: &cursor::ClipStart, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.pianoroll.cursor();
        let start = self.clip_under_cursor(cx).and_then(|(t, id)| {
            let doc = self.document.read(cx);
            Some(doc.project().track(t)?.clip(id)?.start())
        });
        if let Some(tick) = start {
            self.jump_cursor(Cursor { tick, ..c }, cx);
        }
    }

    fn cursor_clip_end(&mut self, _: &cursor::ClipEnd, _: &mut Window, cx: &mut Context<Self>) {
        let c = self.pianoroll.cursor();
        let step = self.pianoroll.step_ticks();
        let end = self.clip_under_cursor(cx).and_then(|(t, id)| {
            let doc = self.document.read(cx);
            Some(sel::clip_last_step(doc.project().track(t)?.clip(id)?, step))
        });
        if let Some(tick) = end {
            self.jump_cursor(Cursor { tick, ..c }, cx);
        }
    }

    /// vim `f`/`F`와 `;`/`,`: 같은 음높이의 다음·이전 노트.
    fn find_same_pitch(
        &mut self,
        direction: FindDirection,
        remember: bool,
        cx: &mut Context<Self>,
    ) {
        let c = self.pianoroll.cursor();
        let found = self.with_track(cx, |t| match direction {
            FindDirection::Forward => sel::next_same_pitch(t, c),
            FindDirection::Backward => sel::prev_same_pitch(t, c),
        });
        if remember {
            self.pianoroll.set_last_find(direction);
        }
        if let Some(Some(to)) = found {
            self.jump_cursor(to, cx);
        }
    }

    fn cursor_next_same_pitch(
        &mut self,
        _: &cursor::NextSamePitch,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.find_same_pitch(FindDirection::Forward, true, cx);
    }

    fn cursor_prev_same_pitch(
        &mut self,
        _: &cursor::PrevSamePitch,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.find_same_pitch(FindDirection::Backward, true, cx);
    }

    fn cursor_repeat_find(
        &mut self,
        _: &cursor::RepeatFind,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(direction) = self.pianoroll.last_find() {
            self.find_same_pitch(direction, false, cx);
        }
    }

    fn cursor_repeat_find_reverse(
        &mut self,
        _: &cursor::RepeatFindReverse,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(direction) = self.pianoroll.last_find() {
            let reverse = match direction {
                FindDirection::Forward => FindDirection::Backward,
                FindDirection::Backward => FindDirection::Forward,
            };
            self.find_same_pitch(reverse, false, cx);
        }
    }

    // ---------- 마크 ----------

    /// 마크 이름: a–z 한 글자.
    fn mark_name(name: &str) -> Option<char> {
        let mut chars = name.trim().chars();
        let c = chars.next()?.to_ascii_lowercase();
        (chars.next().is_none() && MARK_NAMES.contains(c)).then_some(c)
    }

    fn cursor_set_mark(
        &mut self,
        action: &cursor::SetMark,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = Self::mark_name(&action.0) else {
            self.say(format!("마크 이름은 a–z 한 글자다: {}", action.0), cx);
            return;
        };
        let Some(track) = self.current_track() else {
            self.say("마크를 둘 트랙이 없다", cx);
            return;
        };
        self.pianoroll.set_mark(
            name,
            Mark {
                track,
                cursor: self.pianoroll.cursor(),
            },
        );
        self.say(format!("마크 {name}: {}", self.describe_cursor(cx)), cx);
    }

    fn cursor_jump_mark(
        &mut self,
        action: &cursor::JumpMark,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = Self::mark_name(&action.0) else {
            self.say(format!("마크 이름은 a–z 한 글자다: {}", action.0), cx);
            return;
        };
        let Some(mark) = self.pianoroll.mark(name) else {
            self.say(format!("마크 {name}가 없다"), cx);
            return;
        };
        let exists = self.document.read(cx).project().track(mark.track).is_some();
        if !exists {
            self.say(format!("마크 {name}의 트랙이 없다"), cx);
            return;
        }
        if self.current_track() != Some(mark.track) {
            self.set_current_track(Some(mark.track), cx);
        }
        self.jump_cursor(mark.cursor, cx);
    }

    /// 상태줄용 커서 자리: 「마디.박 음이름」.
    fn describe_cursor(&self, cx: &Context<Self>) -> String {
        let c = self.pianoroll.cursor();
        let pos = self
            .document
            .read(cx)
            .project()
            .meter_map()
            .position(c.tick);
        format!(
            "{}.{} {}",
            pos.bar() + 1,
            pos.beat() + 1,
            pitch_name(c.pitch)
        )
    }

    /// 피아노롤 우클릭 메뉴(UI-11, 결정 `2026-10-17-vim-marks-visible-ui` a): 마크 설정·마크로 이동.
    /// 우클릭한 칸으로 커서를 옮긴다(MS-5와 같은 규칙).
    pub(crate) fn open_piano_roll_menu(
        &mut self,
        cell: Cursor,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.seal_history(cx);
        self.jump_cursor(cell, cx);
        self.open_popup_with(
            PopupKind::PianoRollMenu,
            at,
            window,
            cx,
            |menu, window, cx| {
                menu.submenu("마크 설정", window, cx, |menu, _, _| {
                    MARK_NAMES.chars().fold(menu, |menu, c| {
                        let name = c.to_string();
                        menu.item(PopupMenuItem::new(name.clone()).on_click(
                            move |_, window, cx| {
                                run_action(Box::new(cursor::SetMark(name.clone())), window, cx);
                            },
                        ))
                    })
                })
                .submenu("마크로 이동", window, cx, |menu, _, _| {
                    MARK_NAMES.chars().fold(menu, |menu, c| {
                        let name = c.to_string();
                        menu.item(PopupMenuItem::new(name.clone()).on_click(
                            move |_, window, cx| {
                                run_action(Box::new(cursor::JumpMark(name.clone())), window, cx);
                            },
                        ))
                    })
                })
            },
        );
    }

    // ---------- 미리듣기 ----------

    /// 설정 「이동 시 미리듣기」가 켜져 있으면 커서 위 노트를 들려준다(PRE-1). 실패는 이동마다 상태줄을 덮지
    /// 않게 창마다 한 번만 알린다(리뷰 1 제안 11).
    fn preview_cursor_note(&mut self, cx: &mut Context<Self>) {
        let settings = AppSettings::get(cx).preview.clone();
        if !settings.on_move {
            return;
        }
        let Some((track_id, _, note)) = self.note_under_cursor(cx) else {
            return;
        };
        let Some(track) = self.document.read(cx).project().track(track_id).cloned() else {
            return;
        };
        let result = self.sound_note(&track, note.pitch(), note.velocity(), settings.length(), cx);
        if let Some(message) = result.failure("미리듣기")
            && !self.preview_notice_shown
        {
            self.preview_notice_shown = true;
            self.say(format!("{message}(이 알림은 창마다 한 번만)"), cx);
        }
    }

    /// `note::Audition`(vim `enter`, 팔레트): 커서 노트를 설정과 무관하게 들려준다.
    fn note_audition(&mut self, _: &note::Audition, _: &mut Window, cx: &mut Context<Self>) {
        let Some((track_id, _, note)) = self.note_under_cursor(cx) else {
            self.say("커서 위에 노트가 없다", cx);
            return;
        };
        let Some(track) = self.document.read(cx).project().track(track_id).cloned() else {
            return;
        };
        let length = AppSettings::get(cx).preview.length();
        let result = self.sound_note(&track, note.pitch(), note.velocity(), length, cx);
        if let Some(message) = result.failure("미리듣기") {
            self.say(message, cx);
        }
    }

    /// `operator::Cancel`(A13): 오퍼레이터 대기와 카운트를 비운다. 오퍼레이터 상태는 S3-T3가 만든다.
    fn operator_cancel(_: &mut Self, _: &operator::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        crate::keymap::clear_count(cx);
        cx.notify();
    }

    /// 선택·조작·탐색·마크 처리기를 단다(`piano_roll_handlers`에서).
    pub(crate) fn selection_handlers(
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element
            .on_action(cx.listener(Self::select_extend_left))
            .on_action(cx.listener(Self::select_extend_right))
            .on_action(cx.listener(Self::select_extend_up))
            .on_action(cx.listener(Self::select_extend_down))
            .on_action(cx.listener(Self::select_extend_bar_back))
            .on_action(cx.listener(Self::select_extend_bar_forward))
            .on_action(cx.listener(Self::select_extend_octave_up))
            .on_action(cx.listener(Self::select_extend_octave_down))
            .on_action(cx.listener(Self::select_bar))
            .on_action(cx.listener(Self::select_clip))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::select_clear))
            .on_action(cx.listener(Self::note_nudge_earlier))
            .on_action(cx.listener(Self::note_nudge_later))
            .on_action(cx.listener(Self::note_nudge_bar_earlier))
            .on_action(cx.listener(Self::note_nudge_bar_later))
            .on_action(cx.listener(Self::note_transpose_up))
            .on_action(cx.listener(Self::note_transpose_down))
            .on_action(cx.listener(Self::note_octave_up))
            .on_action(cx.listener(Self::note_octave_down))
            .on_action(cx.listener(Self::note_lengthen))
            .on_action(cx.listener(Self::note_shorten))
            .on_action(cx.listener(Self::cursor_next_note))
            .on_action(cx.listener(Self::cursor_prev_note))
            .on_action(cx.listener(Self::cursor_next_clip))
            .on_action(cx.listener(Self::cursor_prev_clip))
            .on_action(cx.listener(Self::cursor_clip_start))
            .on_action(cx.listener(Self::cursor_clip_end))
            .on_action(cx.listener(Self::cursor_next_same_pitch))
            .on_action(cx.listener(Self::cursor_prev_same_pitch))
            .on_action(cx.listener(Self::cursor_repeat_find))
            .on_action(cx.listener(Self::cursor_repeat_find_reverse))
            .on_action(cx.listener(Self::cursor_set_mark))
            .on_action(cx.listener(Self::cursor_jump_mark))
            .on_action(cx.listener(Self::note_audition))
            .on_action(cx.listener(Self::operator_cancel))
    }
}
