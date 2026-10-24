# 적대적 리뷰: S2-T7e S2-T7b·T7c·T7d 리뷰 이월 줄 정리 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: `1569822`(`21a58c6..1569822`, 브랜치 `s2/t7e-carry-overs`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

(제품 코드 diff와 그것이 기대는 `song.rs`·`core`·`client.rs`만 읽고 적었다. 시험 파일은 이 절을 쓴 뒤 열었다.)

A. `autosave.rs`

1. `remove_dead_locks`가 시작 때마다 돈다. 살아 있는 다른 세션의 내용 없는 메타(두 rename 사이)·잠금 파일을 지우는 경로가 있는가. 잠금 없이 뜬 세션(`init` 실패)의 메타·저장본은 어떻게 되는가.
2. `init`의 재시도(20 ms × 2)가 실제로 닿는 경로가 있는가. 다른 실행이 막 만든 잠금 파일을 `remove_dead_locks`가 잠그는 일이 있는가(`old` 조건).
3. `recover`가 `claim`을 쥔 뒤 창 열기가 실패하거나(`open_project_window` Err) 같은 파일을 연 창이 있어 경로 없이 열 때, 잠금이 새거나 저장본이 영영 잠기는가.
4. [복구]와 [버리기]가 다른 실행에서 겹칠 때: B의 `still_dead`(메타 읽기)와 `claimed`(내용 열기) 사이에 A의 메타·내용 rename이 모두 끝나면 B가 A가 방금 쓴 저장본을 지운다. 이 틈이 이번 수정으로 넓어졌는가.
5. 복구 창의 첫 쓰기가 `prepare_autosave`에서 None을 돌려주는 경우가 있는가(`mark_recovered`가 dirty를 세우는가).
6. 내용 없는 메타 판정 `meta.with_extension("ksq")`가 임시 파일(`<키>.json.tmp-N`)이나 `.json`이 아닌 파일, 읽을 수 없는 `.json`에 잘못 닿는가. 메타 없는 내용 파일(읽을 수 없는 저장본)을 건드리는가.

B. `smf/report.rs`, `song.rs`, `import_export.rs`

7. `changed_by_pairing`의 전제(틱 순 시작 ↔ 틱 순 끝)가 `split_track`의 FIFO와 어긋나는 입력: 벨로시티 0 노트(`note_on`이 1로 올리는가), 길이 0이 되는 숨김 꼬리(클립 끝 == 노트 시작), 같은 틱 같은 음 여러 노트, Type 0에서 같은 채널의 다른 트랙, Type 1에서 같은 채널의 다른 트랙(섞이면 안 됨), 노트가 없는 트랙 때문에 트랙 번호가 밀리는 경우.
8. `dropped_same_tick`이 `TempoMap::from_changes`·`MeterMap::from_changes`의 실제 규칙(안정 정렬 뒤 같은 틱의 마지막)과 같은가. 트랙 순서·이벤트 순서가 다르면 어긋나는가. 재배율(PPQ ≠ 960)로 다른 틱이 같은 틱이 되는 경우, 잘못된 템포(0 µs, 3바이트 아님)·잘못된 박자표(dd ≥ 16, 분자 0)가 `kept_meta`와 이중으로 세어지는가.
9. `yields_song_track`이 `split_track`의 「트랙 남김」과 어긋나는 입력: 가사만 있는 트랙, 가사 + 폴리 프레셔 트랙(`is_empty`가 가사를 세지만 가사는 필터 뒤에 붙는다), 짝 없는 Note Off + 이름, Type 0의 이름, 프로그램·CC·채널 프레셔·벤드만 있는 트랙, 끝나지 않은 Note On.
10. `import_text_into`의 `find` 합치기로 전에 들어가던 트랙에 더는 들어가지 않는 입력이 있는가(같은 이름 하나, 조건 맞음 → 그대로여야 한다).

C. `engine.rs`

11. `visual_match`(보이는 모양 같음 가운데 고유 ID 최소)와 `best_match`(정확히 같은 이름 가운데 최소)가 고르는 장치가 어긋나는 경우: 고유 ID가 가장 작은 장치의 정확한 이름이 대체 포트 이름과 다를 때 트랙·설정이 그 이름으로 가고, 그 뒤 `best_match`가 같은 장치를 고르는가.
12. `follow_device_in_settings`: 클럭 목록이 대체 포트 둘을 가리킬 때 한쪽만 바뀌는가, 장치 이름이 이미 목록에 있으면 하나만 남는가, 설정 전역이 없을 때 패닉하지 않는가, `install`에서 엔진 전역이 아직 없을 때 `AppSettings::update`가 엔진을 건드리는가.
13. `remember_user_port`·`rename_virtual_port`의 표시 지우기가 정확한 이름 비교라, 보이는 모양은 같고 글자가 다른(결합자 등) 남은 표시는 지우지 못한다. 그 표시가 다음 시작에 사용자 포트를 대체 포트로 만드는 경로가 있는가.
14. `release_mapped`에서 `follow_device_in_settings`를 `remove_virtual_port`보다 먼저 부른다. `remove_virtual_port`가 실패하면 설정·트랙만 바뀌고 포트는 남는다. 그 경로가 있는가.
15. `install`의 `returned` 메시지와 `follow_device_in_settings`가 정확한 이름이 아닌 보이는 모양으로 고른 장치로 가는가(`collides`는 보이는 모양, `wanted.retain`은 정확한 이름).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 살아 있는 세션의 내용 없는 메타·잠금 파일 보존 | 있음(`live_session_meta_without_content_is_kept`, [버리기] 경로만) | 복구 대상이 없는 시작 정리 경로는 없었다. 리뷰 시험 `live_orphan_meta_survives_startup_cleanup_without_recoverables`로 메웠다(통과). 잠금 없이 뜬 세션은 공개 API로 만들 수 없어 코드 대조: 전과 같이 그 세션의 저장본은 다른 실행의 복구 대상이 된다(`init` 문서에 적힌 한계) |
| 1 이 세션의 내용 없는 메타 | 없음 | 리뷰 시험 `own_orphan_meta_survives_startup_cleanup`(통과) |
| 2 `init` 재시도 | 없음(보고서도 코드 대조만) | 코드 대조: 닿는 경로가 없다(7절 3) |
| 2 막 생긴 잠금 파일 | 있음(`startup_without_recoverables_removes_old_dead_locks`) | 내용 없는 메타 + 막 생긴 잠금 파일 조합은 리뷰 시험 `orphan_meta_goes_now_but_fresh_lock_waits_until_old`(통과) |
| 3 경로 없이 여는 복구 창의 잠금 이어받기 | 없음 | 리뷰 시험 `recover_without_path_still_takes_over_autosave`(통과). 창 열기 실패는 `ProjectDocument::with_limit` 실패뿐이라 재현하지 못했다. 코드 대조: `continue`에서 `claim`이 떨어져 잠금이 풀린다 |
| 3 잠겨 건너뛴 저장본이 잠금이 풀리면 돌아오는가 | 없음 | 리뷰 시험 `skipped_claimed_autosave_returns_when_claim_is_released`(통과) |
| 4 [복구]·[버리기] 경합 | 있음(`review_r4_discard_of_claimed_autosave_is_rewritten_by_holder`, 첫 쓰기 실패 경우) | 첫 쓰기가 성공한 뒤의 남은 틈은 한 스레드에서 끼울 수 없다. 코드 대조(7절 2) |
| 5 `mark_recovered`의 dirty | 있음(복구 창이 dirty인 것을 여러 시험이 본다) | `document.rs:163-169`가 `dirty = true` |
| 6 임시 파일·읽을 수 없는 `.json`·메타 없는 내용 | 없음 | 리뷰 시험 `unreadable_meta_is_left_alone_by_startup_cleanup`, `content_without_meta_is_left_alone_by_startup_cleanup`(통과). 임시 파일만 남은 끝난 세션은 7절 1(부록 A) |
| 7 짝 맞춤 전제 | 있음(B1~B4, 손으로 고른 경우) | 리뷰 시험 `overlap_count_equals_notes_changed_by_rereading`(proptest 300회 × 두 형식, 트랙 1–3·채널 1·2·벨로시티 0 포함·클립 쪼개기)로 실제 다시 읽은 결과와 비교했다(통과). 같은 틱 화음·숨김 꼬리·빈 트랙 뒤 같은 채널 트랙도 따로(통과) |
| 8 같은 틱 메타 | 있음(B5·B6) | 리뷰 시험 `same_tick_meta_count_matches_actual_maps`(proptest 300회, PPQ 1–19999, 잘못된 값 섞음)로 실제 `TempoMap`·`MeterMap`과 비교했다(통과). 이중 셈 없음·트랙 건너 마지막 것 유지도 따로(통과) |
| 9 사라지는 트랙 이름 | 있음(B7: 폴리 프레셔·짝 없는 Off·프로그램) | 가사 + 폴리 프레셔, 가사만, CC·채널 프레셔·벤드만, Type 0 이름 둘, 끝나지 않은 Note On을 리뷰 시험 5개로 더했다(통과) |
| 10 같은 이름 트랙 | 있음(B8, 둘째 트랙 두 경우) | 같은 이름 하나·조건 맞음은 기존 `import_export.rs` 시험이 덮는다. 더하지 않았다 |
| 11 `visual_match`·`best_match` 일치 | 있음(C3: 트랙) | 메트로놈·클럭도 같은 이름을 따르는 것은 코드 대조(`release_mapped`가 같은 `destination`을 넘긴다) |
| 12 설정 전역 없음·엔진 없음 | 없음 | 코드 대조: `follow_device_in_settings`가 `has_global` 검사, `install`의 `AppSettings::update`는 저장 타이머만 건다(`app_state.rs:50-62`). 클럭 둘은 `release_mapped`가 장치마다 한 번씩 돌아 각각 바뀐다 |
| 13 정확한 이름 비교의 남은 표시 | 있음(C4: 같은 글자) | 보이는 모양만 같은 경우는 코드 대조(7절 4) |
| 14 `remove_virtual_port` 실패 | 없음 | 코드 대조: `mapped`는 `engine.mapped`에서 찾았고 그 포트는 `extras`에 있어 `NotFound`가 나지 않는다. 실패 시 알림이 있다 |
| 15 `install`의 보이는 모양 비교 | 있음(C2: 공백 둘) | 코드 대조로 충분 |

## 3. 깨뜨리기 시도

리뷰 시험 파일: `crates/keyscore-app/tests/review_s2_t7e.rs`(8개), `crates/keyscore-midi/tests/review_s2_t7e.rs`(12개, proptest 2개 포함). 모두 `CARGO_TARGET_DIR=target/alt`로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 트랙 1–3개·채널 1·2·음높이 60·62·벨로시티 0~0xFFFF·클립 쪼개기(숨김 꼬리, 클립 경계)를 proptest로 300회 만들어 Type 0·1로 내보내고 다시 읽은 뒤, 실제로 (시작, 끝)이 바뀐 노트 수와 `overlapping_notes`를 비교 | 못 깨뜨림(600회 모두 같음) | `overlap_count_equals_notes_changed_by_rereading` |
| 2 | 같은 틱 같은 음 셋(길이 다름, 벨로시티 0·1·100) | 못 깨뜨림(0, 다시 읽어도 그대로) | `same_tick_same_pitch_chord_is_unchanged` |
| 3 | 클립을 쪼개 숨김 꼬리가 다음 클립의 같은 음(꼬리 안에서 시작)과 맞닿게 함 | 못 깨뜨림(자른 뒤 보므로 0) | `hidden_tail_cut_before_pairing_is_not_counted` |
| 4 | 노트 없는 트랙 뒤에 같은 채널 트랙 둘(Type 1은 따로, Type 0은 섞임) | 못 깨뜨림(0 / 2) | `type1_same_channel_tracks_stay_separate_even_after_empty_track` |
| 5 | PPQ 1–19999, 트랙 1–3개, 델타 0–3에 템포(0 µs·최대값 포함)·박자표(분자 0 포함)를 proptest로 300회 적고, 올바른 것마다 실제 맵의 같은 틱 값과 다르면 버려진 것으로 세어 `skipped_events`와 비교 | 못 깨뜨림 | `same_tick_meta_count_matches_actual_maps` |
| 6 | 0 µs 템포와 올바른 템포가 같은 틱 | 못 깨뜨림(1, 이중 셈 없음) | `invalid_tempo_at_same_tick_is_counted_once` |
| 7 | 같은 틱 박자표가 트랙을 건너 다름 | 못 깨뜨림(뒤 트랙 것이 남고 1) | `same_tick_meters_across_tracks_keep_last_track` |
| 8 | 가사 + 이름 + 폴리 프레셔 트랙, 가사만 있는 이름 트랙 | 못 깨뜨림(이름은 세고 가사는 붙거나 orphan) | `name_of_lyric_and_poly_pressure_track_is_counted_lyric_is_not`, `name_of_lyric_only_track_is_counted` |
| 9 | CC만·채널 프레셔만·벤드만 있는 이름 트랙, Type 0의 이름 둘, 끝나지 않은 Note On만 있는 이름 트랙 | 못 깨뜨림 | `names_of_lane_only_tracks_are_kept`, `type0_names_are_counted_even_with_song_track`, `name_of_unterminated_note_track_is_kept` |
| 10 | 끝난 세션의 내용 없는 메타 + 막 생긴(1분 안) 잠금 파일 → 시작 정리 → 잠금 파일을 늙히고 다시 정리 | 못 깨뜨림(메타는 바로, 잠금 파일은 늙은 뒤) | `orphan_meta_goes_now_but_fresh_lock_waits_until_old` |
| 11 | 이 세션 id의 내용 없는 메타 | 못 깨뜨림(남는다) | `own_orphan_meta_survives_startup_cleanup` |
| 12 | 메타 없는 오래된 내용 파일만 있을 때 시작 정리 | 못 깨뜨림(시트 없음, 파일 그대로) | `content_without_meta_is_left_alone_by_startup_cleanup` |
| 13 | JSON이 아닌 `.json`, 세션이 없는 `.json` | 못 깨뜨림(그대로) | `unreadable_meta_is_left_alone_by_startup_cleanup` |
| 14 | 살아 있는 다른 실행(같은 프로세스의 두 번째 앱)의 내용 없는 메타 + 늙은 잠금 파일, 복구 대상 없는 시작 정리 | 못 깨뜨림(둘 다 남는다) | `live_orphan_meta_survives_startup_cleanup_without_recoverables` |
| 15 | 저장본의 원래 파일을 이미 연 창이 있을 때 [복구] | 못 깨뜨림(경로 없는 창, 메타가 이 세션, 다른 실행의 복구 대상 아님, 먼저 연 창의 경로 그대로) | `recover_without_path_still_takes_over_autosave` |
| 16 | 복구 대상 둘 가운데 하나를 다른 쪽이 잠근 채 [복구], 잠금을 풀면 | 못 깨뜨림(잠긴 동안 0, 풀리면 1로 돌아옴, 내용·메타 그대로) | `skipped_claimed_autosave_returns_when_claim_is_released` |
| 17 | [복구] 뒤 정리에서 아직 끝난 세션 것으로 남은(다른 쪽이 잠근) 저장본의 세션 잠금 파일 | 못 깨뜨림(남는다, 복구한 쪽 것은 지움) | `recover_cleanup_keeps_lock_of_session_whose_autosave_is_still_claimed` |
| 18 | 첫 쓰기의 rename 전에 끝난 세션(세대별 임시 파일 둘 + 늙은 잠금 파일)을 시작 정리가 지우는가 | 깨짐: 잠금 파일은 지우고 임시 파일 둘은 남는다 | 부록 A(파일에 넣지 않음) |
| 19 | SMF의 길이 0 노트(같은 틱 On·Off), 트랙 끝에서 끝나지 않은 Note On만 있는 트랙의 가져오기 | 못 깨뜨림(둘 다 가져온다, 이번 범위 밖 확인) | 없음(임시 시험으로만 확인) |

구현자 시험도 모두 다시 돌렸다: `smf_report`(15), `t7e_autosave`(6), `t7e_port_mapping`(4, 자식 프로세스), `import_export`(15), `review_s2_t7b_r4`(5), `review_s2_t7d_r3`(3) 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 살아 있는 세션의 저장본·메타·잠금 파일은 지우지 않는다 | 아니오 | `session_alive`가 잠금 파일을 잠가 본다. 내용 없는 메타도 `session != own && !session_alive`일 때만(`autosave.rs:549-553`). 시도 10·11·14. 잠금 없이 뜬 세션(`init` 실패)은 전과 같이 예외다(문서화된 한계) |
| 저장본 하나는 한 번에 한 창만 복구한다 | 아니오 | `claim`이 검사와 잠금을 한 번에 한다(`autosave.rs:520-524`, `595-597`). 시도 16, 구현자 A3 |
| [버리기]는 다른 실행이 잠근 저장본을 지우지 않는다 | 아니오(첫 쓰기 전·실패) / 틈 있음(첫 쓰기 뒤) | `autosave.rs:637`. 첫 쓰기가 끝난 뒤의 틈은 7절 2(수정 전에도 있던 틈) |
| 보고 함수는 변환 결과를 바꾸지 않는다 | 아니오 | `smf_import`·`smf_export`는 `Song` 변환을 먼저 하고 보고는 읽기만 한다. 기존 `import/export_matches_existing_conversion_on_samples` |
| `overlapping_notes` = 다시 읽으면 (시작, 끝)이 바뀌는 노트 수 | 아니오 | 시도 1~4. 전제(길이 ≥ 1, 같은 틱 Off가 On보다 앞)는 `Note::new`(`note.rs:181`)와 `finish_track`(`song.rs:794`)이 보장 |
| `skipped_events`의 같은 틱 셈 = `from_changes`가 실제로 버리는 수 | 아니오 | 시도 5~7. 같은 `Rescale::for_ppq`를 쓴다 |
| 트랙·설정 변경은 기존 경로만 쓴다(창마다 undo 한 단위) | 아니오 | `release_mapped`의 `edit_document`, `follow_device_in_settings`의 `AppSettings::update` |
| 같은 트랙 클립 겹침 없음(core) | 아니오 | `import_text_into`의 `find`에 겹침 조건이 그대로 있다(`import_export.rs:77-85`) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-5(자동 저장·복구) | 충족 | `autosave.rs:181-189`(시작 정리), `520-524`·`595-597`(잠금 먼저), `637`(버리기), `549-553`(내용 없는 메타). 시도 10~17 |
| 8.4(복구 시트) | 충족 | 시트 흐름은 바뀌지 않았다. `offer_recovery` 대상 없음 경로만 정리를 더했다 |
| IO-2(SMF 가져오기·내보내기) | 충족 | `report.rs:31-106`·`214`·`259-300`. 변환 결과는 그대로, 보고만 정밀해졌다. 시도 1~9 |
| SEQ-9(Type 1 트랙별, Type 0 채널별) | 충족 | 보고의 묶음(`report.rs:263-267`)이 이 규칙을 따른다. 시도 4 |
| IO-3·5.14(텍스트 가져오기) | 충족 | `import_export.rs:77-85`. 구현자 B8, 기존 시험 |
| OUT-3(같은 이름 포트 재연결) | 충족 | `engine.rs:1295-1299`(`visual_match`), `1286-1290`(`best_match`)이 같은 기준(고유 ID 최소). 구현자 C3 |
| OUT-5(클럭 포트) | 충족 | `engine.rs:1303-1318`이 클럭 목록을 장치 이름으로. 구현자 C1·C2 |
| OUT-8(대체 매핑) | 충족 | `engine.rs:273-281`, `420-421`(남은 표시 지우기), `336-338`·`373`(해제). 구현자 C1~C4 |
| MET-1(메트로놈 포트) | 충족 | `engine.rs:1310-1312`. 구현자 C1·C2, `review_s2_t7d_r3` 단언 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 바뀐 코드는 모두 메인 스레드(설정·문서·파일)와 변환 뒤 보고다.
- 제품 코드의 `unwrap`/`expect`: diff에 없음. `song.rs:514`의 `unwrap_or(u64::MAX)`는 전과 같다.
- 에러가 삼켜지는 곳:
  - `autosave.rs:551`·`580` `let _ = fs::remove_file(...)`: 정리 실패는 다음 시작에 다시 시도한다. 맞다.
  - `autosave.rs:147` `fs::File::create(&path).ok()?`: 잠금 없이 뜬다(전과 같음, 문서화).
  - `autosave.rs:521-522` `claim`: `File::open` 실패와 잠금 실패를 구별하지 않고 건너뛴다. 알림이 없다(7절 6).
  - `autosave.rs:603` `open_project_window` Err → `continue`: 잠금은 풀리고 저장본은 다음 실행의 대상으로 남는다. 알림은 없다(전과 같음).
  - `engine.rs:336-337` `visual_match` None → 나타남 이벤트의 이름으로 되돌아간다. 맞다.
  - `engine.rs:375-386` `remove_virtual_port` Err는 `say_everywhere`로 알린다.

## 7. 지적 사항

차단·수정 없음. 아래는 모두 제안(거절할 수 있다).

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 첫 쓰기의 rename 전에 끝난 세션은 세대별 임시 파일(`<키>.ksq.tmp-N`, `<키>.json.tmp-N`)만 남긴다. 그 키는 복구 대상이 아니라 `remove_generation_temps`에 닿지 않고, 시작 정리는 메타·잠금 파일만 보므로 어떤 정리도 지우지 않는다(잠금 파일은 지운다). 틈이 `write_synced` 둘과 rename 사이라 좁고 결과는 작은 파일 둘이다. 방안: 시작 정리에서 `STALE_TEMP_AGE`보다 오래되고 키의 메타·내용이 없는 임시 파일을 지운다. 재현 부록 A | `autosave.rs:528-583`, `687-698` |
| 2 | 제안 | [복구](A)·[버리기](B)가 다른 실행에서 겹칠 때의 남은 틈: B의 `still_dead`(메타 읽기)와 `claimed`(내용 열기) 사이에 A의 첫 쓰기(메타 rename, 내용 rename)가 모두 끝나면 B는 새 메타를 못 보고 새 내용 inode(잠기지 않음)를 열어 A가 방금 쓴 저장본을 지운다. A는 `Written` 뒤 `claim`을 놓고 다음 편집 전까지 저장본을 다시 쓰지 않으므로, 그 사이 A가 끝나면 복구본이 없다. 수정 전(`claimed` 검사 없음)에도 같은 틈이 있었고 이번 수정은 그보다 넓은 틈(첫 쓰기 전·실패)을 닫았다. 사람 손으로 맞추기 어렵고 한 스레드에서 끼울 수 없어 코드 대조. 방안: `discard`가 내용을 잠근 채 메타를 다시 읽고 세션이 그대로일 때만 지운다(그래도 rename 둘과 완전히 배타적이지는 않다) | `autosave.rs:632-645` |
| 3 | 제안 | `init`의 재시도는 닿는 경로가 없다. 다른 실행이 새 세션의 잠금 파일을 잠그는 길은 `session_alive`뿐인데, 그 id를 가리키는 메타가 아직 없고 잠금 파일 정리는 `old`(1분)가 아닌 파일을 잠그지 않는다. 해는 없다. 보고서 「알려진 한계」의 「다른 실행의 `session_alive` 판정은 잠깐이라 충분」은 실제로 일어나지 않는 경우다. 남겨도 되지만 그 설명은 고친다 | `autosave.rs:140-152` |
| 4 | 제안 | `remember_user_port`·`rename_virtual_port`의 남은 표시 지우기는 정확한 이름 비교다. `duplicate_key`가 무시하는 글자(결합자·이체자 선택자)만 다른, 보이는 모양이 같은 이름으로 포트를 만들면 표시가 남는다. 다음 시작에 `mapped.retain`도 정확한 이름이라 사용자 포트가 대체 포트로 표시되지는 않는다(해 없음). 다만 그 표시와 보이는 모양이 같은 장치가 있으면 `install`이 「장치 「X」이 있어 대체로 만든 가상 포트를 지웠다」를 거짓으로 알리고(`collides`는 보이는 모양), `follow_device_in_settings`를 그 이름으로 부른다(설정이 그 이름이 아니라 아무 일 없음). 코드 대조. 방안: 표시 지우기를 `same_port_name`으로 | `engine.rs:273-281`, `420-421`, `692-711` |
| 5 | 제안(이월 후보) | 이번 범위 밖 관찰: `rename_virtual_port`로 설정 `virtual_ports`에 남은(시작 때 만들지 못한) 이름으로 바꾸면 `virtual_ports`에 같은 이름이 둘이 된다(`check_new_name`은 목적지만, `set_name`은 살아 있는 포트만 본다). 다음 시작에 둘째 만들기가 같은 모양 거부로 실패해 알림이 난다. `remove_virtual_port`가 둘 다 지우므로 쌓이지는 않는다. 코드 대조 | `engine.rs:395-420` |
| 6 | 제안 | `recover`가 잠그지 못해(다른 실행이 먼저) 건너뛴 저장본은 알림이 없다. 시트에 이름이 떴는데 창이 열리지 않는다. 다른 실행이 그 창을 열었으므로 손실은 없다. 방안: 건너뛴 수를 `say_everywhere`로 | `autosave.rs:595-597` |

이월 지적의 수정 확인

| 이월 | 확인 |
| --- | --- |
| T7b 리뷰 4 지적 1(내용 없는 메타) | 고쳐짐. 구현자 A1·A5, 시도 10~14. 살아 있는 세션·이 세션 것은 남는다 |
| T7b 리뷰 4 지적 2(`discard`의 `claimed`) | 고쳐짐. 구현자 A2, `review_r4_discard_of_claimed_autosave_is_rewritten_by_holder` 기대값 변경은 수정 의도와 맞다(지우지 않으므로 1). 남은 틈은 7절 2 |
| T7b 리뷰 4 지적 3(`recover` 잠금 먼저) | 고쳐짐. `claim`이 창보다 먼저, 실패하면 건너뛴다. 시도 15~17 |
| T7b 리뷰 4 이월(시작 때 정리, `init` 재시도) | 고쳐짐 / 재시도는 닿지 않는 코드(7절 3) |
| T7c 리뷰 2 제안 1(겹침 셈) | 고쳐짐. 시도 1~4(proptest가 실제 다시 읽기와 일치) |
| T7c 리뷰 2 제안 2(같은 틱 메타, 사라지는 트랙 이름) | 고쳐짐. 시도 5~9 |
| T7c 리뷰 2 제안 3(같은 이름 트랙 여럿) | 고쳐짐. 구현자 B8 |
| T7d 리뷰 3 제안 1(메트로놈·클럭 설정) | 고쳐짐. 구현자 C1·C2, `review_s2_t7d_r3` 단언 통과 |
| T7d 리뷰 3 제안 2(보이는 모양 같은 장치 여럿) | 고쳐짐. `visual_match`가 `best_match`와 같은 기준. 구현자 C3 |
| T7d 리뷰 3 제안 4(남은 표시) | 고쳐짐(정확한 이름). 보이는 모양만 같은 경우는 7절 4 |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

세 리뷰의 이월 지적 아홉 건이 모두 고쳐졌고, 수정이 만든 회귀를 찾지 못했다. 새 셈(`changed_by_pairing`, `dropped_same_tick`, `yields_song_track`)은 proptest로 실제 변환 결과와 대조해 어긋남이 없었다. 시작 때 정리는 살아 있는 세션·이 세션·막 생긴 파일·읽을 수 없는 파일을 건드리지 않는다. 찾은 것은 제안 여섯(남은 임시 파일 하나, 좁은 경합 하나, 닿지 않는 코드 하나, 보이는 모양 비교 하나, 범위 밖 관찰 하나, 알림 하나)뿐이다. 구현자는 받아들이거나 거절 이유를 보고서에 적는다. 1과 4는 S2 출구 전 이월 후보로 적어도 된다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 diff와 그것이 기대는 `song.rs`·`core`·`client.rs`만 읽고 적었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다(리뷰 시험 20개 통과, 부록 A 1개 실패를 임시 파일로 확인)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`21a58c6..1569822`: `autosave.rs`, `engine.rs`, `import_export.rs`, `smf/report.rs`, `smf/song.rs`, 시험 6개, 계획·보고서, 단계 파일 이월 줄)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`review_s2_t7b_r4.rs` 기대값 하나와 `review_s2_t7d_r3.rs` 관찰 출력 → 단언 변경은 수정 의도대로다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
- [x] 리뷰 시험 파일이 `cargo fmt --all -- --check`와 `cargo clippy -p keyscore-midi -p keyscore-app --all-targets -- -D warnings`를 통과한다

## 부록 A. 실패하는 재현 테스트(7절 1)

`crates/keyscore-app/tests/review_s2_t7e.rs` 끝에 그대로 붙이면 같은 파일의 도우미(`start`, `autosave_dir`, `names`, `age`, `launch_with_recovery`)로 컴파일된다. `1569822`에서 실패한다(남은 파일 `["gone.json.tmp-1", "gone.ksq.tmp-1", "session-<이 실행>.lock"]`: 끝난 세션의 잠금 파일은 지웠지만 임시 파일 둘은 남는다).

~~~rust
/// 첫 쓰기의 rename 전에 끝난 세션은 세대별 임시 파일만 남긴다. 시작 정리가 그 세션의 잠금 파일은 지우지만
/// 임시 파일은 어떤 정리도 지우지 않는다.
#[gpui_kit::test]
fn generation_temps_of_session_dead_before_first_rename_are_removed(cx: &mut TestAppContext) {
    start(cx, "temps-before-rename");
    let auto = autosave_dir(cx);
    std::fs::create_dir_all(&auto).unwrap();
    for n in ["gone.ksq.tmp-1", "gone.json.tmp-1"] {
        std::fs::write(auto.join(n), "x").unwrap();
        age(&auto.join(n), Duration::from_mins(10));
    }
    let lock = auto.join("session-dead-gone.lock");
    std::fs::write(&lock, "").unwrap();
    age(&lock, Duration::from_mins(10));
    launch_with_recovery(cx);
    let left = names(&auto);
    assert!(!lock.exists(), "{left:?}");
    assert!(
        !left.iter().any(|n| n.contains(".tmp-")),
        "끝난 세션의 세대별 임시 파일이 남았다: {left:?}"
    );
}
~~~
