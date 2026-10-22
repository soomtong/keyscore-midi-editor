//! 없는 출력 포트의 대체 매핑 제안(OUT-8, 결정 `2026-10-22-out8-port-mapping-sheet.md`).
//!
//! 다른 Mac에서 만든 프로젝트의 트랙은 이 Mac에 없는 포트 이름을 가리킬 수 있다. 파일을 열거나 복구한 창에
//! 시트로 대체 매핑을 제안하고, 받아들이면 같은 이름의 앱 가상 포트를 만든다. 트랙은 고치지 않아 원래 Mac에서
//! 열어도 그대로 맞는다. 가상 포트로 만들 수 없는 이름만 트랙을 바꾼다.

use std::collections::HashSet;

use gpui_kit::{App, Context, Entity, PromptButton, PromptLevel, Window};
use keyscore_core::{Edit, OutputPort, Project, TrackProp};
use keyscore_midi::{MAX_VIRTUAL_OUTPUTS, normalize_port_name, same_port_name};

use crate::actions::midi;
use crate::engine::{self, Engine};
use crate::window::ProjectWindow;

const MAP: &str = "제안대로 매핑";
const LEAVE: &str = "그대로 두기";

/// 없는 포트 이름 하나에 대한 제안.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mapping {
    /// 보이는 모양이 같은 기존 포트(앱 가상 포트 또는 목적지)로 트랙을 바꾼다. 같은 모양의 가상 포트는 만들 수
    /// 없다. 같은 시트에서 먼저 만들 포트일 수도 있다.
    Use(String),
    /// 이 이름(정규화한 것)으로 가상 포트를 만든다. 원래 이름과 다르면 트랙도 이 이름으로 바꾼다(엔진은 정확히
    /// 같은 이름만 잇는다).
    Create(String),
    /// 가상 포트를 만들 수 없어 기본 포트로 트랙을 바꾼다.
    Main(MainReason),
}

/// 가상 포트를 만들 수 없는 이유.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainReason {
    /// 가상 포트 이름 규칙에 맞지 않는다(제어 문자 등).
    InvalidName,
    /// 앱 가상 포트가 이미 16개다(OUT-2).
    TooMany,
}

/// 시트의 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// 트랙이 가리키는 이름(프로젝트에 저장된 그대로).
    pub saved: String,
    /// 제안.
    pub mapping: Mapping,
    /// 이 이름을 쓰는 트랙 수.
    pub tracks: usize,
}

/// 없는 포트마다 제안을 낸다(트랙 순서, 같은 이름은 한 줄). `own`은 앱 가상 포트 이름(기본 포트가 먼저),
/// `destinations`는 지금 보이는 CoreMIDI 목적지 이름이다. 「없는」은 엔진이 포트를 여는 규칙(정확히 같은
/// 이름)으로 판단한다.
#[must_use]
pub fn plan(project: &Project, own: &[String], destinations: &[String]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for track in project.tracks() {
        let OutputPort::Named(saved) = track.port() else {
            continue;
        };
        if let Some(line) = lines.iter_mut().find(|l| l.saved == *saved) {
            line.tracks += 1;
        } else if !own.iter().chain(destinations).any(|n| n == saved) {
            lines.push(Line {
                saved: saved.clone(),
                mapping: Mapping::Main(MainReason::InvalidName),
                tracks: 1,
            });
        }
    }
    // 앞 줄이 만들 포트를 뒤 줄이 본다(같은 정규화 이름의 둘째 줄, 16개 상한)
    let mut created: Vec<String> = Vec::new();
    for line in &mut lines {
        line.mapping = mapping_for(&line.saved, own, destinations, &mut created);
    }
    lines
}

fn mapping_for(
    saved: &str,
    own: &[String],
    destinations: &[String],
    created: &mut Vec<String>,
) -> Mapping {
    let Ok(name) = normalize_port_name(saved) else {
        return Mapping::Main(MainReason::InvalidName);
    };
    // 앱 포트가 같은 모양의 목적지보다 먼저다(엔진의 `open_port`와 같은 차례)
    if let Some(existing) = own
        .iter()
        .chain(created.iter())
        .chain(destinations)
        .find(|n| same_port_name(n, &name))
    {
        return Mapping::Use(existing.clone());
    }
    if own.len() + created.len() >= MAX_VIRTUAL_OUTPUTS {
        return Mapping::Main(MainReason::TooMany);
    }
    created.push(name.clone());
    Mapping::Create(name)
}

/// 시트에 보일 줄 설명. `main`은 기본 포트 이름이다.
#[must_use]
pub fn describe(line: &Line, main: &str) -> String {
    let saved = shown(&line.saved);
    let n = line.tracks;
    match &line.mapping {
        Mapping::Create(name) if *name == line.saved => {
            format!("「{saved}」: 같은 이름의 가상 포트를 만든다(트랙 {n}개)")
        }
        Mapping::Create(name) => {
            format!("「{saved}」: 가상 포트 「{name}」을 만들고 트랙 {n}개를 그 이름으로 바꾼다")
        }
        Mapping::Use(name) => format!("「{saved}」: 트랙 {n}개를 「{}」로 바꾼다", shown(name)),
        Mapping::Main(MainReason::InvalidName) => format!(
            "「{saved}」: 가상 포트 이름으로 쓸 수 없어 트랙 {n}개를 기본 포트 「{main}」으로 바꾼다"
        ),
        Mapping::Main(MainReason::TooMany) => format!(
            "「{saved}」: 가상 포트가 {MAX_VIRTUAL_OUTPUTS}개에 이르러 트랙 {n}개를 기본 포트 「{main}」으로 바꾼다"
        ),
    }
}

/// 이름 규칙이 거부하는 문자(제어 문자, 양방향 제어, 폭 없는 문자)는 보이게 적는다. 다른 Mac의 장치 이름이
/// 시트 글자를 뒤집거나 거부 이유를 숨기지 않게 한다(리뷰 1 지적 3).
fn shown(name: &str) -> String {
    name.chars()
        .map(|c| {
            if keyscore_midi::is_rejected_name_char(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// 지금 포트 상태로 이 프로젝트의 제안을 낸다. MIDI 엔진이 없으면 `None`(포트를 만들 수 없다).
#[must_use]
pub fn plan_for(cx: &App, project: &Project) -> Option<Vec<Line>> {
    if !cx.has_global::<Engine>() {
        return None;
    }
    let own = engine::virtual_port_names(cx);
    let destinations: Vec<String> = keyscore_midi::destinations()
        .iter()
        .map(|d| d.name().to_owned())
        .collect();
    Some(plan(project, &own, &destinations))
}

/// 이 창의 문서에 없는 포트가 있으면 시트로 대체 매핑을 제안한다. 엔진이 없거나 없는 포트가 없으면 아무것도
/// 하지 않는다. 시트는 그 창의 파일 흐름으로 띄운다(다른 시트와 겹치지 않게).
pub fn offer(view: &Entity<ProjectWindow>, cx: &mut App) {
    let Some(lines) = plan_for(cx, view.read(cx).document().read(cx).project()) else {
        return;
    };
    if lines.is_empty() {
        return;
    }
    view.update(cx, |w, cx| w.ask_port_mapping(&lines, cx));
}

/// 지금 상태로 다시 계산해 적용한다. 시트가 떠 있는 동안 포트가 생겼거나 다른 창이 먼저 만들었을 수 있다.
fn apply(view: &Entity<ProjectWindow>, cx: &mut App) {
    let Some(lines) = plan_for(cx, view.read(cx).document().read(cx).project()) else {
        view.update(cx, |w, cx| w.say(NO_ENGINE, cx));
        return;
    };
    if lines.is_empty() {
        view.update(cx, |w, cx| w.say(NOTHING_MISSING, cx));
        return;
    }
    let mut created = 0;
    let mut failed: HashSet<String> = HashSet::new();
    let mut failures = Vec::new();
    let mut targets: Vec<(&str, OutputPort)> = Vec::new();
    for line in &lines {
        match &line.mapping {
            Mapping::Create(name) => match engine::add_named_virtual_port(cx, name) {
                Ok(made) => {
                    created += 1;
                    if made != line.saved {
                        targets.push((&line.saved, OutputPort::Named(made)));
                    }
                }
                Err(e) => {
                    failed.insert(name.clone());
                    failures.push(format!("「{}」: {e}", shown(&line.saved)));
                }
            },
            // 같은 시트에서 만들지 못한 포트로는 바꾸지 않는다(그대로 「포트 없음」)
            Mapping::Use(name) if failed.contains(name) => {}
            Mapping::Use(name) => targets.push((&line.saved, OutputPort::Named(name.clone()))),
            Mapping::Main(_) => targets.push((&line.saved, OutputPort::Virtual)),
        }
    }
    let edits: Vec<Edit> = view
        .read(cx)
        .document()
        .read(cx)
        .project()
        .tracks()
        .iter()
        .filter_map(|t| {
            let OutputPort::Named(name) = t.port() else {
                return None;
            };
            let (_, to) = targets.iter().find(|(saved, _)| saved == name)?;
            Some(Edit::SetTrackProp {
                track: t.id(),
                prop: TrackProp::Port(to.clone()),
            })
        })
        .collect();
    view.update(cx, |w, cx| {
        let count = edits.len();
        // 창마다 undo 한 단위(가상 포트 이름 변경과 같다). 만든 가상 포트는 설정에 남는다
        let changed = if count > 0 && w.edit_document(edits, cx) {
            count
        } else {
            0
        };
        let mut summary =
            format!("포트 매핑: 가상 포트 {created}개를 만들고 트랙 {changed}개를 바꿨다");
        if !failures.is_empty() {
            summary.push_str(". 만들지 못한 포트: ");
            summary.push_str(&failures.join(", "));
        }
        w.say(summary, cx);
    });
}

const NO_ENGINE: &str = "MIDI 엔진이 없다: 가상 포트를 만들 수 없어 포트 매핑을 제안하지 않는다";
const NOTHING_MISSING: &str = "없는 출력 포트가 없다";

impl ProjectWindow {
    /// `midi::MapMissingPorts`: 시트를 다시 연다. 없는 포트가 없거나 엔진이 없으면 상태줄로 알린다.
    pub(crate) fn midi_map_missing_ports(
        &mut self,
        _: &midi::MapMissingPorts,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match plan_for(cx, self.document().read(cx).project()) {
            None => self.say(NO_ENGINE, cx),
            Some(lines) if lines.is_empty() => self.say(NOTHING_MISSING, cx),
            Some(lines) => self.ask_port_mapping(&lines, cx),
        }
    }

    /// 시트로 묻고, 받아들이면 적용한다. 이미 흐름이 돌고 있으면 띄우지 않는다(창마다 흐름 하나).
    fn ask_port_mapping(&mut self, lines: &[Line], cx: &mut Context<Self>) {
        let main = engine::virtual_port_names(cx)
            .into_iter()
            .next()
            .unwrap_or_default();
        let message = format!("이 Mac에 없는 출력 포트가 {}개 있다", lines.len());
        let detail = lines
            .iter()
            .map(|l| describe(l, &main))
            .collect::<Vec<_>>()
            .join("\n");
        self.run_file_flow(cx, async move |view, cx| {
            let handle = cx.update(|cx| view.read(cx).window_handle());
            let answer = crate::files::ask(
                handle,
                PromptLevel::Info,
                &message,
                Some(&detail),
                &[PromptButton::ok(MAP), PromptButton::cancel(LEAVE)],
                cx,
            )
            .await;
            if answer == Some(0) {
                cx.update(|cx| apply(view, cx));
            }
        });
    }
}
