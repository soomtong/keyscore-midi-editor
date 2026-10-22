# Task 보고: S2-T7c SMF·계이름 텍스트 가져오기·내보내기

- PR: 브랜치 `s2/t7c-import-export`
- 계획 파일: `docs/reports/s2-t7c/plan.md`
- 계획 대비 달라진 점
  - SMF 보고는 `Song`의 메서드 셋 대신 함수 둘(`smf_import`, `smf_export`)로 했다. 기존 변환을 그대로 부르고 입력·출력을 세므로 기존 코드를 바꾸지 않는다.
  - `ConvertError` 변형은 옮긴 코드가 실제로 내던 다섯(`Parse`, `Model`, `ExportUnreadable`, `OutputTooLarge`, `OutputBeyondAnyLimit`)이다. `ksq-cli`는 `From<ConvertError> for CliError`와 얇은 감싸개로 문구·종료 코드를 그대로 둔다.
  - 문서에 `suggested_stem`·`mark_imported`를 더했다. 가져온 SMF 창의 다른 이름으로 저장이 파일 이름(예: `alouette.ksq`)을 제안하고, 내보내기도 같은 줄기를 쓴다.
  - 시험 이름은 아래 표대로 바뀌었다. 위치별 텍스트 가져오기는 엔진 없는 UI 시험에서 재생 위치가 늘 처음이라 문서 수준 함수(`import_text_into`)의 단위 시험으로 본다.
  - SMF 보고 시험은 구현 전 실패(7개)를 확인했지만 시험과 구현을 한 커밋(`f4a229d`)에 넣었다.

## 구현 요약

- `crates/keyscore-text/src/project.rs`(새, `ksq-cli`에서 옮김, `b0ab380`): `Loss`, `ConvertError`, `project_to_text(_limited)`, `text_to_project(_with)`, `MAX_EXPANSION`과 단위 시험 19개. `keyscore-text` 허용 목록에 `serde_json`(모르는 키 손실).
- `crates/keyscore-midi/src/smf/report.rs`(새): `smf_import`(= `Song::from_smf` + `to_project`)와 `ImportReport`(트랙·노트 수, 붙일 트랙 없는 가사, 노트에 붙지 않은 가사, 레인 포인트(CC 0·32 제외), 둘째 뒤 프로그램, SysEx·폴리 프레셔·기타 메타). `smf_export`(= `Song::from_project` + `to_smf`)와 `ExportReport`(숨김 꼬리, 피치 오프셋, 7비트 반올림 벨로시티, 채널 메시지가 없을 트랙).
- `crates/keyscore-app/src/import_export.rs`(새): 네 Action 처리기(창마다 `file_flow` 하나).
  - SMF 가져오기: 파일 대화상자 → 「가져오는 중」 → 백그라운드에서 읽고 변환 → 새 창(저장 안 됨, 제안 이름) → 그 창 상태줄에 요약. 흐름을 띄운 창이 손대지 않은 빈 창이면 닫는다. 오류는 시트, `TooLong`은 상한과 120 BPM 기준 시간으로.
  - SMF 내보내기: 새 경로(`줄기.mid`) → Type 1 → `write_bytes_atomic`(권한 확인 + 임시 파일 + fsync + rename) → 요약.
  - 텍스트 가져오기: 파일 대화상자 → 느슨한 모드 → `import_text_into`(이름 같은 트랙에 겹치지 않으면 넣고, 아니면 비어 있는 「이름 n」 새 트랙, 클립은 재생 위치에서, 편집 묶음 하나) → 요약(헤더 무시·경고 수). 문법 오류는 줄·열 시트.
  - 텍스트 내보내기: 새 경로(`줄기.txt`) → `project_to_text` → 쓰기 → 손실 요약(앞 세 줄).
- `src/files.rs`: 보조 함수를 crate 안에 공개, `write_bytes_atomic`, 다른 이름으로 저장의 제안 이름. `src/document.rs`: `suggested_stem`, `mark_imported`. `src/window.rs`: 처리기 넷.

## 테스트 결과

~~~
cargo test --workspace: 테스트 바이너리 202개 모두 ok, 통과 1754(이전 main 1733 + 새 21), 실패 0, 무시 0
새 테스트: keyscore-midi tests/smf_report.rs 7, keyscore-app tests/import_export.rs 14(옮긴 keyscore-text 단위 시험 19개는 ksq-cli에서 옮겨 수가 그대로)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `ksq-cli` 기존 테스트 전부 | 통과 |
| 2 | `keyscore-text` `project` 모듈 테스트 19개 | 통과 |
| 3 | `import_matches_existing_conversion_on_samples`, `export_matches_existing_conversion_on_samples`(공개 샘플 10곡) | 통과 |
| 4 | `import_report_counts_dropped_items`, `import_report_counts_lyrics_without_song_tracks`, `clean_import_is_lossless` | 통과 |
| 5 | `export_report_counts_losses`, `imported_project_exports_losslessly` | 통과 |
| 6 | `import_smf_opens_new_window_with_summary`, `import_smf_save_as_suggests_file_stem` | 통과 |
| 7 | `import_smf_replaces_pristine_window` | 통과 |
| 8 | `import_smf_errors_show_sheet`, `import_smf_too_long_message_names_the_limit` | 통과 |
| 9 | `export_smf_writes_type1_and_summarizes_losses` | 통과 |
| 10 | `import_text_at_start_puts_overlapping_block_on_new_track`(undo 한 번 포함), `import_text_into_uses_position_and_matches_track_names` | 통과 |
| 11 | `import_text_into_names_new_tracks_uniquely` | 통과 |
| 12 | `import_text_header_is_reported_not_applied` | 통과 |
| 13 | `import_text_syntax_error_shows_sheet_and_keeps_document` | 통과 |
| 14 | `export_text_writes_file_and_summarizes_losses` | 통과 |
| 15 | `import_export_respect_file_flow` | 통과 |
| 16 | `menu_and_palette_run_import_export` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test(1754 통과)
- [x] deny(advisories·bans·licenses·sources ok)
- [x] 추가 게이트: `scripts/check-deps.sh`(keyscore-text에 serde_json 허용), `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh` 통과

## 알려진 한계

- 텍스트 가져오기 위치는 재생 위치다. 커서는 S3에서 생기며 그때 바꾼다(결정 2 A).
- 텍스트 가져오기는 파일 전체를 메인 스레드에서 읽고 파싱한다(SMF처럼 백그라운드가 아니다). 텍스트 파일은 사람이 쓰는 크기라 두었다.
- SMF 가져오기는 레인(CC·벤드·프레셔)을 옮기지 않고 요약에 센다(S6, CTL-7).
- SMF 내보내기는 Type 1뿐이다. 내보내기 쓰기는 메인 스레드다.
- 요약은 상태줄 한 줄이다. 텍스트 손실은 앞 세 줄만 보인다.
- 가져오는 중에는 진행률 없이 「가져오는 중」 글자만 보인다(계획).

## 리뷰어에게

- 옮긴 변환 코드(`keyscore-text/src/project.rs`)는 동작 변경이 없어야 한다. `ksq-cli`의 출력·문구·종료 코드와 기존 시험이 근거다.
- `import_text_into`의 겹침 판정과 새 트랙 이름, 편집 묶음이 하나의 undo 단위인지.
- `smf_import`의 셈(가사·레인·메타)이 실제 버림과 맞는지.
- 흐름을 띄운 빈 창을 SMF 가져오기 끝에 닫는 경로(그 창의 흐름 안에서 닫는다).

## 리뷰 1 대응

| 리뷰 항목 | 대응 |
| --- | --- |
| 지적 1(차단): 내보내기가 열린 프로젝트 파일·상위 버전 원본을 덮는다 | 쓰기 전에 어느 창이든(이 창 포함) 그 파일을 열고 있으면 거부한다(`refuse_open_project_file`, 다른 이름으로 저장과 같은 같은 파일 규칙: 경로·링크·하드 링크·옮긴 파일). 오류 시트 |
| 지적 2: 가져온 SMF 창은 첫 편집 전까지 자동 저장본이 없다 | 가져온 문서로 표시한 뒤 문서에 알려(`notify`) 관찰자가 자동 저장 타이머를 건다 |
| 지적 3: 화음 가사 왕복에서 빈 채움 가사를 「붙지 않은 가사」로 센다 | 가사 셈은 빈 가사를 세지 않는다(파일·곡·노트 모두) |
| 지적 4: 텍스트 블록의 ch·prog·bank가 같은 이름 트랙에 들어가며 버려진다 | 채널이 같고 블록이 적은 프로그램·뱅크가 같을 때만 그 트랙에 넣는다. 아니면 블록의 채널·프로그램·뱅크로 새 트랙(「이름 2」) |
| 지적 5: 같은 트랙의 같은 음 겹침을 내보내기 보고가 세지 않는다 | `ExportReport::overlapping_notes`(펼친 위치로 음높이마다 겹침을 센다). 요약에 넣는다 |
| 제안 6: 벨로시티 0, 지휘 트랙 제목·0 µs 템포·잘못된 박자표를 세지 않는다 | 받아들임. 벨로시티는 SMF가 쓰는 최소 1로 비교한다. 메타는 `Song::from_smf`가 실제로 옮기는 것(곡 트랙이 되는 Type 1 트랙의 첫 이름, 올바른 템포·박자표)만 남긴 것으로 센다 |
| 제안 7: 앱 텍스트 내보내기는 `Loss::Mismatch`가 있어도 쓴다 | 받아들임. CLI처럼 쓰지 않고 오류 시트 |
| 제안 8: 메뉴 시험이 팔레트 실행을 보지 않는다 | 받아들임. 네 줄이 팔레트에서 실행할 수 있고, 실행 함수로 내면 대화상자가 뜨는지 본다 |

리뷰 시험 `keyscore-app tests/review_s2_t7c.rs`(부록 4개 포함 17개)와 `keyscore-midi tests/review_s2_t7c.rs`(부록 4개 포함 7개)는 남기고 모두 통과한다.
