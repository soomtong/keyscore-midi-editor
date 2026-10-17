//! 트랙 목록·트랜스포트 바의 계산(S2-T5b). GPUI와 무관한 함수라 단위 테스트한다.

use std::collections::BTreeSet;

use keyscore_core::{Channel, Key, OutputPort, Project, Tempo, Tick, TimeSignature, TrackId};

/// 그 포트에서 비어 있는 가장 낮은 채널(SEQ-7). 16채널이 다 찼으면 `None`.
#[must_use]
pub fn lowest_free_channel(project: &Project, port: &OutputPort) -> Option<Channel> {
    let _ = (project, port);
    todo!()
}

/// SEQ-8: 같은 포트·채널을 둘 이상의 트랙이 쓰는데 프로그램이 서로 다른 트랙들.
#[must_use]
pub fn program_conflicts(project: &Project) -> BTreeSet<TrackId> {
    let _ = project;
    todo!()
}

/// 위치 표시 `마디.박.틱`(1부터 센다, 7.1).
#[must_use]
pub fn format_position(project: &Project, at: Tick) -> String {
    let _ = (project, at);
    todo!()
}

/// 템포 필드 표시(소수 한 자리).
#[must_use]
pub fn format_tempo(tempo: Tempo) -> String {
    let _ = tempo;
    todo!()
}

/// 템포 필드 입력(BPM, 소수 한 자리까지, 10.0–999.9).
///
/// # Errors
///
/// 숫자가 아니거나 범위 밖이면 사람이 읽을 이유.
pub fn parse_tempo(text: &str) -> Result<Tempo, String> {
    let _ = text;
    todo!()
}

/// 박자표 필드 표시(`4/4`).
#[must_use]
pub fn format_meter(signature: TimeSignature) -> String {
    let _ = signature;
    todo!()
}

/// 박자표 필드 입력(`3/4`, 분모는 2의 거듭제곱).
///
/// # Errors
///
/// 형식이나 값이 틀리면 사람이 읽을 이유.
pub fn parse_meter(text: &str) -> Result<TimeSignature, String> {
    let _ = text;
    todo!()
}

/// 조성 선택지: 없음, 장조 12, 단조 12.
#[must_use]
pub fn key_choices() -> Vec<Key> {
    todo!()
}

/// 조성 표시(`C장조`, `A단조`, `조성 없음`).
#[must_use]
pub fn key_label(key: Key) -> String {
    let _ = key;
    todo!()
}

/// 포트 표시(가상 포트는 기본 가상 포트 이름).
#[must_use]
pub fn port_label(port: &OutputPort, virtual_name: &str) -> String {
    let _ = (port, virtual_name);
    todo!()
}
