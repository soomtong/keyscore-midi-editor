//! 모델 → 스케줄러 이벤트(REQ SEQ-7, 5.10 뮤트·솔로, 결정 2026-10-12 Q1 숨김 꼬리).
//!
//! 재생 도구(`ksq-play`)와 앱(S2)이 같은 변환을 쓴다.

use keyscore_core::{Project, Tick, Track};

use crate::ScheduledEvent;

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
pub fn render(project: &Project, port_of: impl FnMut(usize, &Track) -> Option<u16>) -> Rendered {
    let _ = (project, port_of);
    todo!()
}
