//! OS 출력 포트 목록의 순수 로직: 목록 비교, 이름 기반 재연결, 대체 매핑 제안(OUT-3, OUT-8).
//!
//! CoreMIDI 호출과 분리해 두어, 핫플러그 순서와 재연결 규칙을 장치 없이 시험한다.

/// OS 출력 포트(CoreMIDI 목적지) 하나.
///
/// 같음은 이름과 고유 ID로 정한다. 시스템 안의 객체 참조(`endpoint`)는 같은 포트가 다시 만들어지면
/// 바뀌므로, 목록 비교(`diff_ports`)에서만 따로 본다.
#[derive(Debug, Clone)]
pub struct PortInfo {
    name: String,
    unique_id: i32,
    endpoint: u32,
}

impl PartialEq for PortInfo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.unique_id == other.unique_id
    }
}

impl Eq for PortInfo {}

impl std::hash::Hash for PortInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.unique_id.hash(state);
    }
}

impl PortInfo {
    #[must_use]
    pub fn new(name: impl Into<String>, unique_id: i32) -> Self {
        PortInfo {
            name: name.into(),
            unique_id,
            endpoint: 0,
        }
    }

    /// 열거에서 읽은 객체 참조를 붙인다.
    pub(crate) fn with_endpoint(mut self, endpoint: u32) -> Self {
        self.endpoint = endpoint;
        self
    }

    /// 목록 비교에 쓰는 정체: 이름, 고유 ID, 객체 참조.
    fn identity(&self) -> (&str, i32, u32) {
        (&self.name, self.unique_id, self.endpoint)
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
///
/// 이름과 고유 ID가 같아도 시스템 객체가 바뀌었으면(다른 앱이 같은 ID로 곧바로 다시 만듦) 사라지고
/// 나타난 것으로 본다. 알림이 묶여 와도 다시 연결할 기회를 놓치지 않게 하려는 것이다(리뷰 1 지적 1).
#[must_use]
pub fn diff_ports(before: &[PortInfo], after: &[PortInfo]) -> Vec<PortEvent> {
    let has = |list: &[PortInfo], p: &PortInfo| list.iter().any(|q| q.identity() == p.identity());
    let mut gone: Vec<&PortInfo> = before.iter().filter(|p| !has(after, p)).collect();
    let mut came: Vec<&PortInfo> = after.iter().filter(|p| !has(before, p)).collect();
    gone.sort_by(|a, b| (&a.name, a.unique_id).cmp(&(&b.name, b.unique_id)));
    came.sort_by(|a, b| (&a.name, a.unique_id).cmp(&(&b.name, b.unique_id)));
    gone.into_iter()
        .map(|p| PortEvent::Disappeared(p.clone()))
        .chain(came.into_iter().map(|p| PortEvent::Appeared(p.clone())))
        .collect()
}

/// `name`과 정확히 같은 이름의 포트 가운데 고유 ID가 가장 작은 것(같은 이름이 여럿일 때 결정적으로 고른다).
fn best_match<'a>(
    name: &str,
    ports: impl IntoIterator<Item = &'a PortInfo>,
) -> Option<&'a PortInfo> {
    ports
        .into_iter()
        .filter(|p| p.name == name)
        .min_by_key(|p| p.unique_id)
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
///
/// 지금 있는 포트 목록을 함께 들고 있어, 연결된 포트가 사라졌을 때 같은 이름의 다른 포트가 남아 있으면
/// 바로 그것으로 옮긴다.
#[derive(Debug, Default)]
pub struct Reconnector {
    present: Vec<PortInfo>,
    // 등록 순서를 지켜 동작을 결정적으로 낸다.
    wanted: Vec<(String, Link)>,
}

impl Reconnector {
    #[must_use]
    pub fn new() -> Self {
        Reconnector::default()
    }

    /// `name`인 포트를 원한다고 등록하고, 지금 목록에서 연결 상태를 정한다.
    pub fn want(&mut self, name: &str, now: &[PortInfo]) -> &Link {
        now.clone_into(&mut self.present);
        let link = match best_match(name, &self.present) {
            Some(p) => Link::Connected(p.clone()),
            None => Link::Missing,
        };
        let i = if let Some(i) = self.wanted.iter().position(|(n, _)| n == name) {
            self.wanted[i].1 = link;
            i
        } else {
            self.wanted.push((name.to_owned(), link));
            self.wanted.len() - 1
        };
        &self.wanted[i].1
    }

    /// 등록된 이름의 현재 상태.
    #[must_use]
    pub fn link(&self, name: &str) -> Option<&Link> {
        self.wanted.iter().find(|(n, _)| n == name).map(|(_, l)| l)
    }

    /// 이벤트 하나를 반영하고 필요한 동작을 돌려준다.
    pub fn apply(&mut self, event: &PortEvent) -> Vec<Action> {
        let mut actions = Vec::new();
        match event {
            PortEvent::Disappeared(port) => {
                self.present.retain(|p| p != port);
                for (name, link) in &mut self.wanted {
                    if *link != Link::Connected(port.clone()) {
                        continue;
                    }
                    actions.push(Action::Lost { name: name.clone() });
                    *link = match best_match(name, &self.present) {
                        Some(p) => {
                            actions.push(Action::Reconnect {
                                name: name.clone(),
                                port: p.clone(),
                            });
                            Link::Connected(p.clone())
                        }
                        None => Link::Missing,
                    };
                }
            }
            PortEvent::Appeared(port) => {
                if !self.present.contains(port) {
                    self.present.push(port.clone());
                }
                for (name, link) in &mut self.wanted {
                    if *link == Link::Missing && port.name == *name {
                        actions.push(Action::Reconnect {
                            name: name.clone(),
                            port: port.clone(),
                        });
                        *link = Link::Connected(port.clone());
                    }
                }
            }
        }
        actions
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
    saved
        .iter()
        .map(|name| {
            let suggestion = match best_match(name, available) {
                Some(p) => Suggestion::Present(p.clone()),
                None => Suggestion::Virtual { name: name.clone() },
            };
            (name.clone(), suggestion)
        })
        .collect()
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
    fn diff_reports_recreated_port_with_same_name_and_id() {
        let old = p("Synth", 7).with_endpoint(100);
        let new = p("Synth", 7).with_endpoint(200);
        assert_eq!(
            diff_ports(std::slice::from_ref(&old), std::slice::from_ref(&new)),
            vec![PortEvent::Disappeared(old), PortEvent::Appeared(new)]
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
