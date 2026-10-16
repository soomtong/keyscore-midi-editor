# Task 계획: S1-T6 SMF 입출력

- 브랜치: `s1/t6-smf`
- 관련 REQ: 6.1 SEQ-6, SEQ-9, 6.6 IO-2, 6.7 CTL-10, 5.15 LYR-6, M2-1(벨로시티 7↔16비트)
- 선행 Task: S0-T2(모델), S0-T3(템포·박자표), S1-T3(인코딩·다운스케일)
- 관련 스파이크: 없음
- 이월: S1-T3 리뷰 2 제안 2(벨로시티 0 Note On은 Note Off), S1-T3 리뷰 3 제안 2(벤드는 부호 없는 32비트, 가운데 0x8000_0000), S1-T5 리뷰 1 제안 4(박자표 변화 이분 탐색)

## 끝의 정의 (내 말로)

`keyscore-midi`에 SMF 계층이 셋 생긴다.

1. 원시 계층(`SmfFile`)
   - Type 0/1 바이트를 청크·이벤트로 읽고 쓴다.
   - running status, 가변 길이 수, 메타, SysEx를 다룬다.
   - 거짓 길이·과도한 트랙 수·과도한 이벤트 수·잘린 파일은 패닉 없이 `Err`다. 상한은 `SmfLimits`로 정한다.
2. 곡 계층(`Song`)
   - PPQ 960으로 재배율한 중간 표현이다(SEQ-6).
   - 템포 맵과 박자표 맵을 가진다.
   - 트랙마다 다음을 가진다.
     - 이름과 채널
     - 노트: 절대 틱, 16비트 벨로시티(M2-1의 7→16 확장)
     - 프로그램 체인지 열
     - CC 번호별 포인트 열, 벤드 포인트 열, 채널 프레셔 포인트 열: 값은 32비트(CTL-10, SEQ-4)
     - 가사 열(Lyric 메타 0x05, LYR-6)
   - 가져오기: Type 1은 트랙별로, Type 0은 채널별로 곡 트랙을 만든다. Type 1 트랙 하나에 채널이 여럿이면 채널마다 나눈다(트랙 = 채널 하나, SEQ-7).
   - 내보내기: Type 1(템포·박자표 트랙 + 곡 트랙마다 하나)과 Type 0(하나로 합침)을 쓴다. 값은 7비트로 다운스케일한다(IO-2).
3. 모델 변환
   - `Song::to_project`: 노트가 있는 마디 구간을 마디 단위 클립으로 묶는다(SEQ-9).
     - 가사는 같은 틱에 시작하는 노트에 붙인다.
     - 템포·박자표, 트랙 이름·채널을 옮긴다.
     - 뱅크·프로그램은 첫 값을 트랙 속성으로 옮긴다.
   - `Song::from_project`: 클립을 펼친다(SEQ-9). 숨김 꼬리는 클립 끝에서 자른다(결정 Q1).
     - 노트 가사는 Lyric 메타로 내보낸다.
     - 트랙 뱅크·프로그램은 틱 0의 CC 0/32·프로그램 체인지로 내보낸다.

모델에는 아직 컨트롤러 레인 저장소가 없다. CTL-7에 따르면 레인 데이터는 클립에 속하고, 레인 저장은 S6에서 만든다. 그래서 이 Task에서 CC·벤드·프레셔 보존(CTL-10)은 곡 계층까지다. 곡 계층의 SMF 왕복은 CC를 잃지 않는다. 모델로 옮길 때 레인은 버린다. 이 범위 한정은 보고서와 S6 단계 파일의 이월 줄에 적는다.

## 공개 인터페이스 초안

~~~rust
// smf/raw.rs
pub enum SmfFormat { Single, Parallel }          // Type 0, Type 1 (Type 2는 Err)
pub struct SmfLimits { /* max_tracks, max_events */ }   // 기본: 트랙 1024, 이벤트 200만
pub struct SmfFile { /* format, division(ppq), tracks: Vec<SmfTrack> */ }
pub struct SmfTrack { /* events: Vec<SmfEvent> (원본 PPQ 절대 틱) */ }
pub struct SmfEvent { /* tick: u64, kind: SmfEventKind */ }
pub enum SmfEventKind { Channel(Midi1Message), Meta { kind: u8, data: Vec<u8> }, SysEx(Vec<u8>) }
impl SmfFile {
    pub fn read(bytes: &[u8]) -> Result<SmfFile, SmfError>;               // 기본 상한
    pub fn read_with(bytes: &[u8], limits: SmfLimits) -> Result<SmfFile, SmfError>;
    pub fn write(&self) -> Vec<u8>;
}
pub enum SmfError { /* NotSmf, Truncated{offset}, BadLength, UnsupportedFormat(u16), SmpteDivision,
                      ZeroDivision, TooManyTracks, TooManyEvents, BadVarLen, MissingStatus, … */ }

// smf/song.rs
pub struct Song { /* tempo: TempoMap, meter: MeterMap, tracks: Vec<SongTrack> */ }
pub struct SongTrack { /* name, channel, notes: Vec<SongNote>, programs, cc: BTreeMap<u8, Vec<CcPoint>>,
                          bend: Vec<CcPoint>, pressure: Vec<CcPoint>, lyrics: Vec<(Tick, String)> */ }
pub struct SongNote { /* start: Tick, length: u32, pitch, velocity */ }
impl Song {
    pub fn from_smf(file: &SmfFile) -> Result<Song, SmfError>;
    pub fn to_smf(&self, format: SmfFormat) -> SmfFile;
    pub fn to_project(&self) -> Result<Project, ModelError>;
    pub fn from_project(project: &Project) -> Song;
}
~~~

세부 규칙(REQ에 없어 흔한 방식을 고른 것)
- 노트 짝짓기
  - (채널, 음높이)마다 먼저 켠 노트를 먼저 끈다(FIFO).
  - 벨로시티 0인 Note On은 Note Off다.
  - 끝나지 않은 노트는 트랙 끝(End of Track 틱)에서 끝낸다.
  - 길이 0 노트는 1틱으로 둔다.
- 재배율: `round(tick × 960 / ppq)`로 u128 정수 산술. SMPTE 분할은 `Err`.
- 값 변환
  - CC·프레셔: 7비트는 MIDI 2.0 min-center-max 확장으로 32비트가 된다. 인코더는 >>25로 되돌린다.
  - 벤드: 14비트를 32비트로 확장한다(가운데 0x2000 → 0x8000_0000). 인코더는 >>18로 되돌린다.
  - 7→32→7과 14→32→14는 무손실이다.
- 가사 바이트: UTF-8이면 그대로 쓴다. 아니면 Latin-1로 읽는다. 내보낼 때는 UTF-8이다.
- 버리는 것: 폴리 프레셔, SysEx, 트랙 이름(0x03)·템포·박자표·가사 밖의 메타. `SmfFile` 계층에는 남는다.
- 템포 0, 박자표 분모가 2의 거듭제곱이 아닌 값 같은 잘못된 메타는 건너뛴다.
- 클립 묶기: 노트 구간 `[start, end)`이 걸친 마디를 「찬 마디」로 보고, 이어진 찬 마디를 클립 하나로 묶는다. 클립 이름은 없다.
- 가사 붙이기: 같은 곡 트랙에서 그 틱에 시작하는 노트 가운데 가장 높은 음에 붙인다. 붙일 노트가 없으면 모델로 옮길 때 버린다. 곡 계층에서는 남는다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `public_samples_round_trip_event_sets` | 통합 | `tests/data/smf/` 10개: 읽기 → 곡 → 쓰기 → 읽기 → 곡의 이벤트 집합(노트·CC·벤드·프레셔·프로그램·템포·박자표·가사)이 같다. Type 0/1 둘 다 내보내 본다 |
| 2 | `lyrics_round_trip_through_smf_and_project` | 통합 | 가사가 있는 곡: SMF → 곡 → 모델 → 곡 → SMF → 곡에서 가사·노트 같음(LYR-6) |
| 3 | `velocity_and_cc_survive_round_trip` | 통합 | 벨로시티 1·64·100·127, CC 0·1·64·127, 벤드 0·0x2000·0x3FFF가 왕복 무손실(CTL-10, IO-2) |
| 4 | `note_on_velocity_zero_reads_as_note_off` | 단위 | 벨로시티 0 Note On이 노트를 끝낸다 |
| 5 | `running_status_and_meta_between_reads` | 단위 | running status, 사이에 낀 메타·SysEx 뒤 running status 해제 규칙 |
| 6 | `ppq_rescale_to_960_is_rounded_integer` | 단위 | PPQ 96·480·1000·7에서 재배율 |
| 7 | `type0_splits_by_channel_and_type1_by_track` | 단위 | SEQ-9 트랙 만들기, 여러 채널 Type 1 트랙 나누기 |
| 8 | `to_project_groups_notes_into_bar_clips` | 단위 | 비어 있는 마디가 클립을 나누고, 마디를 넘는 노트가 다음 마디를 채운다 |
| 9 | `from_project_flattens_clips_and_cuts_hidden_tails` | 단위 | 클립 펼치기, 꼬리 자르기, 뱅크·프로그램 틱 0 |
| 10 | `malicious_smf_returns_err_without_panic` | 속성(proptest) + 단위 | 임의 바이트, 거짓 청크 길이, ntrks 65535, 4바이트 넘는 VLQ, 상한 넘는 이벤트 수에서 `Err`이고 패닉 없음 |
| 11 | `smpte_division_and_type2_are_rejected` | 단위 | 지원하지 않는 형식 |
| 12 | `written_smf_is_byte_stable` | 단위 | 같은 곡을 두 번 쓰면 바이트가 같다(결정적 쓰기) |
| 13 | `song_round_trip_is_lossless_property` | 속성 | 임의 곡(7비트 범위) → SMF → 곡이 같다 |

## 불변식

- 읽기는 어떤 입력에도 패닉하지 않는다. 할당은 실제 바이트 수와 `SmfLimits`가 묶는다. 헤더의 트랙 수만 믿고 미리 잡지 않는다.
- 쓰기는 결정적이다. 같은 틱의 이벤트 순서는 Note Off, 그 밖, Note On이다(스케줄러 정렬과 같다).
- `Song`·`SongTrack`·`SmfFile`에 `pub` 필드가 없다. 값 범위는 core 타입(`Channel`, `Pitch`, `Velocity`)이 보장한다.

## 범위 밖

- 모델의 컨트롤러 레인 저장과 레인 ↔ 곡 변환은 S6(CTL-7)이다. 이 Task는 곡 계층까지 보존한다.
- 채널 10 자동 드럼 판별(S6-T5), 클립보드 SMF(S3), 앱의 가져오기 메뉴(S2)도 범위 밖이다.
- SMF2(IO-4, W), 키 서명 메타 → 프로젝트 조성 변환도 하지 않는다. 조성은 REQ에 SMF 규칙이 없다.

## 리스크·결정 요청

- 공개 SMF 샘플 10개는 퍼블릭 도메인 파일만 모은다(`tests/data/smf/SOURCES.md`). 가사가 있는 퍼블릭 도메인 파일을 못 찾으면 가사 왕복(테스트 2)은 직접 만든 SMF로 한다.
- 결정 요청 없음. 레인 보존 범위는 REQ 의미를 바꾸지 않는 단계 나눔이다(CTL-7이 레인을 클립에 둔다고 정하고, 레인은 S6이 만든다).
