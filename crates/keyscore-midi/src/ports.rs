//! OS 출력 포트 목록의 순수 로직: 목록 비교, 이름 기반 재연결, 대체 매핑 제안(OUT-3, OUT-8).
//!
//! CoreMIDI 호출과 분리해 두어, 핫플러그 순서와 재연결 규칙을 장치 없이 시험한다.

/// OS 출력 포트(CoreMIDI 목적지) 하나.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortInfo {
    name: String,
    unique_id: i32,
}

impl PortInfo {
    #[must_use]
    pub fn new(name: impl Into<String>, unique_id: i32) -> Self {
        PortInfo {
            name: name.into(),
            unique_id,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// CoreMIDI `kMIDIPropertyUniqueID`. 같은 장치가 다시 꽂혀도 대개 같은 값이다.
    #[must_use]
    pub fn unique_id(&self) -> i32 {
        self.unique_id
    }
}

/// 포트 목록의 변화.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortEvent {
    Disappeared(PortInfo),
    Appeared(PortInfo),
}

/// 두 목록을 비교해 이벤트를 만든다. 사라진 것을 모두 먼저, 나타난 것을 그 뒤에 내며, 각 무리는
/// 이름순(같으면 고유 ID순)이다. 같은 ID의 이름이 바뀌면 옛 이름 `Disappeared`, 새 이름 `Appeared`다.
#[must_use]
pub fn diff_ports(before: &[PortInfo], after: &[PortInfo]) -> Vec<PortEvent> {
    let _ = (before, after);
    todo!()
}

/// 이름으로 원하는 포트의 연결 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Connected(PortInfo),
    Missing,
}

/// 재연결 상태 기계가 내는 동작.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 연결된 포트가 사라졌다. 상태줄에 표시한다(OUT-3).
    Lost { name: String },
    /// 같은 이름의 포트가 나타나 다시 연결한다.
    Reconnect { name: String, port: PortInfo },
}

/// 이름 기반 자동 재연결(OUT-3).
#[derive(Debug, Default)]
pub struct Reconnector {}

impl Reconnector {
    #[must_use]
    pub fn new() -> Self {
        Reconnector::default()
    }

    /// `name`인 포트를 원한다고 등록하고, 지금 목록에서 연결 상태를 정한다.
    pub fn want(&mut self, name: &str, now: &[PortInfo]) -> &Link {
        let _ = (name, now);
        todo!()
    }

    /// 등록된 이름의 현재 상태.
    #[must_use]
    pub fn link(&self, name: &str) -> Option<&Link> {
        let _ = name;
        todo!()
    }

    /// 이벤트 하나를 반영하고 필요한 동작을 돌려준다.
    pub fn apply(&mut self, event: &PortEvent) -> Vec<Action> {
        let _ = event;
        todo!()
    }
}

/// 저장된 포트 이름의 대체 매핑 제안(OUT-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suggestion {
    /// 같은 이름의 포트가 있다.
    Present(PortInfo),
    /// 없으므로 같은 이름의 가상 포트를 만들어 쓴다.
    Virtual { name: String },
}

/// 저장된 이름마다 제안을 낸다(입력 순서 그대로).
#[must_use]
pub fn suggest_mapping(saved: &[String], available: &[PortInfo]) -> Vec<(String, Suggestion)> {
    let _ = (saved, available);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::{
        Action, Link, PortEvent, PortInfo, Reconnector, Suggestion, diff_ports, suggest_mapping,
    };

    fn p(name: &str, id: i32) -> PortInfo {
        PortInfo::new(name, id)
    }

    #[test]
    fn diff_orders_disappeared_before_appeared_by_name() {
        let before = [p("Synth B", 2), p("Synth A", 1), p("IAC", 3)];
        let after = [p("Synth C", 5), p("IAC", 3), p("Drum", 4), p("Synth A2", 1)];
        assert_eq!(
            diff_ports(&before, &after),
            vec![
                PortEvent::Disappeared(p("Synth A", 1)),
                PortEvent::Disappeared(p("Synth B", 2)),
                PortEvent::Appeared(p("Drum", 4)),
                PortEvent::Appeared(p("Synth A2", 1)),
                PortEvent::Appeared(p("Synth C", 5)),
            ]
        );
        assert_eq!(diff_ports(&after, &after), vec![]);
        // 이름이 같으면 ID순.
        assert_eq!(
            diff_ports(&[], &[p("X", 9), p("X", 2)]),
            vec![
                PortEvent::Appeared(p("X", 2)),
                PortEvent::Appeared(p("X", 9))
            ]
        );
    }

    #[test]
    fn reconnector_reconnects_when_same_name_returns() {
        let mut r = Reconnector::new();
        assert_eq!(
            r.want("Synth", &[p("Synth", 7), p("Other", 8)]),
            &Link::Connected(p("Synth", 7))
        );
        assert_eq!(r.apply(&PortEvent::Disappeared(p("Other", 8))), vec![]);
        assert_eq!(
            r.apply(&PortEvent::Disappeared(p("Synth", 7))),
            vec![Action::Lost {
                name: "Synth".into()
            }]
        );
        assert_eq!(r.link("Synth"), Some(&Link::Missing));
        // 다른 이름은 연결하지 않는다.
        assert_eq!(r.apply(&PortEvent::Appeared(p("synth", 9))), vec![]);
        assert_eq!(
            r.apply(&PortEvent::Appeared(p("Synth", 11))),
            vec![Action::Reconnect {
                name: "Synth".into(),
                port: p("Synth", 11)
            }]
        );
        assert_eq!(r.link("Synth"), Some(&Link::Connected(p("Synth", 11))));
        // 이미 연결돼 있으면 같은 이름이 하나 더 나타나도 바꾸지 않는다.
        assert_eq!(r.apply(&PortEvent::Appeared(p("Synth", 3))), vec![]);
        assert_eq!(r.link("Synth"), Some(&Link::Connected(p("Synth", 11))));
        // 처음부터 없던 이름은 Missing으로 시작한다.
        assert_eq!(r.want("Ghost", &[]), &Link::Missing);
        assert_eq!(r.link("Nope"), None);
    }

    #[test]
    fn reconnector_picks_lowest_id_among_same_names() {
        let mut r = Reconnector::new();
        assert_eq!(
            r.want("Dup", &[p("Dup", 30), p("Dup", 10), p("Dup", 20)]),
            &Link::Connected(p("Dup", 10))
        );
        // 연결된 것이 사라지면, 남은 같은 이름 가운데 가장 작은 ID로 바로 다시 연결한다.
        assert_eq!(
            r.apply(&PortEvent::Disappeared(p("Dup", 10))),
            vec![
                Action::Lost { name: "Dup".into() },
                Action::Reconnect {
                    name: "Dup".into(),
                    port: p("Dup", 20)
                }
            ]
        );
    }

    #[test]
    fn suggest_mapping_present_or_virtual() {
        let available = [p("IAC Bus 1", 1), p("Surge XT", 2), p("Surge XT", 0)];
        let saved = vec![
            "Surge XT".to_string(),
            "Dexed".to_string(),
            "iac bus 1".to_string(),
        ];
        assert_eq!(
            suggest_mapping(&saved, &available),
            vec![
                (
                    "Surge XT".to_string(),
                    Suggestion::Present(p("Surge XT", 0))
                ),
                (
                    "Dexed".to_string(),
                    Suggestion::Virtual {
                        name: "Dexed".into()
                    }
                ),
                (
                    "iac bus 1".to_string(),
                    Suggestion::Virtual {
                        name: "iac bus 1".into()
                    }
                ),
            ]
        );
    }
}
