//! S2-T2 키맵 파일 읽기와 충돌 감지(GPUI 없이).

use std::fmt::Write as _;

use keyscore_app::keymap::conflict::{ConflictKind, conflicts};
use keyscore_app::keymap::source::{ActionRef, Layer, parse};

/// REQ 7.3의 키맵 파일 예(그대로 옮김).
pub const REQ_EXAMPLE: &str = r#"[
  {
    "context": "Normal",
    "bindings": {
      "left": "cursor::Left",
      "h": "cursor::Left",
      "shift-left": "select::ExtendLeft",
      "ctrl-l": "cursor::NextBar",
      "alt-k": "note::TransposeUp",
      "alt-shift-l": "note::Lengthen",
      "enter": "mode::Step",
      "v": "view::Toggle",
      "r": "edit::Repeat",
      "/": "edit::Split",
      "cmd-j": "edit::Join",
      "5": "note::SetLength(sixteenth)",
      "space": "transport::Toggle"
    }
  },
  {
    "context": "Normal > Arrangement",
    "bindings": {
      "enter": "clip::Open",
      "alt-l": "clip::MoveLater",
      "alt-shift-l": "clip::Lengthen"
    }
  },
  {
    "context": "Step",
    "bindings": {
      "space": "step::Rest",
      "backspace": "step::Back",
      "tab": "step::Confirm",
      "escape": "mode::Normal"
    }
  },
  {
    "context": "Keymap > Editing",
    "bindings": {
      "escape": "keymap::CancelCapture"
    }
  }
]"#;

fn render(text: &str) -> String {
    let (bindings, errors) = parse(text, Layer::User);
    let mut out = String::new();
    for b in &bindings {
        let action = match &b.action {
            ActionRef::Named { name, args: None } => name.clone(),
            ActionRef::Named {
                name,
                args: Some(args),
            } => format!("{name} {args}"),
            ActionRef::Unbind => "(지움)".to_string(),
        };
        let _ = writeln!(
            out,
            "{}:{} [{}] {} -> {action}",
            b.pos.line,
            b.pos.column,
            b.context.as_deref().unwrap_or("전역"),
            b.keys
        );
    }
    for e in errors {
        let _ = writeln!(out, "오류 {e}");
    }
    out
}

#[test]
fn req_example_keymap_snapshot() {
    insta::assert_snapshot!(render(REQ_EXAMPLE));
}

#[test]
fn parse_errors_report_line_and_column() {
    // 문법 오류는 그 자리 하나만 보고하고 바인딩은 없다.
    let (b, e) = parse(
        "[\n  {\"context\": \"Normal\",\n   \"bindings\": {\"h\" \"cursor::Left\"}}\n]",
        Layer::User,
    );
    assert!(b.is_empty());
    assert_eq!(
        (e.len(), e[0].pos.line, e[0].pos.column),
        (1, 3, 21),
        "{e:?}"
    );
    // 항목 오류는 그 항목만 빠지고 나머지는 남는다.
    let text = r#"[
  // 주석과 끝 쉼표를 받는다.
  {
    "context": "Normal",
    "bindings": {
      "h": "cursor::Left",
      "j": "not an action",
      "k": ["cursor::Up", {"count": 2}, 3],
      "l": 7,
      "": "cursor::Right",
      "x": null,
    },
    "extra": true,
  },
  "oops",
]"#;
    let (b, e) = parse(text, Layer::User);
    let keys: Vec<_> = b.iter().map(|b| (b.keys.as_str(), b.pos.line)).collect();
    assert_eq!(keys, [("h", 6), ("x", 11)]);
    assert_eq!(b[1].action, ActionRef::Unbind);
    let lines: Vec<_> = e.iter().map(|e| e.pos.line).collect();
    assert_eq!(lines, [7, 8, 9, 10, 13, 15], "{e:?}");
    // 깊은 중첩·닫히지 않은 주석·BOM.
    let deep = format!("{}{}", "[".repeat(100), "]".repeat(100));
    assert_eq!(parse(&deep, Layer::User).1.len(), 1);
    assert_eq!(parse("[ /* ", Layer::User).1.len(), 1);
    assert!(parse("\u{feff}[]", Layer::User).1.is_empty());
}

#[test]
fn action_strings_accept_both_argument_forms() {
    let (b, e) = parse(
        r#"[{"context": "Normal", "bindings": {
            "5": "note::SetLength(sixteenth)",
            "6": ["note::SetLength", "eighth"],
            "7": "note::SetLength()",
            "8": "note::SetLength(eighth",
            "9": "a::b::c"
        }}]"#,
        Layer::Preset,
    );
    assert_eq!(b.len(), 2);
    for binding in &b {
        let ActionRef::Named {
            name,
            args: Some(_),
        } = &binding.action
        else {
            panic!("{binding:?}");
        };
        assert_eq!(name, "note::SetLength");
    }
    assert_eq!(e.len(), 3, "{e:?}");
}

#[test]
fn conflicts_and_prefix_conflicts_are_detected() {
    let text = r#"[
  {"context": "Normal", "bindings": {
    "g": "view::Toggle",
    "g g": "cursor::First",
    "d": "edit::Delete"
  }},
  {"context": "Normal", "bindings": {
    "d": "edit::Cut",
    "e": "edit::Delete"
  }},
  {"context": "Step", "bindings": {
    "d": "edit::Cut"
  }}
]"#;
    let (b, e) = parse(text, Layer::Preset);
    assert!(e.is_empty());
    let found = conflicts(&b);
    let kinds: Vec<_> = found
        .iter()
        .map(|c| (c.kind, c.first.0.as_str(), c.second.0.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (ConflictKind::SameKeys, "d", "d"),
            (ConflictKind::Prefix, "g", "g g"),
        ]
    );
    assert_eq!(found[0].first.2.line, 5);
    assert_eq!(found[0].second.2.line, 8);
    // 다른 레이어의 같은 키는 덮어쓰기라 충돌이 아니다.
    let (mut user, _) = parse(
        r#"[{"context": "Normal", "bindings": {"e": "edit::Cut"}}]"#,
        Layer::User,
    );
    let (preset, _) = parse(
        r#"[{"context": "Normal", "bindings": {"e": "edit::Delete"}}]"#,
        Layer::Preset,
    );
    user.extend(preset);
    assert!(conflicts(&user).is_empty());
}
