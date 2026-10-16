//! 모델 → 스케줄러 이벤트(REQ SEQ-7, 5.10 뮤트·솔로, 결정 2026-10-12 Q1 숨김 꼬리).
//!
//! 재생 도구(`ksq-play`)와 앱(S2)이 같은 변환을 쓴다.

use keyscore_core::{Project, Tick, Track, Velocity};

use crate::scheduler::order_class;
use crate::{ScheduledEvent, bank_select, note_off, note_on, program_change};

/// 펼친 이벤트와 곡 끝.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    events: Vec<ScheduledEvent>,
    end: Tick,
}

impl Rendered {
    #[must_use]
    pub fn events(&self) -> &[ScheduledEvent] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Vec<ScheduledEvent> {
        self.events
    }

    /// 가장 늦은 이벤트(대개 마지막 Note Off)의 틱. 이벤트가 없으면 0.
    #[must_use]
    pub const fn end(&self) -> Tick {
        self.end
    }
}

/// 프로젝트를 스케줄러 이벤트로 펼친다.
///
/// - 뮤트된 트랙은 빼고, 솔로 트랙이 하나라도 있으면 솔로 트랙만 넣는다.
/// - 숨김 꼬리는 클립 끝에서 끈다.
/// - 트랙의 뱅크·프로그램은 틱 0에 CC 0, CC 32, 프로그램 체인지 순으로 보낸다.
/// - `port_of(트랙 인덱스, 트랙)`이 스냅샷의 포트 번호를 정한다. `None`이면 그 트랙은 넣지 않는다.
///
/// 이벤트는 스냅샷과 같은 순서(틱, 같은 틱에서는 Note Off, 그 밖, Note On)로 정렬되어 있다.
/// 피치 오프셋(SEQ-3의 소수 음높이)은 MIDI 1.0 노트에 담을 수 없어 쓰지 않는다(벤드 레인은 S6).
pub fn render(
    project: &Project,
    mut port_of: impl FnMut(usize, &Track) -> Option<u16>,
) -> Rendered {
    let any_solo = project.tracks().iter().any(Track::solo);
    let mut events = Vec::new();
    for (i, track) in project.tracks().iter().enumerate() {
        // 솔로가 있으면 솔로만 들린다. 뮤트이면서 솔로인 트랙은 솔로가 이긴다.
        let audible = if any_solo {
            track.solo()
        } else {
            !track.mute()
        };
        if !audible {
            continue;
        }
        let Some(port) = port_of(i, track) else {
            continue;
        };
        let ch = track.channel();
        let at_zero = |msg| ScheduledEvent::new(Tick::ZERO, port, msg);
        if let Some(bank) = track.bank() {
            let [msb, lsb] = bank_select(ch, bank);
            events.push(at_zero(msb));
            events.push(at_zero(lsb));
        }
        if let Some(program) = track.program() {
            events.push(at_zero(program_change(ch, program)));
        }
        for (_, clip) in track.clips() {
            let base = clip.start().get();
            for n in clip.notes() {
                let start = base.saturating_add(n.start().get());
                // 숨김 꼬리는 클립 끝에서 끈다(결정 2026-10-12 Q1).
                let end = base.saturating_add(n.end().min(clip.length()));
                events.push(ScheduledEvent::new(
                    Tick::new(start),
                    port,
                    note_on(ch, n.pitch(), n.velocity()),
                ));
                events.push(ScheduledEvent::new(
                    Tick::new(end),
                    port,
                    note_off(ch, n.pitch(), Velocity::new(0)),
                ));
            }
        }
    }
    events.sort_by_key(|e| (e.tick(), order_class(e.msg())));
    let end = events.last().map_or(Tick::ZERO, ScheduledEvent::tick);
    Rendered { events, end }
}
