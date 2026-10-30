# 적대적 리뷰: S3-T5 건반 뷰 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: 251273f (`s3/t5-keyboard-view`, `main...` 비교)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 `keyboard_view.rs`, `keyboard_view_window.rs`, `window.rs` 하단 영역, `step_input.rs`, `step.rs`, `pianoroll/keys.rs`, `assets/keymaps/default.json`만 읽고 적었다.

1. 건반형에서 `Step > Pending`의 숫자 `SetOctave`는 두 레이아웃 모두에 걸린다(REQ 5.6 「건반 레이아웃에서는 뒤 숫자가 그 음의 옥타브를 바꾸고 이후에도 유지된다」). 그러면 건반형 뒤 8건반(`E{o+1}`–`B{o+1}`)도 `D` `5`, `F` `5` … 로 닿는다. 계획 규칙 2는 「키 하나로 닿지 않으면 옥타브 숫자를 붙인 입력」인데 건반형만 그 규칙을 따르지 않고 라벨을 비운다. 실제로 `D` `5`가 E5를 쓰는지, 건반 뷰가 그 건반을 비우는지.
2. 계이름형 둘째 옥타브 라벨 `c#5`가 실제로 C#5를 쓰는가(`#`이 `Step > Solfege > Pending`에만 있음, `shift-3` 점음표와 겹침).
3. 옥타브 0·9 경계: 건반형 oct9의 `K`(C10)는 없음으로, 계이름형 oct8 둘째 옥타브 `g9`까지 있고 `g#9`부터 없음. oct0 `[` 버튼에서 범위 메시지, 옥타브 그대로.
4. 옥타브 버튼이 키 `]`와 같은 처리기(`step_octave_by`)를 거치는가: 조합 확정, 화음 닫기, MML `o` 대기 확정. 버튼을 누른 뒤 포커스가 문서에 있어 이후 Step 키가 먹는가. 텍스트 입력란(인스펙터·트랜스포트)에 포커스가 있을 때 버튼을 누르면 Action이 루트 처리기에 닿는가.
5. 보이는 조건: Step만(Normal·Play·Sequence에서는 없음), 하단 영역 접힘이면 없음, 배치 뷰 Step에서도 있음. Step 밖에서 `keyboard-octave-up`이 남지 않음.
6. 갱신: `]` 키, 계이름형 `o5` MML, 조합 뒤 숫자(스티키 옥타브), 트랙 전환(드럼 트랙 ↔ 멜로디), 채널을 10으로 바꾼 뒤 undo, 조성 편집과 undo, 레이아웃 설정 바꾸기.
7. 드럼 라벨: 이름 칸에 GM 라벨, 라벨 없는 음은 비움. 키 라벨은 실제 입력과 같음. REQ 5.13 「`A` = `bd`」와 다른 점이 단계 파일 이월(S6-T4)로 정당한가.
8. 조성 명암: 장조·자연 단조, 조성 없음은 명암 없음. 「끌 수 있음」 미구현을 범위 밖으로 둔 것이 단계 파일 범위를 줄이는가.
9. 그리기 경로가 문서·Step 상태를 바꾸지 않는가(`refresh_step_layout`은 MML을 확정하는데 그 뒤 `keyboard_view`를 계산하는 순서).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 건반형 뒤 8건반이 `D` `5` 등으로 닿는데 라벨이 빔 | 없음. 오히려 `piano_layout_has_sixteen_key_labels`와 스냅샷이 빈 라벨을 고정 | `type_every_label`은 라벨이 있는 건반만 쳐 본다. 라벨이 없는 건반이 실제로 닿는지는 보지 않는다 |
| 2. 계이름형 `c#5`가 C#5를 씀 | 있음(`typed_labels_write_shown_pitches_solfege`) | |
| 3. 옥타브 0·9 경계 | 일부(`labels_at_octave_nine_…`, `octave_button_at_bound_says_range`). 없음 건반을 실제로 쳤을 때 안 써지는지는 없음 | 리뷰 시험으로 보탬 |
| 4. 버튼 = 키 처리기, 포커스 | 일부(조합 확정, 범위 메시지, 이어진 `a`). 화음 닫기·MML 대기·텍스트 입력란 포커스 없음 | 리뷰 시험으로 보탬 |
| 5. 보이는 조건 | 있음(Step 진입·`Esc`, 배치 뷰, 접힘). `Ctrl+Tab`으로 나갈 때 버튼이 남는지는 없음 | |
| 6. 갱신(트랙 전환, 채널 변경 undo, 조성 undo, MML `o`, 스티키 숫자) | 없음(조성 편집·레이아웃 설정·`]`·`z`만 있음) | |
| 7. 드럼 라벨 | 있음(라벨 있는 음, 라벨 없는 음은 비움을 고정). 라벨 없는 음의 표기를 REQ와 대조한 시험 없음 | |
| 8. 조성 명암 | 있음(`out_of_key_follows_project_key`, `key_edit_updates_shading`) | 끄기 설정은 미배정으로 보고됨 |
| 9. 그리기 경로의 상태 변경 | 시험 없음. 코드로 확인: `render` 첫 줄 `refresh_step_layout` 뒤에 `keyboard_view`를 계산하고, 건반 뷰 자체는 순수 계산 | |

## 3. 깨뜨리기 시도

시험 파일 `crates/keyscore-app/tests/review_s3_t5.rs`, `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t5`로 실행했다(13개 중 11 통과, 2 실패).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 건반형 oct4에서 `d` `5` → E5(76)가 써짐을 확인하고, 건반 뷰 16–23번(E5–B5) 라벨이 있는지 | 깨짐: 76이 써지는데 뒤 8건반 라벨이 모두 `None` | `piano_upper_keys_reachable_by_digit_but_unlabeled` |
| 2 | 드럼 트랙 oct4(기본): 라벨 없는 음의 표기 | 깨짐: 24건반 중 23개가 이름 없이 키 글자만(`A`). REQ 5.13 「표에 없는 노트 번호는 `n<번호>`」, 텍스트 내보내기 `export.rs:705`와 다름 | `drum_unlabeled_notes_show_number` |
| 3 | 트랜스포트 템포 입력란에 포커스를 둔 채 `+` 버튼 → 옥타브, 이어 `a` | 못 깨뜨림(5, C5 씀) | `octave_button_with_tempo_field_focused` |
| 4 | 트랙 전환(`track::Next`/`Prev`)으로 드럼 ↔ 멜로디 | 못 깨뜨림 | `track_switch_to_drum_updates_labels` |
| 5 | `track::SetChannel(10)` 뒤 `edit::Undo` | 못 깨뜨림 | `channel_change_and_undo_update_drum_labels` |
| 6 | 조성 D장조 뒤 `edit::Undo`로 명암 되돌림 | 못 깨뜨림 | `key_edit_undo_restores_shading` |
| 7 | `ctrl-tab`으로 Step을 나간 뒤 버튼·건반 뷰가 남는지, `bottom-panel` 자리표시 복귀 | 못 깨뜨림 | `leaving_step_by_cycle_removes_buttons` |
| 8 | 계이름형 MML `o 6` 뒤 라벨(`C6 c`, `C7 c7`) | 못 깨뜨림 | `mml_octave_updates_labels` |
| 9 | 건반형 `a 5`(스티키 옥타브 숫자) 뒤 `oct5` 표시 | 못 깨뜨림 | `piano_digit_suffix_moves_view_octave` |
| 10 | MML `o` 대기 중 버튼 vs 키 `]`(옥타브·메시지 비교) | 못 깨뜨림(같음) | `octave_button_with_mml_pending_matches_key` |
| 11 | 열린 화음(`ctrl-a ctrl-d`) 뒤 버튼 vs 키 `]`(화음 닫힘·커서·노트 비교) | 못 깨뜨림(같음) | `octave_button_closes_chord_like_key` |
| 12 | oct9에서 없음 건반 `K`를 치면 안 써지고 `G`는 127 | 못 깨뜨림 | `absent_keys_cannot_be_written` |
| 13 | 드럼 oct2 `S` 라벨 `sd`가 실제로 38을 씀 | 못 깨뜨림 | `drum_label_matches_written_note` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 건반 뷰는 문서·Step 상태를 바꾸지 않는다 | 아니오 | `keyboard_view::keys`는 순수 함수, `ProjectWindow::keyboard_view`는 `&self`. 바꾸는 길은 버튼의 `run_action`뿐이고 처리기는 키와 같다(시도 10·11) |
| 라벨과 실제 입력이 어긋나지 않는다(라벨이 있으면 그 입력이 그 음을 쓴다) | 아니오(라벨이 있는 건반 한정) | 시도 12·13, 구현자 시험 16. 다만 「닿는 건반인데 라벨이 없음」은 지적 1 |
| 없음(127 초과) 건반에는 라벨이 없다 | 아니오 | `keyboard_view.rs:54-61`, 시도 12 |
| 모델 불변식(겹침·클립 안·채널) | 해당 없음 | 이 Task는 `core`를 건드리지 않는다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.1 옥타브 `[` `]` (건반 뷰 반영) | 충족 | `keyboard_view_window.rs:74-121`, 시험 7·시도 9 |
| 5.4 「2옥타브 건반을 그려 어느 키가 어느 음인지 보여 준다」(Step에 옮겨 씀) | 부분 충족(미충족으로 셈) | 건반형 기본 레이아웃에서 24건반 중 8개에 입력이 적히지 않는다. `keyboard_view.rs:63`, 시도 1 |
| 5.4 「건반 뷰의 옥타브 버튼」 | 충족 | `keyboard_view_window.rs:79-90`, 시도 3·10·11 |
| 5.4 「연주」 버튼, Play 모드 건반 뷰 | 범위 밖(S5-T4 이월, 단계 파일 S3-T5 「Play 모드는 S5」와 일치) | `plan/stages/15-S5-sequence-and-play.md:35` |
| 5.6 뒤 숫자 = 옥타브(두 레이아웃) | 부분 충족 | 계이름형은 `c5`처럼 적지만 건반형은 같은 문법(`D5`)을 적지 않는다. `keyboard_view.rs:63-66` |
| 5.13 드럼 트랙 「음이름 대신 GM 퍼커션 라벨」 | 부분 충족 | 라벨 있는 16음은 충족, 라벨 없는 음은 이름이 비어 음이름도 `n<번호>`도 없다. `keyboard_view.rs:68-70`, 시도 2. `A` = `bd` 키 배치와 킷 페이지는 S6-T4 범위(원래 단계 파일 S6-T4 범위에 「건반 뷰 라벨」이 있음)라 정당한 이월 |
| 2 「조성 표시」 밝게/어둡게 | 충족 | `keyboard_view.rs:77`, `keyboard_view_window.rs:24-33` |
| 2 「조성 표시」 「끌 수 있음」 | 미충족(단계 파일 S3-T5 범위는 「조성 강조」뿐이고 피아노롤도 없음. 사람에게 배정 질의로 올림) | `plan/stages/13-S3-step-and-pianoroll.md:76` |
| 7.1 하단 영역 「건반 뷰 (Play·Step 모드)」 | Step 충족, Play는 S5 | `window.rs:1282-1291`, 시험 6·11·12, 시도 7 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 UI 스레드의 그리기와 Action 처리기만 건드린다.
- 제품 코드의 `unwrap`/`expect`: 없음. `keyboard_view.rs:52` `unwrap_or(0)`(i < 24라 실패하지 않음), `:96`·`:98`은 시험용 `describe`의 표시 기본값.
- 에러가 삼켜지는 곳: `describe`의 `let _ = writeln!`(String 쓰기, 실패 없음). 버튼의 `run_action` 결과는 처리기의 상태줄 메시지로 드러난다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 건반형 뒤 8건반(`E{o+1}`–`B{o+1}`)은 `D` `5`, `F` `5` … 로 실제로 닿는데(REQ 5.6 「건반 레이아웃에서는 뒤 숫자가 그 음의 옥타브를 바꾸고 이후에도 유지된다」, keymap `Step > Pending`의 `SetOctave`는 두 레이아웃 공통) 라벨이 비었다. 계획 규칙 2 자신이 「옥타브 숫자 없이 닿는 키가 없으면 옥타브 숫자를 붙인 입력을 보인다」고 하고 계이름형은 `c5`로 그 규칙을 따르는데, 건반형만 「키 하나로 적히지 않는다」는 이유로 예외다. 계이름형 `c5`도 키 하나가 아니므로 이유가 서지 않는다. 기본 레이아웃에서 2옥타브 건반의 1/3이 「어느 키가 어느 음인지」를 보여 주지 않는다. 스냅샷과 `piano_layout_has_sixteen_key_labels`가 이 상태를 고정한다. 라벨을 `D5` 꼴로 채우거나, REQ에 없는 선택이면 결정 요청으로 올린다. 깨뜨린 시험 `piano_upper_keys_reachable_by_digit_but_unlabeled` | `crates/keyscore-app/src/keyboard_view.rs:63`, `docs/reports/s3-t5/plan.md:16` |
| 2 | 수정 | 드럼 트랙에서 라벨 표에 없는 음은 이름 칸이 비어 음높이를 알 길이 없다(접근성 라벨도 `A`뿐). REQ 5.13은 드럼 트랙에서 음이름 대신 라벨을 쓰고 표에 없는 노트는 `n<번호>`로 보이며, 같은 저장소의 텍스트 내보내기(`keyscore-text/src/export.rs:705`)도 `n<번호>`를 쓴다. 기본 옥타브 4의 드럼 트랙에서 24건반 중 23개가 이름 없이 그려진다. 깨뜨린 시험 `drum_unlabeled_notes_show_number` | `crates/keyscore-app/src/keyboard_view.rs:68-70`, `docs/reports/s3-t5/plan.md:18` |
| 3 | 제안 | 옥타브 버튼의 접근성 라벨이 `+`, `−` 글자뿐이다. 화면 낭독에서 무엇을 하는 버튼인지 드러나도록 「옥타브 올리기」「옥타브 내리기」 같은 라벨을 준다(시각 글자는 그대로 둘 수 있음) | `crates/keyscore-app/src/keyboard_view_window.rs:87` |
| 4 | 제안 | 회귀 방지 시험이 빠졌다: 트랙 전환·채널 변경 undo·조성 undo 뒤 갱신, 화음·MML 대기 중 버튼, 텍스트 입력란 포커스에서 버튼. 리뷰 시험(시도 3–13, 모두 통과)을 PR에 옮겨 담기를 권한다 | `crates/keyscore-app/tests/review_s3_t5.rs` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2는 건반 뷰의 핵심 기능(라벨)이 실제 입력·REQ 표기와 어긋나는 곳이다. 둘 다 계획 「끝의 정의」의 글을 고치는 일이 따르므로, REQ에 없는 선택으로 보면 `docs/decisions/`에 결정 요청을 쓰고 그 답에 맞춘다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`selection.rs` `pitch_name`을 `pub(crate)`, `window.rs` `layout` 필드를 `pub(crate)`로 연 것은 계획·보고에 적힘)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
