//! 계이름 텍스트(KeyScore Text, REQ 5.14) 파서·내보내기와 드럼 라벨 표(REQ 5.13).
//!
//! 파일 가져오기, 클립보드, AI 응답이 같은 문법을 공유하므로 UI 없이 쓸 수 있게
//! `keyscore-core`에만 의존한다.
//!
//! 문법의 모호한 곳은 `docs/decisions/2026-10-11-ktext-grammar-ambiguities.md`(Q1–Q6)와
//! `docs/decisions/2026-10-12-ktext-export-rules.md`가 정한다. 그 밖에 이 crate가 정한
//! 해석은 [`parse`]의 문서에 모아 둔다.
#![deny(unsafe_code)]

mod diag;
pub mod drum;
mod export;
mod lengths;
mod parse;
mod parsed;
pub mod project;

pub use diag::{Diagnostic, Mode, ParseError};
pub use export::{ExportTrack, export};
pub use parse::{ParseOptions, parse, parse_with};
pub use parsed::{Header, Parsed, ParsedTrack};
