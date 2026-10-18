//! 사용자 레이어 편집(KM-3·4, S2-T6a). 키맵 편집기가 `keymap.json`을 고쳐 쓰는 부분으로, GPUI와 무관하다.
//!
//! 비교는 엔진과 같은 자리 규칙을 쓴다: 컨텍스트는 공백을 지운 원문, 키는 `canonical_keys`.

use std::fmt::Write as _;

use serde_json::Value;

use super::source::{ActionRef, Binding, KeymapError, Layer, Pos, parse};
use super::{LayerSet, canonical_keys};

/// 레이어를 겹친 뒤 실제로 걸린 바인딩 하나.
#[derive(Debug, Clone, PartialEq)]
pub struct Effective {
    pub context: Option<String>,
    pub keys: String,
    /// Action 라벨(`ns::Name`, 인자가 있으면 `ns::Name(인자)`)
    pub action: String,
    /// Action 이름(`ns::Name`)
    pub name: String,
    pub args: Option<Value>,
    pub layer: Layer,
}

/// 비교용 컨텍스트(공백 없앰, 전역은 빈 문자열).
fn context_key(context: Option<&str>) -> String {
    context
        .map(|c| c.chars().filter(|ch| !ch.is_whitespace()).collect())
        .unwrap_or_default()
}

/// 같은 자리(컨텍스트·키)인가.
fn same_place(a_ctx: Option<&str>, a_keys: &str, b_ctx: Option<&str>, b_keys: &str) -> bool {
    context_key(a_ctx) == context_key(b_ctx) && canonical_keys(a_keys) == canonical_keys(b_keys)
}

/// Action 라벨. 인자 문자열은 따옴표 없이 쓴다(`step::Note(g)`, 파일 표기와 같다).
fn label(name: &str, args: Option<&Value>) -> String {
    match args {
        None => name.to_owned(),
        Some(Value::String(s)) => format!("{name}({s})"),
        Some(other) => format!("{name}({other})"),
    }
}

/// GPUI의 Action 지정 해제(`["zed::Unbind", "ns::Name"]`). 같은 키의 우선순위가 낮은 바인딩 가운데 그 Action인
/// 것만 끈다. `null`(`NoAction`)은 같은 키의 우선순위가 낮은 바인딩을 Action과 무관하게 모두 끄므로, 전역 키를
/// 지우면 다른 컨텍스트의 같은 키까지 죽는다. 편집기는 그래서 `null` 대신 이것을 쓴다(S2-T6a 리뷰 1 지적 3).
pub const UNBIND: &str = "zed::Unbind";

/// `null`이 `other` 컨텍스트의 앞 바인딩을 가리는가. GPUI의 `NoAction`은 같은 키의 우선순위가 낮은 바인딩,
/// 곧 같거나 얕은 컨텍스트의 앞 바인딩을 가린다. 컨텍스트 없는(전역) `null`은 어느 깊이와도 맞으므로 모든
/// 컨텍스트의 같은 키를 가린다(S2-T6a 리뷰 1 지적 3).
fn null_hides(null_ctx: &str, other: &str) -> bool {
    null_ctx.is_empty() || other.is_empty() || null_ctx == other || ancestor(other, null_ctx)
}

/// `a`가 `b`의 바깥 컨텍스트인가(`Normal`과 `Normal>PianoRoll`).
fn ancestor(a: &str, b: &str) -> bool {
    b.len() > a.len() && b.starts_with(a) && b[a.len()..].starts_with('>')
}

/// 두 컨텍스트가 한 디스패치 경로에서 함께 맞을 수 있는가(같거나, 한쪽이 바깥이거나, 한쪽이 전역). 이런 두
/// 바인딩의 같은 키는 하나가 다른 하나를 가린다(KM-4, 리뷰 1 지적 7).
fn related(a: &str, b: &str) -> bool {
    a.is_empty() || b.is_empty() || a == b || ancestor(a, b) || ancestor(b, a)
}

/// 실효 바인딩: 레이어 순서대로 겹치고, 같은 자리는 뒤 항목이 이기며, `null`은 그 자리와 GPUI가 함께 가리는
/// 자리(같거나 얕은 컨텍스트, 전역 `null`이면 모든 컨텍스트)의 같은 키를 지운다. 결과는 그 자리가 처음 나온
/// 순서다.
///
/// GPUI의 깊이 우선순위는 흉내 내지 않는다. 뒤에 건 얕은 바인딩이 깊은 바인딩을 이기는 경우(전역이 맞는 깊이와
/// 동률) 둘 다 보인다.
#[must_use]
pub fn effective(layers: &LayerSet) -> Vec<Effective> {
    let mut out: Vec<(String, String, Option<Effective>)> = Vec::new();
    for b in layers.iter() {
        let ctx = context_key(b.context.as_deref());
        let keys = canonical_keys(&b.keys);
        let entry = match &b.action {
            ActionRef::Unbind => {
                for (c, k, e) in &mut out {
                    if *k == keys && null_hides(&ctx, c) {
                        *e = None;
                    }
                }
                None
            }
            ActionRef::Named { name, args } if name == UNBIND => {
                let target = args.as_ref().and_then(Value::as_str).unwrap_or_default();
                for (c, k, e) in &mut out {
                    if *k == keys
                        && null_hides(&ctx, c)
                        && e.as_ref().is_some_and(|e| e.name == target)
                    {
                        *e = None;
                    }
                }
                // 지정 해제는 바인딩이 아니다. 그 자리를 차지하지 않는다
                continue;
            }
            ActionRef::Named { name, args } => Some(Effective {
                context: b.context.as_ref().map(|c| c.trim().to_owned()),
                keys: b.keys.clone(),
                action: label(name, args.as_ref()),
                name: name.clone(),
                args: args.clone(),
                layer: b.layer,
            }),
        };
        if let Some(slot) = out.iter_mut().find(|(c, k, _)| *c == ctx && *k == keys) {
            slot.2 = entry;
        } else {
            out.push((ctx, keys, entry));
        }
    }
    out.into_iter().filter_map(|(_, _, e)| e).collect()
}

/// `action`(이름)에 (`context`, `keys`)를 걸 때 부딪히는 실효 바인딩: 한 경로에서 함께 맞을 수 있는
/// 컨텍스트(같거나, 바깥·안쪽이거나, 전역)에서 같은 키이거나 한쪽이 다른 쪽의 앞부분(`g`와 `g g`)이고, 다른
/// Action인 것(KM-4).
#[must_use]
pub fn binding_conflicts(
    effective: &[Effective],
    context: Option<&str>,
    keys: &str,
    action: &str,
) -> Vec<Effective> {
    let ctx = context_key(context);
    let new: Vec<String> = canonical_keys(keys).split(' ').map(str::to_owned).collect();
    effective
        .iter()
        .filter(|e| e.name != action && related(&context_key(e.context.as_deref()), &ctx))
        .filter(|e| {
            let old: Vec<String> = canonical_keys(&e.keys)
                .split(' ')
                .map(str::to_owned)
                .collect();
            let n = old.len().min(new.len());
            old[..n] == new[..n]
        })
        .cloned()
        .collect()
}

/// 그 Action(이름)의 실효 바인딩이 서로 다른 인자를 몇 가지 가지는가(`step::Note(a)`, `step::Note(s)` …).
#[must_use]
pub fn argument_variants(effective: &[Effective], action: &str) -> usize {
    let mut labels: Vec<&str> = effective
        .iter()
        .filter(|e| e.name == action)
        .map(|e| e.action.as_str())
        .collect();
    labels.sort_unstable();
    labels.dedup();
    labels.len()
}

/// 사용자 레이어(`keymap.json`의 항목들).
#[derive(Debug, Clone, Default)]
pub struct UserKeymap {
    entries: Vec<Binding>,
}

impl PartialEq for UserKeymap {
    /// 같은 항목들인가(위치와 표기 차이는 보지 않는다).
    fn eq(&self, other: &Self) -> bool {
        self.entries.len() == other.entries.len()
            && self.entries.iter().zip(&other.entries).all(|(a, b)| {
                same_place(a.context.as_deref(), &a.keys, b.context.as_deref(), &b.keys)
                    && a.action == b.action
            })
    }
}

impl UserKeymap {
    /// 읽는다. 오류가 하나라도 있으면 돌려준다. 편집기가 고쳐 쓰면 읽히지 않은 항목이 사라지기 때문이다.
    ///
    /// # Errors
    ///
    /// 문법 오류나 항목 오류.
    pub fn parse(text: &str) -> Result<Self, Vec<KeymapError>> {
        let (entries, errors) = parse(text, Layer::User);
        if errors.is_empty() {
            Ok(UserKeymap { entries })
        } else {
            Err(errors)
        }
    }

    /// 항목들(사용자 레이어로 걸 바인딩).
    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.entries
    }

    /// GPUI 키맵 JSON으로 쓴다. 컨텍스트마다 한 절, 절 순서는 처음 나온 순서다.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut sections: Vec<(Option<String>, Vec<&Binding>)> = Vec::new();
        for b in &self.entries {
            let ctx = b.context.as_deref().map(str::trim);
            match sections
                .iter_mut()
                .find(|(c, _)| context_key(c.as_deref()) == context_key(ctx))
            {
                Some((_, list)) => list.push(b),
                None => sections.push((ctx.map(str::to_owned), vec![b])),
            }
        }
        let quote = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""));
        let mut out = String::from("[\n");
        for (i, (ctx, list)) in sections.iter().enumerate() {
            out.push_str("  {\n");
            if let Some(ctx) = ctx {
                let _ = writeln!(out, "    \"context\": {},", quote(ctx));
            }
            out.push_str("    \"bindings\": {\n");
            for (j, b) in list.iter().enumerate() {
                let value = match &b.action {
                    ActionRef::Unbind => "null".to_owned(),
                    ActionRef::Named { name, args: None } => quote(name),
                    ActionRef::Named {
                        name,
                        args: Some(args),
                    } => format!("[{}, {args}]", quote(name)),
                };
                let comma = if j + 1 < list.len() { "," } else { "" };
                let _ = writeln!(out, "      {}: {value}{comma}", quote(&b.keys));
            }
            out.push_str("    }\n");
            out.push_str(if i + 1 < sections.len() {
                "  },\n"
            } else {
                "  }\n"
            });
        }
        out.push_str("]\n");
        out
    }

    fn remove_place(&mut self, context: Option<&str>, keys: &str) {
        self.entries
            .retain(|b| !same_place(b.context.as_deref(), &b.keys, context, keys));
    }

    fn push(&mut self, context: Option<&str>, keys: &str, action: ActionRef) {
        self.remove_place(context, keys);
        self.entries.push(Binding {
            context: context.map(str::to_owned),
            keys: canonical_keys(keys),
            action,
            layer: Layer::User,
            pos: Pos::default(),
        });
    }

    /// 그 실효 바인딩을 지운다. 사용자 항목이면 빼고, 아래 레이어 것이면 그 Action만 끄는 지정 해제(`UNBIND`)를
    /// 그 자리에 둔다. 같은 키의 다른 Action은 살아 있다. 표준 단축키를 지웠으면 `true`.
    fn hide(&mut self, target: &Effective) -> bool {
        if target.layer == Layer::User {
            self.remove_place(target.context.as_deref(), &target.keys);
            return false;
        }
        self.push(
            target.context.as_deref(),
            &target.keys,
            ActionRef::Named {
                name: UNBIND.to_owned(),
                args: Some(Value::String(target.name.clone())),
            },
        );
        target.layer == Layer::Standard
    }

    /// `action`(이름)의 `context` 바인딩을 `keys` 하나로 바꾼다. 인자는 그 컨텍스트의 지금 바인딩 인자를 따르고,
    /// 같은 인자의 키만 바꾼다(인자가 다른 바인딩은 다른 명령이다, 리뷰 1 지적 1). 그 자리와 접두로 부딪히는
    /// 다른 Action의 키는 지운다(충돌 시트의 「새 바인딩 유지」). 표준 단축키를 지웠으면 `true`.
    pub fn bind(
        &mut self,
        action: &str,
        context: Option<&str>,
        keys: &str,
        effective: &[Effective],
    ) -> bool {
        let ctx = context_key(context);
        let first = effective
            .iter()
            .find(|e| e.name == action && context_key(e.context.as_deref()) == ctx);
        let args = first.and_then(|e| e.args.clone());
        let target_label = label(action, args.as_ref());
        let mut standard = false;
        for e in effective {
            let same_action = e.name == action
                && e.action == target_label
                && context_key(e.context.as_deref()) == ctx;
            let replaced = same_action && canonical_keys(&e.keys) != canonical_keys(keys);
            let conflicting = e.name != action
                && binding_conflicts(std::slice::from_ref(e), context, keys, action).len() == 1;
            // 같은 자리의 다른 Action은 새 항목이 그 자리를 덮으므로 따로 지우지 않는다
            let same_place_other =
                conflicting && same_place(e.context.as_deref(), &e.keys, context, keys);
            if replaced || (conflicting && !same_place_other) {
                standard |= self.hide(e);
            }
        }
        self.push(
            context,
            keys,
            ActionRef::Named {
                name: action.to_owned(),
                args,
            },
        );
        standard
    }

    /// `action`(이름)의 실효 키를 모두 지운다. 표준 단축키를 지웠으면 `true`.
    pub fn clear(&mut self, action: &str, effective: &[Effective]) -> bool {
        let mut standard = false;
        for e in effective.iter().filter(|e| e.name == action) {
            standard |= self.hide(e);
        }
        standard
    }

    /// `action`(이름)에 관한 사용자 항목을 빼 프리셋 기본값으로 되돌린다: 그 Action으로의 바인딩과, 그
    /// Action의 프리셋 키를 가리는 `null`.
    pub fn restore(&mut self, action: &str, preset: &[Binding]) {
        let defaults: Vec<&Binding> = preset
            .iter()
            .filter(|b| matches!(&b.action, ActionRef::Named { name, .. } if name == action))
            .collect();
        self.entries.retain(|b| match &b.action {
            ActionRef::Named { name, args } if name == UNBIND => {
                args.as_ref().and_then(Value::as_str) != Some(action)
            }
            ActionRef::Named { name, .. } => name != action,
            ActionRef::Unbind => !defaults
                .iter()
                .any(|d| same_place(d.context.as_deref(), &d.keys, b.context.as_deref(), &b.keys)),
        });
    }

    /// 같은 레이어 안의 충돌(`keymap::conflict`)을 풀었다: 충돌마다 파일에서 뒤 항목만 남긴다.
    pub fn keep_later_of_conflicts(&mut self) {
        let conflicts = super::conflict::conflicts(&self.entries);
        let drop: Vec<Pos> = conflicts.iter().map(|c| c.first.2).collect();
        self.entries.retain(|b| !drop.contains(&b.pos));
    }
}
