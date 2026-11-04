#!/usr/bin/env python3
# 노트 10,000개 시험용 프로젝트 텍스트를 만든다(S3 출구 UI-3: 실제 창의 줌·스크롤 확인).
# 트랙 10개 x 16분음표 1,000개, 62.5마디. 채널 10은 드럼 채널이라 열째 트랙은 ch11.
# 사용: python3 -I scripts/gen-notes-10k.py assets/perf/notes-10k.txt
#       cargo run -p ksq-cli --release --bin txt2ksq -- assets/perf/notes-10k.txt -o assets/perf/notes-10k.ksq --strict
import sys
NAMES = ['c','c+','d','d+','e','f','f+','g','g+','a','a+','b']
PROGS = [0, 4, 11, 24, 33, 40, 48, 56, 73, 81]
LOWS  = [48, 48, 60, 52, 36, 55, 48, 60, 67, 60]   # 트랙별 음역 아래쪽 MIDI 번호
out = ['// 노트 10,000개 시험용 프로젝트: 트랙 10개 x 16분음표 1,000개 (S3 출구 UI-3 확인용, 생성 scripts/gen-notes-10k.py)',
       '@tempo 120', '@time 4/4', '@key C major', '']
s = 0x9E3779B97F4A7C15
total = 0
for t in range(10):
    out.append(f'@track T{t+1:02d} ch{t+1 if t < 9 else 11} prog{PROGS[t]}')
    toks = ['l16']
    cur_oct = None
    pitch = LOWS[t] + 7
    for i in range(1000):
        s ^= (s << 13) & 0xFFFFFFFFFFFFFFFF; s ^= s >> 7; s ^= (s << 17) & 0xFFFFFFFFFFFFFFFF
        step = (s % 7) - 3                      # -3..+3 반음 걸음
        pitch = min(LOWS[t] + 24, max(LOWS[t], pitch + step))
        octv, name = divmod(pitch, 12)
        octv -= 1                               # MIDI 60 = c4 (o4)
        tok = ''
        if octv != cur_oct:
            tok += f'o{octv} '; cur_oct = octv
        tok += NAMES[name]
        if (s >> 20) % 16 == 0:                 # 가끔 벨로시티를 바꿔 색·높이 변화를 준다
            tok = f'v{64 + (s >> 24) % 60} ' + tok
        toks.append(tok)
        total += 1
        if i % 16 == 15:
            toks.append('|')
    out.append(' '.join(toks))
    out.append('')
open(sys.argv[1], 'w', encoding='utf-8').write('\n'.join(out))
print('notes', total)
