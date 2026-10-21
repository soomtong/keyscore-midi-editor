# 적대적 리뷰: S2-T7a 새로 만들기·열기·저장·닫기·최근 파일 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트 세션(구현 컨텍스트를 물려받지 않음). 리뷰 1·2 세션과도 다르다
- 대상 커밋: `0083fae`(브랜치 `s2/t7a-save-open`). 리뷰 2 뒤 수정은 `904929e..0083fae`(`f5dd85d` 파일 정체 `FileId`, `614ea42` 열기 실패 시트의 창 고르기·흐름 표시, `cbbae94` 최근 목록 표시 합치기, `56d4ab0` 포맷)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s2_t7a_r3.rs`(깨뜨리지 못한 시도 18개, 모두 통과). 깨뜨린 시도 3개는 부록 A, 디스크 이미지를 붙여야 해서 저장소 테스트에 넣지 않은 통과 시도 1개는 부록 B
- 판정 기준(이번 회차 의뢰): 「수정」 이상은 (a) 결정 위반으로 원본이 바뀜, (b) 저장 안 된 변경을 묻지 않고 잃음, (c) 시트·흐름 표시가 영구히 막힘, (d) 이번 수정의 회귀. 드문 파일 시스템 조합과 표시 문구는 「제안」

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `FileId` 갱신: `set_file`(열기)과 `mark_saved`(저장, rename으로 inode가 바뀜)만 정체를 남긴다. 저장 뒤 바깥 이름 바꿈, 다른 이름 저장 뒤 옛 파일(옛 정체가 남아 옛 파일을 계속 이 창의 것으로 보는가), 다른 창이 옛 파일 위에 저장할 수 있는가.
2. 정체가 낡는 경우: 바깥 프로그램이 원자적으로 다시 쓰면 문서의 정체는 사라진 inode를 가리킨다. inode 번호를 재사용하는 볼륨(FAT 등)에서 새 다른 파일이 그 번호를 받으면 열기·다른 이름 저장이 엉뚱한 창으로 묶이는가. APFS는 재사용하는가.
3. 정체가 살아 있는 다른 이름으로 남는 경우: 「옛 파일을 백업 이름으로 옮기고 새로 쓰기」(vim `backupcopy=no`류). 백업을 열면 원래 경로 창으로 묶이는가.
4. 상위 버전 원본(결정 2026-10-12 A): 다른 폴더로 옮김, 옮긴 뒤 하드 링크·심볼릭 링크, 바깥 원자적 다시 쓰기 뒤 원래 경로, 닫기 [저장] 경유. 모두 원본 바이트가 그대로인가.
5. 열기 실패 시트의 창 고르기(`files.rs:549`): 연 창·명령 창·그 밖의 창 가운데 흐름이 없는 첫 창, 없으면 새 창. 명령 창이 닫기 시트 중일 때 새 창이 생기고 두 흐름이 모두 풀리는가. 연 창이 이미 바쁜 기존 창이고 손대지 않은 명령 창이 닫힐 때 실패 시트가 사라지지 않는가(닫힌 창을 고르면 `ask`가 `None`이라 오류가 삼켜진다).
6. 실패 시트 동안 흐름 표시: 그 창의 저장·닫기·끝내기가 시트를 겹치지 않고, 답하면 반드시 풀리는가(`set_busy(&host, false)`는 `ask`가 `None`이어도 불린다).
7. 끝내기와 실패 시트: 실패 시트가 깨끗한 창에 있으면 다른 바뀐 창에 묻고, 바뀐 창에 있으면 묻지 않고 멈추는가(묻지 않고 잃는 경로가 없는가).
8. 최근 목록 합치기(`recent_entries`): 없는 파일과 있는 파일을 잘못 합치는가. 합친 표시와 `remove_recent`(정규형만 비교)가 어긋나 지운 항목이 다른 이름으로 다시 나타나는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 `FileId` 갱신 | 리뷰 2 부록을 옮긴 두 테스트(옮긴 원본, 이름 바꾼 파일)뿐 | 저장 뒤·다른 이름 저장 뒤 정체가 바뀌는 경우 없음 |
| 2 inode 재사용 | 없음 | |
| 3 백업 이름 | 없음 | |
| 4 상위 버전 원본 변형 | 리뷰 2 부록(같은 폴더 안 옮김)과 리뷰 1·2의 링크·하드 링크 | 옮긴 뒤 링크, 다른 폴더, 바깥 다시 쓰기 없음 |
| 5 실패 시트 창 고르기 | 리뷰 2 부록 둘(바뀐 창에서 실패 시트 → 닫기·끝내기)뿐 | 새 창·다른 창·연 창이 바쁜 경우 없음 |
| 6 실패 시트 흐름 풀림 | 리뷰 2 부록이 「시트가 남지 않음」만 본다 | 흐름이 풀리는지는 보지 않는다 |
| 7 끝내기와 실패 시트 | 리뷰 2 부록(바뀐 창) | 깨끗한 창 경우 없음 |
| 8 최근 목록 합치기 | 리뷰 2 부록(시작 직후 두 모양) | 실패 뒤 빼기와의 어긋남 없음 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 저장(새 inode) 뒤 바깥에서 이름 바꿈 → 새 이름으로 열기 | 못 깨뜨림(같은 창. `mark_saved`가 정체를 갱신) | `review_r3_rename_after_save_activates_existing_window` |
| 2 | 다른 이름 저장 뒤 옛 파일 열기 | 못 깨뜨림(새 창) | `review_r3_old_file_after_save_as_opens_in_new_window` |
| 3 | 다른 이름 저장 뒤 다른 창이 옛 파일 위에 다른 이름 저장 | 못 깨뜨림(막히지 않음) | `review_r3_other_window_may_save_onto_old_file_after_save_as` |
| 4 | 연 뒤 만든 내용 복사본 열기 | 못 깨뜨림(새 창) | `review_r3_copy_made_after_open_opens_separately` |
| 5 | 바깥 원자적 다시 쓰기 뒤 같은 경로 열기 | 못 깨뜨림(같은 창, 경로로 묶임) | `review_r3_atomic_rewrite_same_path_still_activates` |
| 6 | APFS에서 지우고 새로 만들기, 원자적 다시 쓰기 뒤 새 파일의 inode | 못 깨뜨림(셸에서 확인. APFS는 번호를 재사용하지 않는다: 127008042~49 단조 증가) | 없음(셸 실행) |
| 7 | FAT32 볼륨(hdiutil 이미지)에서 연 파일을 바깥에서 원자적으로 다시 쓴 뒤 새 파일 `c.ksq` 열기 | 깨짐: `c.ksq`가 옛 `a.ksq` 창으로 묶여 열리지 않는다(macOS msdosfs는 시작 클러스터를 inode 번호로 쓰고 바로 재사용한다. 셸 확인: a=6 → 다시 쓰기 뒤 a=11, 새 c=6). exFAT은 바로 재사용하지 않았다 | 부록 `review_r3_fat_reused_inode_new_file_opens_in_own_window` |
| 8 | 「옛 파일을 `a.ksq.bak`으로 옮기고 새 `a.ksq` 쓰기」 뒤 백업 열기 | 깨짐(제안 수준): 백업이 `a.ksq` 창으로 묶여 따로 열 수 없다 | 부록 `review_r3_backup_rename_then_open_backup_opens_separately` |
| 9 | 상위 버전 원본을 다른 폴더·다른 이름으로 옮긴 뒤 닫기 [저장] → [다른 이름으로 저장] → 옮긴 원본 | 못 깨뜨림(거부, 창·바뀜 유지, 흐름 풀림) | `review_r3_close_save_onto_newer_original_moved_to_other_folder_refused` |
| 10 | 옮긴 상위 버전 원본의 하드 링크 고르기 | 못 깨뜨림 | `review_r3_save_as_onto_hard_link_of_moved_newer_original_refused` |
| 11 | 옮긴 상위 버전 원본을 가리키는 심볼릭 링크 고르기 | 못 깨뜨림(원본·링크 그대로) | `review_r3_save_as_via_symlink_to_moved_newer_original_refused` |
| 12 | 바깥에서 원본을 원자적으로 다시 쓴 뒤 원래 경로 고르기 | 못 깨뜨림(경로로 거부) | `review_r3_save_as_onto_newer_path_after_external_rewrite_refused` |
| 13 | 상위 버전 문서를 새 파일로 다른 이름 저장한 뒤 고치고 저장 | 못 깨뜨림(새 파일에만 씀, 원본 그대로) | `review_r3_newer_after_save_as_original_untouched_by_save` |
| 14 | FAT32에서 상위 버전 원본을 옮긴 뒤 옮긴 원본 고르기 | 못 깨뜨림(이름을 바꿔도 시작 클러스터가 같아 거부) | 부록 B `review_r3_fat_moved_newer_original_keeps_original` |
| 15 | 바뀐 창의 실패 시트 동안 다른 이름 저장·닫기 → 답 → 닫기 | 못 깨뜨림(대화상자·시트 겹침 없음, 흐름 풀림, 그 뒤 닫기는 묻는다) | `review_r3_open_error_on_origin_blocks_then_releases` |
| 16 | 명령 창이 닫기 시트 중일 때 열기 실패 | 못 깨뜨림(새 창에 실패 시트, 두 흐름 모두 풀림, 새 창은 묻지 않고 닫힘) | `review_r3_open_error_while_origin_has_close_sheet_uses_new_window` |
| 17 | 명령 창은 바쁘고 다른 바뀐 창은 한가할 때 열기 실패 → 그 창에 닫기 | 못 깨뜨림(새 창 없이 그 창에 시트, 닫기는 겹치지 않음, 둘 다 풀림) | `review_r3_open_error_prefers_other_free_window` |
| 18 | [성공, 실패]를 손대지 않은 명령 창에서 열기 | 못 깨뜨림(명령 창 닫힘, 연 창에 시트, 풀림) | `review_r3_partial_open_failure_hosts_on_opened_window` |
| 19 | 성공 파일이 이미 닫기 시트 중인 창에 열려 있고 명령 창은 손대지 않은 창 | 못 깨뜨림(닫힌 명령 창을 고르지 않고 새 창에 시트, 오류가 삼켜지지 않음) | `review_r3_open_error_when_opened_window_busy_still_shown` |
| 20 | 실패 시트가 깨끗한 창에 있을 때 끝내기 | 못 깨뜨림(다른 바뀐 창에 묻고 취소면 끝내지 않음) | `review_r3_quit_with_error_sheet_on_clean_window_asks_dirty_one` |
| 21 | 실패 시트가 바뀐 창에 있을 때 끝내기, 답한 뒤 다시 끝내기 | 못 깨뜨림(첫째는 묻지 않고 멈춤, 둘째는 묻는다) | `review_r3_quit_with_error_sheet_on_dirty_window_aborts_then_asks` |
| 22 | 최근 목록에 없는 파일과 있는 파일 | 못 깨뜨림(합치지 않음) | `review_r3_recent_missing_and_existing_not_merged` |
| 23 | 설정에 깨진 파일의 하드 링크 두 이름 → 하나를 열어 실패 | 깨짐(제안 수준): 실패한 이름은 빠지지만 같은 파일의 다른 이름이 메뉴에 다시 나타난다 | 부록 `review_r3_recent_broken_hard_link_pair_removed_together` |

회귀 확인: `review_s2_t7a`(22), `review_s2_t7a_r2`(23), `files`(30), `commands`(12) 모두 통과. 리뷰 테스트 파일은 `cargo fmt --check`, `cargo clippy -p keyscore-app --all-targets -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 읽기 전용 사유가 있으면 원본 바이트를 바꾸지 않는다(결정 2026-10-12 A) | 아니오 | 시도 9~14와 리뷰 2 부록. 정체 비교는 거부만 늘리고 쓰기 대상을 바꾸지 않는다(쓰기는 언제나 고른 경로·문서 경로). 낡은 정체(시도 7)도 거부 쪽으로만 틀린다 |
| 쓸 권한이 없으면 원본 바이트를 바꾸지 않는다 | 아니오 | 변경 없음 |
| 저장은 옛 내용 또는 완전한 새 내용 | 아니오 | 변경 없음(kill 게이트) |
| 한 파일은 한 창에서만 | 아니오(반대 방향으로 지나치다) | 이름 바꿈은 막힌다. 대신 FAT32의 번호 재사용(시도 7)과 백업 이름(시도 8)에서 다른 파일이 한 창으로 묶인다 |
| 한 창에 시트 하나, 흐름이 끝나면 풀림 | 아니오 | 시도 15~21. 실패 시트도 흐름을 세우고, `ask`가 `None`이어도 `set_busy(&host, false)`가 불린다. 시트 자리로 닫힌 창을 고르는 경로가 없다(시도 19) |
| 바뀐 창은 묻지 않고 닫히거나 끝나지 않는다(7.5) | 아니오 | 시도 20·21, 리뷰 2 시도 11~14 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| IO-1 + 결정 2026-10-12 A(원본은 바뀌지 않는다) | 충족 | `files.rs:372`(`is_document_file`로 읽기 전용 검사), `files.rs:717`, `document.rs:146-158`(열기·저장 때 정체). 시도 9~14 |
| 결정 2026-10-21 1 A(쓸 권한 없는 파일 거부) | 충족 | 변경 없음(`files.rs`의 `check_writable`) |
| 7.5 `Cmd+N/O/S/Shift+S` | 충족 | 변경 없음 |
| 7.5 `Cmd+W/Q` 저장 안 된 변경은 표준 시트로 묻기 | 충족 | `files.rs:145-169`, `lib.rs:70-86`. 실패 시트가 바뀐 창에 있을 때 끝내기는 묻지 않고 멈춘다(끝내지도 않는다. 알려진 한계) |
| 7.6 최근 파일, UI-11 | 충족 | `files.rs:191`(합친 표시). 시도 23은 제안 |
| 7.7 온보딩(`Cmd+S`를 누를 때 파일이 된다) | 해당 없음 | 이 Task 범위 밖(자동 저장은 S2-T7b 이후) |
| 8.4 이전 프로젝트 파일을 덮어쓰지 않음 | 충족 | `ksq::write_file`, `tests/save_kill.rs` |
| 8.4 실시간 규율 | 해당 없음 | 스케줄러 코드 변경 없음 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 리뷰 2 뒤 변경은 `files.rs`, `document.rs`(UI 스레드)뿐이다.
- 제품 코드의 `unwrap`/`expect`: 리뷰 2 뒤 더한 것 없음.
- 에러가 삼켜지는 곳
  - `FileId::of`는 `metadata` 실패를 `None`으로 본다. 정체 비교에서 `None`은 「같지 않음」이라 경로 비교로 돌아간다. 타당하다.
  - 실패 시트를 띄울 창도 새 창도 못 만들면 `eprintln!`만 한다(`files.rs:563`, 리뷰 2와 같음).
  - `recent_entries`는 메뉴·팔레트를 만들 때마다 항목 쌍마다 `canonicalize`·`metadata`를 부른다(10개면 최대 45쌍). UI 스레드라 응답이 느린 네트워크 볼륨의 항목이 있으면 메뉴 갱신이 멈출 수 있다. 재현 테스트를 만들 수 없어 지적으로 올리지 않는다(설정을 읽을 때 한 번 합치고 결과를 들고 있는 방안을 참고로 적는다).

## 7. 지적 사항

리뷰 2 지적 대응 확인: 1(차단) 고침 확인(리뷰 2 부록 재현 녹색, 시도 9~14로 변형도 막힘). 2(제안) 받아들여 고침 확인(시도 1). 3(수정) 고침 확인. 실패 시트가 흐름을 세우고 반드시 푼다. 다른 창·새 창·닫힌 명령 창 경우 모두 흐름이 풀린다(시도 15~19). 4(제안) 받아들여 고침 확인(시도 22). 다만 빼기 쪽은 아래 지적 3.

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | FAT32 볼륨에서 문서의 정체(`FileId`)가 낡으면 다른 파일이 그 번호를 물려받는다. 연 파일을 바깥 프로그램이 원자적으로 다시 쓰거나 지운 뒤 같은 볼륨에 새 파일이 생기면, 그 새 파일을 열 때 옛 창이 앞으로 오고 새 파일은 열리지 않는다(다른 창의 다른 이름 저장도 「이미 열고 있다」로 거부). `f5dd85d`가 들여온 회귀지만 쓰기 대상은 바뀌지 않아 데이터를 잃거나 원본이 바뀌지 않고, 드문 파일 시스템 조합이라 제안으로 둔다. APFS·exFAT은 바로 재사용하지 않았다. 정체에 만든 시각(`st_birthtime`)을 더하거나, 정체로만 맞을 때 크기·수정 시각도 비교하는 방안. 재현 `review_r3_fat_reused_inode_new_file_opens_in_own_window`(hdiutil로 FAT32 이미지를 붙인다) | `crates/keyscore-app/src/files.rs:717-720`, `files.rs:725-746` |
| 2 | 제안 | 바깥 프로그램이 「옛 파일을 `a.ksq.bak`으로 옮기고 새 `a.ksq`를 쓰는」 방식(vim `backupcopy=no`, 일부 동기화 도구)으로 고치면, 백업을 열 때 `a.ksq` 창이 앞으로 오고 백업은 따로 열 수 없다. 정체 추적의 설계상 결과이고 원본 보호에는 오히려 맞다(상위 버전 원본의 옛 바이트를 가진 백업도 덮지 못한다). 열기(`window_with_path`)에서만 「문서 경로가 더는 없을 때」로 정체 비교를 좁히는 방안. 거절해도 된다. 재현 `review_r3_backup_rename_then_open_backup_opens_separately` | `crates/keyscore-app/src/files.rs:695-704` |
| 3 | 제안 | 최근 목록 표시는 `same_file`(하드 링크 포함)로 합치지만 실패한 파일 빼기(`remove_recent`)는 정규형만 비교한다. 설정에 깨진 파일의 하드 링크 두 이름이 있으면 메뉴에는 한 줄인데, 열기 실패로 그 줄을 빼면 같은 파일의 다른 이름이 다시 나타난다. 표시만의 문제다. `remove_recent`도 `same_file`로 빼는 방안. 재현 `review_r3_recent_broken_hard_link_pair_removed_together` | `crates/keyscore-app/src/files.rs:607-615` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단·수정 지적이 없다. 리뷰 2의 차단 1건(지적 1)과 수정 1건(지적 3)은 고쳐졌고, 이번 수정의 회귀는 FAT32에서만 나타나며 데이터를 잃거나 원본을 바꾸지 않는다(지적 1, 제안). 제안 1~3은 받아들이거나 거절 이유를 보고서에 적는다. 거절하면 부록 A의 해당 재현은 저장소에 옮기지 않는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(부록 A 3개 실패, 부록 B 1개 통과, 리뷰 테스트 18개 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 2 뒤 변경은 `files.rs`, `document.rs`, 리뷰 2 테스트, 보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록 A. 실패하는 재현 테스트

`crates/keyscore-app/tests/review_s2_t7a_r3.rs` 끝에 부록 A·B를 함께 그대로 붙여 넣으면 같은 파일의 도우미(`start`, `dir_for`, `open`, `open_file`, `open_newer`, `run`, `answer`, `answer_new_path`, `doc_path`, `rewrite_atomically`, `write_sample`, `sample`)와 `use`로 컴파일된다. `cargo test -p keyscore-app --test review_s2_t7a_r3`로 리뷰 시점(`0083fae`)에 부록 A의 3개가 실패하고 부록 B가 통과하는 것을 확인했다. `FatVolume`은 `hdiutil`로 8 MB FAT32 이미지를 만들어 시험 폴더 아래에 붙이고, 떨어뜨릴 때 뗀다(권한 상승 불필요).

~~~rust
/// 설정에 하드 링크 두 이름이 있고 둘 다 깨졌으면, 하나를 열어 실패해 뺀 뒤 다른 이름이 메뉴에 다시 나타나는가.
#[gpui_kit::test]
fn review_r3_recent_broken_hard_link_pair_removed_together(cx: &mut TestAppContext) {
    let dir = dir_for("recent-link");
    let a = dir.join("a.ksq");
    std::fs::write(&a, "{ not json").unwrap();
    let b = dir.join("b.ksq");
    std::fs::hard_link(&a, &b).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": [
            a.canonicalize().unwrap().display().to_string(),
            b.canonicalize().unwrap().display().to_string()
        ] })
        .to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    assert_eq!(cx.read(files::recent_files).len(), 1);
    let (h, _) = open(cx, Project::new());
    run(cx, h, file::OpenRecent(a.canonicalize().unwrap().display().to_string()));
    answer(cx, "확인");
    assert!(
        cx.read(files::recent_files).is_empty(),
        "실패한 파일이 다른 이름으로 최근 목록에 다시 나타났다: {:?}",
        cx.read(files::recent_files)
    );
}

/// 바깥에서 「옛 파일을 백업 이름으로 옮기고 새 파일을 쓴」 뒤, 백업을 열면 어느 창인가.
#[gpui_kit::test]
fn review_r3_backup_rename_then_open_backup_opens_separately(cx: &mut TestAppContext) {
    let dir = start(cx, "backup");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    let backup = dir.join("a.ksq.bak");
    std::fs::rename(&path, &backup).unwrap();
    write_sample(&path);
    let (again, _) = cx.update(|cx| files::open_path(&backup, cx)).unwrap();
    assert_ne!(again, handle, "백업이 새 파일 창으로 묶였다");
}

/// FAT 볼륨(USB 메모리)을 붙인다. 떨어뜨리면 뗀다.
struct FatVolume {
    mount: PathBuf,
}

impl FatVolume {
    fn new(name: &str) -> FatVolume {
        let dir = dir_for(&format!("fat-{name}"));
        let image = dir.join("vol.dmg");
        let mount = dir.join("mnt");
        std::fs::create_dir_all(&mount).unwrap();
        let ok = std::process::Command::new("hdiutil")
            .args(["create", "-size", "8m", "-fs", "MS-DOS", "-volname", "KSR3"])
            .arg(&image)
            .status()
            .unwrap()
            .success();
        assert!(ok, "hdiutil create");
        let ok = std::process::Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-mountpoint"])
            .arg(&mount)
            .arg(&image)
            .status()
            .unwrap()
            .success();
        assert!(ok, "hdiutil attach");
        FatVolume { mount }
    }
}

impl Drop for FatVolume {
    fn drop(&mut self) {
        let _ = std::process::Command::new("hdiutil")
            .args(["detach", "-force"])
            .arg(&self.mount)
            .status();
    }
}

/// FAT: 연 파일을 바깥에서 원자적으로 다시 쓴 뒤 새로 만든 다른 파일이 옛 inode 번호를 물려받으면, 그 파일을 열 수 없다.
#[gpui_kit::test]
fn review_r3_fat_reused_inode_new_file_opens_in_own_window(cx: &mut TestAppContext) {
    start(cx, "fat-reuse");
    let vol = FatVolume::new("reuse");
    let path = vol.mount.join("a.ksq");
    write_sample(&path);
    let (handle, _) = open_file(cx, &path);
    rewrite_atomically(&path, ksq::to_string(&sample()).as_bytes());
    let other = vol.mount.join("c.ksq");
    write_sample(&other);
    let (again, view) = cx.update(|cx| files::open_path(&other, cx)).unwrap();
    assert_ne!(
        again,
        handle,
        "다른 파일이 옛 창으로 묶였다: {:?}",
        doc_path(cx, &view)
    );
}

~~~

## 부록 B. 통과했지만 저장소 테스트에 넣지 않은 시도

디스크 이미지를 붙이므로 `cargo test --workspace`에 넣지 않았다. 부록 A의 `FatVolume`이 필요하다.

~~~rust
/// FAT: 상위 버전 원본을 옮긴 뒤 다른 이름으로 저장에서 옮긴 원본을 골라도 막힌다(macOS msdosfs의 inode 번호는
/// 시작 클러스터라 이름을 바꿔도 그대로다).
#[gpui_kit::test]
fn review_r3_fat_moved_newer_original_keeps_original(cx: &mut TestAppContext) {
    start(cx, "fat-moved");
    let vol = FatVolume::new("moved");
    let path = vol.mount.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let moved = vol.mount.join("moved.ksq");
    std::fs::rename(&path, &moved).unwrap();
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(moved.clone()));
    if cx.has_pending_prompt() {
        answer(cx, "확인");
    }
    assert_eq!(std::fs::read(&moved).unwrap(), before, "옮긴 원본이 덮였다");
}
~~~
