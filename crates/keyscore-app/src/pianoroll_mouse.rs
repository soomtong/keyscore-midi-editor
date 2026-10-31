//! 피아노롤 마우스 선택(S3-T6a)과 드래그 이동·조절(S3-T6b), REQ 7.4. 규칙은
//! `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 2–3절·6절, 구현 단위는 `docs/reports/s3-t6a/plan.md`·
//! `docs/reports/s3-t6b/plan.md` 「끝의 정의」.
//!
//! 마우스는 키 조작과 같은 상태(`Selection`, 커서, 모드 FSM)와 같은 편집 경로(`edit::Delete` 처리기, 붙여넣기의
//! `plan_paste`, 5.7 `Alt` 조작의 `plan_shift`)를 쓴다. 여기에는 누른 자리를 그 상태로 바꾸는 일만 있다.

use std::collections::BTreeSet;
use std::rc::Rc;

use gpui_kit::{Context, Modifiers, Subscription, Window};
use keyscore_core::{ClipId, Note, Pitch, Tick, TrackId};

use crate::actions::edit;
use crate::app_state::AppSettings;
use crate::clipboard::ClipNotes;
use crate::dispatch::run_action;
use crate::mode::{Mode, View};
use crate::pianoroll::edit::plan_paste;
use crate::pianoroll::element::Press;
use crate::pianoroll::mouse::{self as m, Drag, Gesture, Grab, Grip};
use crate::pianoroll::ops::{plan_shift, plan_shift_copy};
use crate::pianoroll::{Cursor, Selection, tick_u};
use crate::window::ProjectWindow;

/// `keystroke`가 키맵에서 `edit::CancelDrag`에 묶인 키인가. 드래그 수식키(`Cmd`·`Alt`·`Shift`)를 뺀 모양도 본다.
fn cancels_drag(keystroke: &gpui_kit::Keystroke, cx: &gpui_kit::App) -> bool {
    let mut bare = keystroke.clone();
    bare.modifiers.platform = false;
    bare.modifiers.alt = false;
    bare.modifiers.shift = false;
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    keymap.bindings_for_action(&edit::CancelDrag).any(|b| {
        b.match_keystrokes(std::slice::from_ref(keystroke)) == Some(false)
            || b.match_keystrokes(std::slice::from_ref(&bare)) == Some(false)
    })
}

impl ProjectWindow {
    /// 누른 채 `edit::CancelDrag`의 키(기본 `escape`, 키맵 컨텍스트 「Dragging」)는 진행 중 마우스 조작만 취소한다
    /// (P2, 5.5 「가장 안쪽 하나만」). 이 창의 키만 본다. 키맵 엔진에 맡기지 않고 앞에서 가로채는 것은 vim 시퀀스
    /// 대기 등 다른 바인딩보다 먼저 받기 위해서다. 다만 키는 키맵에서 읽어 사용자가 바꿔 묶을 수 있다(KM-1, S3-T6b
    /// 리뷰 2 지적 2). 수식키를 쥐고 있어도 취소한다: `Alt` 복제·`Cmd` 자유·`Shift` 한 축 드래그는 수식키를 쥔 채
    /// 끈다(리뷰 1 지적 2). 진행 중 조작이 없으면 키를 그대로 흘려보낸다.
    pub(crate) fn intercept_mouse_keys(window: &Window, cx: &mut Context<Self>) -> Subscription {
        let me = cx.entity().downgrade();
        let handle = window.window_handle();
        cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != handle || !cancels_drag(&event.keystroke, cx) {
                return;
            }
            let taken = me
                .update(cx, |this, cx| this.cancel_piano_roll_gesture(true, cx))
                .unwrap_or(false);
            if taken {
                cx.stop_propagation();
            }
        })
    }

    /// `edit::CancelDrag`(팔레트 등 키 밖의 디스패치): 진행 중 마우스 조작을 취소한다.
    pub(crate) fn cancel_drag_action(
        &mut self,
        _: &edit::CancelDrag,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_piano_roll_gesture(true, cx);
    }

    /// 진행 중 마우스 조작을 취소한다. `restore`면 러버밴드가 누르기 전 선택으로 돌아간다(`Esc`). 문서가 바뀌어
    /// 취소할 때는 바뀐 문서에 맞춘 선택을 그대로 둔다. 취소했으면 참.
    pub(crate) fn cancel_piano_roll_gesture(
        &mut self,
        restore: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.live_gesture().is_none() {
            return false;
        }
        let gesture = {
            let mut inner = self.pianoroll.inner().borrow_mut();
            match inner.gesture.take() {
                Some(g) if g.is_live() => {
                    inner.gesture = Some(Gesture::Cancelled);
                    g
                }
                other => {
                    inner.gesture = other;
                    return false;
                }
            }
        };
        if restore && let Gesture::Band { before, .. } = gesture {
            self.set_selection(before, cx);
        }
        cx.notify();
        true
    }

    /// 마우스 편집(선택·러버밴드·더블클릭)을 하는가: Normal 모드의 피아노롤이고 트랙이 있다(결정 2절 1).
    fn mouse_edits(&self) -> bool {
        self.mode_state().mode() == Mode::Normal
            && self.mode_state().view() == View::PianoRoll
            && self.current_track().is_some()
    }

    /// 진행 중 조작. 마우스 편집을 할 수 없는 자리(다른 모드·뷰, 트랙 없음)면 조작을 버리고 `None`이다. 뗌은
    /// 피아노롤을 떠난 뒤 오지 않을 수 있어(배치 뷰에는 이 처리기가 없다) 끌기·뗌·`Esc` 모두 여기서 확인한다
    /// (리뷰 1 지적 1·2). 모드·뷰 전환(`apply_mode_event`)도 조작을 버린다.
    fn live_gesture(&mut self) -> Option<Gesture> {
        if !self.mouse_edits() {
            self.pianoroll.drop_gesture();
            return None;
        }
        self.pianoroll.inner().borrow().gesture.clone()
    }

    /// 왼쪽 단추 누름.
    pub fn piano_roll_press(&mut self, press: &Press, window: &mut Window, cx: &mut Context<Self>) {
        // 새 누름은 남은 조작을 버린다(뗌을 놓친 경우)
        self.pianoroll.inner().borrow_mut().gesture = None;
        let Some(cell) = press.cell else {
            self.focus_document(window, cx);
            return;
        };
        if !self.mouse_edits() {
            self.click_piano_roll_cell(cell, cx);
            self.focus_document(window, cx);
            return;
        }
        // 비주얼을 먼저 끝낸다. 비주얼 중이면 커서 이동이 선택을 기준 칸 ↔ 커서로 다시 계산한다.
        self.pianoroll.set_anchor(None);
        self.click_piano_roll_cell(cell, cx);
        self.focus_document(window, cx);
        let origin = (press.x, press.y);
        let mods = press.modifiers;
        let hit = self.note_under_cursor(cx);
        if press.click_count == 2 {
            match hit {
                Some((track, clip, note)) => {
                    self.replace_selection(track, clip, BTreeSet::from([note]), cx);
                    // 키 `x`와 같은 처리기. 창 엔티티를 빌린 채 디스패치하면 재진입이라 미룬다.
                    window.defer(cx, |window, cx| {
                        run_action(Box::new(edit::Delete), window, cx);
                    });
                }
                None => self.create_note_at(cell, mods.platform, cx),
            }
            cx.notify();
            return;
        }
        let gesture = match hit {
            Some((track, clip, note)) => {
                let grab = self.grab(track, clip, note.clone(), origin, cx);
                let selection = self.pianoroll.selection();
                let mut collapse = None;
                if mods.platform {
                    self.toggle_note(track, clip, note, cx);
                } else if mods.shift {
                    self.extend_by_time(track, clip, Some(&note), cell.tick, cx);
                } else if selection.contains(clip, &note) {
                    collapse = Some((track, clip, note));
                } else {
                    self.click_note(track, clip, note, cx);
                }
                Some(Gesture::Note {
                    origin,
                    collapse,
                    grab,
                })
            }
            None if mods.shift => {
                if let Some((track, clip)) = self.clip_under_cursor(cx) {
                    self.extend_by_time(track, clip, None, cell.tick, cx);
                } else {
                    self.set_selection(Selection::default(), cx);
                }
                None
            }
            None => Some(Gesture::Empty {
                origin,
                anchor: m::anchor_at(&self.pianoroll.viewport(), origin),
                add: mods.platform,
                before: self.pianoroll.selection(),
            }),
        };
        self.pianoroll.inner().borrow_mut().gesture = gesture;
        cx.notify();
    }

    /// 왼쪽 단추를 누른 채 움직임.
    pub fn piano_roll_drag(
        &mut self,
        at: (f32, f32),
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let gesture = self.live_gesture();
        match gesture {
            Some(Gesture::Note { origin, grab, .. }) if m::past_threshold(origin, at) => {
                // 끌었으면 떼어도 줄이지 않는다. 잡은 노트가 선택에 있으면 선택 전부를 옮긴다(결정 6절 2).
                let selection = self.pianoroll.selection();
                let gesture = if selection.track() == Some(grab.track)
                    && selection.contains(grab.clip, &grab.note)
                {
                    let sounded = grab.note.pitch();
                    Gesture::Move(Box::new(Drag {
                        grab,
                        notes: selection.notes().cloned().collect::<Vec<_>>().into(),
                        now: at,
                        modifiers,
                        key: None,
                        ghost: Rc::from([]),
                        blocked: false,
                        sounded,
                    }))
                } else {
                    Gesture::Note {
                        origin,
                        collapse: None,
                        grab,
                    }
                };
                self.pianoroll.inner().borrow_mut().gesture = Some(gesture);
                self.update_move(Some(at), Some(modifiers), cx);
            }
            Some(Gesture::Move(_)) => self.update_move(Some(at), Some(modifiers), cx),
            Some(Gesture::Empty {
                origin,
                anchor,
                add,
                before,
            }) if m::past_threshold(origin, at) => {
                self.update_band(anchor, at, add, before, cx);
            }
            Some(Gesture::Band {
                anchor,
                add,
                before,
                ..
            }) => self.update_band(anchor, at, add, before, cx),
            _ => {}
        }
    }

    /// 수식키가 바뀜(움직이지 않고 `Cmd`·`Shift`·`Alt`를 누르거나 뗌). 끄는 중이면 고스트를 다시 계산한다.
    pub fn piano_roll_modifiers(&mut self, modifiers: Modifiers, cx: &mut Context<Self>) {
        if let Some(Gesture::Move(_)) = self.live_gesture() {
            self.update_move(None, Some(modifiers), cx);
        }
    }

    /// 왼쪽 단추 뗌.
    pub fn piano_roll_release(
        &mut self,
        at: (f32, f32),
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        if let Some(Gesture::Move(_)) = self.live_gesture() {
            // 뗀 자리·수식키로 마지막 계산을 하고 그 계획을 적용한다(고스트와 같은 계산)
            self.update_move(Some(at), Some(modifiers), cx);
            let gesture = self.pianoroll.inner().borrow_mut().gesture.take();
            if let Some(Gesture::Move(drag)) = gesture {
                self.finish_move(&drag, cx);
            }
            cx.notify();
            return;
        }
        let gesture = self.live_gesture();
        self.pianoroll.inner().borrow_mut().gesture = None;
        match gesture {
            Some(Gesture::Note {
                collapse: Some((track, clip, note)),
                ..
            }) => self.click_note(track, clip, note, cx),
            Some(Gesture::Empty { add: false, .. }) => {
                self.set_selection(Selection::default(), cx);
            }
            Some(Gesture::Band {
                anchor,
                now,
                add,
                before,
            }) => {
                // 마지막 움직임 뒤 스크롤·줌이 있었어도 그린 직사각형과 같은 선택으로 끝낸다(리뷰 2 지적 1)
                self.update_band(anchor, now, add, before, cx);
                self.pianoroll.drop_gesture();
                self.pull_cursor_into_selection(cx);
            }
            _ => return,
        }
        cx.notify();
    }

    /// 휠·핀치로 뷰포트가 바뀐 뒤: 러버밴드면 포인터 자리(`now`)의 새 내용 위치로 선택을 다시 계산한다. 선택은
    /// 누른 점·지금 점·뷰포트로 정해지므로 뷰포트가 바뀌는 길에서도 같은 계산을 한다(리뷰 2 지적 1).
    pub fn piano_roll_view_changed(&mut self, cx: &mut Context<Self>) {
        match self.live_gesture() {
            Some(Gesture::Band {
                anchor,
                now,
                add,
                before,
            }) => self.update_band(anchor, now, add, before, cx),
            // 끄는 중이면 포인터 아래 내용이 바뀌었다(S3-T6b, 결정 6절 3)
            Some(Gesture::Move(_)) => self.update_move(None, None, cx),
            _ => {}
        }
    }

    /// 노트 위 누름이 잡은 것: 누른 x가 노트 화면 사각형의 어디인지(MS-6, 결정 6절 1).
    fn grab(
        &self,
        track: TrackId,
        clip: ClipId,
        note: Note,
        origin: (f32, f32),
        cx: &Context<Self>,
    ) -> Grab {
        let viewport = self.pianoroll.viewport();
        let clip_start = self
            .document
            .read(cx)
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .map_or(0, |c| c.start().get());
        #[allow(clippy::cast_precision_loss, reason = "틱은 화면 좌표로만 쓴다")]
        let start = (clip_start + note.start().get()) as f64;
        let x = viewport.x_of(start);
        let w = crate::pianoroll::px_f32(f64::from(note.length()) * viewport.px_per_tick);
        Grab {
            track,
            clip,
            grip: m::grip_at(x, w, origin.0),
            anchor: m::anchor_at(&viewport, origin),
            note,
        }
    }

    /// 마우스 스냅 격자 한 칸(설정 > 입력 「마우스 스냅 단위」, MS-3).
    fn mouse_grid(&self, cx: &Context<Self>) -> u64 {
        m::snap_grid(AppSettings::get(cx).input.mouse_snap, self.step.length)
    }

    /// 끄는 중인 이동·조절을 지금 점(`at`, 없으면 그대로)·수식키(없으면 그대로)로 다시 계산한다. 결과는 고스트이고,
    /// 놓을 때 같은 계획을 적용한다(결정 6절 3–9). 잡은 노트 고스트의 음높이가 바뀌면 들려준다(MS-4).
    fn update_move(
        &mut self,
        at: Option<(f32, f32)>,
        modifiers: Option<Modifiers>,
        cx: &mut Context<Self>,
    ) {
        let viewport = self.pianoroll.viewport();
        let mut drag = {
            let mut inner = self.pianoroll.inner().borrow_mut();
            match inner.gesture.take() {
                Some(Gesture::Move(drag)) => drag,
                other => {
                    inner.gesture = other;
                    return;
                }
            }
        };
        if let Some(at) = at {
            drag.now = at;
        }
        if let Some(modifiers) = modifiers {
            drag.modifiers = modifiers;
        }
        // 그리기 전(크기 0)에는 지금 점의 내용 위치를 알 수 없다
        if viewport.width > 0.0 && viewport.height > 0.0 && viewport.row_height > 0.0 {
            self.plan_move(&mut drag, cx);
        }
        let grabbed_now = Self::grabbed_ghost_pitch(&drag);
        let to_sound = grabbed_now.filter(|p| *p != drag.sounded);
        if let Some(pitch) = to_sound {
            drag.sounded = pitch;
        }
        let (track, velocity) = (drag.grab.track, drag.grab.note.velocity());
        self.pianoroll.inner().borrow_mut().gesture = Some(Gesture::Move(drag));
        if let Some(pitch) = to_sound
            && AppSettings::get(cx).preview.on_move
        {
            self.preview_note(track, pitch, velocity, cx);
        }
        cx.notify();
    }

    /// 끄는 중 잡은 노트 고스트의 음높이(본체 이동만). 계획이 거부돼도 고스트는 있다.
    fn grabbed_ghost_pitch(drag: &Drag) -> Option<Pitch> {
        if drag.grab.grip != Grip::Body {
            return None;
        }
        let shift = drag.key?.0;
        let pitch = i32::from(drag.grab.note.pitch().get()) + shift.pitch;
        u8::try_from(pitch).ok().and_then(|p| Pitch::new(p).ok())
    }

    /// `drag`의 지금 점·수식키로 변환을 정하고 계획을 고스트로 둔다. 변환이 직전과 같으면 그대로 둔다.
    fn plan_move(&self, drag: &mut Drag, cx: &Context<Self>) {
        let viewport = self.pianoroll.viewport();
        let now = m::anchor_at(&viewport, drag.now);
        let anchor = drag.grab.anchor;
        let ticks = now.tick - anchor.tick;
        #[allow(clippy::cast_possible_truncation, reason = "행은 0–127")]
        let semitones = anchor.row.floor() as i32 - now.row.floor() as i32;
        let mods = drag.modifiers;
        let axis = mods.shift.then(|| {
            #[allow(clippy::cast_possible_truncation, reason = "화면 거리")]
            let dx = (ticks * viewport.px_per_tick) as f32;
            m::dominant_axis(dx, (now.row - anchor.row) * viewport.row_height)
        });
        let grid = if mods.platform {
            0
        } else {
            self.mouse_grid(cx)
        };
        let copy = mods.alt && drag.grab.grip == Grip::Body;
        let doc = self.document.read(cx);
        let project = doc.project();
        let Some(clip) = project
            .track(drag.grab.track)
            .and_then(|t| t.clip(drag.grab.clip))
        else {
            return;
        };
        let clip_start = clip.start().get();
        let shift = m::drag_shift(&m::DragInput {
            meter: project.meter_map(),
            clip_start,
            notes: &drag.notes,
            grabbed: &drag.grab.note,
            grip: drag.grab.grip,
            ticks,
            semitones,
            grid,
            axis,
        });
        if drag.key == Some((shift, copy)) {
            return;
        }
        drag.key = Some((shift, copy));
        let (track, clip_id) = (drag.grab.track, drag.grab.clip);
        let planned = if copy {
            plan_shift_copy(project, track, clip_id, &drag.notes, shift)
        } else {
            plan_shift(project, track, clip_id, &drag.notes, shift)
        };
        drag.blocked = planned.is_err();
        drag.ghost = Rc::from(match planned {
            Ok(Some(p)) => p
                .moved
                .iter()
                .map(|(_, to)| (clip_start + to.start().get(), to.length(), to.pitch()))
                .collect(),
            Ok(None) => Vec::new(),
            // 거부돼도 노트마다의 결과는 보인다(다음 클립과 겹침은 편집 묶음 단계의 거부다)
            Err(_) => crate::pianoroll::ops::shifted(project, track, clip_id, &drag.notes, shift)
                .unwrap_or_default()
                .iter()
                .map(|(_, to)| (clip_start + to.start().get(), to.length(), to.pitch()))
                .collect(),
        });
    }

    /// 놓음: 마지막 계획을 한 번 적용한다(undo 한 단위, 결정 6절 10).
    fn finish_move(&mut self, drag: &Drag, cx: &mut Context<Self>) {
        let Some((shift, copy)) = drag.key else {
            return;
        };
        let (track, clip) = (drag.grab.track, drag.grab.clip);
        let label = match (drag.grab.grip, copy) {
            (Grip::Body, true) => "노트 복제",
            (Grip::Body, false) => "노트 옮기기",
            (Grip::Start, _) => "시작 조절",
            (Grip::End, _) => "길이 조절",
        };
        let planned = {
            let doc = self.document.read(cx);
            let project = doc.project();
            if copy {
                plan_shift_copy(project, track, clip, &drag.notes, shift)
            } else {
                plan_shift(project, track, clip, &drag.notes, shift)
            }
        };
        let planned = match planned {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(e) => {
                self.say(format!("{label}: {e}"), cx);
                return;
            }
        };
        if !self.apply_edit(label, planned.edits, None, cx) {
            return;
        }
        let grabbed = planned
            .moved
            .iter()
            .find(|(from, _)| *from == drag.grab.note)
            .map(|(_, to)| to.clone());
        let selected: BTreeSet<Note> = if copy {
            planned.moved.iter().map(|(_, to)| to.clone()).collect()
        } else {
            // 바뀐 노트는 새 값으로, 그대로 둔 노트(1틱 미만이 되는 길이 조절)는 그대로 선택에 남긴다
            let mut rest: Vec<Note> = drag.notes.to_vec();
            for (from, _) in &planned.moved {
                if let Some(i) = rest.iter().position(|n| n == from) {
                    rest.swap_remove(i);
                }
            }
            rest.into_iter()
                .chain(planned.moved.iter().map(|(_, to)| to.clone()))
                .collect()
        };
        if let Some(to) = &grabbed {
            self.pianoroll.inner().borrow_mut().last_clicked = Some((clip, to.clone()));
        }
        self.replace_selection(track, clip, selected, cx);
        // 본체 이동이면 커서가 잡은 노트를 따라간다(결정 6절 10)
        if drag.grab.grip == Grip::Body
            && let Some(to) = grabbed
        {
            let cursor = self.pianoroll.cursor();
            let dt = i128::from(to.start().get()) - i128::from(drag.grab.note.start().get());
            let dp = i32::from(to.pitch().get()) - i32::from(drag.grab.note.pitch().get());
            let tick = u64::try_from(i128::from(cursor.tick.get()) + dt).unwrap_or(0);
            let pitch = u8::try_from(i32::from(cursor.pitch.get()) + dp)
                .ok()
                .and_then(|p| Pitch::new(p).ok())
                .unwrap_or(cursor.pitch);
            self.pianoroll.set_cursor(Cursor {
                tick: Tick::new(tick),
                pitch,
            });
            self.cursor_moved_quietly(false, cx);
        }
    }

    /// 러버밴드를 `at`까지로 다시 계산한다(끄는 동안 선택이 바뀐다, MS-1).
    fn update_band(
        &mut self,
        anchor: m::Anchor,
        at: (f32, f32),
        add: bool,
        before: Selection,
        cx: &mut Context<Self>,
    ) {
        let viewport = self.pianoroll.viewport();
        // 그리기 전(크기 0)에는 지금 점의 내용 위치를 알 수 없다(리뷰 2 제안 3)
        if viewport.width <= 0.0 || viewport.height <= 0.0 || viewport.row_height <= 0.0 {
            return;
        }
        let now = m::anchor_at(&viewport, at);
        let current_track = self.current_track();
        // `Cmd+드래그`는 지금 트랙의 선택 클립이 먼저다(그 선택에 더한다)
        let preferred = before
            .clip()
            .filter(|_| add && before.track().is_some() && before.track() == current_track);
        let picked = m::band_between(anchor, now).and_then(|band| {
            let track_id = current_track?;
            let (press, now) = (Tick::new(tick_u(anchor.tick)), Tick::new(tick_u(now.tick)));
            self.with_track(cx, |track| {
                let clip_id = m::band_clip(track, press, now, band, preferred)?;
                Some((track_id, clip_id, m::band_notes(track.clip(clip_id)?, band)))
            })
            .flatten()
        });
        let selection = match picked {
            Some((track, clip, mut notes)) => {
                if add && before.track() == Some(track) && before.clip() == Some(clip) {
                    notes.extend(before.notes().cloned());
                }
                let mut s = Selection::default();
                s.set(track, clip, notes);
                s
            }
            None if add => before.clone(),
            None => Selection::default(),
        };
        self.pianoroll.inner().borrow_mut().gesture = Some(Gesture::Band {
            anchor,
            now: at,
            add,
            before,
        });
        self.set_selection(selection, cx);
        cx.notify();
    }

    /// 선택이 생겼는데 커서가 그 클립 밖이면 커서를 선택의 첫 노트 칸으로(결정 2절 5).
    fn pull_cursor_into_selection(&mut self, cx: &mut Context<Self>) {
        let selection = self.pianoroll.selection();
        let (Some(track), Some(clip)) = (selection.track(), selection.clip()) else {
            return;
        };
        if self.clip_under_cursor(cx) == Some((track, clip)) {
            return;
        }
        let first = {
            let doc = self.document.read(cx);
            doc.project()
                .track(track)
                .and_then(|t| t.clip(clip))
                .and_then(|c| {
                    let notes: BTreeSet<Note> = selection.notes().cloned().collect();
                    m::first_cell(c, &notes)
                })
        };
        if let Some(cell) = first {
            self.pianoroll.set_cursor(cell);
            self.cursor_moved(false, cx);
        }
    }

    /// 클릭: 선택을 그 노트 하나로 하고 「마지막 선택」으로 기억한다.
    fn click_note(&mut self, track: TrackId, clip: ClipId, note: Note, cx: &mut Context<Self>) {
        self.pianoroll.inner().borrow_mut().last_clicked = Some((clip, note.clone()));
        self.replace_selection(track, clip, BTreeSet::from([note]), cx);
    }

    /// `Cmd+클릭`: 같은 클립이면 넣거나 빼고, 아니면 그 노트 하나(결정 3절 3).
    fn toggle_note(&mut self, track: TrackId, clip: ClipId, note: Note, cx: &mut Context<Self>) {
        let selection = self.pianoroll.selection();
        if selection.track() != Some(track) || selection.clip() != Some(clip) {
            self.click_note(track, clip, note, cx);
            return;
        }
        let mut notes: BTreeSet<Note> = selection.notes().cloned().collect();
        if !notes.remove(&note) {
            notes.insert(note.clone());
            self.pianoroll.inner().borrow_mut().last_clicked = Some((clip, note));
        }
        self.replace_selection(track, clip, notes, cx);
    }

    /// `Shift+클릭`: 기준 노트부터 누른 노트(빈 곳이면 누른 틱 한 틱)까지 시간 범위의 노트를 더한다(결정 3절 4).
    fn extend_by_time(
        &mut self,
        track: TrackId,
        clip: ClipId,
        clicked: Option<&Note>,
        tick: Tick,
        cx: &mut Context<Self>,
    ) {
        let selection = self.pianoroll.selection();
        let same_clip = selection.track() == Some(track) && selection.clip() == Some(clip);
        let last = self.pianoroll.inner().borrow().last_clicked.clone();
        let base = last
            .filter(|(c, n)| same_clip && *c == clip && selection.contains(clip, n))
            .map(|(_, n)| n)
            .or_else(|| {
                same_clip
                    .then(|| selection.notes().min_by_key(|n| n.start()).cloned())
                    .flatten()
            });
        let Some(base) = base else {
            // 기준이 없으면 보통 클릭과 같다
            match clicked {
                Some(note) => self.click_note(track, clip, note.clone(), cx),
                None => self.set_selection(Selection::default(), cx),
            }
            return;
        };
        let notes = {
            let doc = self.document.read(cx);
            let Some(c) = doc.project().track(track).and_then(|t| t.clip(clip)) else {
                return;
            };
            let at = c.start().get();
            let (start, end) = match clicked {
                Some(n) => (at + n.start().get(), at + n.end()),
                None => (tick.get(), tick.get() + 1),
            };
            let from = start.min(at + base.start().get());
            let to = end.max(at + base.end());
            let mut notes = m::time_range_notes(c, Tick::new(from), Tick::new(to));
            notes.extend(selection.notes().cloned());
            notes
        };
        self.replace_selection(track, clip, notes, cx);
    }

    /// 더블클릭(빈 곳): 길이 코드의 노트 하나를 만든다(결정 3절 6). 붙여넣기와 같은 계산·적용이다.
    fn create_note_at(&mut self, cell: Cursor, free: bool, cx: &mut Context<Self>) {
        let Some(track) = self.current_track() else {
            return;
        };
        // 시작은 마우스 스냅 격자(MS-3), 길이는 현재 길이 코드다(결정 3절 6, 6절 4)
        let grid = self.mouse_grid(cx);
        let length = crate::step::step_ticks(self.step.length, false, false);
        let planned = {
            let doc = self.document.read(cx);
            let project = doc.project();
            let start = if free {
                cell.tick
            } else {
                m::snap_floor(project.meter_map(), cell.tick, grid)
            };
            project.track(track).map(|t| {
                let velocity = t.default_velocity();
                Note::new(Tick::ZERO, length, cell.pitch, velocity)
                    .and_then(|n| {
                        let content = ClipNotes::new(vec![n])
                            .ok_or(keyscore_core::ModelError::UnknownClip)?;
                        plan_paste(project, track, start, &content)
                    })
                    .map(|placed| (start, placed))
            })
        };
        let (start, placed) = match planned {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                self.say(format!("노트 만들기: {e}"), cx);
                return;
            }
            None => return,
        };
        if self.apply_placed("노트 만들기", track, placed, None, cx) {
            self.pianoroll.set_cursor(Cursor {
                tick: start,
                pitch: cell.pitch,
            });
            self.cursor_moved(false, cx);
        }
    }
}
