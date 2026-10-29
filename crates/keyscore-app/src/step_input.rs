//! Step 입력 창 처리기(S3-T4a, REQ 5.1, 5.6, 5.10, 5.11). 상태는 `step`, 규칙은
//! `docs/decisions/2026-10-29-s3-t4-step-input-details.md`.
//!
//! 음 키는 조합을 열고 확정이 노트를 쓴다. 쓰기는 삽입이다: 커서가 든 클립에서 커서 뒤 노트를 민다(core
//! `transform::splice`). 확정 하나가 `Document::apply` 하나, 곧 undo 한 단위다.

use std::time::Duration;

use gpui_kit::{Context, Div, InteractiveElement as _, Stateful, Window};
use keyscore_core::transform::{Splice, clip_for_notes, splice};
use keyscore_core::{ClipId, Note, Pitch, Tick, TrackId};

use crate::actions::args::{AccidentalKind, KeyLayout};
use crate::actions::step;
use crate::app_state::AppSettings;
use crate::mode::Mode;
use crate::pianoroll::Cursor;
use crate::pianoroll::keys::semitone_of_key;
use crate::step::{
    Composing, LastStep, OCTAVE_MAX, OpenChord, StepLength, StepStatus, code_digit, pitch_of,
    semitone_name, step_ticks,
};
use crate::window::ProjectWindow;

/// Step 모드의 키 컨텍스트 경로(바깥 → 안). 레이아웃, 조합 중 `Pending`, 고스트 `Ghost`.
const PATHS: [[&[&str]; 4]; 2] = [
    [
        &["Step", "Piano"],
        &["Step", "Piano", "Pending"],
        &["Step", "Piano", "Ghost"],
        &["Step", "Piano", "Pending", "Ghost"],
    ],
    [
        &["Step", "Solfege"],
        &["Step", "Solfege", "Pending"],
        &["Step", "Solfege", "Ghost"],
        &["Step", "Solfege", "Pending", "Ghost"],
    ],
];

/// 노트를 쓸 자리: 있던 클립(클립 기준 시작) 또는 틈.
enum Host {
    Clip { id: ClipId, start: u64 },
    Gap,
}

impl ProjectWindow {
    // ---------- 상태 ----------

    /// Step 상태(길이·옥타브·조합 표시). 시험·접근성용.
    #[must_use]
    pub fn step_status(&self) -> StepStatus {
        let s = &self.step;
        StepStatus {
            length: s.length,
            octave: s.octave,
            composing: s
                .composing
                .as_ref()
                .map(|c| format!("{}{}_", s.prefix, c.shown)),
            chord_open: s.chord.is_some(),
        }
    }

    /// Step 모드의 키 컨텍스트 경로. FSM 경로(`Step`, `Step > Ghost`)에 레이아웃과 조합 중 `Pending`을 더한다.
    pub(crate) fn step_context_path(&self) -> &'static [&'static str] {
        let layout = usize::from(self.step.layout == KeyLayout::Solfege);
        let pending = usize::from(self.step.composing.is_some());
        let ghost = usize::from(self.mode_state().has_ghost());
        PATHS[layout][pending + 2 * ghost]
    }

    /// 상태줄 모드 표시(`STEP ♩ oct4`, 조합 중 `STEP 1/16 oct4 5c_`).
    pub(crate) fn step_label(&self) -> String {
        self.step.label()
    }

    /// 그리기 전에: 설정의 레이아웃을 읽어 둔다(키 컨텍스트는 `&self`에서 고른다).
    pub(crate) fn refresh_step_layout(&mut self, cx: &Context<Self>) {
        self.step.layout = AppSettings::get(cx).input.key_layout;
    }

    fn in_step(&self) -> bool {
        self.mode_state().mode() == Mode::Step
    }

    /// 지금 스텝 길이(셋잇단·절반 없이).
    fn plain_step(&self) -> u64 {
        u64::from(step_ticks(self.step.length, false, false))
    }

    // ---------- 확정 ----------

    /// 조합을 확정하고 열린 화음을 닫는다. 다른 Step 동작·모드 나가기·undo 전에 부른다.
    pub(crate) fn step_settle(&mut self, cx: &mut Context<Self>) {
        self.step_commit(cx);
        self.step_close_chord(cx);
    }

    /// 열린 화음을 닫는다: 스텝 길이만큼 전진하고 직전 스텝이 된다.
    fn step_close_chord(&mut self, cx: &mut Context<Self>) {
        if let Some(chord) = self.step.chord.take() {
            let end = Tick::new(chord.at.get() + chord.len);
            self.step.push_step(chord.at, end);
            self.step_move_to(end, None, cx);
            self.step.last = Some(LastStep {
                track: chord.track,
                clip: chord.clip,
                notes: chord.notes,
                end,
            });
        }
    }

    /// 조합 중인 음을 확정한다(노트 쓰기 + 전진, 화음 음은 전진 없음). 조합이 없으면 아무 일도 없다.
    pub(crate) fn step_commit(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.step.composing.take() else {
            return;
        };
        self.step.prefix.clear();
        cx.notify();
        let Some(pitch) = c.pitch() else {
            self.say("Step: 음높이가 0–127을 벗어나 쓰지 않았다", cx);
            return;
        };
        let len = step_ticks(self.step.length, c.triplet, c.half);
        let chord = self.step.chord.clone().filter(|ch| ch.track == c.track);
        if self.step.chord.is_some() && chord.is_none() {
            // 다른 트랙의 화음은 그 자리에서 닫는다.
            self.step_close_chord(cx);
        }
        let (at, push) = match &chord {
            Some(ch) => (ch.at, 0),
            None => (self.pianoroll.cursor().tick, i64::from(len)),
        };
        let Some((clip, written)) = self.step_write(c.track, at, push, len, pitch, cx) else {
            return;
        };
        if c.sounded != Some(pitch) {
            self.step_sound(c.track, pitch, cx);
        }
        match (c.chord, chord) {
            // 화음을 이어 간다
            (true, Some(mut ch)) => {
                ch.notes.push(written);
                self.step.chord = Some(ch);
                self.step_move_to(at, Some(pitch), cx);
            }
            // 화음을 연다
            (true, None) => {
                self.step.chord = Some(OpenChord {
                    track: c.track,
                    clip,
                    at,
                    len: u64::from(len),
                    notes: vec![written],
                });
                self.step.last = None;
                self.step_move_to(at, Some(pitch), cx);
            }
            // 화음의 마지막 음: 닫고 전진
            (false, Some(mut ch)) => {
                ch.notes.push(written);
                self.step.chord = Some(ch);
                self.step_close_chord(cx);
                let tick = self.pianoroll.cursor().tick;
                self.step_move_to(tick, Some(pitch), cx);
            }
            (false, None) => {
                let end = Tick::new(at.get() + u64::from(len));
                self.step.push_step(at, end);
                self.step_move_to(end, Some(pitch), cx);
                self.step.last = Some(LastStep {
                    track: c.track,
                    clip,
                    notes: vec![written],
                    end,
                });
            }
        }
    }

    /// 커서를 옮기고(음높이는 주면 그것으로) 보이게 한다.
    fn step_move_to(&mut self, tick: Tick, pitch: Option<Pitch>, cx: &mut Context<Self>) {
        let cursor = self.pianoroll.cursor();
        self.pianoroll.set_cursor(Cursor {
            tick,
            pitch: pitch.unwrap_or(cursor.pitch),
        });
        self.pianoroll.ensure_cursor_visible();
        cx.notify();
    }

    /// 커서가 든 클립: 시작 ≤ 커서 < 끝, 없으면 끝 = 커서(이어 쓰기). 둘 다 없으면 틈.
    fn step_host(&self, track: TrackId, at: Tick, cx: &Context<Self>) -> Option<Host> {
        let doc = self.document.read(cx);
        let t = doc.project().track(track)?;
        let at = at.get();
        let inside = t
            .clips()
            .find(|(_, c)| c.start().get() <= at && at < c.end());
        let ending = || t.clips().find(|(_, c)| c.end() == at);
        Some(match inside.or_else(ending) {
            Some((id, c)) => Host::Clip {
                id,
                start: c.start().get(),
            },
            None => Host::Gap,
        })
    }

    /// 노트 하나를 `at`(절대)에 쓰고 커서 뒤 노트를 `push`만큼 민다. 성공하면 (클립, 쓴 노트(클립 기준)).
    fn step_write(
        &mut self,
        track: TrackId,
        at: Tick,
        push: i64,
        len: u32,
        pitch: Pitch,
        cx: &mut Context<Self>,
    ) -> Option<(ClipId, Note)> {
        let host = self.step_host(track, at, cx);
        let Some(host) = host else {
            self.say("Step: 트랙이 없다", cx);
            return None;
        };
        let velocity = self
            .document
            .read(cx)
            .project()
            .track(track)?
            .default_velocity();
        let planned = {
            let doc = self.document.read(cx);
            let project = doc.project();
            match host {
                Host::Clip { id, start } => {
                    let rel = at.get() - start;
                    Note::new(Tick::new(rel), len, pitch, velocity).and_then(|n| {
                        let s = Splice {
                            at: rel,
                            by: push,
                            insert: vec![n.clone()],
                            ..Splice::default()
                        };
                        splice(project, track, id, &s).map(|edits| (edits, Some(id), n))
                    })
                }
                Host::Gap => Note::new(at, len, pitch, velocity).and_then(|n| {
                    clip_for_notes(project, track, std::slice::from_ref(&n), at)
                        .map(|edit| (vec![edit], None, n))
                }),
            }
        };
        let (edits, clip, note) = match planned {
            Ok(p) => p,
            Err(e) => {
                self.say(format!("Step: 쓰지 못했다: {e}"), cx);
                return None;
            }
        };
        if !self.apply_edit("Step", edits, None, cx) {
            return None;
        }
        self.step_edited(cx);
        if let Some(id) = clip {
            return Some((id, note));
        }
        // 새 클립의 id는 적용한 뒤에 정해진다. 같은 트랙에 그 노트를 품은 클립은 하나뿐이다.
        let doc = self.document.read(cx);
        let (id, c) = doc
            .project()
            .track(track)?
            .clips()
            .find(|(_, c)| c.start() <= at && at.get() < c.end())?;
        let rel = note
            .with_start(Tick::new(at.get() - c.start().get()))
            .ok()?;
        Some((id, rel))
    }

    /// Step이 문서를 바꿨다: 노트 선택을 비운다(밀린 노트를 가리키는 선택이 남지 않게).
    fn step_edited(&mut self, cx: &mut Context<Self>) {
        if !self.pianoroll.selection().is_empty() || self.pianoroll.anchor().is_some() {
            self.pianoroll
                .set_selection(crate::pianoroll::Selection::default());
            self.pianoroll.set_anchor(None);
            self.sync_selection_mode(cx);
        }
    }

    /// 미리듣기(PRE-1 「Step 입력 중 음 키」). 실패는 창마다 한 번만 알린다.
    fn step_sound(&mut self, track: TrackId, pitch: Pitch, cx: &mut Context<Self>) {
        let Some(t) = self.document.read(cx).project().track(track).cloned() else {
            return;
        };
        let length = AppSettings::get(cx).preview.length();
        let result = self.sound_note(&t, pitch, t.default_velocity(), length, cx);
        if let Some(message) = result.failure("미리듣기")
            && !self.preview_notice_shown
        {
            self.preview_notice_shown = true;
            self.say(format!("{message}(이 알림은 창마다 한 번만)"), cx);
        }
    }

    // ---------- 음 키 ----------

    fn step_key(&mut self, key: &str, chord: bool, half: bool, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        // 다음 음 키는 앞 조합을 확정한다. 화음은 열린 채 둔다(이 음이 닫거나 잇는다).
        self.step_commit(cx);
        let Some(track) = self.current_track() else {
            self.say("Step: 트랙이 없다", cx);
            return;
        };
        let Some(semitone) = semitone_of_key(self.step.layout, key) else {
            return;
        };
        let semitone = i16::from(semitone);
        let octave = self.step.octave;
        let Some(pitch) = pitch_of(octave, semitone) else {
            self.say("Step: 음높이가 0–127을 벗어난다", cx);
            return;
        };
        let triplet = std::mem::take(&mut self.step.triplet_next);
        self.step.generation += 1;
        let generation = self.step.generation;
        // 계이름형은 친 글자, 건반형은 그 키의 음이름을 보인다(키 글자는 음이름이 아니다).
        let shown = match self.step.layout {
            KeyLayout::Solfege => key.to_ascii_lowercase(),
            KeyLayout::Piano => semitone_name(semitone).to_owned(),
        };
        self.step.composing = Some(Composing {
            track,
            semitone,
            shown,
            octave,
            chord,
            half,
            triplet,
            sounded: Some(pitch),
            generation,
        });
        self.step_sound(track, pitch, cx);
        Self::step_wait(generation, cx);
        cx.notify();
    }

    /// 옥타브 숫자 대기(설정 `input.octave_wait_ms`). 만료되면 그 세대의 조합을 확정한다.
    fn step_wait(generation: u64, cx: &mut Context<Self>) {
        let wait = Duration::from_millis(u64::from(AppSettings::get(cx).input.octave_wait_ms));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .step
                    .composing
                    .as_ref()
                    .is_some_and(|c| c.generation == generation)
                {
                    this.step_commit(cx);
                }
            });
        })
        .detach();
    }

    fn step_note(&mut self, a: &step::Note, _: &mut Window, cx: &mut Context<Self>) {
        self.step_key(&a.0, false, false, cx);
    }

    fn step_chord(&mut self, a: &step::Chord, _: &mut Window, cx: &mut Context<Self>) {
        self.step_key(&a.0, true, false, cx);
    }

    fn step_half_note(&mut self, a: &step::HalfNote, _: &mut Window, cx: &mut Context<Self>) {
        self.step_key(&a.0, false, true, cx);
    }

    fn step_set_octave(&mut self, a: &step::SetOctave, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        let Some(octave) = u8::try_from(a.0.0).ok().filter(|o| *o <= OCTAVE_MAX) else {
            return;
        };
        self.step.octave = octave;
        if let Some(c) = &mut self.step.composing {
            c.octave = octave;
        }
        self.step_commit(cx);
        cx.notify();
    }

    fn step_accidental(&mut self, a: &step::Accidental, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        let Some(c) = &mut self.step.composing else {
            self.say("변화표는 음이름 바로 뒤에 친다", cx);
            return;
        };
        let (delta, mark) = match a.0 {
            AccidentalKind::Sharp => (1, '#'),
            AccidentalKind::Flat => (-1, '-'),
        };
        c.semitone += delta;
        c.shown.push(mark);
        // 변화표 뒤에도 옥타브 숫자를 기다린다.
        self.step.generation += 1;
        c.generation = self.step.generation;
        let generation = c.generation;
        Self::step_wait(generation, cx);
        cx.notify();
    }

    fn step_commit_key(&mut self, _: &step::Commit, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step_commit(cx);
        self.step.prefix.clear();
        cx.notify();
    }

    // ---------- 길이·옥타브 ----------

    fn step_set_length(&mut self, a: &step::SetLength, _: &mut Window, cx: &mut Context<Self>) {
        self.step_length(StepLength::plain(a.0), cx);
    }

    fn step_set_dotted(
        &mut self,
        a: &step::SetDottedLength,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_length(
            StepLength {
                code: a.0,
                dotted: true,
            },
            cx,
        );
    }

    fn step_reset_length(&mut self, _: &step::ResetLength, _: &mut Window, cx: &mut Context<Self>) {
        let code = AppSettings::get(cx).input.step_default_length;
        self.step_length(StepLength::plain(code), cx);
    }

    fn step_length(&mut self, length: StepLength, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step_settle(cx);
        self.step.length = length;
        // 앞의 `n`(아직 쓰지 않은 셋잇단 접두)은 표시에 남긴다(리뷰 1 제안 6).
        self.step.prefix.retain(|ch| ch == 'n');
        self.step.prefix.push(code_digit(length.code));
        cx.notify();
    }

    fn step_triplet(&mut self, _: &step::Triplet, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        // 화음은 닫는다(결정 5절). 화음 안의 다른 옥타브는 음 뒤 숫자로 친다(리뷰 1 지적 3).
        self.step_settle(cx);
        self.step.triplet_next = true;
        self.step.prefix.push('n');
        cx.notify();
    }

    fn step_octave_by(&mut self, delta: i8, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step_settle(cx);
        match self.step.octave.checked_add_signed(delta) {
            Some(o) if o <= OCTAVE_MAX => self.step.octave = o,
            _ => self.say(format!("옥타브는 0–{OCTAVE_MAX}이다"), cx),
        }
        cx.notify();
    }

    fn step_octave_down(&mut self, _: &step::OctaveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.step_octave_by(-1, cx);
    }

    fn step_octave_up(&mut self, _: &step::OctaveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.step_octave_by(1, cx);
    }

    // ---------- 쉼표·지우기·이동 ----------

    fn step_rest(&mut self, _: &step::Rest, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step_settle(cx);
        let triplet = std::mem::take(&mut self.step.triplet_next);
        self.step.prefix.clear();
        let len = step_ticks(self.step.length, triplet, false);
        let at = self.pianoroll.cursor().tick;
        // 커서가 든 클립(끝 = 커서 포함)이면 커서 뒤 노트를 밀고, 쉼표가 클립 끝을 넘으면 클립을 늘린다(REQ 5.10,
        // 리뷰 1 지적 2). 틈에서는 클립을 만들지 않는다.
        if !self.step_rest_room(at, u64::from(len), cx) {
            return;
        }
        self.step.last = None;
        let end = Tick::new(at.get() + u64::from(len));
        self.step.push_step(at, end);
        self.step_move_to(end, None, cx);
    }

    /// 쉼표의 자리: 커서가 든 클립에서 `at` 이상의 노트를 `len`만큼 밀고 `at + len`까지 클립을 늘린다. 틈이거나
    /// 바꿀 것이 없으면 편집 없이 참. 거부되면 알리고 거짓.
    fn step_rest_room(&mut self, at: Tick, len: u64, cx: &mut Context<Self>) -> bool {
        let Some(track) = self.current_track() else {
            return true;
        };
        let Some(Host::Clip { id, start }) = self.step_host(track, at, cx) else {
            return true;
        };
        let rel = at.get() - start;
        let s = Splice {
            at: rel,
            by: i64::try_from(len).unwrap_or(i64::MAX),
            reach: rel + len,
            ..Splice::default()
        };
        let planned = splice(self.document.read(cx).project(), track, id, &s);
        match planned {
            Ok(edits) if edits.is_empty() => true,
            Ok(edits) => {
                let ok = self.apply_edit("Step", edits, None, cx);
                if ok {
                    self.step_edited(cx);
                }
                ok
            }
            Err(e) => {
                self.say(format!("Step: 쉼표 자리를 만들지 못했다: {e}"), cx);
                false
            }
        }
    }

    fn step_back(&mut self, _: &step::Back, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        // 조합 중이면 조합만 버린다.
        if self.step.composing.take().is_some() {
            self.step.prefix.clear();
            cx.notify();
            return;
        }
        self.step_close_chord(cx);
        self.step.last = None;
        let old = self.pianoroll.cursor().tick.get();
        let plain = self.plain_step();
        let new = self.step.step_back_from(Tick::new(old), plain).get();
        // 지울 스텝은 커서 바로 앞 틱을 품은 클립 안이다(클립 시작에서 자른다).
        let host = self.current_track().and_then(|track| {
            let doc = self.document.read(cx);
            let t = doc.project().track(track)?;
            let (id, c) = t
                .clips()
                .find(|(_, c)| old > 0 && c.start().get() < old && old <= c.end())?;
            Some((track, id, c.start().get(), c.notes().to_vec()))
        });
        if let Some((track, id, start, notes)) = host {
            let from = new.max(start) - start;
            let to = old - start;
            let s = Splice {
                at: to,
                by: -i64::try_from(to - from).unwrap_or(0),
                remove: notes
                    .into_iter()
                    .filter(|n| (from..to).contains(&n.start().get()))
                    .collect(),
                ..Splice::default()
            };
            let planned = splice(self.document.read(cx).project(), track, id, &s);
            match planned {
                Ok(edits) if edits.is_empty() => {}
                Ok(edits) => {
                    if !self.apply_edit("Step", edits, None, cx) {
                        return;
                    }
                    self.step_edited(cx);
                }
                Err(e) => {
                    self.say(format!("Step: 지우지 못했다: {e}"), cx);
                    return;
                }
            }
        }
        self.step_move_to(Tick::new(new), None, cx);
    }

    fn step_prev(&mut self, _: &step::Prev, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step_settle(cx);
        self.step.last = None;
        let at = self.pianoroll.cursor().tick;
        let plain = self.plain_step();
        let back = self.step.step_back_from(at, plain);
        self.step_move_to(back, None, cx);
    }

    fn step_confirm(&mut self, _: &step::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step.prefix.clear();
        if self.step.composing.is_some() {
            self.step_commit(cx);
            // 조합이 화음 음이었으면 화음까지 확정하고 전진한다(5.11 화음 줄).
            self.step_close_chord(cx);
        } else if self.step.chord.is_some() {
            self.step_close_chord(cx);
        } else {
            self.step.last = None;
            let at = self.pianoroll.cursor().tick;
            let end = Tick::new(at.get() + self.plain_step());
            self.step.push_step(at, end);
            self.step_move_to(end, None, cx);
        }
    }

    // ---------- 타이 ----------

    fn step_tie(&mut self, _: &step::Tie, _: &mut Window, cx: &mut Context<Self>) {
        if !self.in_step() {
            return;
        }
        self.step_settle(cx);
        let cursor = self.pianoroll.cursor().tick;
        let Some(last) = self
            .step
            .last
            .clone()
            .filter(|l| l.end == cursor && Some(l.track) == self.current_track())
        else {
            self.say("타이: 바로 앞에 친 음이 없다", cx);
            return;
        };
        let len = self.plain_step();
        let Ok(add) = u32::try_from(len) else {
            return;
        };
        let start = {
            let doc = self.document.read(cx);
            doc.project()
                .track(last.track)
                .and_then(|t| t.clip(last.clip))
                .map(|c| c.start().get())
        };
        let Some(start) = start else {
            self.say("타이: 바로 앞에 친 음이 없다", cx);
            return;
        };
        let replace: Result<Vec<(Note, Note)>, _> = last
            .notes
            .iter()
            .map(|n| {
                n.length()
                    .checked_add(add)
                    .ok_or(keyscore_core::ModelError::TickOverflow)
                    .and_then(|l| n.with_length(l))
                    .map(|to| (n.clone(), to))
            })
            .collect();
        let planned = replace.and_then(|replace| {
            let s = Splice {
                at: cursor.get() - start,
                by: i64::from(add),
                replace: replace.clone(),
                ..Splice::default()
            };
            let doc = self.document.read(cx);
            splice(doc.project(), last.track, last.clip, &s).map(|e| (e, replace))
        });
        let (edits, replace) = match planned {
            Ok(p) => p,
            Err(e) => {
                self.say(format!("타이: {e}"), cx);
                return;
            }
        };
        if !self.apply_edit("타이", edits, None, cx) {
            return;
        }
        self.step_edited(cx);
        let end = Tick::new(cursor.get() + len);
        self.step.extend_step(cursor, end);
        self.step.last = Some(LastStep {
            notes: replace.into_iter().map(|(_, to)| to).collect(),
            end,
            ..last
        });
        self.step_move_to(end, None, cx);
    }

    /// 처리기를 단다(`mode_handlers`에서).
    pub(crate) fn step_handlers(element: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        element
            .on_action(cx.listener(Self::step_note))
            .on_action(cx.listener(Self::step_chord))
            .on_action(cx.listener(Self::step_half_note))
            .on_action(cx.listener(Self::step_set_octave))
            .on_action(cx.listener(Self::step_accidental))
            .on_action(cx.listener(Self::step_commit_key))
            .on_action(cx.listener(Self::step_set_length))
            .on_action(cx.listener(Self::step_set_dotted))
            .on_action(cx.listener(Self::step_reset_length))
            .on_action(cx.listener(Self::step_triplet))
            .on_action(cx.listener(Self::step_octave_down))
            .on_action(cx.listener(Self::step_octave_up))
            .on_action(cx.listener(Self::step_rest))
            .on_action(cx.listener(Self::step_back))
            .on_action(cx.listener(Self::step_prev))
            .on_action(cx.listener(Self::step_confirm))
            .on_action(cx.listener(Self::step_tie))
    }
}
