//! 키 충돌 감지(KM-4의 엔진 부분). 편집 화면의 충돌 시트는 S2-T6이 이 결과를 쓴다.
//!
//! 같은 레이어 안에서만 본다. 사용자 레이어가 프리셋의 같은 키를 덮는 것은 의도이고 충돌이 아니다.

use std::collections::BTreeMap;

use super::source::{ActionRef, Binding, Layer, Pos};

/// 충돌 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// 같은 컨텍스트·같은 키 시퀀스에 서로 다른 Action.
    SameKeys,
    /// 같은 컨텍스트에서 한 시퀀스가 다른 시퀀스의 앞부분(`g`와 `g g`). 짧은 쪽은 대기 시간(1 s)이
    /// 지나야 실행된다.
    Prefix,
}

/// 충돌 하나. `first`가 파일에서 앞선 바인딩이다.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub layer: Layer,
    pub context: Option<String>,
    pub first: (String, String, Pos),
    pub second: (String, String, Pos),
}

/// 컨텍스트 원문을 비교용으로 맞춘다. 컨텍스트 이름에는 공백이 없으므로 공백을 모두 지운다
/// (`Normal>PianoRoll`과 `Normal > PianoRoll`은 같다, 리뷰 1 지적 5).
fn context_key(context: Option<&str>) -> String {
    context
        .map(|c| c.chars().filter(|ch| !ch.is_whitespace()).collect())
        .unwrap_or_default()
}

fn action_label(action: &ActionRef) -> Option<String> {
    match action {
        // Action 지정 해제(`zed::Unbind`)도 지우기라 충돌로 세지 않는다.
        ActionRef::Named { name, .. } if name == super::user::UNBIND => None,
        ActionRef::Named { name, args: None } => Some(name.clone()),
        ActionRef::Named {
            name,
            args: Some(args),
        } => Some(format!("{name}({args})")),
        // 지우기는 다른 레이어를 향한 것이라 같은 레이어의 충돌로 세지 않는다.
        ActionRef::Unbind => None,
    }
}

/// 충돌을 찾는다. 결과는 (레이어, 컨텍스트, 위치) 순이라 결정적이다.
#[must_use]
pub fn conflicts(bindings: &[Binding]) -> Vec<Conflict> {
    // (레이어, 컨텍스트) → 키 시퀀스 → (Action, 위치). 같은 레이어에 같은 키가 둘이면 뒤가 이긴다.
    type Group<'a> = BTreeMap<String, Vec<(String, Pos)>>;
    let mut groups: BTreeMap<(Layer, String), (Option<&str>, Group<'_>)> = BTreeMap::new();
    for b in bindings {
        let Some(label) = action_label(&b.action) else {
            continue;
        };
        let entry = groups
            .entry((b.layer, context_key(b.context.as_deref())))
            .or_insert_with(|| (b.context.as_deref(), BTreeMap::new()));
        // 키도 GPUI 키 입력으로 정규화해 묶는다(수식키 순서·대소문자·별칭, 리뷰 2 지적 4).
        entry
            .1
            .entry(super::canonical_keys(&b.keys))
            .or_default()
            .push((label, b.pos));
    }
    let mut out = Vec::new();
    for ((layer, _), (context, by_keys)) in &groups {
        let context = context.map(str::to_string);
        for (keys, actions) in by_keys {
            for pair in actions.windows(2) {
                if pair[0].0 != pair[1].0 {
                    out.push(Conflict {
                        kind: ConflictKind::SameKeys,
                        layer: *layer,
                        context: context.clone(),
                        first: (keys.clone(), pair[0].0.clone(), pair[0].1),
                        second: (keys.clone(), pair[1].0.clone(), pair[1].1),
                    });
                }
            }
        }
        // 접두: 키 시퀀스를 토큰으로 나눠, 짧은 쪽이 긴 쪽의 앞부분인지 본다.
        let sequences: Vec<(&String, Vec<&str>, &(String, Pos))> = by_keys
            .iter()
            .filter_map(|(keys, actions)| {
                actions
                    .last()
                    .map(|last| (keys, keys.split(' ').collect(), last))
            })
            .collect();
        for (short_keys, short, short_action) in &sequences {
            for (long_keys, long, long_action) in &sequences {
                if short.len() < long.len() && long.starts_with(short) {
                    let (first, second) = if short_action.1.line <= long_action.1.line {
                        (
                            (
                                (*short_keys).clone(),
                                short_action.0.clone(),
                                short_action.1,
                            ),
                            ((*long_keys).clone(), long_action.0.clone(), long_action.1),
                        )
                    } else {
                        (
                            ((*long_keys).clone(), long_action.0.clone(), long_action.1),
                            (
                                (*short_keys).clone(),
                                short_action.0.clone(),
                                short_action.1,
                            ),
                        )
                    };
                    out.push(Conflict {
                        kind: ConflictKind::Prefix,
                        layer: *layer,
                        context: context.clone(),
                        first,
                        second,
                    });
                }
            }
        }
    }
    out
}
