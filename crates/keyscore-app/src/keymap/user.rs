//! 사용자 레이어 편집(KM-3·4, S2-T6a). 키맵 편집기가 `keymap.json`을 고쳐 쓰는 부분으로, GPUI와 무관하다.

use super::LayerSet;
use super::source::{Binding, KeymapError, Layer};

/// 레이어를 겹친 뒤 실제로 걸린 바인딩 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub context: Option<String>,
    pub keys: String,
    /// Action 라벨(`ns::Name`, 인자가 있으면 `ns::Name(인자)`)
    pub action: String,
    pub layer: Layer,
}

/// 실효 바인딩.
#[must_use]
pub fn effective(layers: &LayerSet) -> Vec<Effective> {
    let _ = layers;
    todo!()
}

/// `action`에 (`context`, `keys`)를 걸 때 부딪히는 실효 바인딩(같은 키 또는 접두 관계, 다른 Action).
#[must_use]
pub fn binding_conflicts(
    effective: &[Effective],
    context: Option<&str>,
    keys: &str,
    action: &str,
) -> Vec<Effective> {
    let _ = (effective, context, keys, action);
    todo!()
}

/// 사용자 레이어(`keymap.json`의 항목들).
#[derive(Debug, Clone, Default)]
pub struct UserKeymap {
    entries: Vec<Binding>,
}

impl PartialEq for UserKeymap {
    fn eq(&self, other: &Self) -> bool {
        let _ = other;
        todo!()
    }
}

impl UserKeymap {
    /// 읽는다. 문법 오류가 있으면 그 오류들.
    ///
    /// # Errors
    ///
    /// 파일 전체를 읽을 수 없는 문법 오류.
    pub fn parse(text: &str) -> Result<Self, Vec<KeymapError>> {
        let _ = text;
        todo!()
    }

    /// GPUI 키맵 JSON으로 쓴다.
    #[must_use]
    pub fn to_json(&self) -> String {
        todo!()
    }

    /// `action`의 `context` 바인딩을 `keys` 하나로 바꾼다.
    pub fn bind(
        &mut self,
        action: &str,
        context: Option<&str>,
        keys: &str,
        effective: &[Effective],
    ) {
        let _ = (action, context, keys, effective);
        todo!()
    }

    /// `action`의 실효 키를 모두 지운다.
    pub fn clear(&mut self, action: &str, effective: &[Effective]) {
        let _ = (action, effective);
        todo!()
    }

    /// `action`에 관한 사용자 항목을 빼 프리셋 기본값으로 되돌린다.
    pub fn restore(&mut self, action: &str, preset: &[Binding]) {
        let _ = (action, preset);
        todo!()
    }
}
