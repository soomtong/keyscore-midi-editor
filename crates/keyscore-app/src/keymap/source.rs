//! 키맵 파일 읽기(KM-1, KM-6). GPUI에 기대지 않는 부분.
//!
//! 형식은 GPUI 키맵 JSON 배열이다: `[{ "context": "...", "bindings": { "키": 값 } }]`. 사람이 고치는
//! 파일이라 `//`·`/* */` 주석과 끝 쉼표를 받는다. 오류를 사람이 찾아가게 모든 값의 줄·열을 들고 다닌다.
//! `serde_json`은 문법 오류 위치만 주므로 작은 읽기를 직접 둔다.

use serde_json::Value;

/// 파일 안의 위치(1부터).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

/// 바인딩이 온 레이어. 뒤 레이어가 이긴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// macOS 표준 `Cmd` 단축키(7.5). 프리셋과 무관하다.
    Standard,
    /// 기본 프리셋(`default`, 5.8).
    Preset,
    /// 기본 프리셋 위에 얹는 프리셋(`vim`, 5.8.1). 기본 프리셋의 같은 키를 덮는 것은 의도라 충돌이
    /// 아니다.
    PresetOverlay,
    /// 사용자 오버라이드(`keymap.json`).
    User,
}

/// 바인딩이 가리키는 것.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionRef {
    /// Action 이름(`ns::Name`)과 인자. 인자는 `ns::Name(arg)`이면 문자열 `"arg"`, GPUI 표기
    /// `["ns::Name", {...}]`이면 그 값이다.
    Named { name: String, args: Option<Value> },
    /// `null`: 아래 레이어의 같은 (컨텍스트, 키) 바인딩을 지운다.
    Unbind,
}

/// 파일의 바인딩 하나. 키와 컨텍스트는 아직 검사하지 않은 원문이다.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// GPUI 컨텍스트 술어 원문. `None`이면 전역.
    pub context: Option<String>,
    /// 키 시퀀스 원문(`g g`, `cmd-s`).
    pub keys: String,
    pub action: ActionRef,
    pub layer: Layer,
    /// 키가 적힌 자리.
    pub pos: Pos,
}

/// 키맵 오류. 오류가 난 항목만 빠지고 나머지는 적용된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapError {
    pub layer: Layer,
    pub pos: Pos,
    pub message: String,
}

impl std::fmt::Display for KeymapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.pos.line, self.pos.column, self.message)
    }
}

/// 파일을 읽는다. 문법 오류면 바인딩 없이 그 오류 하나를 돌려준다. 항목 단위 오류는 그 항목만 뺀다.
#[must_use]
pub fn parse(text: &str, layer: Layer) -> (Vec<Binding>, Vec<KeymapError>) {
    let mut reader = Reader::new(text);
    let value = match reader.document() {
        Ok(v) => v,
        Err((pos, message)) => {
            return (
                Vec::new(),
                vec![KeymapError {
                    layer,
                    pos,
                    message,
                }],
            );
        }
    };
    let mut bindings = Vec::new();
    let mut errors = Vec::new();
    let mut error = |pos: Pos, message: String| {
        errors.push(KeymapError {
            layer,
            pos,
            message,
        });
    };
    let Node::Array(sections) = value.node else {
        error(value.pos, "키맵 파일은 배열이어야 한다".to_string());
        return (bindings, errors);
    };
    for section in sections {
        let Node::Object(fields) = section.node else {
            error(section.pos, "배열의 항목은 객체여야 한다".to_string());
            continue;
        };
        let mut context = None;
        let mut entries = None;
        for (key, key_pos, value) in fields {
            match key.as_str() {
                "context" => match value.node {
                    Node::String(s) if !s.trim().is_empty() => context = Some(s),
                    Node::Null => {}
                    _ => error(value.pos, "context는 문자열이어야 한다".to_string()),
                },
                "bindings" => match value.node {
                    Node::Object(map) => entries = Some(map),
                    _ => error(value.pos, "bindings는 객체여야 한다".to_string()),
                },
                // GPUI 키맵의 다른 필드(use_key_equivalents 등)는 이 앱에서 쓰지 않는다.
                other => error(key_pos, format!("모르는 필드 '{other}'")),
            }
        }
        for (keys, pos, value) in entries.unwrap_or_default() {
            if keys.trim().is_empty() {
                error(pos, "빈 키".to_string());
                continue;
            }
            match action_ref(value) {
                Ok(action) => bindings.push(Binding {
                    context: context.clone(),
                    keys: normalize_keys(&keys),
                    action,
                    layer,
                    pos,
                }),
                Err((pos, message)) => error(pos, message),
            }
        }
    }
    // 파일을 위에서부터 고칠 수 있게 위치 순으로 돌려준다.
    errors.sort_by_key(|e| (e.pos.line, e.pos.column));
    (bindings, errors)
}

/// 키 시퀀스의 공백을 하나로 맞춘다(`g  g` → `g g`).
#[must_use]
pub fn normalize_keys(keys: &str) -> String {
    keys.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn action_ref(value: Spanned) -> Result<ActionRef, (Pos, String)> {
    match value.node {
        Node::Null => Ok(ActionRef::Unbind),
        Node::String(s) => parse_action_string(&s).ok_or_else(|| {
            (
                value.pos,
                format!("Action 표기가 잘못됐다: '{s}' (ns::Name 또는 ns::Name(인자))"),
            )
        }),
        Node::Array(items) => {
            let mut items = items.into_iter();
            let (Some(first), second, None) = (items.next(), items.next(), items.next()) else {
                return Err((
                    value.pos,
                    "[\"ns::Name\", {인자}] 형태여야 한다".to_string(),
                ));
            };
            let Node::String(name) = first.node else {
                return Err((first.pos, "Action 이름은 문자열이어야 한다".to_string()));
            };
            if !is_action_name(&name) {
                return Err((first.pos, format!("Action 이름이 잘못됐다: '{name}'")));
            }
            Ok(ActionRef::Named {
                name,
                args: second.map(Spanned::into_json),
            })
        }
        _ => Err((
            value.pos,
            "값은 Action 이름 문자열, [이름, 인자], null 가운데 하나다".to_string(),
        )),
    }
}

/// `ns::Name` 또는 `ns::Name(arg)`. 인자는 JSON 문자열 하나가 된다(REQ 7.3 예시 표기).
fn parse_action_string(s: &str) -> Option<ActionRef> {
    let s = s.trim();
    let (name, args) = match s.find('(') {
        Some(open) if s.ends_with(')') => {
            let inner = s[open + 1..s.len() - 1].trim();
            if inner.is_empty() {
                return None;
            }
            (&s[..open], Some(Value::String(inner.to_string())))
        }
        Some(_) => return None,
        None => (s, None),
    };
    is_action_name(name).then(|| ActionRef::Named {
        name: name.to_string(),
        args,
    })
}

fn is_action_name(name: &str) -> bool {
    let mut parts = name.split("::");
    let (Some(ns), Some(item), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ident = |p: &str| {
        !p.is_empty()
            && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !p.starts_with(|c: char| c.is_ascii_digit())
    };
    ident(ns) && ident(item)
}

// ---------------------------------------------------------------- 위치를 지키는 JSONC 읽기

#[derive(Debug, Clone)]
struct Spanned {
    pos: Pos,
    node: Node,
}

#[derive(Debug, Clone)]
enum Node {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Spanned>),
    /// 넣은 순서를 지킨다(뒤 항목이 이기는 규칙에 필요하다).
    Object(Vec<(String, Pos, Spanned)>),
}

impl Spanned {
    fn into_json(self) -> Value {
        match self.node {
            Node::Null => Value::Null,
            Node::Bool(b) => Value::Bool(b),
            Node::Number(n) => Value::Number(n),
            Node::String(s) => Value::String(s),
            Node::Array(items) => Value::Array(items.into_iter().map(Spanned::into_json).collect()),
            Node::Object(fields) => Value::Object(
                fields
                    .into_iter()
                    .map(|(k, _, v)| (k, v.into_json()))
                    .collect(),
            ),
        }
    }
}

/// 깊이 상한. 악의적이거나 망가진 파일이 재귀로 스택을 넘기지 않게 한다.
const MAX_DEPTH: usize = 64;

type ReadResult<T> = Result<T, (Pos, String)>;

struct Reader<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    pos: Pos,
}

impl<'a> Reader<'a> {
    fn new(text: &'a str) -> Self {
        let mut chars = text.chars().peekable();
        // UTF-8 BOM은 건너뛴다(편집기가 붙이는 일이 있다).
        if chars.peek() == Some(&'\u{feff}') {
            chars.next();
        }
        Reader {
            chars,
            pos: Pos { line: 1, column: 1 },
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.pos.line += 1;
            self.pos.column = 1;
        } else {
            self.pos.column += 1;
        }
        Some(c)
    }

    fn document(&mut self) -> ReadResult<Spanned> {
        self.trivia()?;
        let value = self.value(0)?;
        self.trivia()?;
        if self.peek().is_some() {
            return Err((self.pos, "값 뒤에 남은 글자가 있다".to_string()));
        }
        Ok(value)
    }

    /// 공백과 주석.
    fn trivia(&mut self) -> ReadResult<()> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') => {
                    let start = self.pos;
                    self.bump();
                    match self.bump() {
                        Some('/') => {
                            while self.peek().is_some_and(|c| c != '\n') {
                                self.bump();
                            }
                        }
                        Some('*') => loop {
                            match self.bump() {
                                Some('*') if self.peek() == Some('/') => {
                                    self.bump();
                                    break;
                                }
                                Some(_) => {}
                                None => return Err((start, "닫히지 않은 /* 주석".to_string())),
                            }
                        },
                        _ => return Err((start, "'/' 뒤에는 '/'나 '*'가 와야 한다".to_string())),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self, depth: usize) -> ReadResult<Spanned> {
        if depth > MAX_DEPTH {
            return Err((self.pos, "중첩이 너무 깊다".to_string()));
        }
        let pos = self.pos;
        let node = match self.peek() {
            Some('{') => self.object(depth)?,
            Some('[') => self.array(depth)?,
            Some('"') => Node::String(self.string()?),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number()?,
            Some(c) if c.is_ascii_alphabetic() => match self.word().as_str() {
                "true" => Node::Bool(true),
                "false" => Node::Bool(false),
                "null" => Node::Null,
                other => return Err((pos, format!("모르는 값 '{other}'"))),
            },
            Some(c) => return Err((pos, format!("값이 올 자리에 '{c}'"))),
            None => return Err((pos, "값이 올 자리에서 파일이 끝났다".to_string())),
        };
        Ok(Spanned { pos, node })
    }

    fn word(&mut self) -> String {
        let mut w = String::new();
        while let Some(c) = self.peek().filter(char::is_ascii_alphanumeric) {
            w.push(c);
            self.bump();
        }
        w
    }

    fn number(&mut self) -> ReadResult<Node> {
        let pos = self.pos;
        let mut text = String::new();
        while let Some(c) = self
            .peek()
            .filter(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
        {
            text.push(c);
            self.bump();
        }
        serde_json::from_str::<serde_json::Number>(&text)
            .map(Node::Number)
            .map_err(|_| (pos, format!("잘못된 숫자 '{text}'")))
    }

    fn string(&mut self) -> ReadResult<String> {
        let start = self.pos;
        self.bump(); // 여는 따옴표
        let mut s = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(s),
                Some('\\') => {
                    let esc_pos = self.pos;
                    match self.bump() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('b') => s.push('\u{8}'),
                        Some('f') => s.push('\u{c}'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            let mut code = 0u32;
                            for _ in 0..4 {
                                let d = self
                                    .bump()
                                    .and_then(|c| c.to_digit(16))
                                    .ok_or((esc_pos, "\\u 뒤에는 16진수 네 자리".to_string()))?;
                                code = code * 16 + d;
                            }
                            // 대리 쌍은 키맵에 쓸 일이 없어 대체 문자로 둔다.
                            s.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err((esc_pos, "잘못된 이스케이프".to_string())),
                    }
                }
                Some('\n') | None => return Err((start, "닫히지 않은 문자열".to_string())),
                Some(c) => s.push(c),
            }
        }
    }

    fn array(&mut self, depth: usize) -> ReadResult<Node> {
        self.bump(); // [
        let mut items = Vec::new();
        loop {
            self.trivia()?;
            if self.peek() == Some(']') {
                self.bump();
                return Ok(Node::Array(items));
            }
            items.push(self.value(depth + 1)?);
            self.trivia()?;
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(']') => {}
                _ => return Err((self.pos, "배열 안에는 ',' 또는 ']'가 와야 한다".to_string())),
            }
        }
    }

    fn object(&mut self, depth: usize) -> ReadResult<Node> {
        self.bump(); // {
        let mut fields = Vec::new();
        loop {
            self.trivia()?;
            match self.peek() {
                Some('}') => {
                    self.bump();
                    return Ok(Node::Object(fields));
                }
                Some('"') => {}
                _ => return Err((self.pos, "객체의 키는 문자열이어야 한다".to_string())),
            }
            let key_pos = self.pos;
            let key = self.string()?;
            self.trivia()?;
            if self.peek() != Some(':') {
                return Err((self.pos, "키 뒤에는 ':'가 와야 한다".to_string()));
            }
            self.bump();
            self.trivia()?;
            let value = self.value(depth + 1)?;
            fields.push((key, key_pos, value));
            self.trivia()?;
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some('}') => {}
                _ => return Err((self.pos, "객체 안에는 ',' 또는 '}'가 와야 한다".to_string())),
            }
        }
    }
}
