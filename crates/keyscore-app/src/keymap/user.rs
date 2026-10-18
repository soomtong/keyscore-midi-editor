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

/// 실효 바인딩: 레이어 순서대로 겹치고, 같은 자리는 뒤 항목이 이기며, `null`은 그 자리를 지운다.
/// 결과는 그 자리가 처음 나온 순서다.
#[must_use]
pub fn effective(layers: &LayerSet) -> Vec<Effective> {
    let mut out: Vec<Option<Effective>> = Vec::new();
    let mut index: std::collections::HashMap<(String, String), usize> =
        std::collections::HashMap::new();
    for b in layers.iter() {
        let place = (context_key(b.context.as_deref()), canonical_keys(&b.keys));
        let entry = match &b.action {
            ActionRef::Unbind => None,
            ActionRef::Named { name, args } => Some(Effective {
                context: b.context.as_ref().map(|c| c.trim().to_owned()),
                keys: b.keys.clone(),
                action: label(name, args.as_ref()),
                name: name.clone(),
                args: args.clone(),
                layer: b.layer,
            }),
        };
        if let Some(&i) = index.get(&place) {
            out[i] = entry;
        } else {
            index.insert(place, out.len());
            out.push(entry);
        }
    }
    out.into_iter().flatten().collect()
}

/// `action`(이름)에 (`context`, `keys`)를 걸 때 부딪히는 실효 바인딩: 같은 컨텍스트에서 같은 키이거나 한쪽이
/// 다른 쪽의 앞부분(`g`와 `g g`)이고, 다른 Action인 것(KM-4).
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
        .filter(|e| e.name != action && context_key(e.context.as_deref()) == ctx)
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

    /// `action`(이름)의 `context` 바인딩을 `keys` 하나로 바꾼다. 그 자리의 다른 Action은 그 키를 잃는다(충돌
    /// 시트의 「새 바인딩 유지」). 인자는 그 컨텍스트의 지금 바인딩 인자를 따른다.
    pub fn bind(
        &mut self,
        action: &str,
        context: Option<&str>,
        keys: &str,
        effective: &[Effective],
    ) {
        let ctx = context_key(context);
        let mine: Vec<&Effective> = effective
            .iter()
            .filter(|e| e.name == action && context_key(e.context.as_deref()) == ctx)
            .collect();
        let args = mine.first().and_then(|e| e.args.clone());
        for e in &mine {
            if canonical_keys(&e.keys) == canonical_keys(keys) {
                continue;
            }
            if e.layer == Layer::User {
                self.remove_place(e.context.as_deref(), &e.keys);
            } else {
                self.push(e.context.as_deref(), &e.keys, ActionRef::Unbind);
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
    }

    /// `action`(이름)의 실효 키를 모두 지운다. 사용자 항목은 빼고, 프리셋·표준 항목은 `null`로 가린다.
    pub fn clear(&mut self, action: &str, effective: &[Effective]) {
        for e in effective.iter().filter(|e| e.name == action) {
            if e.layer == Layer::User {
                self.remove_place(e.context.as_deref(), &e.keys);
            } else {
                self.push(e.context.as_deref(), &e.keys, ActionRef::Unbind);
            }
        }
    }

    /// `action`(이름)에 관한 사용자 항목을 빼 프리셋 기본값으로 되돌린다: 그 Action으로의 바인딩과, 그
    /// Action의 프리셋 키를 가리는 `null`.
    pub fn restore(&mut self, action: &str, preset: &[Binding]) {
        let defaults: Vec<&Binding> = preset
            .iter()
            .filter(|b| matches!(&b.action, ActionRef::Named { name, .. } if name == action))
            .collect();
        self.entries.retain(|b| match &b.action {
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
