# 적대적 리뷰: S3-T4c 미리듣기 가벼운 경로와 배치 뷰 Step 표시 (리뷰 1회차)

- 리뷰어 세션: 구현 세션과 다른 새 서브 에이전트(구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `944a0fc` (`main...HEAD`, 브랜치 `s3/t4c-preview-arrangement-step`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 기억한 경로표(`Engine::preview`)가 지금 스케줄러 스냅샷과 어긋나는 길: 스케줄러 `swap`을 부르는 모든 곳(`engine.rs` 540·651·942·988·1024, `transport.rs` 270·369, `preview.rs` 168)에 `forget_preview`가 앞서는가.
2. 포트 구성 열쇠(`preview_ports`)에 빠진 것: 메트로놈 포트·클럭 포트 설정 변경, 다른 창(다른 프로젝트)이 같은 엔진 기억을 쓰는 경우, 서버 복구.
3. 음색 기록 열쇠(목적지, 채널): 기본 가상 포트를 「가상」과 이름으로 섞어 가리킴, 뱅크만 다름, 다른 창의 재생(chase)이 음색을 바꾼 뒤.
4. 배치 뷰 표시(`arrangement_step`)가 `step_host`·`step_write`와 같은 결론을 내는가: 경계(앞 클립 끝 = 다음 클립 시작), 이어 쓰기·클립 안에서 쓰기가 거부되는 자리, 조합 중(셋잇단·절반이 이미 조합에 들어간 뒤), 박자 바뀜, 트랙 없음.
5. 시험이 타이밍 우연으로 통과하지 않는가(수집 조건, 늦게 오는 메시지).

(정직하게 적는다: 이 목록은 코드를 읽으며 세웠으나, 문서에 옮기기 전에 구현자의 시험 파일도 읽었다. 아래 체크리스트 1번은 그 이유로 비워 둔다.)

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 스냅샷 교체 뒤 무효화 | 일부 | 재생·정지(3), 트랙 포트 바꿈(2), 포트 추가(8), 이름 바꾸기(9)만. 서버 복구·포트 지우기·포트 감시(`port_lost`·`housekeeping`)·`reregister_extras`는 없음. 리뷰 시험이 복구를 더함 |
| 2. 메트로놈·클럭·다른 창 | 없음 | 리뷰 시험이 메트로놈 포트 바꿈과 두 창(포트 차례가 다름)을 더함 |
| 3. 음색 열쇠 | 일부 | 같은 채널 두 트랙(6)은 있음. 「가상」과 기본 포트 이름 섞기, 뱅크만 다름, 다른 창 재생 뒤는 없음. 리뷰 시험이 더함 |
| 4. 표시 = 실제 쓰기 | 일부 | 틈에서 막힘(15)은 있음. 이어 쓰기·클립 안 막힘, 조합 중, 박자 바뀜, 경계, 트랙 없음은 없음 |
| 5. 타이밍 | 있음 | `collect`가 개수 조건까지 받고, 프로그램은 노트보다 먼저 오므로 늦은 중복도 잡힌다. 준비 횟수는 결정적 |

## 3. 깨뜨리기 시도

모두 `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t4c`로 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 A 끝(마디 2 시작)에 커서, 다음 클립 B가 BAR+480에서 시작. 4분음표 쓰기는 `splice`가 `ClipOverlap`으로 거부한다. 표시는 「클립 「A」 끝에 이어 쓴다」 | 깨짐(표시는 쓴다고 하는데 노트가 쓰이지 않음) | `arrangement_extend_display_agrees_with_write_when_next_clip_blocks` |
| 2 | 클립 A 안(틱 960), A 끝 근처 노트(3360)를 밀면 A가 마디 2로 늘어 바로 붙은 B와 겹친다. 표시는 「클립 「A」 안」 | 깨짐(쓰기 거부, 표시는 이유 없음) | `arrangement_inside_display_agrees_with_write_when_push_blocks` |
| 3 | 점4분, 마디 3 넷째 박, `n` 뒤 `c`(조합 중). 셋잇단 점4분(960)은 마디 3에 든다 | 깨짐(`n` 뒤 표시는 「3–3」로 맞고, `c`를 친 조합 중 표시는 「3–4」, 확정 결과는 3–3) | `arrangement_display_while_composing_triplet_matches_write` |
| 4 | 2분음표 길이, 마디 3 넷째 박에서 `step::HalfNote("c")`(절반 = 960) 조합 중 | 깨짐(표시 「3–4」, 실제 3–3) | `arrangement_display_while_composing_half_matches_write` |
| 5 | 박자 바뀜(마디 2부터 3/4), 마디 3 셋째 박에 2분음표 | 못 깨뜨림(표시와 실제 모두 3–4) | `arrangement_new_clip_bars_follow_meter_change` |
| 6 | 커서 = A 끝 = B 시작 | 못 깨뜨림(표시 「B 안」, 노트도 B에) | `arrangement_boundary_names_next_clip_like_write` |
| 7 | 트랙 없는 프로젝트에서 배치 뷰 Step | 못 깨뜨림(표시 없음, 자리표시 그대로) | `arrangement_step_without_tracks_shows_nothing` |
| 8 | 이름 포트 미리듣기 → `recover_virtual_port` → 미리듣기 | 못 깨뜨림(다시 준비, 프로그램 다시 보냄, 노트 도착) | `preview_after_recover_prepares_again` |
| 9 | 「가상」 트랙(프로그램 1)과 기본 포트를 이름으로 가리킨 트랙(프로그램 2), 같은 채널 번갈아 | 못 깨뜨림(프로그램 1·2·1) | `preview_alternating_virtual_and_main_by_name_resend_program` |
| 10 | 같은 채널·같은 프로그램, 뱅크만 다른 두 트랙 번갈아 | 못 깨뜨림(CC 0·32, 프로그램이 세 번씩) | `preview_alternating_bank_only_resend_bank` |
| 11 | 두 창: A는 [X], B는 [Y, X]. A·B·A·B 번갈아 미리듣기 | 못 깨뜨림(준비 네 번, X에는 채널 1만, Y에는 채널 2만) | `preview_two_windows_with_different_port_layouts_route_correctly` |
| 12 | 미리듣기 뒤 메트로놈 포트 설정을 다른 이름 포트로 바꿈 | 못 깨뜨림(다시 준비, 노트는 트랙 포트로만) | `preview_after_metronome_port_change_prepares_again` |
| 13 | A가 프로그램 1을 보낸 뒤, B(같은 채널 프로그램 2)가 재생하는 동안 A 미리듣기, B 정지 뒤 A 미리듣기 | 못 깨뜨림(정지 뒤 프로그램 1이 노트보다 먼저) | `preview_after_other_window_playback_resends_program` |

결과: 새 시험 13개 가운데 9개 통과, 4개 실패(1–4). 구현자 시험 `--test preview_light` 9개, `--test arrangement_step` 9개는 모두 통과했다.

코드 대조로 확인한 것(시험 없이 읽기만, 위 표에는 넣지 않음): 스케줄러 `swap`을 부르는 곳은 모두 `forget_preview` 뒤이거나(`remove_virtual_port` 525→540, `send_port_test_note` 647→651, `reregister_main` 938→942·988, `reregister_extras` 1021→1024), `prepare` 뒤다(`transport.rs` 270·369, `preview.rs` 168). `prepare`가 맨 앞에서 잊으므로 재생 시작·재생 중 편집·정지 중 준비는 모두 잊는다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 기억한 포트 번호는 그 스냅샷이 스케줄러에 있을 때만 쓴다 | 못 깨뜨림(시도 8·11·12, 코드 대조) | 스케줄러 교체 일곱 곳 모두 잊음 앞섬 |
| 미리듣기가 다른 창의 재생 스냅샷을 바꾸지 않는다 | 못 깨뜨림(시도 13) | `has_owner` 갈래가 기억 경로보다 먼저 |
| 같은 포트·채널의 음색은 마지막으로 보낸 것과 같을 때만 건너뛴다 | 못 깨뜨림(시도 9·10·13) | `setup_key`가 기본 포트 이름을 「」로 모음 |
| 표시는 문서·커서를 바꾸지 않는다 | 못 깨뜨림 | `arrangement_step`은 `&self`·`&App`만 받는다 |
| 표시는 실제 쓰기와 같은 결론을 말한다(계획 끝의 정의 3) | 깨뜨림 | 시도 1–4 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| PRE-1(트랙 포트·채널로 미리듣기) | 충족 | `preview.rs` 126–139(기억 경로), 시도 8·11·12 |
| PRE-3(테스트 노트) | 충족 | `preview.rs` 247–249(`TestNote`는 건너뛰지 않음), 구현자 시험 7 |
| OUT-4(트랙별 뱅크·프로그램) | 충족 | `preview.rs` 239–257, 시도 9·10·13 |
| 5.10(클립 밖 입력은 마디 단위 새 클립) | 충족(쓰기 자체) | 쓰기 경로는 바뀌지 않음. 표시만 어긋남(7절 지적 1·2) |
| 5.6(셋잇단·절반) | 충족(`n n` 표시 `nc_`) | `step_input.rs` 645–649, 구현자 시험 18. 배치 뷰 표시는 조합 중 셋잇단·절반을 빼먹음(지적 2) |
| 5.11 | 이 Task가 바꾼 것 없음 | — |

REQ에는 배치 뷰 Step 표시의 모양이 없다(결정 `2026-10-17-mode-entry-view.md` 이월). 지적 1·2는 REQ 미충족이 아니라 계획 끝의 정의 3과 다른 버그로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 기억과 기록은 메인 스레드의 `Engine` 안에만 있고 `keyscore-midi`는 바뀌지 않았다.
- 제품 코드의 `unwrap`/`expect`: 이 diff에서 새로 생긴 것 없음.
- 에러가 삼켜지는 곳: `send_note`에서 뱅크·프로그램 중간 송신이 실패하면 기록을 갱신하지 않고 `?`로 올린다(다음에 다시 보냄, 맞음). `arrangement_step`은 `clip_for_notes` 오류를 글로 보인다. 다만 클립 안·끝 갈래는 `splice`를 시험해 보지 않아 거부를 숨긴다(지적 1).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 클립 안·클립 끝 갈래는 쓰기를 시험하지 않고 「클립 … 안」「… 끝에 이어 쓴다」라고 말한다. 다음 클립이 스텝 안에서 시작하거나(이어 쓰기) 밀어 넣기로 클립이 늘어 다음 클립과 겹치면(클립 안) `step_write`의 `splice`가 `ClipOverlap`으로 거부하는데 표시는 쓴다고 한다. 틈 갈래만 「쓸 수 없다(이유)」를 보여 계획 끝의 정의 3「쓸 수 없으면 그 이유」와 어긋난다. 클립 갈래도 `step_write`와 같은 `splice`(덮어쓰기면 같은 `remove`·`by`)로 계획해 보고 오류면 이유를 보여야 한다. 시도 1·2 | `crates/keyscore-app/src/step_input.rs:150-152` |
| 2 | 수정 | 조합 중(음 키를 친 뒤 확정 전)에는 `triplet_next`가 이미 `Composing::triplet`으로 옮겨졌고 `half`도 조합에 있는데, 표시는 `step_ticks(length, triplet_next, false)`로 계산해 그 음이 만들 클립 마디를 틀리게 말한다(`n` 뒤에는 맞다가 `c`를 치면 틀려진다). 조합이 있으면 `c.triplet`·`c.half`를 써야 쓰기(`step_commit` 273줄)와 같다. 시도 3·4 | `crates/keyscore-app/src/step_input.rs:154` |
| 3 | 제안 | `forget_preview`를 `has_owner` 문서 주석 사이에 넣어, `forget_preview`의 문서가 「재생 중인 창이 있는가.」로 시작하고 `has_owner`는 문서를 잃었다 | `crates/keyscore-app/src/engine.rs:1079-1082`, `1135` |
| 4 | 제안 | 무효화 경로 가운데 시험이 없는 것: `port_lost`·`port_returned`, 포트 감시 `housekeeping`(목적지 변화), `remove_virtual_port`, `reregister_extras`. 리뷰 시험의 복구·메트로놈·두 창 시험을 `preview_light.rs`로 옮기고, 지우기(`remove_virtual_port` 뒤 미리듣기가 「포트 없음」을 알림)도 더하기를 권한다 | `crates/keyscore-app/tests/preview_light.rs` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

미리듣기 가벼운 경로(무효화, 음색 건너뛰기)는 13개 시도 가운데 미리듣기 쪽 6개를 모두 못 깨뜨렸다. 배치 뷰 표시가 실제 쓰기와 다른 말을 하는 버그 둘(지적 1·2)을 고친 뒤 다시 본다. 고칠 때 `crates/keyscore-app/tests/review_s3_t4c.rs`의 실패 넷이 통과해야 한다.

## 체크리스트 (판정 전에 전부 확인)

- [ ] 테스트를 읽기 전에 1번을 썼다(1절 끝의 설명 참고: 목록은 코드에서 세웠으나 적기 전에 시험 파일도 읽었다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
