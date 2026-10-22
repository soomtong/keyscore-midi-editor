# 적대적 리뷰: S2-T7c SMF·계이름 텍스트 가져오기·내보내기 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `195db0d`(브랜치 `s2/t7c-import-export`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7c.rs`(13개), `crates/keyscore-midi/tests/review_s2_t7c.rs`(3개). 깨뜨리지 못한 시도만 남겼고 모두 통과한다. 깨뜨린 시도 8개는 부록 A·B

## 1. 코드만 읽고 적은 「내가 검사할 것」

제품 코드(`import_export.rs`, `report.rs`, `files.rs`·`document.rs` 변경, `convert.rs`↔`project.rs` diff)를 읽고 적었다. 구현자의 시험 파일은 이 목록을 만든 뒤 도움 함수를 빌리려고 열었다.

1. 내보내기의 쓰기 대상 검사. `write_bytes_atomic`은 권한(`check_writable`)만 본다. 다른 이름으로 저장이 막는 두 경우(상위 형식 버전 원본, 다른 창이 연 파일, `files.rs:417-428`)를 내보내기도 막는가.
2. `mark_imported`가 `cx.notify()` 없이 바뀜만 켠다(`import_export.rs:196`). 문서 관찰자가 돌지 않으면 자동 저장 타이머가 걸리는가.
3. SMF 가져오기 보고의 셈. `KEPT_META`에 든 메타(이름·템포·박자표)가 실제로는 버려지는 경우(지휘 트랙 제목, Type 0 이름, 잘못된 템포·박자표), 빈 가사(`bar_groups`가 붙이지 않는다)를 「붙지 않은 가사」로 세는가.
4. SMF 내보내기 보고의 셈. 같은 음 겹침(SMF FIFO로 길이가 바뀐다), 벨로시티 0(`note_on`의 `max(1)`)을 세는가.
5. 텍스트 가져오기: 같은 이름 트랙을 고를 때 블록의 ch·prog·bank를 버리는가. 겹침 판정 경계(끝 = 시작), 같은 이름 블록 여럿, 새 이름 충돌, 틱 상한에서 원자성, undo·redo 한 단위, 빈 텍스트가 바뀜을 켜는가.
6. SMF 가져오기 창: 흐름을 띄운 빈 창만 닫는가(빈 창 둘), 실패면 닫지 않는가, 백그라운드 중 그 창을 닫으면.
7. 옮긴 변환: `ConvertError` 문구가 `CliError`와 같은가, `From` 변환이 변형을 빠뜨리지 않는가. CLI의 `Loss::Mismatch` 거부(`cli.rs:522`)가 앱 내보내기에도 있는가.
8. 메인 스레드 비용: 텍스트 내보내기(되읽기 검증 포함)와 가져오기 파싱이 큰 프로젝트에서 UI를 멈추는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 내보내기 대상 검사 | 없음 | 권한(0444)도 구현자 시험에 없다. 리뷰 시험 `export_to_read_only_file_is_refused`로 확인(통과) |
| 2 가져온 창 자동 저장 | 없음 | 깨짐(지적 2) |
| 3 가져오기 보고 과소·과대 | 일부 | `import_report_counts_dropped_items`는 손으로 만든 파일만 본다. 자기 내보내기의 재가져오기는 없다 |
| 4 내보내기 보고 과소 | 일부 | 겹침·벨로시티 0 없음 |
| 5 텍스트 가져오기 경계 | 일부 | 같은 이름 트랙의 ch·prog 차이, 경계, 원자성, redo 없음 |
| 6 SMF 가져오기 창 | 일부 | 빈 창 둘, 실패 시 유지, 백그라운드 중 닫기 없음 |
| 7 옮긴 변환 | 있음 | `ksq-cli` 기존 시험 130개 통과로 확인. `Mismatch` 거부는 앱에 없다(제안 7) |
| 8 메인 스레드 비용 | 없음 | 리뷰어가 release로 쟀다(시도 15) |
| 메뉴·팔레트(UI-11) | 일부 | `menu_and_palette_run_import_export`는 이름과 달리 팔레트를 보지 않는다(제안 8) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 상위 형식 버전 원본(`format_version` 7, 읽기 전용)에서 텍스트 내보내기로 그 원본 경로를 고른다 | 깨짐: 원본이 텍스트로 덮인다 | 부록 A `export_over_newer_format_original_keeps_it` |
| 2 | 창 A가 연 `a.ksq`를 창 B의 SMF 내보내기 대상으로 고른다 | 깨짐: `a.ksq`가 MIDI로 덮이고 A는 깨끗한 채(닫으면 묻지 않음) | 부록 A `export_over_file_open_in_other_window_is_refused` |
| 3 | SMF를 가져와 편집 없이 31 s를 보낸다 | 깨짐: 바뀜이 켜졌는데 자동 저장본이 없다 | 부록 A `imported_smf_window_is_autosaved` |
| 4 | 위와 같되 가져온 창에서 트랙 하나를 더한다 | 못 깨뜨림(편집 뒤에는 저장본이 생긴다) | `imported_smf_window_autosaves_after_edit` |
| 5 | 화음 아래 음에만 가사 → SMF 내보내기 → 다시 가져오기 | 깨짐: 가사는 그대로인데 `unattached_lyrics` 1 | 부록 B `reimport_of_chord_lyric_reports_nothing_unattached` |
| 6 | 같은 트랙의 같은 음 겹침(0–1920, 480–960) 내보내기 | 깨짐: 다시 읽으면 (0,960)·(480,1440)인데 보고는 손실 없음 | 부록 B `overlapping_same_pitch_export_is_counted` |
| 7 | 벨로시티 0 노트 내보내기 | 깨짐: 512로 돌아오는데 세지 않는다 | 부록 B `velocity_zero_export_is_counted` |
| 8 | 지휘 트랙의 곡 제목·0 µs 템포·분모 2^20 박자표 | 깨짐: 모두 버리는데 손실 없음 보고 | 부록 B `dropped_title_and_bad_meta_are_counted` |
| 9 | `@track Lead ch5 prog40`을 ch1·프로그램 없는 「Lead」가 있는 문서에 겹치지 않게 가져오기 | 깨짐: ch1 트랙에 들어가 ch·prog가 소리 없이 사라진다 | 부록 A `import_text_into_same_name_track_keeps_block_channel` |
| 10 | 텍스트 가져오기 undo → redo, 빈 텍스트(주석만) | 못 깨뜨림 | `import_text_undo_redo_round_trips`, `import_empty_text_keeps_document_clean` |
| 11 | 같은 이름 블록 둘, 「A」·「A 2」가 모두 겹칠 때 새 이름, 끝=시작 경계, 틱 상한 원자성 | 못 깨뜨림(파서가 같은 이름 블록을 합친다. 텍스트 이름에는 공백이 없다) | `import_text_same_name_blocks_become_one_clip`, `import_text_new_names_do_not_collide_with_each_other`, `import_text_adjacent_clip_goes_to_same_track`, `import_text_at_tick_limit_fails_atomically` |
| 12 | 빈 창 둘에서 가져오기, 깨진 파일로 빈 창에서 가져오기 | 못 깨뜨림 | `import_smf_closes_only_the_requesting_pristine_window`, `import_smf_failure_keeps_pristine_window` |
| 13 | 백그라운드 가져오기 중에 흐름을 띄운 창을 닫는다 | 못 깨뜨림(창은 닫히고 가져온 창이 열린다, 시트 없음) | `closing_requesting_window_during_import_still_opens_result` |
| 14 | 0444 대상 파일로 내보내기, 저장된 문서에서 두 내보내기 뒤 원래 `.ksq`·경로·바뀜·임시 파일 | 못 깨뜨림 | `export_to_read_only_file_is_refused`, `export_keeps_saved_document_and_its_file` |
| 15 | 노트 102 400개(8트랙 × 50클립) 프로젝트의 `project_to_text`와 그 텍스트(483 KB) 파싱 시간(release, 임시 시험으로 재고 지웠다) | 못 깨뜨림: 134 ms, 12 ms. 메인 스레드라도 크게 멈추지 않는다 | 없음(측정) |
| 16 | 느슨 모드 경고가 요약에 수로 나오는가 | 못 깨뜨림 | `import_text_lenient_warnings_are_summarized` |
| 17 | 가사만 담은 트랙이 곡 트랙에 붙는 파일, CC만 있는 트랙(뱅크 CC 제외), Type 0 채널 나누기의 셈 | 못 깨뜨림 | midi `lyric_track_attached_to_song_track_is_not_counted`, `cc_only_track_counts_lane_points_but_not_bank`, `type0_split_counts_match_model` |
| 18 | `ksq-cli` 시험 전부(옮긴 변환의 문구·종료 코드) | 못 깨뜨림(130 통과). `ConvertError` 다섯 변형의 문구가 `CliError`와 글자까지 같다(diff 확인) | 기존 시험 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | `import_text_into`가 원래 프로젝트 기준으로 판정하지만 파서가 같은 이름 블록을 하나로 합쳐 한 트랙에 둘이 들어가지 않는다. 틀려도 `Document::apply`가 거부해 원자적이다(시도 11) |
| 노트는 클립 안 | 못 깨뜨림 | 원본 클립 길이 그대로 `Clip::new` + `insert_note` |
| 채널 1–16 | 못 깨뜨림 | 파서의 `Channel` |
| 텍스트 가져오기는 undo 하나 | 못 깨뜨림 | `document.apply(edits)` 한 번(시도 10) |
| 내보내기는 문서를 바꾸지 않는다 | 문서는 그대로. 그러나 디스크의 다른 문서 파일을 바꾼다 | 지적 1 |
| `keyscore-text`가 UI·MIDI에 의존하지 않음 | 못 깨뜨림 | `serde_json`만 더했다(`check-deps.sh`) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-2 SMF Type 0/1 가져오기·내보내기 | 충족(모델 범위). CC 보존은 S6 이월로 요약에 센다 | `import_export.rs:164-218`, `220-236` |
| IO-3 텍스트 가져오기·내보내기 | 부분 충족: 같은 이름 트랙에 넣으며 블록의 ch·prog·bank를 알리지 않고 버린다(지적 4) | `import_export.rs:74-104` |
| SEQ-9 트랙 나누기·마디 클립 | 충족(S1-T6 변환 그대로) | `report.rs:83-85` |
| 5.14 `@track` 블록 = 클립, 위치 | 충족(재생 위치, 결정 2 A) | `import_export.rs:252-262` |
| LYR-6 SMF Lyric 왕복 | 충족(가사 자체는 왕복). 보고가 왕복을 손실로 센다(지적 3) | `report.rs:113-117` |
| 7.7 파일 메뉴 네 항목 | 충족 | `commands/table.rs:560-571`, `window.rs:661-664` |
| IO-5 자동 저장(변경 후 30 s) | 부분 충족: 가져온 창은 첫 편집 전까지 저장본이 없다(지적 2) | `import_export.rs:196` |
| 결정 2026-10-21 1 A(읽기 전용 거부)·T7a 저장 정책 | 부분 충족: 권한은 지키나 상위 버전 원본·열린 파일 덮어쓰기 검사가 없다(지적 1) | `files.rs:752-779` |
| 결정 2026-10-22 3 A(손실 종류와 수) | 부분 충족: 겹친 같은 음, 벨로시티 0을 세지 않는다(지적 5·6) | `report.rs:130-153` |

「부분 충족」은 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(이 Task는 스케줄러를 건드리지 않는다).
- 제품 코드의 `unwrap`/`expect`: 없음. `unwrap_or(0)`(색 번호), `unwrap_or_else`(새 이름 대비)는 도달하지 않는 갈래의 기본값이다.
- 에러가 삼켜지는 곳
  - `import_export.rs:196`: `mark_imported`가 알림 없이 상태를 바꾼다(지적 2).
  - `import_export.rs:211-217`: 가져온 창을 연 뒤 빈 창 닫기의 결과를 보지 않는다(닫기 실패는 해가 없다).
  - `import_export.rs:285`: `project_to_text`의 `Loss::Mismatch`를 손실 요약의 한 줄로만 보이고 쓴다. CLI는 같은 경우 쓰지 않고 실패한다(`cli.rs:520-528`, 제안 7).
  - `files.rs:770`: 임시 파일 지우기 실패를 버린다(원래 저장 경로와 같은 관례).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 내보내기가 쓰기 대상을 권한으로만 거른다. (a) 상위 형식 버전 원본(읽기 전용 문서 자신의 파일)을 고르면 원본이 텍스트·MIDI로 덮인다. 다른 이름으로 저장은 이것을 막는다(`files.rs:417-423`, 결정 2026-10-12). (b) 다른 창이 연 `.ksq`(또는 자기 문서 파일)를 고르면 그 파일이 덮이고, 그 창은 바뀜이 꺼진 채라 닫아도 묻지 않아 프로젝트가 사라진다. 저장은 `AlreadyOpen`으로 막는다(`files.rs:424-428`). 데이터 손실. 재현: 부록 A `export_over_newer_format_original_keeps_it`, `export_over_file_open_in_other_window_is_refused`. 고칠 길: 내보내기도 `window_with_path(&path, None, cx)`(자기 창 포함)면 거부 시트 | `import_export.rs:231`, `286`; `files.rs:752` |
| 2 | 수정 | 가져온 SMF 창은 바뀜이 켜졌는데 `mark_imported`가 `cx.notify()`를 하지 않아 문서 관찰자(`autosave_document_changed`)가 돌지 않는다. 첫 편집 전에는 30 s 자동 저장본이 생기지 않는다(IO-5). 재현: 부록 A `imported_smf_window_is_autosaved` | `import_export.rs:196`, `document.rs:144-147` |
| 3 | 수정 | 가져오기 보고가 빈 가사를 「노트 시작에 붙지 않은 가사」로 센다. 우리 내보내기는 화음의 아래 음에 가사가 있으면 위 음 자리를 빈 가사로 채우고(`song.rs` `from_project`), `bar_groups`는 빈 가사를 붙이지 않는다. 그래서 자기 SMF를 다시 가져오면 잃은 것이 없는데 「버림: 가사 1개」를 알린다(과대). 재현: 부록 B `reimport_of_chord_lyric_reports_nothing_unattached` | `report.rs:94-117` |
| 4 | 수정 | 텍스트 가져오기가 이름이 같은 트랙을 고르면 블록의 ch·prog·bank를 소리 없이 버린다. `@track Lead ch5 prog40`이 ch1·프로그램 없는 트랙에 들어가 다른 소리로 난다. 요약에도 없다. 결정 2 A는 「같은 이름이면 그 트랙」이지만 손실을 알리지 않는 것은 결정 3 A의 원칙과 어긋난다. 고칠 길: ch(와 prog·bank)까지 같을 때만 그 트랙, 아니면 새 트랙, 또는 요약에 적기(정책은 구현자 판단, 필요하면 결정 요청). 재현: 부록 A `import_text_into_same_name_track_keeps_block_channel` | `import_export.rs:74-83` |
| 5 | 수정 | 내보내기 보고가 같은 트랙에서 같은 음이 겹치는 노트를 세지 않는다. SMF는 (채널, 음높이)마다 먼저 켠 것을 먼저 끄므로 다시 읽으면 길이가 바뀐다(0–1920·480–960 → 0–960·480–1920). 요약은 「잃은 정보」 없이 끝난다. 다른 클립의 겹침(숨김 꼬리 포함)도 같다. 재현: 부록 B `overlapping_same_pitch_export_is_counted` | `report.rs:130-153` |
| 6 | 제안 | 보고의 과소 셈 둘. (a) 벨로시티 0 노트는 `note_on`이 1로 써서 512로 돌아오지만 `from_midi1(to_midi1(0)) == 0`이라 세지 않는다. (b) `KEPT_META`의 메타라도 실제로 버리는 것(지휘 트랙·Type 0의 이름, 0 µs 템포, 분모 2^16 이상 박자표, 곡 트랙의 둘째 이름)을 세지 않는다. 재현: 부록 B `velocity_zero_export_is_counted`, `dropped_title_and_bad_meta_are_counted` | `report.rs:9`, `91`, `145` |
| 7 | 제안 | 텍스트 내보내기가 `Loss::Mismatch`(되읽기가 손실로 설명되지 않게 다름, keyscore-text 버그)가 있어도 파일을 쓴다. CLI는 같은 경우 `ExportMismatch`로 쓰지 않는다(S0-T8에서 정한 동작). 앱도 쓰지 않고 시트로 알리는 편이 같은 변환의 같은 판단이다. 공개 API로 `Mismatch`를 만들지 못해 재현 시험은 없다 | `import_export.rs:283-288`, `tools/ksq-cli/src/cli.rs:520-528` |
| 8 | 제안 | `menu_and_palette_run_import_export`는 이름과 계획(#16 「팔레트에서 실행된다」)과 달리 파일 메뉴의 Action 이름만 본다. 팔레트 실행(UI-11)을 보는 시험을 더하거나 이름을 고친다 | `crates/keyscore-app/tests/import_export.rs` 끝 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 1(디스크의 다른 문서를 덮는 데이터 손실)과 수정 4개(2–5). 제안 6–8은 거절할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [ ] 테스트를 읽기 전에 1번을 썼다(제품 코드를 먼저 읽고 목록을 정했으나, 문서에 적기 전에 도움 함수를 빌리려 구현자 시험 파일을 열었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 8개 모두 리뷰 시험 파일 끝에 붙여 실패를 확인했다)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`plan/stages/12-S2-app-shell.md` 한 줄, `scripts/check-deps.sh` 허용 목록은 이 Task의 것)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`ksq-cli` 단위 시험은 `keyscore-text`로 옮겨 수가 같다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(`keyscore-text` → `serde_json`, MIT/Apache-2.0, 이미 워크스페이스에 있다)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 앱 재현 시험(실패)

`crates/keyscore-app/tests/review_s2_t7c.rs` 끝에 그대로 붙이면 돌아간다(도움 함수·`use`는 그 파일 것을 쓴다). 네 시험 모두 지금 실패한다.

~~~rust
/// 지적 1: 상위 형식 버전 원본(읽기 전용)에 내보내면 원본이 텍스트로 덮인다.
#[gpui_kit::test]
fn export_over_newer_format_original_keeps_it(cx: &mut TestAppContext) {
    let dir = start(cx, "x-newer");
    let path = dir.join("future.ksq");
    std::fs::write(
        &path,
        r#"{"format_version": 7, "future": {"x": 1}, "tracks": []}"#,
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, _view) = open_file(cx, &path);
    run(cx, handle, file::ExportText);
    answer_new_path(cx, path.clone());
    if cx.has_pending_prompt() {
        cx.simulate_prompt_answer("확인");
        cx.run_until_parked();
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "상위 버전 원본이 내보내기로 덮였다"
    );
}

/// 지적 1: 다른 창이 연 `.ksq`에 내보내면 그 파일이 MIDI로 덮이고, 그 창은 깨끗한 채라 닫아도 묻지 않는다.
#[gpui_kit::test]
fn export_over_file_open_in_other_window_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "x-other");
    let a = dir.join("a.ksq");
    std::fs::write(&a, ksq::to_string(&sample())).unwrap();
    let before = std::fs::read(&a).unwrap();
    let (_ha, va) = open_file(cx, &a);
    let (hb, _) = open(cx, Project::new());
    run(cx, hb, file::ExportSmf);
    answer_new_path(cx, a.clone());
    if cx.has_pending_prompt() {
        cx.simulate_prompt_answer("확인");
        cx.run_until_parked();
    }
    let a_dirty = dirty(cx, &va);
    assert!(
        std::fs::read(&a).unwrap() == before || a_dirty,
        "다른 창이 연 a.ksq를 MIDI로 덮었는데 그 창은 깨끗하다"
    );
}

/// 지적 2: 가져와 저장 안 된 창(바뀜 켜짐)인데 편집하기 전에는 자동 저장본이 생기지 않는다.
#[gpui_kit::test]
fn imported_smf_window_is_autosaved(cx: &mut TestAppContext) {
    start(cx, "x-autosave");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let imported = windows(cx).pop().unwrap();
    assert!(dirty(cx, &imported));
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY + Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(autosaves(cx), 1, "가져와 저장 안 된 창의 자동 저장본이 없다");
}

/// 지적 4: 같은 이름 트랙의 채널·프로그램이 블록과 다르면 블록의 ch·prog가 소리 없이 사라진다.
#[test]
fn import_text_into_same_name_track_keeps_block_channel() {
    let mut doc = ProjectDocument::new(sample()).unwrap(); // Lead ch1, 프로그램 없음
    let parsed = keyscore_text::parse("@track Lead ch5 prog40\nc\n", Mode::Lenient).unwrap();
    import_text_into(&mut doc, &parsed, Tick::new(7680)).unwrap();
    let p = doc.project();
    let placed = p
        .tracks()
        .iter()
        .find(|t| t.clips().any(|(_, c)| c.start().get() == 7680))
        .unwrap();
    assert_eq!(
        (placed.channel().get(), placed.program().map(|p| p.get())),
        (5, Some(40)),
        "블록의 ch5 prog40이 ch1 트랙에 들어가 바뀌었다"
    );
}
~~~

## 부록 B. SMF 보고 재현 시험(실패)

`crates/keyscore-midi/tests/review_s2_t7c.rs` 끝에 그대로 붙이면 돌아간다(`smf`·`track` 도움 함수는 그 파일 것을 쓴다). 네 시험 모두 지금 실패한다.

~~~rust
// 아래 시험은 crates/keyscore-midi/tests/review_s2_t7c.rs 끝에 붙인다. 쓰는 이름을 더 들여온다.
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use keyscore_midi::{SmfFormat, smf_export};

fn note(start: u64, len: u32, pitch: u8, v: u16) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::new(v),
    )
    .unwrap()
}

fn one_clip(notes: Vec<Note>) -> Project {
    let mut p = Project::new();
    let mut t = Track::new("V", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 3840).unwrap();
    for n in notes {
        clip.insert_note(n).unwrap();
    }
    t.insert_clip(clip).unwrap();
    p.push_track(t).unwrap();
    p
}

fn reimport(p: &Project) -> (Project, keyscore_midi::ImportReport, keyscore_midi::ExportReport) {
    let (file, ex) = smf_export(p, SmfFormat::Parallel).unwrap();
    let back = SmfFile::read(&file.write()).unwrap();
    let (q, report) = smf_import(&back).unwrap();
    (q, report, ex)
}

/// 지적 3: 화음의 아래 음에만 가사가 있으면 내보내기가 빈 가사로 자리를 채운다. 다시 가져오면 가사는 그대로인데
/// 빈 가사를 「노트 시작에 붙지 않은 가사」로 센다(과대 보고).
#[test]
fn reimport_of_chord_lyric_reports_nothing_unattached() {
    let v = Velocity::from_midi1(100).unwrap().get();
    let p = one_clip(vec![note(0, 480, 60, v).with_lyric("la"), note(0, 480, 64, v)]);
    let (q, report, ex) = reimport(&p);
    assert!(ex.is_lossless(), "{ex:?}");
    let lyrics: Vec<String> = q.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter().filter_map(|n| n.lyric().map(str::to_owned)).collect::<Vec<_>>())
        .collect();
    assert_eq!(lyrics, vec!["la".to_owned()], "가사는 그대로 왕복한다");
    assert_eq!(report.unattached_lyrics, 0, "{report:?}");
}

/// 지적 5: 같은 트랙에서 같은 음이 겹치면 SMF에서 먼저 켠 것을 먼저 끄므로 길이가 바뀐다. 보고는 손실 없음.
#[test]
fn overlapping_same_pitch_export_is_counted() {
    let v = Velocity::from_midi1(100).unwrap().get();
    let p = one_clip(vec![note(0, 1920, 60, v), note(480, 480, 60, v)]);
    let (q, _, ex) = reimport(&p);
    let notes: Vec<(u64, u32)> = q.tracks()[0]
        .clips()
        .flat_map(|(_, c)| c.notes().iter().map(|n| (n.start().get(), n.length())).collect::<Vec<_>>())
        .collect();
    assert!(
        notes == vec![(0, 1920), (480, 480)] || !ex.is_lossless(),
        "겹친 같은 음이 {notes:?}로 바뀌었는데 손실 없음"
    );
}

/// 지적 6: 벨로시티 0은 Note On 1로 써서 바뀌는데 세지 않는다.
#[test]
fn velocity_zero_export_is_counted() {
    let p = one_clip(vec![note(0, 480, 60, 0)]);
    let (q, _, ex) = reimport(&p);
    let got = q.tracks()[0].clips().next().unwrap().1.notes()[0].velocity().get();
    assert!(got == 0 || ex.velocity_rounded == 1, "0이 {got}로 바뀌었는데 세지 않았다");
}

/// 지적 6: 지휘 트랙의 곡 제목, 0 µs 템포, 분모 2^20 박자표는 버리지만 세지 않는다.
#[test]
fn dropped_title_and_bad_meta_are_counted() {
    let file = smf(
        1,
        &[
            track(&[
                (0, &[0xFF, 0x03, 0x05, b'T', b'i', b't', b'l', b'e']),
                (0, &[0xFF, 0x51, 0x03, 0, 0, 0]),
                (0, &[0xFF, 0x58, 0x04, 4, 20, 24, 8]),
            ]),
            track(&[(0, &[0x90, 60, 100]), (480, &[0x80, 60, 0])]),
        ],
    );
    let (_, report) = smf_import(&file).unwrap();
    assert!(!report.is_lossless(), "{report:?}");
}
~~~
