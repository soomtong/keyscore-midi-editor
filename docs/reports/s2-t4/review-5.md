# S2-T4 적대적 리뷰 5 (좁은 확인)

근거: `docs/decisions/2026-10-17-s2-t4-review-limit.md` (A). 범위는 커밋 `5adfd17`이 review-4의 지적 1·제안 1을 닫았는지와 같은 함수(`action_item`, `effective_keys`, `runs_somewhere`, `menu_context_stacks`, `native_key_equivalent`, `is_bare`)의 회귀뿐이다.

## 판정

병합 가능

## 체크리스트

- [x] 지적 1(사용자 키맵이 Cmd 키를 다른 명령에 걸면 가려진 키 등가물이 그려짐): `action_item`이 GPUI가 구울 키(`native_key_equivalent`)를 `runs_somewhere`로 확인하고, 어느 모드 경로에서도 그 명령이 나오지 않으면 `RunCommand`로 감싼다. 감싸면 `native_is_shown`이 거짓이라 실제로 누를 키가 제목 꼬리로 붙는다. `review_s2_t4_r4`의 `cmd-j` 핫리로드 지도가 이를 확인한다.
- [x] 제안 1(가사 분배 ⌘V): 확인 경로가 `ModeState::all()`의 모든 `context_path`(정렬·중복 제거)로 넓어져 Lyric 전용 키도 남는다. 「어느 모드에서든 그 키가 그 명령을 실행하면 사실」이라는 기준이 KM-13의 모드별 키 의미와 어긋나지 않는다.
- [x] `effective_keys`: 후보 수집(keymap 빌림)과 `runs_somewhere`(다시 빌림)가 블록으로 분리되어 이중 borrow가 없다. 우선순위 역순 순회는 이전과 같다.
- [x] `is_bare` 감싸기 조건은 그대로 유지되고 `||`로 추가만 되었다. 맨 키·Alt 감싸기 테스트(`alt_binding_is_wrapped_and_shown`, `ctrl_and_function_keys_are_not_wrapped`)가 통과한다.
- [x] 시퀀스 키: `native_key_equivalent`가 단일 키만 돌려주므로 시퀀스는 `runs_somewhere` 감싸기 경로를 타지 않고 제목 꼬리로만 나온다(이전과 같음).
- [x] GPUI `create_menu_item`의 `find_or_first` 규칙과 `native_key_equivalent`의 선택이 일치한다.
- [x] `cargo test -p keyscore-app --test review_s2_t4_r4 --test review_s2_t4_r3 --test commands`: commands 17개, r4 2개, r3 포함 모두 통과.

## 지적

없음.

## 제안(수정 불필요)

- 한 키가 모드 A에서는 이 명령, 모드 B에서는 다른 명령을 실행하면 `runs_somewhere`는 참이라 메뉴에 키가 남는다. 결정한 기준(어느 모드에서든 사실이면 표시)과 일치하므로 한계로 둔다.
