# Task 계획: S0-T6 `.ksq` 직렬화

- 브랜치: `s0/t6-ksq`
- 관련 REQ: 6.6 IO-1, 9.5, 2(프로젝트 파일 포맷·저장 행), 5.15 LYR-1, 6.1 SEQ-3, 6.5 M2-1
- 선행 Task: S0-T2, S0-T3, S0-T4
- 관련 결정: `docs/decisions/2026-10-12-ksq-note-format.md`(노트 선택 속성 객체), `docs/decisions/2026-10-12-absolute-pitch-range.md`(B: 모델이 거부), `docs/decisions/2026-10-12-clip-split-and-velocity.md`(벨로시티 16비트, 숨김 꼬리), `docs/decisions/2026-10-12-tempo-unit-and-meter-changes.md`(템포 µs)
- 이월 조건: `plan/stages/10-S0-foundation.md` S0-T6 「이월」

## 끝의 정의 (내 말로)

- `Project`를 `.ksq` 텍스트로 쓰고 읽는다. 쓰기는 결정적이다: 키 순서가 고정되고, 2칸 들여쓰기이며, 노트·템포·박자표 항목은 한 줄에 하나다. 같은 내용이면 같은 바이트가 나온다.
- 노트는 `[Δt, 길이, 음높이, 벨로시티]`이고, 선택 속성(`offset`, `lyric`)이 있으면 끝에 객체 하나가 붙는다. Δt는 클립 안에서 앞 노트 시작과의 차이다(노트는 전체 순서로 정렬되어 있어 Δt ≥ 0). 벨로시티는 16비트, 템포는 4분음표당 µs다.
- 읽기는 모르는 키를 보존한다. 프로젝트·트랙·클립 객체와 노트 선택 속성 객체 모두 해당한다. 다시 쓸 때 알려진 키 뒤에 이름 순으로 쓴다. 없는 키는 기본값이다.
- `format_version`은 1이다. 더 높은 버전은 열되 경고를 돌려준다. 버전이 없으면 1로 본다.
- 읽기는 모델 검사를 모두 거친다. 클립 겹침, 노트 시작이 클립 밖, 값 범위, 절대 피치 범위(결정 B) 위반은 「어디서」(트랙·클립·노트 번호)를 담은 오류다. 숨김 꼬리(노트 끝 > 클립 길이)는 저장된 분할 상태의 복원이므로 받는다.
- `TrackId`·`ClipId`는 쓰지 않는다. 읽을 때 새로 발급한다. 왕복 비교는 id를 뺀 내용으로 한다.
- gzip(`.ksqz`): 쓰기는 호출자가 압축 여부를 고르고, 읽기는 첫 두 바이트(1f 8b)로 판별한다.
- 파일 쓰기는 같은 디렉터리의 임시 파일에 쓰고 fsync한 뒤 rename한다. 실패하면 원본은 그대로이고 임시 파일은 지운다.
- 절대 피치 범위(결정 B)를 모델에 반영한다. `Note::with_offset`이 `Result`를 돌려주고, 음높이 + 오프셋이 0 이상 128반음 미만이 아니면 거부한다.
- 노트 10만 개 파일 읽기 < 100 ms. criterion 벤치와 기준값을 커밋한다.

원문과 다른 점: 노트 형식(선택 속성 객체)과 오프셋 원소는 원문에 없고, 결정 문서를 따른다. CC 포인트 배열은 모델에 레인이 아직 없어서(S6) 쓰지 않는다. 파일에 있으면 모르는 키로 보존된다.

## 형식

~~~json
{
  "format_version": 1,
  "key": "none",
  "tempo": [
    [0, 500000]
  ],
  "meter": [
    [0, 4, 4]
  ],
  "tracks": [
    {
      "name": "Lead",
      "color": 0,
      "port": "virtual",
      "channel": 1,
      "bank": null,
      "program": null,
      "default_velocity": 51492,
      "mute": false,
      "solo": false,
      "lyric_lang": null,
      "clips": [
        {
          "start": 0,
          "length": 3840,
          "name": null,
          "color": null,
          "notes": [
            [0, 480, 60, 51492],
            [480, 480, 62, 51492, {"lyric": "사"}]
          ]
        }
      ]
    }
  ]
}
~~~

- `key`: `"none"` 또는 `{"tonic": 0–11, "mode": "major" | "minor"}`
- `port`: `"virtual"` 또는 `{"named": "이름"}`
- `tempo`: `[틱, µs/4분음표]`, `meter`: `[틱, 분자, 분모]`. 절대 틱이고 틱 순이다.
- 없는 키의 기본값: `key` none, `tempo` `[[0, 500000]]`, `meter` `[[0, 4, 4]]`, `tracks` 빈 배열. 트랙은 이름 `""`, 색 0, 가상 포트, 채널 1, 뱅크·프로그램 없음, 기본 벨로시티 0xC924, 뮤트·솔로 꺼짐, 가사 언어 없음, 클립 없음. 클립은 시작 0, 이름·색 없음, 노트 없음, 길이는 「가장 늦은 노트 끝, 최소 1」이다.

## 공개 인터페이스 초안

~~~rust
// ksq.rs (keyscore-core)
pub const FORMAT_VERSION: u64 = 1;
pub enum Compression { None, Gzip }
pub struct Warning(String);                       // 표시용 문장
#[derive(thiserror::Error)] pub enum KsqError {
    Json { line: usize, column: usize, message: String },
    Invalid { path: String, message: String },    // path 예: "tracks[0].clips[2].notes[15]"
    Model { path: String, source: ModelError },
    Io(std::io::Error),
}
pub fn to_string(project: &Project) -> String;
pub fn from_str(text: &str) -> Result<(Project, Vec<Warning>), KsqError>;
pub fn to_bytes(project: &Project, compression: Compression) -> Vec<u8>;
pub fn from_bytes(bytes: &[u8]) -> Result<(Project, Vec<Warning>), KsqError>;   // gzip 자동 판별
pub fn write_file(path: &Path, project: &Project, compression: Compression) -> Result<(), KsqError>;  // 임시 파일 + rename
pub fn read_file(path: &Path) -> Result<(Project, Vec<Warning>), KsqError>;
// note.rs: Note::with_offset -> Result<Note, ModelError> (결정 B), ModelError::AbsolutePitchOutOfRange
// 모델에 모르는 키 보관: Project·Track·Clip은 serde_json::Map, Note는 선택 속성 객체의 모르는 키(정규화한 JSON 문자열). crate 내부.
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `ksq_round_trip_random_projects` | 속성 | 임의 프로젝트(분할 꼬리·가사·오프셋·템포·박자표 포함) → 쓰기 → 읽기 → 쓰기가 같은 문자열, 내용(id 제외)이 같음(게이트) |
| 2 | `ksq_unknown_keys_survive_read_write` | 단위 | 프로젝트·트랙·클립·노트 객체의 모르는 키가 다시 쓴 파일에 남음(게이트) |
| 3 | `ksq_missing_keys_take_defaults` | 단위 | `{}`와 키가 빠진 트랙·클립 |
| 4 | `ksq_newer_format_version_opens_with_warning` | 단위 | 게이트 |
| 5 | `ksq_snapshot_format` | 스냅샷(insta) | 키 순서·들여쓰기·한 줄 노트 |
| 6 | `ksq_invalid_files_report_where` | 단위 | 클립 겹침, 노트 밖, 채널 0, 절대 피치 범위 밖, 잘못된 타입이 경로를 담은 오류 |
| 7 | `ksq_gzip_round_trip_and_detection` | 단위 | `.ksqz` 쓰기·읽기, 평문·gzip 자동 판별 |
| 8 | `ksq_write_file_failure_keeps_original` | 통합 | 쓰기 실패 시 원본 유지, 임시 파일 정리(게이트 리뷰 초점) |
| 9 | `note_offset_outside_midi2_range_is_rejected` | 단위 | 결정 B |
| 10 | 벤치 `ksq/read_100k_notes` | 벤치 | < 100 ms, 기준값 커밋 |

## 불변식

- 읽은 프로젝트는 모델 불변식을 모두 만족한다(모델 API를 거쳐 만든다). 숨김 꼬리만 crate 내부 복원 경로로 받는다.
- 쓰기는 결정적이다. 쓰기 → 읽기 → 쓰기가 같은 바이트다.
- 파일 쓰기 실패가 원본을 손상하지 않는다.

## 범위 밖

- CC·벤드 레인(S6). 자동 저장·복구(S2-T7, IO-5). 다른 버전으로의 마이그레이션 코드(첫 버전이라 없음).

## 리스크·결정 요청

- 파일에서 읽은 숨김 꼬리는 「분할로만 생긴다」의 예외다. 저장된 분할 상태를 복원하는 경로로 본다. 손으로 고친 파일이 분할 없이 꼬리를 가질 수도 있다. 리뷰에서 문제로 보면 결정 요청으로 돌린다.
- 클립 길이 기본값(가장 늦은 노트 끝)은 REQ에 없는 해석이다. 「없는 키는 기본값」을 지키면서 길이 0 클립을 피하기 위해서다.
- 의존 추가: `serde_json`(REQ 9.6 표에 있음), `flate2`(gzip, MIT/Apache-2.0, `check-deps.sh` 허용 목록에 더함), `insta`(dev, 이미 허용 목록에 있음).
