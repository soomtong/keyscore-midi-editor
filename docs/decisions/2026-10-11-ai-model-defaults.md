# 결정 요청: AI 기본 모델·reasoning 설정과 SP-5 제외 기준

- 관련 REQ: 5.12, 6.8(AI-6, AI-16), 12.1
- 관련 Task: S7-T1(클라이언트), S7-T3(생성), S7-T4·S7-T5(어시스트). S7 착수 전까지만 답이 있으면 된다. S0~S6은 이 결정에 의존하지 않는다.
- 근거: `docs/spikes/sp5-llm-text-stability.md`, `docs/spikes/sp6-assist-latency.md`
- 상태: 결정 대기

## 상황

- 02-spikes의 SP-5 규칙은 「파싱 성공률 90% 미만인 모델은 기본값에서 제외」다. 템플릿 초안 1벌 기준으로 이를 넘은 것은 `openai/gpt-5.6-luna`의 reasoning 기본 설정(20/20)뿐이고, 이 설정은 완료 p50 9.5 s, p95 19 s다.
- reasoning을 끈 설정은 두 상용 모델 모두 60~70%로 미달이다. 실패의 대부분은 방언 혼입(`d6`, `c'`, `fis`)과 마디 길이 오산이라 템플릿 보강으로 나아질 여지가 크지만, 재측정은 하지 않았다.
- 어시스트(SP-6)는 reasoning을 끈 설정에서 p95 1.7 s로 2 s 상한 안이다. reasoning을 켜면 첫 토큰만 8 s 이상이라 어시스트에 쓸 수 없다.
- `~anthropic/claude-haiku-latest`의 reasoning 기본 설정은 max_tokens 2,500에서 30%가 잘렸다.
- 로컬 모델은 런타임이 없어 `google/gemma-3-12b-it`(OpenRouter)로 대신 쟀고 50%였다.

## 선택지

- A. 생성은 reasoning 켬(지연 10 s 안팎 허용, max_tokens 4,000 이상), 어시스트는 reasoning 끔. 90% 기준은 S7의 템플릿 보강 후 실험실(AI-16)에서 다시 적용하고, 그때까지 어떤 모델도 기본값에서 빼지 않는다.
- B. 생성·어시스트 모두 reasoning 끔. 템플릿 보강 후 재측정에서 90%를 넘는 모델만 기본값으로 둔다.
- C. 90% 기준을 지금 그대로 적용해 생성 기본값을 `openai/gpt-5.6-luna`(reasoning 켬)로만 둔다.

## 구현자 추천

A. 생성은 고스트로 제시되므로 10 s 대기가 편집 흐름을 막지 않고, 어시스트는 2 s 상한 때문에 reasoning을 켤 수 없다. 기본값 제외는 템플릿이 확정된 뒤의 수치로 판단하는 것이 SP-5의 취지에 맞다.

함께 정할 것: 로컬 모델 실측을 위해 Ollama를 설치해 S7 전에 재측정할지.

- 결정:
