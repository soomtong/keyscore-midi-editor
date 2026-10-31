# 결정: 건반 뷰 키 라벨은 키 하나로 닿는 건반에만

- 날짜: 2026-10-31
- 상태: 결정(사람 승인, YoungTaek, 2026-10-31). 에이전트 제안을 사람이 받아들였다
- 관련 REQ: 5.4(「화면 하단에 2옥타브 건반을 그려 어느 키가 어느 음인지 보여 준다」), 5.6(뒤 숫자 옥타브), P3
- 관련 Task: S3-T5(병합 5c99eed), S3-T5b, S5-T4(Play 건반 뷰)

## 상황

S3-T5는 리뷰 1 지적 1에 따라 키 하나로 닿지 않는 건반에도 「키 + 옥타브 숫자」 라벨을 달았다(건반형 `D5`, 계이름형 `c5`). 실제 창을 찍어 보니(2026-10-31) 다음 문제가 드러났다.

- `D5`(E5 건반, 「D 키 + 5」)는 음 이름 D5와 꼴이 같다. 바로 왼쪽이 실제 D5 건반(키 `L`)이라 더 헷갈린다. 리뷰 2 제안 1이 접근성 라벨에서 같은 점을 지적했다.
- Play 모드(S5)는 같은 건반 뷰를 쓰지만 「음 뒤 숫자」 문법이 없다. 거기서 `D5` 라벨은 사실이 아니다.

## 결정

1. 키 라벨은 옥타브 숫자 없이 키 하나로 그 음을 쓰는 건반에만 단다. 건반형은 `C{o}`–`D#{o+1}` 16건반(`A` … `P`), 계이름형은 첫 옥타브 12건반(`c` … `b`, `c#` …). 나머지 건반은 키 라벨 없이 음 이름만 둔다. 그 음은 옥타브 버튼·`[` `]`·뒤 숫자로 옥타브를 바꾸면 닿는다(옥타브 버튼과 `oct<n>` 표시가 이미 보인다).
2. 키 라벨은 키캡 모양(테두리 있는 작은 상자)으로 그리고, 음 이름은 상자 밖의 작은 글자로 둔다. 모양으로 「누르는 키」와 「음 이름」을 가른다.
3. 건반 크기에 상한을 둔다. 흰건반 너비 최대 40 px(넘으면 왼쪽 정렬, 남는 자리는 비움), 건반 높이 최대 120 px. 넓은 창에서 건반이 지나치게 커지지 않게 한다.

S3-T5 계획 「끝의 정의」 규칙 2와 리뷰 1 지적 1의 대응(fb4d2e7)은 이 결정으로 바뀐다. 「라벨 = 실제 입력」 불변식(라벨이 있는 건반은 그 라벨을 치면 그 음이 써진다)은 그대로다.

## 바꾸는 시험

라벨이 있다고 기대하던 시험의 기대값을 이 결정에 맞춘다(지우지 않는다). 괄호 안은 바꾸기 전 이름이다: `keyboard_view` 스냅샷과 `piano_layout_labels_follow_p3_keys_only`(`piano_layout_labels_follow_p3_keys_then_octave_digit`), `keyboard_view_ui` `layout_setting_change_updates_labels`·`type_every_label`, `review_s3_t5` `piano_upper_keys_reachable_by_digit_have_no_label`(`piano_upper_keys_reachable_by_digit_but_unlabeled`)·`mml_octave_updates_labels`, `review_s3_t5_r2` `piano_octave_eight_upper_keys_unlabeled_and_stop_at_127`(`piano_octave_eight_upper_labels_stop_at_127`)·`typed_digit_input_moves_view_and_labels_reached_key`(`typed_upper_label_moves_view_and_relabels_same_key`)·`zero_octave_wait_upper_label_commits_before_digit`, 모든 옥타브 시험 `type_every_label_every_octave`(S3-T5b 리뷰 1 지적 1로 단언을 강화).
