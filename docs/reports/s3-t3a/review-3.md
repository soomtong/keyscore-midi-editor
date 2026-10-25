# 적대적 리뷰: S3-T3a 삭제·클립보드·반복 (리뷰 3회차)

- 리뷰어 세션: 구현자·1회차·2회차 리뷰어와 다른 세션(구현 컨텍스트를 물려받지 않은 새 에이전트)
- 대상 커밋: `0e88e1a057aa47d19feaccaf961eb464b531ff5e`(브랜치 `s3/t3a-delete-clipboard-repeat`). 2회차 뒤 수정은 `9ae85b3`(core `insert_notes` 연장 끝), `e007fce`(알림 문구), `0e88e1a`(보고서)
- 범위: 2회차 지적 1 수정의 정확성과 회귀, 이전 두 리뷰가 보지 않은 결함. 이전 시도 표에 있는 것은 반복하지 않았다.
- 리뷰 시험: `crates/keyscore-core/tests/review_s3_t3a_r3.rs`(8개, 속성 1개 500건 포함, 모두 통과), `crates/keyscore-app/tests/review_s3_t3a_r3.rs`(4개, 모두 통과)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-core --test review_s3_t3a_r3`, `-p keyscore-app --test review_s3_t3a_r3`. 회귀 확인으로 core `transform_insert`(6)·`review_s3_t3a`(3)·`review_s3_t3a_r2`(7), app `clipboard_notes`(7)·`note_edit`(10)·`review_s3_t3a`(20)·`review_s3_t3a_r2`(9)를 다시 돌려 모두 통과를 봤다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git show 9ae85b3 e007fce`, `transform.rs`의 `insert_notes`·`clip_for_notes`·`bar_aligned_length`, `edit.rs`의 `ExtendClip` 적용, `pianoroll/edit.rs`, `note_edit.rs`, `clipboard.rs`, `window.rs` `select_appeared`를 읽고 적었다.

1. 새 끝 `stop = min(노트 끝 이상 마디 끝 미만에서 시작하는 다음 클립의 시작, 마디 끝)`이 늘 기대값과 같은가(완전성과 정확성). 클립 A의 시작·길이가 마디에 맞지 않을 때, 다음 클립이 여럿일 때, 박자표가 바뀐 뒤 마디.
2. 노트 끝이 정확히 다음 클립 시작(맞닿음)이면 받아들이는가. 다음 클립이 노트 끝 앞에서 시작하면(닿음) 먼 다음 클립이 비어도 거부하는가.
3. 숨김 꼬리 노트가 있는 클립: 꼬리가 다음 클립 구간까지 뻗어 있을 때 늘린 끝, 꼬리 노트 자체의 복제.
4. 박자표가 마디 중간에서 바뀌어 짧은 마디가 생길 때 `bar_aligned_length`가 짧은 마디 끝을 쓰는가.
5. 앱 경로: `r` 연타가 마디 중간 다음 클립까지 채우고 그 뒤에서 거부될 때의 undo 묶음(늘린 길이까지 한 번에 원복), redo, 선택.
6. vim `p`(커서 + 한 스텝)와 `cmd-v`가 같은 연장 규칙을 타는가.
7. 수정 커밋이 기존 시험을 약하게 바꾸지 않았는가(`insert_notes_preserves_invariants`, `plan_duplicate_offset_is_exact_span`, 리뷰 2 시험의 기대값).
8. 실패한 반복(거부)이 묶음 상태를 망가뜨리는가(`History::apply`의 `sealed`).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 새 끝의 완전성·정확성 | 부분 | 구현자 속성 시험은 A가 틱 0, 다음 클립 하나, 박자표 없음. 늘린 길이는 「그대로·마디 배수·다음 클립 시작」 셋 중 하나인지만 보고 정확한 값은 보지 않는다 |
| 2 맞닿음·가까운 쪽 닿음 | 부분 | 고정 예 `plan_duplicate_offset_is_exact_span`의 5760·5000 두 자리 |
| 3 숨김 꼬리 | 없음 | |
| 4 마디 중간 박자표 변화 | 없음 | |
| 5 `r` 연타 + 마디 중간 다음 클립 | 없음 | 1회차 `coalesced_repeat_with_extension_undoes_length`는 마디 끝 연장 |
| 6 vim `p`·박자표 뒤 `cmd-v` | 없음 | 2회차는 A 안 3000의 `cmd-v`, `cmd-d`만 |
| 7 기존 시험 약화 | 해당 없음 | 차분을 읽어 확인. 속성 시험의 거부 조건은 오히려 엄격해졌다(`bar_end > next` → `end > next`) |
| 8 거부 뒤 묶음 | 없음 | 시도 1에서 함께 봄 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | core 속성 500건: A 시작·길이(16분음표 단위, 마디에 맞지 않음 포함), 다음 클립 0–3개(맞닿음·마디 중간 포함), 마디선 위 3/4 변화, 노트 1–6개. 기대 끝을 따로 계산(min(첫 다음 클립 시작, 노트 끝 이상 첫 마디선))해 비교, 닿으면 `Err` | 못 깨뜨림 | core `insert_notes_extends_exactly_to_min_of_bar_line_and_next_clip` |
| 2 | core: 다음 클립 둘(5000, 6000, 넣은 순서 뒤바꿈) | 못 깨뜨림: 5000에서 멈춤, 5000에 닿는 노트는 6000이 비어도 거부 | core `insert_notes_stops_at_nearest_of_several_next_clips` |
| 3 | core: 1마디부터 3/4, 다음 클립이 3/4 마디 안(5840) / 3/4 마디 끝(6720) | 못 깨뜨림: 5840, 6720 | core `insert_notes_after_meter_change_stops_at_mid_bar_next_clip` |
| 4 | core: 박자표가 마디 중간(5000)에서 바뀌어 짧은 마디 [3840, 5000). 다음 클립 없음 / 5000에 맞닿음 | 못 깨뜨림: 둘 다 5000 | core `insert_notes_short_bar_before_mid_bar_meter_change` |
| 5 | core: 1박에서 시작한 클립 A [960, 3840), 다음 클립 4500. 노트 끝 3040 / 노트 끝 정확히 4500 | 못 깨뜨림: 둘 다 끝 4500 | core `insert_notes_unaligned_clip_stops_at_next_and_touching_end_is_ok` |
| 6 | core: 분할로 숨김 꼬리(3000, 끝 4920)가 생긴 A. B가 6000 / 꼬리 구간 안 4500에서 시작할 때 3500 노트, 꼬리 노트의 폭 복제(4920..6840) | 못 깨뜨림: 6000, 4500(꼬리는 숨김 그대로, undo로 3840), 꼬리 사본은 거부 | core `insert_notes_with_hidden_tail_extends_to_mid_bar_next_clip`, `insert_notes_with_hidden_tail_over_next_clip_stops_at_next_start`, `insert_notes_tail_copy_reaching_next_clip_is_rejected` |
| 7 | 앱: A [0, 1마디) E4(2880, 8분음표), B 5000. `r` 넷 → `cmd-z` → `cmd-shift-z` | 못 깨뜨림: 사본 셋이 A를 5000까지 늘려 들어가고 넷째는 「반복: …」로 거부, 겹침 없음, undo 한 단위로 노트·길이 원복, redo로 5000. redo 뒤 선택은 사본 셋 전부(제안 1) | `repeat_chain_fills_up_to_mid_bar_next_clip_and_undoes_once` |
| 8 | 앱: 1마디부터 3/4, B 5840. C4를 3000에 `cmd-v` | 못 깨뜨림: A 5840, 선택은 붙인 노트 | `paste_after_meter_change_stops_at_mid_bar_next_clip` |
| 9 | 앱(vim): 2박 C4, 커서 1920에서 `p`(자리 2880, 끝 4800), B 5000 | 못 깨뜨림: A 5000 | `vim_p_near_clip_end_extends_up_to_mid_bar_next_clip` |
| 10 | 앱: 숨김 꼬리 A + 꼬리 구간 안 B(4500). C4를 3500에 `cmd-v` → `escape` → 꼬리 노트에 `cmd-d` → `cmd-z` | 못 깨뜨림: A 4500, 꼬리 사본은 「복제: …」로 거부, undo로 3840 | `paste_into_clip_with_hidden_tail_before_next_clip` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | 새 `stop`은 노트 끝 이상인 다음 클립 시작 이하이고, 겹침 검사는 그대로 남았다(`transform.rs:231-235`). `ExtendClip` 적용도 다시 검사한다. 시도 1·7 |
| 노트는 클립 안 | 못 깨뜨림 | `stop ≥ 노트 끝`(필터 `s >= note_end`). 숨김 꼬리는 원래대로 꼬리다(시도 6) |
| 전부 아니면 전무 | 못 깨뜨림 | 거부 시 노트·길이·`undo_len` 그대로(시도 7·10) |
| 선택은 그 클립의 실제 값 | 못 깨뜨림 | 시도 7·8 |
| 채널 1–16, 틱 ≥ 0 | 해당 없음 | |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.5 클립보드 규칙 2 「노트는 클립 경계를 넘으면 클립을 늘리고」 | 충족 | `crates/keyscore-core/src/transform.rs:215-235`. 2회차 실패 3개(`insert_notes_fits_before_mid_bar_next_clip`, `paste_near_clip_end_fits_before_mid_bar_next_clip`, `duplicate_near_clip_end_fits_before_mid_bar_next_clip`) 통과, 시도 1–10 |
| 5.10 입력이 클립 끝을 넘으면 마디 단위로 늘어남 | 충족(계획의 읽기) | 다음 클립이 없거나 마디 끝 뒤면 마디 끝. 마디 중간 다음 클립 앞에서 멈추는 것은 계획 4에 적혔고 일괄 연산과의 차이도 적혔다(`plan.md` 4) |
| 7.5 `Cmd+D`·5.8 `r` 「바로 뒤에」「누를 때마다 하나 더」 | 충족 | 시도 7, 1회차 시험 통과 |
| 5.8.1 vim `p` | 충족 | 시도 9 |
| 5.7 되돌리기 묶음 | 충족 | 시도 7(거부가 끼어도 한 단위, 연장 포함 원복) |
| 7.5 클립보드 규칙 1·3, KM-15, 5.8 삭제·잘라내기 | 충족 | 2회차 뒤 바뀐 것은 알림 문구뿐(`note_edit.rs:204-214`, `:340`). 기존 시험 통과 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 2회차 뒤 차분은 순수 함수와 메인 스레드 처리기의 문구다.
- 제품 코드의 `unwrap`/`expect`: 2회차 뒤 차분에 없음. `unwrap_or(bar_stop)`은 다음 클립이 없을 때의 기본값이다. `stop - start`는 `stop ≥ note_end > start`라 넘치지 않는다.
- 에러가 삼켜지는 곳: 새로 생긴 곳 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| - | - | 차단·수정 없음. 2회차 지적 1의 수정은 정확하고 회귀를 만들지 않았다 | - |

### 제안

| # | 내용 | 근거 시험(결과) | 파일:줄 |
| --- | --- | --- | --- |
| 1 | `r` 연타를 undo한 뒤 redo하면 선택은 「새로 나타난 노트」인 사본 전부(셋)가 되어, 이어 누른 `r`은 마지막 사본 하나가 아니라 셋 묶음을 복제한다. 계획 6대로라 지적은 아니다. 연타 묶음의 redo는 마지막 사본만 고르는 편이 「누를 때마다 하나 더」의 이어짐에 맞다(S3-T3b `.` 반복을 만들 때 함께 정할 만하다) | `repeat_chain_fills_up_to_mid_bar_next_clip_and_undoes_once`(통과, 동작 기록) | `crates/keyscore-app/src/window.rs:458` |
| 2 | 계획 「공개 인터페이스 초안」의 `insert_notes`·`clip_for_notes` 주석이 처음 규칙(「마디 단위로 늘린다, 다음 클립과 겹치면 거부」「시작은 at이 든 마디 시작」)에 머물러 있다. 끝의 정의 4와 코드 주석은 새 규칙이다. 초안 주석도 맞추거나 「끝의 정의 4 참조」로 | 문서 읽기 | `docs/reports/s3-t3a/plan.md` 공개 인터페이스 초안 |
| 3 | 리뷰 2 시험 `cut_status_line_says_copied`는 기대값이 「잘라냈다」로 바뀌어 이름과 내용이 어긋난다. 이름을 `cut_status_line_says_cut`으로 바꿀 만하다(지우지 않고 이름만) | `review_s3_t3a_r2::cut_status_line_says_copied`(통과) | `crates/keyscore-app/tests/review_s3_t3a_r2.rs:368` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

2회차 지적 1의 수정(`insert_notes` 연장 끝을 노트 끝 이상에서 시작하는 다음 클립 시작에서 멈춤)은 속성 500건의 정확한 기대값 비교, 꼬리 노트, 마디선·마디 중간 박자표 변화, 다음 클립 여럿, 마디에 맞지 않는 클립, 붙여넣기·vim `p`·복제·반복 연타와 undo·redo 경로에서 깨지지 않았다. 2회차 실패 시험 3개는 통과하고 이전 리뷰·구현자 시험도 모두 통과한다. 이전 리뷰가 보지 않은 자리에서 결함을 찾지 못했다. 제안 3개는 구현자가 거절할 수 있다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(core 8·app 4, 모두 통과)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(2회차 뒤 차분은 `transform.rs` `insert_notes`, `note_edit.rs` 문구, 해당 시험·계획·보고서뿐)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`insert_notes_preserves_invariants`·`plan_duplicate_offset_is_exact_span`·`cut_status_line_says_copied`의 기대값 변경은 지적·제안 대응이고 보고서에 적혀 있다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(2회차 뒤 새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 부록: 리뷰 산출물

- 새 `.proptest-regressions` 파일: 없음(속성 시험은 통과).
- 리뷰 시험 두 파일은 `rustfmt`·`clippy -D warnings`(pedantic 포함)를 통과한다.
