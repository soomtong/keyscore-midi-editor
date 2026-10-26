//! 피아노롤의 분할·합치기·길이 코드·음높이 바꾸기·퀀타이즈·레가토·마지막 편집 반복(S3-T3b, REQ 5.8, 5.8.1,
//! 5.7, 5.6). 규칙은 `docs/reports/s3-t3b/plan.md` 「끝의 정의」에 있다.
//!
//! 편집 계산은 `pianoroll::edit`(순수), 음 키 해석은 `pianoroll::keys`, 여기는 창 처리기다. 대상 고르기와 적용은
//! S3-T3a `note_edit`의 도우미를 함께 쓴다.

use std::collections::BTreeSet;

use gpui_kit::{
    Action, Context, Div, InteractiveElement as _, Keystroke, Stateful, Subscription, Window,
};
use keyscore_core::{Clip, ClipId, ModelError, Note, TrackId};

use crate::actions::args::KeyLayout;
use crate::actions::{edit, note};
use crate::app_state::AppSettings;
use crate::mode::View;
use crate::note_edit::Targets;
use crate::pianoroll::edit::{
    Changed, DEFAULT_LENGTH, length_ticks, plan_join, plan_legato, plan_quantize, plan_restore,
    plan_set_length, plan_set_pitch, plan_split, selection_after,
};
use crate::pianoroll::keys::{pitch_in_octave_of, semitone_of_key};
use crate::pianoroll::ops::NoteOp;
use crate::window::ProjectWindow;

/// 창마다 두는 노트 편집 상태.
#[derive(Default)]
pub(crate) struct NoteOpsState {
    /// `e` 뒤 음 키를 기다리는 중.
    pub(crate) pitch_pending: bool,
    /// 마지막 레가토(토글 되돌리기용).
    pub(crate) legato: Option<LegatoMemory>,
    /// `.`이 되풀이할 마지막 편집.
    pub(crate) last: Option<LastEdit>,
}

/// 마지막 레가토: 적용한 뒤의 문서 판, (원래, 바뀐) 짝, 적용 뒤 대상의 값 집합(바뀌지 않은 대상 포함).
pub(crate) struct LegatoMemory {
    pub(crate) revision: u64,
    pub(crate) track: TrackId,
    pub(crate) clip: ClipId,
    pub(crate) pairs: Vec<(Note, Note)>,
    pub(crate) targets: BTreeSet<Note>,
}

/// `.`(마지막 편집 반복)이 기억하는 편집.
pub(crate) enum LastEdit {
    /// 같은 Action을 다시 보낸다(같은 처리기·같은 검사).
    Action(Box<dyn Action>),
    /// 5.7 `Alt` 조작.
    Op(NoteOp),
    /// 음높이 바꾸기: 받은 음 키의 옥타브 안 반음 자리. 반복은 대기 없이 지금 커서 노트의 옥타브에 둔다.
    Pitch(u8),
}

impl LastEdit {
    pub(crate) fn action(action: &dyn Action) -> Self {
        LastEdit::Action(action.boxed_clone())
    }
}

/// 처리기마다 다른 부분.
struct Finish<'a> {
    label: &'a str,
    /// 바뀌는 노트가 없을 때의 문구(라벨 뒤).
    nothing: &'a str,
    /// 커서 노트에서 시작했을 때 커서가 바뀐 노트를 따라가는가.
    follow: bool,
    /// 성공하면 `.`이 기억할 편집.
    remember: LastEdit,
}

impl ProjectWindow {
    // ---------- 공통 ----------

    pub(crate) fn remember_edit(&mut self, edit: LastEdit) {
        self.note_ops.last = Some(edit);
    }

    fn clip_snapshot(&self, track: TrackId, clip: ClipId, cx: &Context<Self>) -> Option<Clip> {
        let doc = self.document.read(cx);
        doc.project().track(track)?.clip(clip).cloned()
    }

    /// 대상과 선택에서 시작했는지. 배치 뷰·입력란·선택이 현재 클립 밖이면 `None`(알림은 도우미가 낸다).
    fn note_targets(
        &mut self,
        label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(Targets, bool)> {
        if !self.edit_allowed(None, window, cx) {
            return None;
        }
        let from_selection = !self.pianoroll.selection().is_empty();
        let targets = self.edit_targets(label, true, cx)?;
        Some((targets, from_selection))
    }

    /// 계산 결과를 적용하고 선택·커서를 맞춘다. 성공하면 바뀐 짝을 돌려준다.
    fn finish(
        &mut self,
        t: &Targets,
        from_selection: bool,
        planned: Result<Option<Changed>, ModelError>,
        how: Finish<'_>,
        cx: &mut Context<Self>,
    ) -> Option<Vec<(Note, Note)>> {
        let changed = match planned {
            Ok(Some(c)) => c,
            Ok(None) => {
                self.say(format!("{}: {}", how.label, how.nothing), cx);
                return None;
            }
            Err(e) => {
                self.say(format!("{}: {e}", how.label), cx);
                return None;
            }
        };
        let before = self.clip_snapshot(t.track, t.clip, cx);
        if !self.apply_edit(how.label, changed.edits, None, cx) {
            return None;
        }
        if from_selection {
            if let (Some(before), Some(after)) = (before, self.clip_snapshot(t.track, t.clip, cx)) {
                let notes = selection_after(&before, &after, &t.notes);
                self.pianoroll.set_anchor(None);
                self.replace_selection(t.track, t.clip, notes, cx);
            }
        } else if how.follow
            && let Some((from, to)) = changed.moved.first()
        {
            self.follow_cursor_note(t.track, t.clip, from, to, cx);
        }
        self.remember_edit(how.remember);
        cx.notify();
        Some(changed.moved)
    }

    // ---------- 분할·합치기 ----------

    /// `/`, vim `s`: 커서 틱에서 대상 노트를 나눈다.
    fn note_split(&mut self, _: &edit::Split, window: &mut Window, cx: &mut Context<Self>) {
        let Some((t, from_selection)) = self.note_targets("분할", window, cx) else {
            return;
        };
        let at = self.pianoroll.cursor().tick;
        let planned = plan_split(
            self.document.read(cx).project(),
            t.track,
            t.clip,
            &t.notes,
            at,
        );
        let how = Finish {
            label: "분할",
            nothing: "커서 자리를 가로지르는 노트가 없다",
            follow: false,
            remember: LastEdit::action(&edit::Split),
        };
        self.finish(&t, from_selection, planned, how, cx);
    }

    /// `Cmd+J`, vim `J`: 같은 음높이끼리 합친다.
    fn note_join(&mut self, _: &edit::Join, window: &mut Window, cx: &mut Context<Self>) {
        let Some((t, from_selection)) = self.note_targets("합치기", window, cx) else {
            return;
        };
        let planned = plan_join(self.document.read(cx).project(), t.track, t.clip, &t.notes);
        let how = Finish {
            label: "합치기",
            nothing: "같은 음높이 노트를 둘 이상 고른다",
            follow: false,
            remember: LastEdit::action(&edit::Join),
        };
        self.finish(&t, from_selection, planned, how, cx);
    }

    // ---------- 길이 ----------

    fn set_length(
        &mut self,
        length: u32,
        remember: LastEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((t, from_selection)) = self.note_targets("길이", window, cx) else {
            return;
        };
        let planned = plan_set_length(
            self.document.read(cx).project(),
            t.track,
            t.clip,
            &t.notes,
            length,
        );
        let how = Finish {
            label: "길이",
            nothing: "바뀌는 노트가 없다",
            follow: true,
            remember,
        };
        self.finish(&t, from_selection, planned, how, cx);
    }

    /// `1`–`7`, vim `g 1`…`g 7`.
    fn note_set_length(
        &mut self,
        a: &note::SetLength,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_length(length_ticks(a.0, false), LastEdit::action(a), window, cx);
    }

    /// `!`…`&`(Shift+숫자의 입력 글자, B25).
    fn note_set_dotted_length(
        &mut self,
        a: &note::SetDottedLength,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_length(length_ticks(a.0, true), LastEdit::action(a), window, cx);
    }

    /// `0`, vim `g 0`.
    fn note_reset_length(
        &mut self,
        a: &note::ResetLength,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_length(DEFAULT_LENGTH, LastEdit::action(a), window, cx);
    }

    // ---------- 퀀타이즈·레가토 ----------

    /// `q`: 앱 설정의 퀀타이즈를 적용한다.
    fn note_quantize(&mut self, _: &edit::Quantize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((t, from_selection)) = self.note_targets("퀀타이즈", window, cx) else {
            return;
        };
        let settings = AppSettings::get(cx).quantize.to_core();
        let planned = plan_quantize(
            self.document.read(cx).project(),
            t.track,
            t.clip,
            &t.notes,
            &settings,
        );
        let how = Finish {
            label: "퀀타이즈",
            nothing: "바뀌는 노트가 없다",
            follow: true,
            remember: LastEdit::action(&edit::Quantize),
        };
        self.finish(&t, from_selection, planned, how, cx);
    }

    /// `~`: 레가토. 방금 붙인 노트가 그대로면 원래 길이로 되돌린다(결정 2-A).
    fn note_legato(&mut self, _: &edit::Legato, window: &mut Window, cx: &mut Context<Self>) {
        let Some((t, from_selection)) = self.note_targets("레가토", window, cx) else {
            return;
        };
        let revision = self.document.read(cx).revision();
        let values: BTreeSet<Note> = t.notes.iter().cloned().collect();
        let restore = self.note_ops.legato.take().filter(|m| {
            m.revision == revision && m.track == t.track && m.clip == t.clip && m.targets == values
        });
        let project = self.document.read(cx).project();
        // 숨김 꼬리 노트는 레가토하지 않는다(리뷰 1 지적 1). 대상에 꼬리가 있으면 바뀌는 노트가 없을 때 그 까닭을 알린다
        // (리뷰 2 제안 2).
        let has_tail = project
            .track(t.track)
            .and_then(|tr| tr.clip(t.clip))
            .is_some_and(|c| t.notes.iter().any(|n| n.end() > c.length()));
        let (planned, restoring) = match &restore {
            Some(m) => (plan_restore(project, t.track, t.clip, &m.pairs), true),
            None => (plan_legato(project, t.track, t.clip, &t.notes), false),
        };
        let how = Finish {
            label: "레가토",
            nothing: if has_tail {
                "바뀌는 노트가 없다(클립 끝을 넘는 숨김 꼬리 노트는 레가토하지 않는다)"
            } else {
                "바뀌는 노트가 없다"
            },
            follow: true,
            remember: LastEdit::action(&edit::Legato),
        };
        let Some(moved) = self.finish(&t, from_selection, planned, how, cx) else {
            return;
        };
        if !restoring {
            // 대상 가운데 다음 노트가 없어 그대로인 노트(마지막 노트)도 다음 `~`의 대상에 든다.
            let mut targets = values;
            for (from, to) in &moved {
                targets.remove(from);
                targets.insert(to.clone());
            }
            self.note_ops.legato = Some(LegatoMemory {
                revision: self.document.read(cx).revision(),
                track: t.track,
                clip: t.clip,
                pairs: moved,
                targets,
            });
        }
    }

    // ---------- 음높이 ----------

    /// `e`, vim `r`: 커서 노트가 있으면 다음 음 키 하나를 기다린다.
    fn note_set_pitch(&mut self, _: &edit::SetPitch, window: &mut Window, cx: &mut Context<Self>) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        if self.note_under_cursor(cx).is_none() {
            self.say("음높이 바꾸기: 커서 위에 노트가 없다", cx);
            return;
        }
        self.note_ops.pitch_pending = true;
        self.say("음높이: 음 키를 누른다(Esc 취소)", cx);
    }

    /// `e`(음높이 바꾸기) 뒤의 음 키 하나는 키맵 디스패치 전에 받는다. 이 창의 키만 본다.
    pub(crate) fn intercept_pitch_keys(window: &Window, cx: &mut Context<Self>) -> Subscription {
        let me = cx.entity().downgrade();
        let handle = window.window_handle();
        cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != handle {
                return;
            }
            let taken = me
                .update(cx, |this, cx| {
                    this.pitch_keystroke(&event.keystroke, window, cx)
                })
                .unwrap_or(false);
            if taken {
                cx.stop_propagation();
            }
        })
    }

    /// 음 키 대기 중의 키 입력(창의 키 가로채기에서). 먹었으면 참.
    pub(crate) fn pitch_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.note_ops.pitch_pending {
            return false;
        }
        self.note_ops.pitch_pending = false;
        // 대기 중에 마우스·메뉴로 피아노롤을 떠났거나 입력란으로 갔으면 대기만 풀고 키는 평소대로 둔다.
        if self.mode_state().view() != View::PianoRoll || Self::text_input_focused(window) {
            return false;
        }
        let layout = AppSettings::get(cx).input.key_layout;
        // KM-7: 계이름형 음이름은 문자 기준(입력 글자), 건반형은 키 자리.
        let key = match layout {
            KeyLayout::Solfege => keystroke.key_char.as_deref().unwrap_or(&keystroke.key),
            KeyLayout::Piano => keystroke.key.as_str(),
        };
        match semitone_of_key(layout, key).filter(|_| !keystroke.modifiers.modified()) {
            Some(semitone) => self.set_pitch_semitone(semitone, cx),
            None => self.say("음높이 바꾸기를 취소했다", cx),
        }
        cx.notify();
        true
    }

    /// 커서 노트를 커서 옥타브의 `semitone` 자리 음높이로 바꾼다.
    fn set_pitch_semitone(&mut self, semitone: u8, cx: &mut Context<Self>) {
        let Some((track, clip, value)) = self.note_under_cursor(cx) else {
            self.say("음높이 바꾸기: 커서 위에 노트가 없다", cx);
            return;
        };
        let Some(pitch) = pitch_in_octave_of(self.pianoroll.cursor().pitch, semitone) else {
            self.say("음높이 바꾸기: 음높이가 0–127을 벗어난다", cx);
            return;
        };
        let planned = plan_set_pitch(self.document.read(cx).project(), track, clip, &value, pitch);
        let t = Targets {
            track,
            clip,
            notes: vec![value],
        };
        let how = Finish {
            label: "음높이 바꾸기",
            nothing: "바뀌는 노트가 없다",
            follow: true,
            remember: LastEdit::Pitch(semitone),
        };
        self.finish(&t, false, planned, how, cx);
    }

    // ---------- 반복 ----------

    /// `.`: 마지막 편집을 지금 대상에 되풀이한다.
    fn note_repeat_last(
        &mut self,
        _: &edit::RepeatLast,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        // 키 관찰의 묶음 끊기는 이 처리기 뒤에 돈다. `Alt` 조작을 여기서 바로 적용하면 앞 조작과 묶이므로 먼저
        // 끊는다(`.` 하나가 undo 한 단위).
        self.seal_history(cx);
        match &self.note_ops.last {
            None => self.say("반복할 편집이 없다", cx),
            // 같은 처리기를 거치도록 Action을 다시 보낸다(디스패치는 다음 효과 주기에 돈다).
            Some(LastEdit::Action(action)) => {
                window.dispatch_action(action.boxed_clone(), cx);
                // 디스패치는 다음 효과 주기라 키 관찰의 묶음 끊기보다 늦게 적용된다. 복제·반복은 묶음 키로 적용되므로
                // 그 뒤에 다시 끊어야 다음 `cmd-d`가 이 단위에 묶이지 않는다(리뷰 1 지적 2). 지연 순서는 등록 순이다.
                cx.defer_in(window, |this, _, cx| this.seal_history(cx));
            }
            Some(LastEdit::Op(op)) => {
                let op = *op;
                self.apply_note_op(op, cx);
            }
            Some(LastEdit::Pitch(semitone)) => {
                let semitone = *semitone;
                self.set_pitch_semitone(semitone, cx);
            }
        }
    }

    /// 처리기를 단다(`piano_roll_handlers`에서).
    pub(crate) fn note_ops_handlers(
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element
            .on_action(cx.listener(Self::note_split))
            .on_action(cx.listener(Self::note_join))
            .on_action(cx.listener(Self::note_set_length))
            .on_action(cx.listener(Self::note_set_dotted_length))
            .on_action(cx.listener(Self::note_reset_length))
            .on_action(cx.listener(Self::note_quantize))
            .on_action(cx.listener(Self::note_legato))
            .on_action(cx.listener(Self::note_set_pitch))
            .on_action(cx.listener(Self::note_repeat_last))
    }
}
