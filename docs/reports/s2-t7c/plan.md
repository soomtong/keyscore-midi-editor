# Task 계획: S2-T7c SMF·계이름 텍스트 가져오기·내보내기

- 브랜치: `s2/t7c-import-export`
- 관련 REQ: IO-2(SMF Type 0/1, 내보낼 때 클립 펼침·7비트, 가져올 때 벨로시티·CC 보존), IO-3(계이름 텍스트), SEQ-9(Type 1 트랙별, Type 0 채널별, 마디 단위 클립), 5.14(텍스트 문법, `@track` 블록 = 클립, 느슨한 가져오기), LYR-6(SMF Lyric 메타), 7.7 파일 메뉴
- 선행 Task: S1-T6(SMF 입출력), S0-T5·S0-T8(텍스트 파서·`ksq-cli` 변환), S2-T7a(파일 흐름)
- 관련 결정: `docs/decisions/2026-10-22-s2-t7c-import-export.md`(1–4 모두 A), `docs/decisions/2026-10-21-s2-t7-save-policy-and-split.md` 3 A(SMF 가져오기는 백그라운드 + 결과 요약), `docs/decisions/2026-10-16-s1-t6-review-limit.md`(이월 세 항목)

## 끝의 정의 (내 말로)

1. 텍스트↔프로젝트 변환을 `keyscore-text`로 옮긴다
   - `tools/ksq-cli/src/convert.rs`의 `project_to_text`(손실 목록 `Loss`)와 `text_to_project`를 `keyscore_text::project` 모듈로 옮긴다. 오류 타입은 `keyscore_text::ConvertError`(파서 오류, 모델 오류, 출력 크기 상한). `ksq-cli`는 그것을 다시 내보내 쓰고 동작·출력·종료 코드는 그대로다(기존 테스트가 그대로 통과).
   - `keyscore-text`의 외부 의존 허용 목록에 `serde_json`을 더한다(`.ksq`의 모르는 키 손실을 본다. REQ 9.6 의존 열, core가 이미 쓴다).
2. SMF 결과 보고(`keyscore-midi`)
   - 가져오기: `Song::from_smf_report`·`Song::to_project_report`가 버린 것을 센다. 곡 트랙이 남지 않는 파일의 가사, 폴리 프레셔·SysEx·이름·템포·박자·가사 밖의 메타, 레인(CC·벤드·채널 프레셔) 포인트(S6 전까지 모델에 없다), 노트 시작에 붙지 않은 가사, 첫 번째 뒤의 프로그램·뱅크 변경.
   - 내보내기: `Song::from_project_report`가 센다. 숨김 꼬리를 자른 노트, 피치 오프셋을 잃은 노트, 7비트로 줄며 값이 바뀐 벨로시티, 노트·프로그램·뱅크가 없어 다른 프로그램이 읽을 때 사라질 수 있는 트랙.
   - 기존 `from_smf`·`to_project`·`from_project`는 그대로 두고 보고 함수가 같은 결과에 보고를 더한다(같은 결과임을 시험).
3. 앱 메뉴(파일 메뉴, 결정 1–3)
   - SMF 가져오기 `file::ImportSmf`: 파일 대화상자 → 백그라운드에서 읽고 변환(상태줄에 「가져오는 중: 이름」) → 새 프로젝트 창(제목 없음, 바뀜 켜짐, 저장하면 `.ksq`). 손대지 않은 빈 창이면 그 창을 바꾼다. 새 창 상태줄에 결과 요약(트랙·노트 수, 버린 항목). 읽지 못하면 오류 시트. `TooLong`은 「곡이 너무 길다(약 N시간)」로 알린다.
   - SMF 내보내기 `file::ExportSmf`: 새 경로 대화상자(이름.mid) → Type 1로 쓴다(임시 파일 + rename). 상태줄에 결과 요약(버린 정보). `TooLong`·`TooManyTracks`는 오류 시트.
   - 계이름 텍스트 가져오기 `file::ImportText`: 파일 대화상자 → 느슨한 모드로 읽는다 → 지금 창의 문서에 undo 하나로 넣는다. `@track` 이름이 같은 트랙이 있으면 그 트랙, 없으면 새 트랙. 클립은 재생 위치(정지면 처음, 일시정지면 그 자리)에서 시작한다. 그 트랙의 기존 클립과 겹치면 그 블록은 새 트랙(같은 이름 + 「 2」 등)에 둔다(같은 트랙 클립 겹침 금지 불변식). 헤더(`@tempo`·`@time`·`@key`)는 반영하지 않고 요약에 적는다. 파서 경고 수와 결과를 상태줄에 요약한다. 문법 오류는 오류 시트(줄·열).
   - 계이름 텍스트 내보내기 `file::ExportText`: 새 경로 대화상자(이름.txt) → `project_to_text` 결과를 쓴다. 손실 목록을 상태줄에 요약(종류별 수).
   - 네 Action 모두 프로젝트 창의 처리기이고, 창마다 흐름 하나(`file_flow`)를 지킨다.
4. S1-T6 이월 세 항목: 가져오기를 백그라운드에서 돌린다(가사 붙이기 비용), 버린 항목을 요약한다, `TooLong` 문구.

## 공개 인터페이스 초안

~~~rust
// keyscore_text::project (ksq-cli에서 옮김)
pub enum Loss { … }                    // 그대로
pub enum ConvertError { Parse(ParseError), Model(ModelError), TooLarge { limit: u64 } }
pub fn project_to_text(project: &Project) -> Result<(String, Vec<Loss>), ConvertError>;
pub fn project_to_text_limited(project: &Project, max_bytes: u64) -> Result<(String, Vec<Loss>), ConvertError>;
pub fn text_to_project(text: &str, mode: Mode) -> Result<(Project, Vec<Diagnostic>), ConvertError>;
pub fn text_to_project_with(text: &str, mode: Mode, options: ParseOptions) -> Result<(Project, Vec<Diagnostic>), ConvertError>;

// keyscore_midi::smf
pub struct ImportReport { pub orphan_lyrics: usize, pub skipped_events: usize, pub lane_points: usize,
                          pub unattached_lyrics: usize, pub extra_programs: usize, pub tracks: usize, pub notes: usize }
pub struct ExportReport { pub hidden_tails: usize, pub pitch_offsets: usize, pub velocity_rounded: usize, pub empty_tracks: usize }
impl Song {
    pub fn from_smf_report(file: &SmfFile) -> Result<(Song, ImportReport), SmfError>;
    pub fn to_project_report(&self, report: &mut ImportReport) -> Result<Project, ModelError>;
    pub fn from_project_report(project: &Project) -> (Song, ExportReport);
}

// keyscore_app::import_export (새 모듈): 네 Action 처리기, 요약 문구 함수(시험용 pub)
pub fn import_text_into(document: &mut ProjectDocument, text: &str, at: Tick) -> Result<TextImportSummary, ImportTextError>;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `ksq-cli` 기존 테스트 전부 | 통합 | 옮긴 뒤에도 그대로 통과 |
| 2 | `keyscore-text` `project` 모듈 테스트(옮긴 단위 테스트) + `text_to_project_round_trips_without_losses` | 단위·속성 | 옮긴 변환이 같은 결과 |
| 3 | `from_smf_report_matches_from_smf`, `to_project_report_matches_to_project`, `from_project_report_matches_from_project` | 속성(core 픽스처·공개 샘플 10곡) | 보고 함수의 결과가 기존 함수와 같다 |
| 4 | `import_report_counts_dropped_items` | 단위 | 가사만 있는 파일, SysEx·폴리 프레셔, CC·벤드, 노트 밖 가사, 둘째 프로그램을 센다 |
| 5 | `export_report_counts_losses` | 단위 | 숨김 꼬리, 피치 오프셋, 7비트 반올림, 빈 트랙 |
| 6 | `import_smf_opens_new_window_with_summary` | UI | 대화상자 → 백그라운드 → 새 창(제목 없음, 바뀜), 상태줄 요약, 템포·트랙 수 |
| 7 | `import_smf_replaces_pristine_window` | UI | 손대지 않은 창은 바뀐다 |
| 8 | `import_smf_errors_show_sheet` | UI | 깨진 파일 → 오류 시트, `TooLong` 문구 |
| 9 | `export_smf_writes_type1_and_summarizes_losses` | UI | 쓴 파일을 다시 읽으면 노트가 같다, 상태줄 요약, 원래 문서는 깨끗한 채 |
| 10 | `import_text_adds_clips_at_transport_position` | UI | 같은 이름 트랙에 클립, 새 이름은 새 트랙, 위치는 재생 위치, undo 한 번에 모두 사라짐 |
| 11 | `import_text_overlapping_clip_goes_to_new_track` | UI | 겹치면 「이름 2」 트랙 |
| 12 | `import_text_header_is_reported_not_applied` | UI | 템포 그대로, 요약에 헤더 |
| 13 | `import_text_syntax_error_shows_sheet_and_keeps_document` | UI | 줄·열 시트, 문서 그대로 |
| 14 | `export_text_writes_file_and_summarizes_losses` | UI | 다시 읽으면 노트가 같다, 손실 요약 |
| 15 | `import_export_respect_file_flow` | UI | 시트·대화상자가 떠 있으면 다시 시작하지 않는다 |
| 16 | `menu_and_palette_run_import_export` | UI | 네 메뉴 항목이 켜져 있고(UI-11) 팔레트에서 실행된다 |

## 불변식

- 모델은 `ProjectDocument`의 undo 엔진만 거쳐 바뀐다. 텍스트 가져오기는 편집 묶음 하나(`InsertTrack`·`InsertClip`)다.
- 같은 트랙의 클립 겹침 없음(겹치면 새 트랙).
- 내보내기는 문서를 바꾸지 않는다(바뀜·경로 그대로).
- `keyscore-text`는 UI·MIDI·네트워크에 의존하지 않는다(의존 검사).

## 범위 밖

- OUT-8 대체 매핑 제안과 S2-T7b 리뷰 4 이월(T7d, 결정 4).
- 클립보드 `Cmd+V`(7.5, S3 편집과 함께), 커서 위치 삽입(S3에서 재생 위치를 커서로 바꾼다).
- SMF Type 0 내보내기 선택, 레인(CC) 가져오기(S6).
- 가져오기 진행률 막대: 상태줄 「가져오는 중」 글자만 둔다.

## 리스크·결정 요청

- `ksq-cli`는 S0-T8에서 리뷰 11회를 거쳤다. 변환 코드는 동작을 바꾸지 않고 옮기며, 도구의 오류 문구는 `ConvertError`를 `CliError`로 감싸 그대로 둔다.
- 텍스트 가져오기의 겹침 처리(새 트랙)는 REQ에 없는 세부다. 불변식을 지키는 가장 덜 놀라운 선택으로 두고 보고서에 적는다.
