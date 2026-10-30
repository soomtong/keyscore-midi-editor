# 적대적 리뷰: S3-T4c 미리듣기 가벼운 경로와 배치 뷰 Step 표시 (리뷰 3회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트(구현 컨텍스트를 물려받지 않음). 리뷰 1·2 세션과도 다르다
- 대상 커밋: `983b555` (범위는 리뷰 2 뒤의 제품 수정 `6bd96d9` 하나. `crates/keyscore-app/src/step_input.rs`의 `next_step_write`·`arrangement_step`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 모습 (가): 같은 트랙 화음이 열리고 조합이 없을 때 자리 = 화음 자리, 밀기 0인가(`step_commit`의 `chord` 갈래와 같은가).
2. 모습 (나): 다른 트랙 화음이 열려 있을 때 조합 전·중 모두 자리 = 화음 끝(`step_close_chord`의 `chord.at + chord.len`), 밀기 = 길이인가. 쓸 트랙이 조합의 트랙(조합 없으면 `current_track`)인가.
3. 모습 (다): MML `l` 대기의 길이를 `step_settle_mml`과 같은 규칙(`mml_length`, 틀리면 이전 길이 유지)으로 쓰는가.
4. 셋을 겹친 경우: 셋잇단 접두 `n`과 `l` 대기, 다른 트랙 화음과 화음 조합(`ctrl-<음>`).
5. 쓰기 경로 회귀: `step_commit`·`step_key`·`step_write`·`plan_step_write`가 이 커밋에서 바뀌지 않았는가. `next_step_write`가 표시 밖에서 불리지 않는가.
6. `arrangement_step`이 `write_track`으로 `step_host`·클립을 찾는데, 커서 줄(`Step: <트랙> <위치>`)은 여전히 `current_track`인가(바뀌면 표시 의미가 달라짐).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 모습 (가) | 있음 | `review_s3_t4c_r2::open_chord_without_composing_display_agrees_with_next_chord_note`. 리뷰 시험이 클립 밖 새 클립 화음을 더함 |
| 2. 모습 (나) | 있음 | `review_s3_t4c_r2::other_track_chord_display_matches_where_the_note_lands`. 리뷰 시험이 화음 조합을 더함 |
| 3. 모습 (다) | 있음 | `review_s3_t4c_r2::mml_length_pending_display_matches_next_note` |
| 4. 겹친 경우 | 없음 | 리뷰 시험이 더함 |
| 5. 쓰기 회귀 | 있음(간접) | `step_input`·`step_vim_mml`. 코드 대조로 확인 |
| 6. 커서 줄 | 있음 | `arrangement_step` 시험의 커서 문구 |

## 3. 깨뜨리기 시도

모두 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t4c_r3`로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 온음표, 마디 3 끝 1300틱 앞, `step::Triplet` 뒤 `l 2`(대기) 중 표시 → `c enter`. 셋잇단 2분음표(1280)만 마디 3 안 | 못 깨뜨림(표시·실제 모두 마디 3–3, 노트 1280틱) | `triplet_with_mml_length_pending_display_matches_next_note` |
| 2 | Lead에서 마디 3 셋째 박 4분음표 화음을 열고 `track::Next`, 2분음표로 바꾼 뒤 Bass에서 `ctrl-e`(화음 조합) 전·중 표시 → `enter`. 화음을 닫으면 마디 3–4, 닫지 않으면 3–3 | 못 깨뜨림(조합 전·중 표시 모두 마디 3–4, 실제 3–4) | `other_track_chord_then_chord_composing_display_matches` |
| 3 | 빈 트랙 마디 3 넷째 박에서 화음을 열어 새 클립이 생긴 뒤(조합 없음) 표시 → `e enter tab` | 못 깨뜨림(표시 「클립 … 안」, 두 음이 같은 자리, 클립 하나) | `open_chord_in_new_clip_display_points_at_that_clip` |

결과: 새 시험 3개 모두 통과. 기존 시험 `arrangement_step`(9), `review_s3_t4c`(13), `review_s3_t4c_r2`(7), `step_input`(28), `step_vim_mml`(22) 모두 통과했다.

코드 대조로 확인한 것(위 표 밖):

- `6bd96d9`의 diff는 `arrangement_step`과 `next_step_write` 두 함수만 바꾼다. `step_commit`·`step_key`·`step_close_chord`·`step_settle_mml`·`step_write`·`plan_step_write`는 그대로이고, `next_step_write`는 `arrangement_step`에서만 불린다(`&self`). 쓰기 경로는 바뀌지 않았다.
- `next_step_write`의 차례가 `step_key` → `step_commit`과 같다. 길이: `step_settle_mml`은 `mml_length`가 `None`이면 길이를 두는데 표시도 `unwrap_or(self.step.length)`. `MmlKind::Octave` 대기는 길이에 영향이 없다. 화음: 같은 트랙 → `(ch.at, 0)`, 다른 트랙 → 닫아 커서가 `ch.at + ch.len`으로 간 뒤 `(커서, len)`, 없음 → `(커서, len)`. 트랙: 조합이 있으면 `c.track`, 없으면 `current_track`(다음 `step_key`가 쓰는 트랙).
- MML 대기와 화음·조합은 함께 있지 않다(`step_mml_open`이 `step_settle`을 먼저 부름). `step::Triplet`도 화음을 닫는다. 그래서 (다)와 (가)·(나)가 겹치는 상태는 없고, 겹칠 수 있는 것은 `n`과 `l` 대기(시도 1)뿐이다.
- 조합이 없을 때 다음 키를 보통 음 키(`half` = false, 음높이 = 커서 음높이)로 보는 가정은 리뷰 1 대응 뒤의 정의 그대로다. 이번 수정이 바꾼 것이 아니다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 쓰기는 이 수정 전과 같다 | 못 깨뜨림 | diff 범위, 기존 Step 시험 전부 |
| 표시는 문서·커서를 바꾸지 않는다 | 못 깨뜨림 | `arrangement_step`·`next_step_write`는 `&self`, `plan_step_write`는 `&Project` |
| 표시는 다음 확정과 같은 결론을 말한다 | 못 깨뜨림 | 리뷰 2 시험 1–3, 이번 시도 1–3 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.10(클립 밖 입력은 마디 단위 새 클립) | 충족 | `step_input.rs:149-216`(표시), `380-415`(`plan_step_write`), 시도 1·2 |
| 5.6(셋잇단·절반·화음) | 충족 | `step_input.rs:421-445`(`next_step_write`), 시도 1–3 |
| 5.1 | 이 수정이 바꾼 것 없음 | — |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(메인 스레드 표시 계산만 바뀜).
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음. `next_step_write`는 트랙이 없으면 `None`(`?`)이고 `arrangement_step`도 이미 같은 조건에서 `None`을 돌려준다.
- 에러가 삼켜지는 곳: 없음. `mml_length`의 `None`은 확정과 같게 이전 길이로 본다(알림은 확정 때 나온다).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| — | — | 없음 | — |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

`6bd96d9`는 리뷰 2 지적 1의 원인(다음 음 키가 거치는 앞 단계를 따라 하지 않음)을 한 함수에서 고쳤다. 세 모습(같은 트랙 열린 화음·조합 없음, 다른 트랙 열린 화음, MML `l` 대기)이 리뷰 2 시험으로 모두 고쳐졌고, 겹친 경우(시도 1·2)와 새 클립 화음(시도 3)에서도 표시와 확정이 같다. 수정은 표시 함수 둘에 그치고 쓰기 경로는 바뀌지 않았다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(수정 diff와 `step_key`·`step_commit` 코드만 보고 세웠다. 리뷰 2 시험 파일은 도우미 함수를 빌리려고 그 뒤에 열었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
