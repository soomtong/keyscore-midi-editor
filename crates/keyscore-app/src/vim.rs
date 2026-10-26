//! vim 오퍼레이터 대기·모션·텍스트 객체·비주얼 선택의 창 처리기(S3-T3c, REQ 5.8.1, 5.7). 규칙은
//! `docs/reports/s3-t3c/plan.md` 「끝의 정의」, 결정은 `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`.
//!
//! 계산은 `pianoroll::vim`(순수), 대상 고르기·삭제·복사는 S3-T3a `note_edit`의 도우미를 쓴다. 모션 처리기는
//! 모두 [`ProjectWindow::run_motion`]을 거친다: 오퍼레이터 대기가 있으면 그 범위에 작용하고, 없으면 커서를
//! 카운트만큼 옮긴다(비주얼 중이면 `cursor_moved`가 선택을 다시 계산한다).

use gpui_kit::{
    Action, Context, Div, InteractiveElement as _, Keystroke, Stateful, Subscription, Window,
};
use keyscore_core::Tick;

use crate::actions::{cursor, edit, object, operator, select};
use crate::keymap::{self, FLUSH_KEY};
use crate::mode::{Mode, ModeEvent, View};
use crate::note_edit::Targets;
use crate::note_ops::LastEdit;
use crate::pianoroll::edit::targets;
use crate::pianoroll::vim::{
    Motion, Operator, Reach, TextObject, VisualKind, motion_notes, motion_target, object_notes,
    visual_notes,
};
use crate::pianoroll::{Cursor, FindDirection, moved};
use crate::window::ProjectWindow;

/// 오퍼레이터 대기와 비주얼의 키 컨텍스트 경로. 메뉴 키 표시도 이 경로를 본다(`commands`).
pub(crate) const OPERATOR_PATH: &[&str] = &["Normal", "PianoRoll", "Operator"];
pub(crate) const VISUAL_PATH: &[&str] = &["Normal", "PianoRoll", "Selection", "Visual"];

/// 카운트 상한(키맵 카운트와 같다).
const MAX_COUNT: u32 = 9999;

/// 오퍼레이터 대기: 오퍼레이터와 그 앞의 카운트.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) op: Operator,
    pub(crate) count: Option<u32>,
}

/// 창마다 두는 vim 상태.
#[derive(Default)]
pub(crate) struct VimState {
    pub(crate) pending: Option<Pending>,
    /// 대기 입력을 비울 때 재생되는 접두 키의 오퍼레이터를 무시한다. 대기 취소 뒤 남은 `y`(`y y`의 접두)를
    /// 비우면 GPUI가 `operator::Yank`를 재생해 대기가 다시 생기기 때문이다.
    pub(crate) ignore_replayed_operator: bool,
    /// GPUI 대기 입력(텍스트 객체 접두 `i` 등)을 남긴 키가 맞춰진 확장 경로(`Operator`·`Visual`). 대기 입력이
    /// 남아 있는 동안 창은 이 경로로 그린다. 키가 아닌 경로(클릭, 메뉴 선택)로 대기·비주얼이 끝나도 그 입력이
    /// Normal 컨텍스트에서 `i`(Step 모드)로 재생되지 않게 하려는 것이다(리뷰 2 지적 1).
    pub(crate) held: Option<&'static [&'static str]>,
}

/// 오퍼레이터가 작용할 대상. `.`이 되풀이할 때도 쓴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OperatorTarget {
    /// 모션과 쓴 카운트(곱한 값)
    Motion {
        motion: Motion,
        count: u32,
    },
    Object(TextObject),
    /// 편집 대상: 선택, 없으면 커서 노트(S3-T3a `edit_targets`). 비주얼·선택에 바로 작용하거나 겹친 오퍼레이터.
    Selection,
}

impl Operator {
    fn label(self) -> &'static str {
        match self {
            Operator::Delete => "삭제",
            Operator::Yank => "복사",
            Operator::Change => "변경",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Operator::Delete => "d",
            Operator::Yank => "y",
            Operator::Change => "c",
        }
    }

    /// 문서를 바꾸는가(선택이 현재 클립 밖이면 거부할지).
    fn edits_document(self) -> bool {
        !matches!(self, Operator::Yank)
    }
}

/// 상태줄 모드 표시의 비주얼 이름.
fn visual_label(kind: VisualKind) -> &'static str {
    match kind {
        VisualKind::Note => "VISUAL",
        VisualKind::Time => "V-LINE",
        VisualKind::Block => "V-BLOCK",
    }
}

/// 카운트 둘의 곱(없으면 1). 상한에서 자른다.
fn total_count(a: Option<u32>, b: Option<u32>) -> u32 {
    a.unwrap_or(1)
        .saturating_mul(b.unwrap_or(1))
        .clamp(1, MAX_COUNT)
}

/// 오퍼레이터 대기 중에 실행해도 되는 Action인가(계획 2절).
fn allowed_while_pending(action: &dyn Action, op: Operator) -> bool {
    let a = action.as_any();
    let motion = a.is::<cursor::Left>()
        || a.is::<cursor::Right>()
        || a.is::<cursor::PrevBar>()
        || a.is::<cursor::NextBar>()
        || a.is::<cursor::NextNote>()
        || a.is::<cursor::PrevNote>()
        || a.is::<cursor::NextBeat>()
        || a.is::<cursor::PrevBeat>()
        || a.is::<cursor::NoteEnd>()
        || a.is::<cursor::LastNoteEndInBar>()
        || a.is::<cursor::NextSamePitch>()
        || a.is::<cursor::PrevSamePitch>()
        || a.is::<cursor::RepeatFind>()
        || a.is::<cursor::RepeatFindReverse>()
        || a.is::<cursor::NextClip>()
        || a.is::<cursor::PrevClip>()
        || a.is::<cursor::ClipStart>()
        || a.is::<cursor::ClipEnd>();
    let own = a.is::<operator::Count>()
        || a.is::<operator::Cancel>()
        || a.is::<operator::Delete>()
        || a.is::<operator::Yank>()
        || a.is::<operator::Change>()
        || a.is::<object::InnerNote>()
        || a.is::<object::InnerBar>()
        || a.is::<object::AroundClip>();
    // `y`로 시작하는 `y y`(복사)·`y y p`(반복)는 `y` 대기에서만(`y` 뒤 대기 시간을 넘긴 `y y`)
    let yank_sequence = op == Operator::Yank && (a.is::<edit::Copy>() || a.is::<edit::Repeat>());
    motion || own || yank_sequence
}

/// 오퍼레이터 대기를 시작하는 Action인가(대기 입력의 접두 판단).
fn is_operator_start(action: &dyn Action) -> bool {
    let a = action.as_any();
    a.is::<operator::Delete>() || a.is::<operator::Yank>() || a.is::<operator::Change>()
}

/// 수식키만 눌렀다 뗀 키 입력(GPUI가 `shift` 같은 이름으로 보낸다).
fn is_modifier_only(k: &Keystroke) -> bool {
    matches!(
        k.key.as_str(),
        "shift" | "control" | "alt" | "platform" | "function"
    )
}

impl ProjectWindow {
    // ---------- 상태 ----------

    /// 대기 중인 오퍼레이터와 그 앞의 카운트(S3-T3c).
    #[must_use]
    pub fn pending_operator(&self) -> Option<(Operator, Option<u32>)> {
        self.vim.pending.map(|p| (p.op, p.count))
    }

    /// vim 상태가 뜻이 있는 자리: Normal 모드의 피아노롤.
    fn in_piano_roll_normal(&self) -> bool {
        self.mode_state().mode() == Mode::Normal && self.mode_state().view() == View::PianoRoll
    }

    /// 창의 키 컨텍스트 경로(바깥 → 안): 모드 FSM의 경로에 오퍼레이터 대기면 `Operator`, 비주얼이면 `Visual`.
    /// 오퍼레이터 대기는 선택이 없을 때만, 비주얼은 기준 칸이 있어 늘 `Selection` 안이다.
    #[must_use]
    pub fn context_path(&self) -> &'static [&'static str] {
        let base = self.mode_state().context_path();
        if !self.in_piano_roll_normal() {
            return base;
        }
        if self.vim.pending.is_some() && *base == OPERATOR_PATH[..2] {
            return OPERATOR_PATH;
        }
        if self.pianoroll.visual().is_some() && *base == VISUAL_PATH[..3] {
            return VISUAL_PATH;
        }
        base
    }

    /// 상태줄 모드 표시: 비주얼이면 그 이름, 오퍼레이터 대기면 `NORMAL 3d`.
    #[must_use]
    pub fn mode_label(&self) -> String {
        let base = self.mode_state().mode().label();
        if !self.in_piano_roll_normal() {
            return base.to_owned();
        }
        if let Some(p) = self.vim.pending {
            let count = p.count.map(|c| c.to_string()).unwrap_or_default();
            return format!("{base} {count}{}", p.op.key());
        }
        match self.pianoroll.visual() {
            Some(kind) => visual_label(kind).to_owned(),
            None => base.to_owned(),
        }
    }

    /// 창이 그릴 키 컨텍스트 경로. GPUI 대기 입력이 확장 경로에서 남았으면 그 경로를 붙든다(`held`).
    pub(crate) fn key_context_path(&self, window: &Window) -> &'static [&'static str] {
        match self.vim.held {
            Some(held) if window.has_pending_keystrokes() => held,
            _ => self.context_path(),
        }
    }

    /// 오퍼레이터 대기를 끝낸다(모드·뷰·트랙 바뀜, 클릭, 선택이 생김). 대기 앞뒤의 카운트도 비운다(리뷰 2 지적 3).
    pub(crate) fn clear_operator(&mut self, cx: &mut Context<Self>) {
        if self.vim.pending.take().is_some() {
            keymap::clear_count(cx);
            cx.notify();
        }
    }

    fn cancel_operator(&mut self, cx: &mut Context<Self>) {
        self.vim.pending = None;
        keymap::clear_count(cx);
        self.say("오퍼레이터를 취소했다", cx);
    }

    // ---------- 키 가로채기 ----------

    /// 오퍼레이터 대기 중 허용되지 않는 키와 `y` 접두 뒤의 키를 키맵 디스패치 전에 받는다(계획 2절).
    pub(crate) fn intercept_vim_keys(window: &Window, cx: &mut Context<Self>) -> Subscription {
        let me = cx.entity().downgrade();
        let handle = window.window_handle();
        cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != handle {
                return;
            }
            let taken = me
                .update(cx, |this, cx| {
                    this.vim_keystroke(&event.keystroke, window, cx)
                })
                .unwrap_or(false);
            if taken {
                cx.stop_propagation();
            }
        })
    }

    /// 키 하나를 본다. 삼켰으면 참.
    fn vim_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if keystroke.key == FLUSH_KEY
            || is_modifier_only(keystroke)
            || Self::text_input_focused(window)
        {
            return false;
        }
        let pending_input: Vec<Keystroke> = window
            .pending_input_keystrokes()
            .map(<[Keystroke]>::to_vec)
            .unwrap_or_default();
        let escape = keystroke.key == "escape" && !keystroke.modifiers.modified();
        // 붙든 경로의 대기 입력이 남았는데 대기·비주얼이 키 밖에서 끝났다(경로가 달라졌다): 그 경로에서 대기
        // 입력을 비우고(접두는 `null`이라 아무 일도 없다) 이 키를 새로 보낸다. Esc는 비우기만 한다(A13).
        if let Some(held) = self.vim.held
            && !pending_input.is_empty()
            && !std::ptr::eq(held, self.context_path())
        {
            let replay = (!escape).then(|| keystroke.clone());
            self.flush_held_then(replay, window, cx);
            return true;
        }
        if !self.in_piano_roll_normal() {
            self.vim.held = None;
            return false;
        }
        let mut keys = pending_input.clone();
        keys.push(keystroke.clone());
        let stack = window.context_stack();
        let (found, still_pending) = cx.key_bindings().borrow().bindings_for_input(&keys, &stack);
        let through = self.let_through_vim(
            keystroke,
            &pending_input,
            found.first().map(gpui_kit::KeyBinding::action),
            still_pending,
            window,
            cx,
        );
        if !through {
            return true;
        }
        let path = self.context_path();
        self.vim.held =
            (std::ptr::eq(path, OPERATOR_PATH) || std::ptr::eq(path, VISUAL_PATH)).then_some(path);
        false
    }

    /// 대기 입력을 비운 뒤(붙든 경로에서) 붙든 경로를 놓고, `replay`가 있으면 다시 그린 다음 보낸다.
    fn flush_held_then(
        &mut self,
        replay: Option<Keystroke>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.vim.ignore_replayed_operator = true;
        let me = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            keymap::flush_pending_keystrokes(window, cx);
            let _ = me.update(cx, |this, cx| {
                this.vim.ignore_replayed_operator = false;
                this.vim.held = None;
                cx.notify();
            });
            if let Some(replay) = replay {
                window.defer(cx, move |window, cx| {
                    window.dispatch_keystroke(replay, cx);
                });
            }
        });
    }

    /// 대기·`y` 접두 규칙(계획 2절). 키를 그대로 보내면 참, 삼켰으면 거짓.
    fn let_through_vim(
        &mut self,
        keystroke: &Keystroke,
        pending_input: &[Keystroke],
        found: Option<&dyn Action>,
        still_pending: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let escape = keystroke.key == "escape" && !keystroke.modifiers.modified();
        // 대기 입력이 오퍼레이터 키 하나(`y`, `y y`의 접두)인가
        let operator_prefix = pending_input.len() == 1 && {
            let stack = window.context_stack();
            let (prefix, _) = cx
                .key_bindings()
                .borrow()
                .bindings_for_input(pending_input, &stack);
            prefix
                .first()
                .is_some_and(|b| is_operator_start(b.action()))
        };
        if let Some(p) = self.vim.pending {
            // A13: 시퀀스 대기 중(`d i` 뒤)의 Esc는 시퀀스만 취소한다(키맵의 가로채기가 한다). 대기는 남는다.
            // 그 시퀀스가 오퍼레이터 키(`y` 대기의 `y`)면 비우기가 재생하는 오퍼레이터를 무시한다. 그대로 두면 겹친
            // 오퍼레이터(`y y`)로 커서 노트를 복사한다(리뷰 1 지적 2).
            if escape && !pending_input.is_empty() {
                if operator_prefix {
                    self.ignore_replayed_operator_once(window, cx);
                }
                return true;
            }
            let ok = match found {
                Some(action) => allowed_while_pending(action, p.op),
                None => still_pending,
            };
            if ok {
                return true;
            }
            if pending_input.is_empty() {
                self.cancel_operator(cx);
            } else {
                self.flush_then_cancel(window, cx);
            }
            return false;
        }
        if !operator_prefix {
            return true;
        }
        // `y` 뒤의 Esc: 키맵의 가로채기가 대기를 비울 때 재생되는 `y`를 무시한다
        if escape {
            self.ignore_replayed_operator_once(window, cx);
            return true;
        }
        if found.is_some() || still_pending {
            return true;
        }
        // `y` 뒤에 이어지는 바인딩이 없는 키(`i`, `w`, 숫자): GPUI는 `y`를 재생한 뒤 이 키를 낡은 키 컨텍스트로
        // 맞춘다(`i`가 Step 모드로 간다). 키를 삼키고, 대기를 비워 `y`를 실행한 뒤(대기 상태 → `Operator`
        // 컨텍스트) 다시 그린 다음 이 키를 다시 보낸다. 두 번째 지연은 첫 지연의 알림이 창을 더럽힌 뒤에 돈다.
        let replay = keystroke.clone();
        window.defer(cx, move |window, cx| {
            keymap::flush_pending_keystrokes(window, cx);
            window.defer(cx, move |window, cx| {
                window.dispatch_keystroke(replay, cx);
            });
        });
        false
    }

    /// 키맵의 Esc 가로채기가 대기 입력을 비울 때 재생되는 오퍼레이터를 무시한다. 그 비우기는 키맵이 먼저 건
    /// 지연에서 돌고, 이 지연이 그 뒤에 표시를 내린다.
    fn ignore_replayed_operator_once(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.vim.ignore_replayed_operator = true;
        cx.defer_in(window, |this, _, _| {
            this.vim.ignore_replayed_operator = false;
        });
    }

    /// 대기 입력을 비운 뒤 오퍼레이터를 취소한다. 접두 키가 오퍼레이터(`y`)면 그 재생은 무시한다.
    ///
    /// 비우기는 지금 키 컨텍스트(`Operator`)에서 해야 한다. 먼저 취소하면 다시 그린 Normal 컨텍스트에서 접두
    /// `i`가 Step 모드로 재생된다. 디스패치가 이 창의 처리기를 부르므로 창 엔티티를 빌리지 않은 지연에서 한다.
    fn flush_then_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.vim.ignore_replayed_operator = true;
        let me = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            keymap::flush_pending_keystrokes(window, cx);
            let _ = me.update(cx, |this, cx| {
                this.vim.ignore_replayed_operator = false;
                this.cancel_operator(cx);
            });
        });
    }

    /// 키 입력이 끝난 뒤(창의 키 관찰에서): 대기 중 실행된 Action이 모션·객체·오퍼레이터가 아니면(대기의 `y y`
    /// 복사·반복) 대기는 끝났다.
    pub(crate) fn vim_after_keystroke(
        &mut self,
        action: Option<&dyn Action>,
        cx: &mut Context<Self>,
    ) {
        if self.vim.pending.is_none() {
            return;
        }
        let Some(action) = action else {
            return;
        };
        let a = action.as_any();
        let keeps = a.is::<operator::Count>()
            || a.is::<operator::Delete>()
            || a.is::<operator::Yank>()
            || a.is::<operator::Change>();
        if !keeps {
            self.clear_operator(cx);
        }
    }

    // ---------- 오퍼레이터 ----------

    /// `d` `y` `c`: 선택·비주얼이 있으면 바로, 아니면 대기(계획 2절).
    fn start_operator(&mut self, op: Operator, window: &mut Window, cx: &mut Context<Self>) {
        if self.vim.ignore_replayed_operator {
            self.vim.ignore_replayed_operator = false;
            return;
        }
        let count = keymap::take_count(cx);
        if !self.in_piano_roll_normal() {
            self.say(
                "오퍼레이터(d y c)는 피아노롤의 Normal에서 쓴다(g a로 피아노롤)",
                cx,
            );
            return;
        }
        if let Some(p) = self.vim.pending.take() {
            if p.op == op {
                // 겹친 오퍼레이터(`d d`, `c c`, 대기 뒤 `y`): 편집 대상에
                self.apply_operator(op, OperatorTarget::Selection, window, cx);
            } else {
                self.cancel_operator(cx);
            }
            cx.notify();
            return;
        }
        if self.pianoroll.visual().is_some() || !self.pianoroll.selection().is_empty() {
            self.apply_operator(op, OperatorTarget::Selection, window, cx);
            return;
        }
        // 노트 없이 기준 칸만 남은 선택(빈 칸을 넓히는 중)은 오퍼레이터에게 선택이 아니다. 기준 칸을 지워 선택을
        // 끝내고 대기에 든다. 그대로 두면 `Selection` 컨텍스트라 `Operator`가 붙지 않는다(리뷰 2 지적 2).
        if self.pianoroll.anchor().is_some() {
            self.pianoroll.set_anchor(None);
            self.sync_selection_mode(cx);
        }
        self.vim.pending = Some(Pending { op, count });
        cx.notify();
    }

    fn operator_delete(
        &mut self,
        _: &operator::Delete,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_operator(Operator::Delete, window, cx);
    }

    fn operator_yank(&mut self, _: &operator::Yank, window: &mut Window, cx: &mut Context<Self>) {
        self.start_operator(Operator::Yank, window, cx);
    }

    fn operator_change(
        &mut self,
        _: &operator::Change,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_operator(Operator::Change, window, cx);
    }

    /// `operator::Cancel`(A13, vim `Normal > Operator`의 `escape`): 오퍼레이터 대기와 카운트를 비운다.
    fn operator_cancel(&mut self, _: &operator::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.vim.pending = None;
        keymap::clear_count(cx);
        cx.notify();
    }

    /// 대상 노트와 커서를 옮길 앞쪽 칸. 대상을 못 고르면 알리고 `None`.
    fn operator_targets(
        &mut self,
        op: Operator,
        target: OperatorTarget,
        cx: &mut Context<Self>,
    ) -> Option<(Targets, Option<Cursor>)> {
        let label = op.label();
        if target == OperatorTarget::Selection {
            return self
                .edit_targets_allow_empty(label, op.edits_document(), cx)
                .map(|t| (t, None));
        }
        let Some((track, clip_id)) = self.clip_under_cursor(cx) else {
            self.say(format!("{label}: 커서가 클립 밖이다"), cx);
            return None;
        };
        let from = self.pianoroll.cursor();
        let (set, front) = match target {
            OperatorTarget::Motion { motion, count } => {
                if motion.reach() == Reach::Pitch {
                    self.say("오퍼레이터를 취소했다(음높이 이동은 범위가 없다)", cx);
                    return None;
                }
                let Some(to) = self.motion_destination(motion, count, cx) else {
                    self.say(format!("{label}: 대상이 없다"), cx);
                    return None;
                };
                let doc = self.document.read(cx);
                let clip = doc.project().track(track)?.clip(clip_id)?;
                let backward = (to.tick, to.pitch) < (from.tick, from.pitch);
                let front = if backward { to } else { from };
                (motion_notes(clip, from, to, motion.reach()), Some(front))
            }
            OperatorTarget::Object(obj) => {
                let doc = self.document.read(cx);
                let project = doc.project();
                let clip = project.track(track)?.clip(clip_id)?;
                (object_notes(clip, project.meter_map(), from, obj), None)
            }
            OperatorTarget::Selection => unreachable!("위에서 돌려줬다"),
        };
        let notes = {
            let doc = self.document.read(cx);
            let clip = doc.project().track(track)?.clip(clip_id)?;
            targets(clip, &set)
        };
        Some((
            Targets {
                track,
                clip: clip_id,
                notes,
            },
            front,
        ))
    }

    /// 편집 대상(선택, 없으면 커서 노트). 커서 위에 노트가 없으면 빈 대상(`c`는 그래도 Step으로 간다).
    fn edit_targets_allow_empty(
        &mut self,
        label: &str,
        edits_document: bool,
        cx: &mut Context<Self>,
    ) -> Option<Targets> {
        if self.pianoroll.selection().is_empty() && self.note_under_cursor(cx).is_none() {
            let Some((track, clip)) = self.clip_under_cursor(cx) else {
                self.say(format!("{label}: 커서가 클립 밖이다"), cx);
                return None;
            };
            return Some(Targets {
                track,
                clip,
                notes: Vec::new(),
            });
        }
        self.edit_targets(label, edits_document, cx)
    }

    /// 오퍼레이터를 대상에 적용한다(계획 3절). 성공하면 참.
    pub(crate) fn apply_operator(
        &mut self,
        op: Operator,
        target: OperatorTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.vim.pending = None;
        if self.mode_state().view() != View::PianoRoll {
            self.say("오퍼레이터(d y c)는 피아노롤에서 쓴다", cx);
            return false;
        }
        let was_visual = self.pianoroll.visual().is_some();
        // 대상을 못 고르면(거부·대상 없음) 선택과 비주얼은 그대로 둔다. S3-T3a의 거부와 같고, 커서를 선택의 클립으로
        // 되돌려 이어 쓸 수 있다(리뷰 1 지적 3, 사람 결정 2026-10-27 「거부, 선택 유지」).
        let Some((t, front)) = self.operator_targets(op, target, cx) else {
            cx.notify();
            return false;
        };
        let cursor = self.pianoroll.cursor();
        let earliest = {
            let doc = self.document.read(cx);
            let base = doc
                .project()
                .track(t.track)
                .and_then(|tr| tr.clip(t.clip))
                .map_or(0, |c| c.start().get());
            t.notes
                .iter()
                .map(|n| base.saturating_add(n.start().get()))
                .min()
        };
        let label = op.label();
        match op {
            Operator::Delete | Operator::Yank if t.notes.is_empty() => {
                let what = if op == Operator::Delete {
                    "지울"
                } else {
                    "복사할"
                };
                self.say(format!("{label}: {what} 노트가 없다"), cx);
                if was_visual {
                    self.end_visual(cx);
                }
                cx.notify();
                return false;
            }
            Operator::Delete => {
                if !self.delete_notes(label, &t, cx) {
                    return false;
                }
                self.clear_selection_after_edit(cx);
                self.remember_edit(LastEdit::Operator { op, target });
            }
            Operator::Yank => {
                self.copy_targets(&t, "복사했다", cx);
                if was_visual {
                    self.end_visual(cx);
                }
            }
            Operator::Change => {
                if !t.notes.is_empty() && !self.delete_notes(label, &t, cx) {
                    return false;
                }
                self.clear_selection_after_edit(cx);
                self.remember_edit(LastEdit::Operator { op, target });
            }
        }
        let to = match (op, front) {
            (Operator::Change, Some(front)) => Some(Cursor {
                tick: front.tick,
                pitch: cursor.pitch,
            }),
            (Operator::Change, None) => earliest.map(|tick| Cursor {
                tick: Tick::new(tick),
                pitch: cursor.pitch,
            }),
            (_, front) => front,
        };
        if let Some(to) = to
            && to != cursor
        {
            self.pianoroll.set_cursor(to);
            self.cursor_moved(false, cx);
        }
        if op == Operator::Change {
            self.apply_mode_event(ModeEvent::Enter(Mode::Step), window, cx);
        }
        cx.notify();
        true
    }

    /// `C`(vim `shift-c`): `D`와 같은 노트를 지우고 커서 자리에서 Step 모드로.
    fn edit_change_to_bar_end(
        &mut self,
        _: &edit::ChangeToBarEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.edit_allowed(None, window, cx) {
            return;
        }
        let label = "마디 끝까지 변경";
        let Some(t) = self.bar_end_targets(label, cx) else {
            return;
        };
        if !t.notes.is_empty() && !self.delete_notes(label, &t, cx) {
            return;
        }
        self.clear_selection_after_edit(cx);
        self.remember_edit(LastEdit::action(&edit::ChangeToBarEnd));
        self.apply_mode_event(ModeEvent::Enter(Mode::Step), window, cx);
    }

    // ---------- 모션 ----------

    /// `motion`을 `count`번 되풀이한 목적지. 첫 번도 못 가면 `None`, 더 갈 수 없으면 거기서 멈춘다.
    fn motion_destination(&self, motion: Motion, count: u32, cx: &Context<Self>) -> Option<Cursor> {
        let from = self.pianoroll.cursor();
        let step = self.pianoroll.step_ticks();
        let doc = self.document.read(cx);
        let project = doc.project();
        let meter = project.meter_map();
        // 스텝·반음·마디 이동은 트랙이 없어도 된다(S3-T1과 같다)
        if let Some(delta) = motion.as_cursor_move() {
            let mut at = from;
            for _ in 0..count {
                let next = moved(at, delta, step, meter);
                if next == at {
                    break;
                }
                at = next;
            }
            return Some(at);
        }
        let track = project.track(self.current_track()?)?;
        let times = if motion.counts() { count } else { 1 };
        let mut at = motion_target(track, meter, step, from, motion)?;
        for _ in 1..times {
            match motion_target(track, meter, step, at, motion) {
                Some(next) if next != at => at = next,
                _ => break,
            }
        }
        Some(at)
    }

    /// 모션 처리기의 공통 길(계획 4절). 오퍼레이터 대기가 있으면 범위에 작용하고, 없으면 커서를 옮긴다.
    pub(crate) fn run_motion(
        &mut self,
        motion: Motion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = keymap::take_count(cx);
        if let Some(p) = self.vim.pending.take() {
            if motion.reach() == Reach::Pitch {
                self.cancel_operator(cx);
                cx.notify();
                return;
            }
            let count = total_count(p.count, count);
            self.apply_operator(p.op, OperatorTarget::Motion { motion, count }, window, cx);
            return;
        }
        if let Some(to) = self.motion_destination(motion, total_count(count, None), cx) {
            self.pianoroll.set_cursor(to);
            self.cursor_moved(false, cx);
        }
    }

    /// `f` `F` `;` `,`: 방향을 정해(`;` `,`는 마지막 `f`의 방향) 모션으로.
    pub(crate) fn run_find(
        &mut self,
        direction: FindDirection,
        remember: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if remember {
            self.pianoroll.set_last_find(direction);
        }
        let motion = match direction {
            FindDirection::Forward => Motion::NextSamePitch,
            FindDirection::Backward => Motion::PrevSamePitch,
        };
        self.run_motion(motion, window, cx);
    }

    fn cursor_next_beat(
        &mut self,
        _: &cursor::NextBeat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::NextBeat, window, cx);
    }

    fn cursor_prev_beat(
        &mut self,
        _: &cursor::PrevBeat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::PrevBeat, window, cx);
    }

    fn cursor_note_end(
        &mut self,
        _: &cursor::NoteEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::NoteEnd, window, cx);
    }

    fn cursor_last_note_end_in_bar(
        &mut self,
        _: &cursor::LastNoteEndInBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_motion(Motion::LastNoteEndInBar, window, cx);
    }

    // ---------- 텍스트 객체 ----------

    /// 텍스트 객체: 오퍼레이터 대기면 그 노트에 작용, 비주얼이면 선택을 그 노트로(계획 5절).
    fn apply_object(&mut self, obj: TextObject, window: &mut Window, cx: &mut Context<Self>) {
        // 객체 앞의 카운트는 쓰지 않는다
        let _ = keymap::take_count(cx);
        if let Some(p) = self.vim.pending.take() {
            self.apply_operator(p.op, OperatorTarget::Object(obj), window, cx);
            return;
        }
        if self.pianoroll.visual().is_some() && self.in_piano_roll_normal() {
            let picked = self.clip_under_cursor(cx).and_then(|(track, clip_id)| {
                let doc = self.document.read(cx);
                let project = doc.project();
                let clip = project.track(track)?.clip(clip_id)?;
                let notes = object_notes(clip, project.meter_map(), self.pianoroll.cursor(), obj);
                Some((track, clip_id, notes))
            });
            match picked {
                Some((track, clip, notes)) => self.replace_selection(track, clip, notes, cx),
                None => self.say("커서가 클립 밖이다", cx),
            }
            return;
        }
        self.say(
            "텍스트 객체(i n, i b, a c)는 오퍼레이터(d y c)나 비주얼(v) 뒤에 쓴다",
            cx,
        );
    }

    fn object_inner_note(
        &mut self,
        _: &object::InnerNote,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_object(TextObject::InnerNote, window, cx);
    }

    fn object_inner_bar(
        &mut self,
        _: &object::InnerBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_object(TextObject::InnerBar, window, cx);
    }

    fn object_around_clip(
        &mut self,
        _: &object::AroundClip,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_object(TextObject::AroundClip, window, cx);
    }

    // ---------- 비주얼 ----------

    /// `v` `V` `Ctrl+v`: 시작, 종류 바꾸기, 같은 키면 끝(계획 6절).
    fn toggle_visual(&mut self, kind: VisualKind, cx: &mut Context<Self>) {
        if !self.in_piano_roll_normal() {
            self.say(
                "비주얼 선택(v)은 피아노롤의 Normal에서 쓴다(g a로 피아노롤)",
                cx,
            );
            return;
        }
        self.vim.pending = None;
        match self.pianoroll.visual() {
            Some(current) if current == kind => {
                self.end_visual(cx);
                return;
            }
            Some(_) => {}
            None => self.pianoroll.set_anchor(Some(self.pianoroll.cursor())),
        }
        self.pianoroll.set_visual(Some(kind));
        self.recompute_visual(cx);
        cx.notify();
    }

    /// 비주얼을 끝내고 선택과 기준 칸을 비운다.
    fn end_visual(&mut self, cx: &mut Context<Self>) {
        self.pianoroll.set_anchor(None);
        self.set_selection(crate::pianoroll::Selection::default(), cx);
    }

    /// 비주얼 중: 기준 칸과 커서 칸으로 선택을 다시 계산한다. 클립은 기준 칸을 품은 클립, 없으면 커서 칸을 품은
    /// 클립이다(S3-T2 넓히기 규칙의 앞 두 단계).
    pub(crate) fn recompute_visual(&mut self, cx: &mut Context<Self>) {
        let (Some(kind), Some(anchor)) = (self.pianoroll.visual(), self.pianoroll.anchor()) else {
            return;
        };
        let cursor = self.pianoroll.cursor();
        let step = self.pianoroll.step_ticks();
        let picked = self.current_track().and_then(|track_id| {
            let doc = self.document.read(cx);
            let track = doc.project().track(track_id)?;
            let within = |tick: Tick| {
                track
                    .clips()
                    .find(|(_, c)| c.start() <= tick && tick.get() < c.end())
            };
            let (clip_id, clip) = within(anchor.tick).or_else(|| within(cursor.tick))?;
            Some((
                track_id,
                clip_id,
                visual_notes(clip, anchor, cursor, step, kind),
            ))
        });
        match picked {
            Some((track, clip, notes)) => self.replace_selection(track, clip, notes, cx),
            None => self.set_selection(crate::pianoroll::Selection::default(), cx),
        }
    }

    fn select_visual_note(
        &mut self,
        _: &select::VisualNote,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_visual(VisualKind::Note, cx);
    }

    fn select_visual_time(
        &mut self,
        _: &select::VisualTime,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_visual(VisualKind::Time, cx);
    }

    fn select_visual_block(
        &mut self,
        _: &select::VisualBlock,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_visual(VisualKind::Block, cx);
    }

    /// 처리기를 단다(`piano_roll_handlers`에서).
    pub(crate) fn vim_handlers(element: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        element
            .on_action(cx.listener(Self::operator_delete))
            .on_action(cx.listener(Self::operator_yank))
            .on_action(cx.listener(Self::operator_change))
            .on_action(cx.listener(Self::operator_cancel))
            .on_action(cx.listener(Self::object_inner_note))
            .on_action(cx.listener(Self::object_inner_bar))
            .on_action(cx.listener(Self::object_around_clip))
            .on_action(cx.listener(Self::select_visual_note))
            .on_action(cx.listener(Self::select_visual_time))
            .on_action(cx.listener(Self::select_visual_block))
            .on_action(cx.listener(Self::cursor_next_beat))
            .on_action(cx.listener(Self::cursor_prev_beat))
            .on_action(cx.listener(Self::cursor_note_end))
            .on_action(cx.listener(Self::cursor_last_note_end_in_bar))
            .on_action(cx.listener(Self::edit_change_to_bar_end))
    }
}
