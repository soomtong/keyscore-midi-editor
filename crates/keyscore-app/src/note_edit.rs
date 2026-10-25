//! 피아노롤의 삭제·잘라내기·복사·붙여넣기·복제·반복(S3-T3a, REQ 5.8, 5.8.1, 7.5, KM-15). 규칙은
//! `docs/reports/s3-t3a/plan.md` 「끝의 정의」에 있다.
//!
//! 편집 계산은 `pianoroll::edit`(순수), 클립보드는 `clipboard`, 여기는 창 처리기다. 대상은 선택, 선택이 없으면 커서
//! 위 노트 하나다. 선택이 현재 클립 밖이면 문서를 바꾸는 동작은 거부한다(5.3, S3-T2 `Alt` 조작과 같다).

use std::collections::BTreeSet;

use gpui_kit::{Context, Div, InteractiveElement as _, Stateful, Window};
use keyscore_core::{ClipId, Note, Tick, TrackId};

use crate::actions::edit;
use crate::clipboard::{self, ClipNotes};
use crate::mode::View;
use crate::pianoroll::edit::{
    Placed, PlacedClip, plan_delete, plan_duplicate, plan_paste, targets,
};
use crate::pianoroll::select as sel;
use crate::window::ProjectWindow;

/// 되돌리기 묶음 키. `pianoroll::ops`의 자리(`0x5337_0000`대)와 겹치지 않는다.
const COALESCE_DUPLICATE: u64 = 0x5337_0100;
const COALESCE_REPEAT: u64 = 0x5337_0101;

/// 편집 대상: (트랙, 클립, 노트). 노트는 그 클립의 실제 값이다.
struct Targets {
    track: TrackId,
    clip: ClipId,
    notes: Vec<Note>,
}

/// 붙여넣기 자리.
#[derive(Clone, Copy)]
enum PasteAt {
    /// 커서 자리(`Cmd+V`, vim `P`)
    Cursor,
    /// 커서 + 한 스텝(vim `p`)
    AfterCursor,
}

impl ProjectWindow {
    /// 편집 처리기를 시작해도 되는가. 입력란에 포커스가 있으면 그 입력란의 같은 동작(`input_action`)으로 보내고,
    /// 배치 뷰면(클립 단위 편집은 S4) 알린다.
    fn edit_allowed(
        &mut self,
        input_action: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if Self::text_input_focused(window) {
            // 키로 왔든 메뉴로 왔든 입력란의 동작으로 보낸다(KM-14의 undo와 같은 태도).
            if let Some(name) = input_action
                && let Ok(action) = cx.build_action(name, None)
            {
                window.dispatch_action(action, cx);
            }
            return false;
        }
        if self.mode_state().view() != View::PianoRoll {
            self.say(
                "배치 뷰의 클립 편집(삭제·복사·붙여넣기·복제)은 아직 없다. v로 피아노롤에서 한다",
                cx,
            );
            return false;
        }
        true
    }

    /// 대상(선택, 없으면 커서 위 노트). `edits_document`면 선택이 현재 클립 밖일 때 거부한다.
    fn edit_targets(
        &mut self,
        label: &str,
        edits_document: bool,
        cx: &mut Context<Self>,
    ) -> Option<Targets> {
        let selection = self.pianoroll.selection();
        let (track, clip, values) = if selection.is_empty() {
            let Some((t, c, n)) = self.note_under_cursor(cx) else {
                self.say(
                    format!("{label}: 커서 위에 노트가 없다(선택하거나 노트 위로 간다)"),
                    cx,
                );
                return None;
            };
            (t, c, BTreeSet::from([n]))
        } else {
            let (Some(t), Some(c)) = (selection.track(), selection.clip()) else {
                return None;
            };
            if edits_document && self.clip_under_cursor(cx) != Some((t, c)) {
                self.say(
                    format!(
                        "{label}: 선택이 현재 클립에 없다(Shift+p·Cmd+←로 그 클립에 돌아가거나 Esc로 선택을 비운다)"
                    ),
                    cx,
                );
                return None;
            }
            (t, c, selection.notes().cloned().collect())
        };
        let notes = {
            let doc = self.document.read(cx);
            let clip_ref = doc.project().track(track).and_then(|t| t.clip(clip))?;
            targets(clip_ref, &values)
        };
        if notes.is_empty() {
            self.say(format!("{label}: 노트가 없다"), cx);
            return None;
        }
        Some(Targets { track, clip, notes })
    }

    /// 편집을 적용한다. `key`가 있으면 같은 키의 연속은 undo 한 단위다. 실패하면 알리고 거짓.
    fn apply_edit(
        &mut self,
        label: &str,
        edits: Vec<keyscore_core::Edit>,
        key: Option<u64>,
        cx: &mut Context<Self>,
    ) -> bool {
        let result = self.document.update(cx, |doc, cx| {
            let r = match key {
                Some(key) => doc.apply_coalesced(edits, key),
                None => doc.apply(edits),
            };
            if r.is_ok() {
                cx.notify();
            }
            r
        });
        match result {
            Ok(()) => true,
            Err(e) => {
                self.say(format!("{label}: {e}"), cx);
                false
            }
        }
    }

    /// 넣기 결과를 적용하고 넣은 노트를 선택으로 둔다.
    fn apply_placed(
        &mut self,
        label: &str,
        track: TrackId,
        placed: Placed,
        key: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        if !self.apply_edit(label, placed.edits, key, cx) {
            return;
        }
        let clip = match placed.clip {
            PlacedClip::Existing(id) => Some(id),
            // 새 클립의 id는 적용한 뒤에 정해진다. 같은 트랙에 같은 시작의 클립은 하나뿐이다.
            PlacedClip::New { start } => self
                .with_track(cx, |t| {
                    t.clips()
                        .find(|(_, c)| c.start() == start)
                        .map(|(id, _)| id)
                })
                .flatten(),
        };
        if let Some(clip) = clip {
            self.pianoroll.set_anchor(None);
            self.replace_selection(track, clip, placed.notes.into_iter().collect(), cx);
        }
        cx.notify();
    }

    /// 대상을 클립보드로 복사한다(내부 구조 + 시스템 클립보드 SMF·텍스트). 텍스트·SMF는 문서를 빌린 채 만들어
    /// 프로젝트를 복제하지 않는다.
    fn copy_targets(&mut self, targets: &Targets, cx: &mut Context<Self>) {
        let Some(content) = ClipNotes::new(targets.notes.clone()) else {
            return;
        };
        let (text, midi) = {
            let doc = self.document.read(cx);
            let project = doc.project();
            let Some(track) = project.track(targets.track) else {
                return;
            };
            let clip_start = track.clip(targets.clip).map_or(0, |c| c.start().get());
            let first = targets
                .notes
                .iter()
                .map(|n| n.start().get())
                .min()
                .unwrap_or(0);
            let at = Tick::new(clip_start.saturating_add(first));
            (
                content.to_text(project, track, at),
                content.to_smf(project, track, at),
            )
        };
        clipboard::store(cx, content, text.as_deref(), midi.as_deref());
    }

    // ---------- 삭제 ----------

    fn delete_notes(&mut self, label: &str, t: &Targets, cx: &mut Context<Self>) -> bool {
        self.apply_edit(label, plan_delete(t.track, t.clip, &t.notes), None, cx)
    }

    /// `x` `Delete` `Backspace`: 대상을 지우고 선택·기준 칸을 비운다.
    fn edit_delete(&mut self, _: &edit::Delete, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        let Some(t) = self.edit_targets("삭제", true, cx) else {
            return;
        };
        if self.delete_notes("삭제", &t, cx) {
            self.clear_selection_after_edit(cx);
        }
    }

    fn clear_selection_after_edit(&mut self, cx: &mut Context<Self>) {
        self.pianoroll.set_anchor(None);
        self.set_selection(crate::pianoroll::Selection::default(), cx);
    }

    /// vim `X`: 커서 앞 노트 하나(`p`의 순서). 선택과 무관하다.
    fn edit_delete_before(
        &mut self,
        _: &edit::DeleteBefore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        let cursor = self.pianoroll.cursor();
        let Some(track) = self.current_track() else {
            return;
        };
        let found = self
            .with_track(cx, |t| {
                let (clip, at) = sel::prev_note(t, cursor)?;
                let c = t.clip(clip)?;
                let value = c
                    .notes()
                    .iter()
                    .find(|n| {
                        c.start().get() + n.start().get() == at.tick.get() && n.pitch() == at.pitch
                    })?
                    .clone();
                Some((clip, targets(c, &BTreeSet::from([value]))))
            })
            .flatten();
        let Some((clip, notes)) = found else {
            self.say("앞 노트 삭제: 커서 앞에 노트가 없다", cx);
            return;
        };
        if self.clip_under_cursor(cx) != Some((track, clip)) {
            self.say("앞 노트 삭제: 앞 노트가 현재 클립에 없다", cx);
            return;
        }
        self.delete_notes("앞 노트 삭제", &Targets { track, clip, notes }, cx);
    }

    /// vim `D`: 현재 클립에서 시작이 [커서 틱, 커서가 든 마디 끝)인 노트 전부.
    fn edit_delete_to_bar_end(
        &mut self,
        _: &edit::DeleteToBarEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        let Some((track, clip)) = self.clip_under_cursor(cx) else {
            self.say("마디 끝까지 삭제: 커서가 클립 밖이다", cx);
            return;
        };
        let cursor = self.pianoroll.cursor().tick.get();
        let notes = {
            let doc = self.document.read(cx);
            let project = doc.project();
            let meter = project.meter_map();
            let bar = meter.position(Tick::new(cursor)).bar();
            let end = meter.bar_start(bar + 1).map_or(u64::MAX, Tick::get);
            project
                .track(track)
                .and_then(|t| t.clip(clip))
                .map(|c| {
                    let base = c.start().get();
                    c.notes()
                        .iter()
                        .filter(|n| (cursor..end).contains(&(base + n.start().get())))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        if notes.is_empty() {
            self.say("마디 끝까지 삭제: 노트가 없다", cx);
            return;
        }
        self.delete_notes("마디 끝까지 삭제", &Targets { track, clip, notes }, cx);
    }

    // ---------- 복사·잘라내기 ----------

    /// `Cmd+C`, vim `y y`: 대상을 복사한다. 선택은 그대로다.
    fn edit_copy(&mut self, _: &edit::Copy, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(Some("input::Copy"), window, cx) {
            return;
        }
        if let Some(t) = self.edit_targets("복사", false, cx) {
            self.copy_targets(&t, cx);
            self.say(format!("노트 {}개를 복사했다", t.notes.len()), cx);
        }
    }

    /// `Cmd+X`, `Shift+x`: 복사한 뒤 지운다(undo 한 단위).
    fn edit_cut(&mut self, _: &edit::Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(Some("input::Cut"), window, cx) {
            return;
        }
        let Some(t) = self.edit_targets("잘라내기", true, cx) else {
            return;
        };
        self.copy_targets(&t, cx);
        if self.delete_notes("잘라내기", &t, cx) {
            self.clear_selection_after_edit(cx);
        }
    }

    /// vim `Y`: 커서가 든 마디의 노트를 복사한다(`b`와 같은 기준). 선택은 바꾸지 않는다.
    fn edit_copy_bar(&mut self, _: &edit::CopyBar, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        let Some((track, clip)) = self.clip_under_cursor(cx) else {
            self.say("마디 복사: 커서가 클립 밖이다", cx);
            return;
        };
        let cursor = self.pianoroll.cursor().tick;
        let notes: Vec<Note> = {
            let doc = self.document.read(cx);
            let project = doc.project();
            project
                .track(track)
                .and_then(|t| t.clip(clip))
                .map(|c| {
                    let values = sel::bar_notes(c, project.meter_map(), cursor);
                    targets(c, &values)
                })
                .unwrap_or_default()
        };
        if notes.is_empty() {
            self.say("마디 복사: 노트가 없다", cx);
            return;
        }
        let count = notes.len();
        self.copy_targets(&Targets { track, clip, notes }, cx);
        self.say(format!("노트 {count}개를 복사했다"), cx);
    }

    // ---------- 붙여넣기 ----------

    fn paste(&mut self, at: PasteAt, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(Some("input::Paste"), window, cx) {
            return;
        }
        let content = match clipboard::paste_contents(cx) {
            Ok(c) => c,
            Err(why) => {
                self.say(format!("붙여넣을 노트가 없다: {why}"), cx);
                return;
            }
        };
        let Some(track) = self.current_track() else {
            self.say("붙여넣기: 트랙이 없다", cx);
            return;
        };
        let cursor = self.pianoroll.cursor().tick;
        let tick = match at {
            PasteAt::Cursor => cursor,
            PasteAt::AfterCursor => cursor
                .checked_add(self.pianoroll.step_ticks())
                .unwrap_or(cursor),
        };
        let planned = plan_paste(self.document.read(cx).project(), track, tick, &content);
        match planned {
            Ok(placed) => self.apply_placed("붙여넣기", track, placed, None, cx),
            Err(e) => self.say(format!("붙여넣기: {e}"), cx),
        }
    }

    fn edit_paste(&mut self, _: &edit::Paste, window: &mut Window, cx: &mut Context<Self>) {
        self.paste(PasteAt::Cursor, window, cx);
    }

    fn edit_paste_before(
        &mut self,
        _: &edit::PasteBefore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.paste(PasteAt::Cursor, window, cx);
    }

    fn edit_paste_after(
        &mut self,
        _: &edit::PasteAfter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.paste(PasteAt::AfterCursor, window, cx);
    }

    // ---------- 복제·반복 ----------

    fn duplicate(&mut self, label: &str, key: u64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        let Some(t) = self.edit_targets(label, true, cx) else {
            return;
        };
        let planned = plan_duplicate(self.document.read(cx).project(), t.track, t.clip, &t.notes);
        match planned {
            Ok(placed) => self.apply_placed(label, t.track, placed, Some(key), cx),
            Err(e) => self.say(format!("{label}: {e}"), cx),
        }
    }

    /// `Cmd+D`: 대상을 폭만큼 바로 뒤에 복제한다.
    fn edit_duplicate(&mut self, _: &edit::Duplicate, window: &mut Window, cx: &mut Context<Self>) {
        self.duplicate("복제", COALESCE_DUPLICATE, window, cx);
    }

    /// `r`, vim `y y p`: 반복 복제. 사본이 선택이 되어 다시 누르면 하나 더 붙는다.
    fn edit_repeat(&mut self, _: &edit::Repeat, window: &mut Window, cx: &mut Context<Self>) {
        self.duplicate("반복", COALESCE_REPEAT, window, cx);
    }

    /// 편집 처리기를 단다(`piano_roll_handlers`에서).
    pub(crate) fn note_edit_handlers(
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element
            .on_action(cx.listener(Self::edit_delete))
            .on_action(cx.listener(Self::edit_delete_before))
            .on_action(cx.listener(Self::edit_delete_to_bar_end))
            .on_action(cx.listener(Self::edit_copy))
            .on_action(cx.listener(Self::edit_cut))
            .on_action(cx.listener(Self::edit_copy_bar))
            .on_action(cx.listener(Self::edit_paste))
            .on_action(cx.listener(Self::edit_paste_before))
            .on_action(cx.listener(Self::edit_paste_after))
            .on_action(cx.listener(Self::edit_duplicate))
            .on_action(cx.listener(Self::edit_repeat))
    }
}
