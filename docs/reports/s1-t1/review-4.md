# 적대적 리뷰: S1-T1 CoreMIDI 가상 출력 포트 (리뷰 4회차)

- 리뷰어 세션: 구현 맥락을 물려받지 않은 별도 서브 에이전트 세션(구현자·리뷰 1–3 리뷰어와 다른 세션)
- 대상 커밋: `4ff9561` (브랜치 `s1/t1-virtual-port`, 리뷰 3 뒤 변경은 `git diff dcc07ac HEAD`)
- 범위: 리뷰 3 수정(이름 규칙, `run_isolated` 강화)만. 1–3회차에서 확인된 성질은 다시 훑지 않았다.
- 남긴 테스트: `crates/keyscore-midi/tests/review_s1_t1_r4.rs` (실패 2, 통과 5)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`src/client.rs`·`error.rs`의 diff와 리뷰 3 대응 표만 보고 적었다. 리뷰 1·2 파일의 `run_isolated` diff는 과제가 지정한 같은 `git diff` 출력에 함께 있었다.

1. 리뷰어 테스트 `review_s1_t1_r3.rs`가 그대로이고, 실패하던 2건이 HEAD에서 통과하며 `client.rs`·`error.rs`를 `dcc07ac`로 되돌린 사본에서 다시 실패하는가. 리뷰 1·2 파일의 변경이 `run_isolated` 강화뿐인가.
2. 거부 목록이 Unicode `DerivedCoreProperties.txt`의 `Default_Ignorable_Code_Point`와 한 칸도 어긋나지 않는가. 허용 예외 범위(태그 U+E0020–E007F, 이체자 선택자 U+FE00–FE0F·U+E0100–E01EF)의 바로 바깥(U+E0000, U+E0001, U+E0080, U+E01F0)이 거부되는가.
3. 허용 예외로 눈에 보이지 않거나 구분되지 않는 이름을 만들 수 있는가. 예외 문자만 다른 두 이름, 예외 문자만으로 된 이름, 예외 문자와 점자 빈칸·공백의 조합.
4. NFC가 한글 밖의 정준 동치(옴 기호·옹스트롬 기호·결합 문자 순서)도 합치는가. 정규화 뒤 앞뒤 공백 제거가 NFC를 깨거나, 저장 이름·`name()`·열거 이름이 어긋나는가.
5. NFC로 합쳐지지 않는 혼동: 공백 종류(NBSP 등)·개수, 호환 분해(전각, 합자, 한글 호환 자모). 각각 현실적 위험인지 과잉 대응인지.
6. 보이는 글자 판정의 빈틈: `Default_Ignorable_Code_Point`가 아닌데 빈칸으로 그려지는 문자.
7. `rename`이 같은 규칙을 쓰는가. 자기 이름의 NFD로 바꾸기, 다른 포트 이름의 NFD로 바꾸기, 실패한 rename 뒤 이름.
8. `run_isolated`가 이름 오타와 자식 실패를 모두 잡는가. 「1 passed」 문자열 검사가 「10 passed」·「1 ignored」에 속지 않는가.
9. 반복 실행에서 간헐 실패(리뷰 3 제안 6)가 다시 나오면 출력을 남긴다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 리뷰 3 테스트 재확인 | 있음(`review_s1_t1_r3.rs`) | 3절 시도 1, 2 |
| 2 DI 목록 일치 | 없음(단위 테스트는 표본 8개) | 전체 코드 포인트 경계 테스트 추가 |
| 3 허용 예외 악용 | 없음(정상 텍스트가 받아지는지만) | 깨짐, 실패 테스트 추가 |
| 4 한글 밖 정준 동치, 저장·열거 일치 | 부분(한글 NFD만) | 경계 테스트 추가 |
| 5 공백 변형, 호환 분해 | 없음 | 공백 변형 깨짐, 호환 분해는 과잉으로 판단 |
| 6 빈칸처럼 그려지는 비DI 문자 | 부분(U+2800만) | 실험만(제안 3) |
| 7 rename 조합 | 없음 | 경계 테스트 추가 |
| 8 `run_isolated` | 없음 | 수동 확인(시도 3) |
| 9 반복 실행 | 해당 없음 | 24회, 1회 일괄 실패, 출력 확보(제안 4) |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `git diff --quiet dcc07ac HEAD -- tests/review_s1_t1_r3.rs`, `git diff dcc07ac HEAD -- tests/` 판독 | 못 깨뜨림. r3 파일은 바뀌지 않았다. `review_s1_t1.rs`·`review_s1_t1_r2.rs`·`virtual_port.rs`의 변경은 `run_isolated` 본문(`status()`를 `output()`으로, 「1 passed」 확인 추가) 한 덩어리씩뿐이고 테스트 입력·기대는 그대로다. 새 `#[ignore]` 없음(있는 둘은 자식 전용으로 이전부터 있던 것) | 없음 |
| 2 | r3 테스트를 HEAD에서 실행하고, `client.rs`·`error.rs`만 `dcc07ac`로 되돌린 `/tmp` 사본에서 실행 | 못 깨뜨림. HEAD 7 통과. 사본에서 정확히 2건 실패(`..._nfd_variant_of_existing_name_fails_with_duplicate`, `..._only_invisible_format_chars_fails_with_invalid_name`), 나머지 5 통과 | 없음 |
| 3 | `/tmp` 사본에서 (가) `review_s1_t1_r2.rs`의 `run_isolated` 인자를 `..._create_portz`로 바꾸고 실행, (나) 자식 쪽 본문 첫 줄에 환경 변수로 켜는 `assert!` 실패를 넣고 실행 | 못 깨뜨림. (가) 자식은 「running 0 tests … 0 passed; 9 filtered out」로 성공하지만 부모가 「테스트를 찾지 못했다」로 실패한다. (나) 자식의 「주입 실패」 패닉이 부모 실패로 전파된다. 문자열 「test result: ok. 1 passed」는 「10 passed」의 부분 문자열이 아니고, 자식 테스트가 `#[ignore]`면 「0 passed; … 1 ignored」라 걸린다 | 없음 |
| 4 | DI 전체 대조: 구현과 따로 옮겨 적은 `Default_Ignorable_Code_Point` 17개 범위(Unicode 16.0, 인터넷 없이 알려진 목록)에서 허용 예외를 뺀 모든 코드 포인트를 `"a{c}b"`로 생성 | 못 깨뜨림. 모두 `InvalidName`. 구현 목록(`client.rs:71-92`)은 이 목록과 범위 단위로 같다(U+180F, U+2065, U+FFF0–FFF8, U+1BCA0–1BCA3, U+1D173–1D17A 포함). 허용 예외 바로 바깥 U+E0000, U+E0001, U+E0080도 거부 | `create_virtual_output_every_rejected_default_ignorable_fails_with_invalid_name`, `create_virtual_output_name_without_visible_char_fails_with_invalid_name` |
| 5 | 허용 예외만 다른 이름: `"PadSynth"` 뒤 `"Pad\u{200D}Synth"`, `"Pad\u{200C}Synth"`, `"Pad Synth"` 뒤 `"\u{200C}Pad Synth"`, `"Pad Synth\u{FE0F}"`, `"Pad Synth\u{E0100}"`, `"Pad Synth\u{E0041}\u{E007F}"` | 깨짐. 6개 모두 따로 만들어지고 열거에 그대로 보인다. 라틴 글자 사이·뒤의 ZWJ·ZWNJ, 비이모지 뒤의 이체자 선택자, 깃발이 아닌 글자 뒤의 태그 문자는 아무것도 그리지 않으므로 눈으로 같은 소스가 둘 생긴다. rename으로도 같다(`"드럼"` 포트가 있을 때 다른 포트를 `"드럼\u{200D}"`로 바꾸면 `Ok`) | `create_virtual_output_name_differing_only_by_allowed_ignorable_fails_with_duplicate`(실패) |
| 6 | 공백 변형: `"Lead Synth"` 뒤 `"Lead\u{00A0}Synth"`(NBSP), `"Lead  Synth"`(두 칸). 실험으로 U+2009, U+202F, U+3000도 | 깨짐. 모두 따로 만들어진다. NBSP는 macOS에서 Option+Space로 잘못 눌러 흔히 들어오고 보통 공백과 같은 너비로 그려진다. 앞뒤 공백은 떼면서(리뷰 1 지적 4) 안쪽 공백은 그대로 비교하는 것이 같은 근거와 어긋난다. U+2009·U+202F·U+3000은 너비가 달라 구분될 수 있어 테스트에는 넣지 않았다 | `create_virtual_output_whitespace_variant_of_existing_name_fails_with_duplicate`(실패) |
| 7 | 허용 예외·빈칸만으로 된 이름: `"\u{200D} \u{FE0F}"`, `"\u{200C}\u{200D}"`, 태그만, `"\u{E0100}"`, `"\u{2800}\u{200D}"`, `"\u{FE0F}\u{2800}"`, `"\u{3164}\u{3164}"` | 못 깨뜨림. 모두 `InvalidName`(`client.rs:53`의 보이는 글자 판정이 허용 예외를 빼고 센다) | 7번 시도는 시도 4의 둘째 테스트에 포함 |
| 8 | 한글 밖 정준 동치: U+2126 뒤 U+03A9, U+212B 뒤 `"A\u{030A}"`, `"e\u{0301}\u{0327}"` 뒤 `"e\u{0327}\u{0301}"`. 각 `name()`과 열거 이름 | 못 깨뜨림. 둘째는 `DuplicateName`, `name()`과 열거 이름은 NFC(U+03A9, U+00C5, U+0229 U+0301)로 같고 원래 입력 형태로는 열거되지 않는다. NFC 뒤 앞뒤 공백 제거는 공백이 조합의 시작 문자가 아니라 NFC를 깨지 않는다(`" \u{0301}"`는 `"\u{0301}"`로 저장, 열거 일치) | `create_virtual_output_canonical_equivalents_fail_with_duplicate_and_enumerate_nfc` |
| 9 | rename 조합: NFC `"드럼"` 포트를 NFD `"드럼"`으로, 다른 포트를 NFD `"드럼"`으로, 그 포트를 `"\u{202E}"`로 | 못 깨뜨림. 첫째는 아무 일 없이 `Ok`(이름 NFC 유지, 열거 1개), 둘째 `DuplicateName`, 셋째 `InvalidName`, 실패 뒤 이름 그대로 | `rename_to_canonical_variant_is_noop_for_self_and_duplicate_for_other` |
| 10 | 정상 텍스트: 잉글랜드 깃발 태그 연쇄, ZWJ 이모지 가족, 페르시아어 ZWNJ | 못 깨뜨림. 만들어지고 `name()` 그대로 열거된다. 비문자 U+FFFE·U+FFFF, 미할당 U+0378, 사용자 영역 U+F8FF도 실험에서 그대로 왕복했다 | `create_virtual_output_real_text_with_joiners_and_tags_is_enumerated_verbatim` |
| 11 | 호환 분해(NFKC로만 합쳐지는 것): `"X"`와 전각 `"\u{FF38}"`, `"fi"`와 합자 `"\u{FB01}"`, 호환 자모 `"\u{3137}"`와 첫소리 자모 `"\u{1103}"` | 실험에서 모두 따로 만들어진다. 깨짐으로 세지 않는다. 전각 문자는 너비가 두 배라 목록에서 구분되고, 합자·단독 첫소리 자모는 사용자가 입력기로 우연히 넣는 경로가 드물다. NFKC는 `"²"`→`"2"`, `"Ⅳ"`→`"IV"`, `"㎏"`→`"kg"`처럼 사용자가 쓴 이름을 바꿔 저장해 OUT-2의 「이름은 사용자가 정한다」와 부딪친다. 키릴·라틴 동형 문자(confusables)도 같은 이유로 범위 밖이다. 과잉 대응으로 판단 | 없음(실험만) |
| 12 | 보이는 글자 판정의 빈틈: DI가 아닌데 빈칸으로 그려지는 문자 하나만으로 된 이름(U+1D159 음악 기호 빈 음표머리, U+16FE4 거란 소자 채움, U+FFF9–FFFB 행간 주석, U+13430 상형문자 서식 제어) | 실험에서 모두 `Ok`. 사용자가 일부러 넣지 않는 한 들어오지 않는 문자라 제안으로 둔다(제안 3) | 없음(실험만) |
| 13 | `cargo test -p keyscore-midi --no-fail-fast` 24회(r4 파일 포함) | 23회는 r4의 깨뜨린 2건만 실패. 2회차에서 `review_s1_t1.rs`의 포트를 만드는 테스트 6건이 모두 `Os { call: "MIDIClientCreate", status: -304 }`로 실패했다(그 바이너리 18.96초). 다른 바이너리는 정상. 출력은 4절 아래 「반복 실행 기록」에 남겼다(제안 4) | 없음 |
| 14 | 실패 테스트 2건이 맞출 수 있는 요구인지: `/tmp` 사본에서 저장 이름의 안쪽 공백 연속을 한 칸으로 줄이고, 중복 비교를 허용 예외 문자를 뺀 키로 바꾼 약 5줄 수정 | r4 7건과 r1·r2·r3·`virtual_port.rs`·단위 테스트가 모두 통과했다. 요구가 다른 테스트와 충돌하지 않는다 | 없음 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 프로세스 전체 가상 포트 16개 이하 | 아니오 | 리뷰 3 뒤 생성·이름 변경·drop의 잠금 구조는 바뀌지 않았다(이름 계산만 잠금 앞에서 한다) |
| 프로세스 안 이름 중복 없음(NFC 비교) | 아니오 | 시도 8, 9 |
| 신디 목록에서 구분되지 않는 이름 금지(plan.md 세부 결정의 근거) | 예 | 시도 5(허용 예외만 다름), 시도 6(NBSP·공백 개수만 다름) |
| `name()`이 시스템이 보여 주는 이름과 같음 | 아니오 | 시도 8, 10. NFC 저장 이름이 그대로 열거된다 |
| 송신 경로 할당·락·로그 없음 | 아니오 | `send`는 리뷰 3 뒤 바뀌지 않음. 정규화의 할당은 생성·이름 변경 경로에만 있다 |

### 반복 실행 기록 (시도 13, 24회 가운데 2회차)

~~~
     Running tests/review_s1_t1.rs (target/debug/deps/review_s1_t1-fa80737dc79ca698)
running 10 tests
test child_recreates_client_after_server_idle ... ignored, ...
test client_source_send_sync_auto_traits_suffice_has_no_unsafe_impl ... ok
test create_virtual_output_concurrent_threads_never_exceeds_max ... ok
test create_and_drop_same_name_concurrently_never_shows_two_sources ... FAILED
test send_after_all_client_handles_dropped_succeeds ... FAILED
test create_virtual_output_name_differing_only_in_surrounding_whitespace_fails ... FAILED
test drop_after_rename_frees_both_names ... FAILED
test client_new_after_last_client_dropped_and_server_idle_succeeds ... ok
test send_four_word_message_fits_stack_event_list ... FAILED
test source_names_while_earlier_source_disposed_lists_live_source ... FAILED
(6건 모두) called `Result::unwrap()` on an `Err` value: Os { call: "MIDIClientCreate", status: -304 }
test result: FAILED. 3 passed; 6 failed; 1 ignored; 0 measured; 0 filtered out; finished in 18.96s
~~~

`review_s1_t1.rs`는 매 실행에서 MIDI를 쓰는 첫 바이너리다(단위 테스트는 MIDI를 쓰지 않는다). 직전 실행의 마지막 바이너리가 끝나 MIDI 서버가 유휴 종료하는 순간과 겹쳐 `-304`가 `CLIENT_CREATE_PATIENCE`(3초)보다 길게 이어진 것으로 보인다. 같은 바이너리에서 자식 프로세스로 도는 상한 테스트 둘은 몇 초 뒤 시작해 통과했다. 리뷰 1 시도 14의 「-304로 전 테스트 실패」와 같은 모양이다. 자식 둘이 통과하고 실패 테스트들이 서로 다른 시점에 3초씩 재시도했으므로, 서버 재기동이 3초를 넘는 경우가 있다는 뜻이다.

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| OUT-1 | 충족 | 리뷰 3 판정 그대로. 리뷰 3 뒤 생성 경로는 이름 계산만 바뀌었다(`client.rs:186-192`) |
| OUT-2 | 충족 | 최대 16, 이름 변경 그대로. 지적 1은 OUT-2 문언이 아니라 plan.md의 위임 결정(중복 금지 근거)에 대한 것이라 REQ 미충족으로 세지 않는다(리뷰 3과 같은 기준) |
| 8.4 비정상 종료 시 포트 미잔존 | 충족 | `virtual_port.rs` `killed_process_leaves_no_virtual_source`, 24회 통과 |
| 9.2 스케줄러 스레드 할당·락 금지 | 충족 | `send` 변경 없음 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: `send`에 없음(리뷰 3과 같음). `normalized_name`의 `String` 할당은 생성·이름 변경·`MidiClient::new`에서만 일어난다.
- 제품 코드의 `unwrap`/`expect`: 없음. `#[cfg(test)]` 단위 테스트의 `unwrap`만 있다.
- 에러가 삼켜지는 곳: `VirtualOutput::drop`의 dispose 결과(불가피, 리뷰 3과 같음).
- 새 의존성: `unicode-normalization 0.1.25`(MIT/Apache-2.0)와 `tinyvec`(Zlib/Apache-2.0/MIT). `cargo deny check` 통과(licenses ok).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 중복 판정이 「신디 목록에서 구분되지 않는 이름」을 다 잡지 못한다. (가) 리뷰 3 대응으로 받기로 한 허용 예외 문자(ZWJ, ZWNJ, 이체자 선택자, 태그 문자)는 정상 텍스트 안에서만 그려지는 효과가 있고, 라틴 글자 사이·뒤에서는 아무것도 그리지 않는다. 그래서 `"PadSynth"`와 `"Pad\u{200D}Synth"`, `"Pad Synth"`와 `"Pad Synth\u{FE0F}"`가 따로 만들어진다(시도 5). (나) 안쪽 공백의 종류·개수만 다른 이름(`"Lead Synth"`와 NBSP·두 칸)이 따로 만들어진다. NBSP는 macOS의 Option+Space 오입력으로 흔히 들어오고 공백과 똑같이 그려진다(시도 6). 리뷰 3 지적 1(NFC/NFD)과 같은 부류이고 같은 근거다. 고치는 방법은 작다: 저장 이름에서 안쪽 White_Space 연속을 U+0020 한 칸으로 줄이고(앞뒤 공백 제거와 같은 성격), 중복 비교는 허용 예외 문자를 뺀 키로 한다. 정상 텍스트(깃발·이모지·페르시아어)는 그대로 받고 그대로 열거된다. `/tmp` 사본에서 약 5줄로 모든 테스트가 통과함을 확인했다(시도 14) | `crates/keyscore-midi/src/client.rs:45-58`, `:192`, `:234-238` |
| 2 | 제안 | plan.md 세부 결정의 이름 규칙이 리뷰 3 수정을 따라오지 않았다. 「앞뒤 공백을 뗀 이름」, 「보이지 않는 서식 문자가 있으면 `InvalidName`」이라고 적혀 있지만, 이제 NFC로 저장하고, 기준은 `Default_Ignorable_Code_Point`이며, ZWNJ·ZWJ·이체자 선택자·태그 문자는 받는다. 코드 주석(`client.rs:36-44`)은 맞다. 「개정」 절에 한 줄 더하면 된다 | `docs/reports/s1-t1/plan.md:51` |
| 3 | 제안 | 보이는 글자 판정이 U+2800만 빈칸으로 본다. `Default_Ignorable_Code_Point`가 아니면서 빈칸으로 그려지는 문자(U+1D159, U+16FE4, U+FFF9–FFFB, U+13430–1343F 등) 하나만으로 된 이름이 만들어진다(시도 12). 사용자가 일부러 넣어야 하는 문자라 판정을 막지 않는다. 다루려면 일반 범주 Cf 전체를 「보이지 않음」으로 세는 정도가 적당하다 | `crates/keyscore-midi/src/client.rs:95-97` |
| 4 | 제안 | 리뷰 3 제안 6(S1-T2 이월)의 일괄 실패 출력을 잡았다. 원인은 `MIDIClientCreate`가 3초 넘게 `-304`를 돌려준 것이다(4절 기록). S1-T2 이월 항목에 「`-304`가 3초 재시도를 넘김, MIDI를 처음 쓰는 바이너리 시작 시」를 더하고, 제품에서는 앱 시작이 서버 재기동과 겹칠 때 3초 뒤 포기하는 것이 맞는지(더 기다릴지, 알림으로 다시 시도할지)를 그 Task에서 정하기를 권한다 | `crates/keyscore-midi/src/client.rs:106-107`; `plan/stages/11-S1-midi-engine.md:22` |

리뷰 3 대응 확인:

- 지적 1(수정, NFC/NFD): 해소. 리뷰 3 실패 테스트가 통과하고 되돌린 사본에서 다시 실패한다(시도 2). 한글 밖 정준 동치와 rename도 맞다(시도 8, 9). 같은 부류의 남은 빈틈은 새 지적 1.
- 지적 2(수정, 거부 목록): 해소. 목록이 `Default_Ignorable_Code_Point`와 범위 단위로 같다(시도 4).
- 제안 3: 반영. 정상 텍스트는 받는다(시도 10). 다만 허용 예외가 중복 판정을 우회한다(새 지적 1).
- 제안 4(`run_isolated`): 반영. 오타와 자식 실패를 모두 잡는다(시도 3). 리뷰 1·2 파일의 변경은 이것뿐이다(시도 1).
- 제안 5(오류 문서): 반영. `InvalidName`·`DuplicateName` 문서와 `# Errors`가 새 규칙과 맞다.
- 제안 6: 단계 파일 이월 확인. 이번에 출력을 잡았다(새 제안 4).

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 0건, 수정 1건(지적 1), 제안 3건. 리뷰 3 지적은 모두 해소됐지만, 같은 근거(신디 목록에서 구분되지 않는 이름 금지)의 빈틈이 리뷰 3 대응으로 새로 열렸거나(허용 예외) 남아 있다(안쪽 공백). 이번이 세 번째 재리뷰라 프로토콜 [8]에 따라 지적 1은 결정 요청으로 바뀐다. 결정 요청에는 다음 선택지를 권한다.

- A: 지적 1대로 안쪽 공백을 한 칸으로 줄여 저장하고, 허용 예외 문자를 뺀 키로 중복을 판정한다. 약 5줄이고 모든 기존·리뷰어 테스트와 함께 통과함을 확인했다(시도 14). 리뷰어 추천.
- B: 중복 금지의 근거를 「같은 NFC 문자열」로 좁혀 plan.md에 적고, 눈으로 구분되지 않는 이름은 S2 UI(이름 입력 검증)에서 다룬다. 이 경우 `review_s1_t1_r4.rs`의 실패 2건을 어떻게 남길지(기대를 뒤집을지)도 결정에 적어야 한다(기존 테스트 삭제 금지 규칙).

A로 고치면 `review_s1_t1_r4.rs` 7건, `review_s1_t1_r3.rs` 7건, `review_s1_t1.rs`·`review_s1_t1_r2.rs`·`virtual_port.rs` 전부가 통과해야 한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(리뷰어 테스트 diff는 과제가 지정해 함께 보았다)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`crates/keyscore-midi`, 워크스페이스 `Cargo.toml`·`Cargo.lock`의 새 의존성, `docs/reports/s1-t1/report.md`, 단계 파일의 S1-T2 이월 한 줄)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`unicode-normalization`, `tinyvec`, `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다
