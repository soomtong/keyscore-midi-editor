"""KeyScore Text (REQ 5.14) 스파이크용 검증 파서.

간략화한 부분:
- 틱이 아니라 Fraction(전음표 단위)으로 길이를 센다. 반올림 문제는 다루지 않는다.
- 옥타브 범위(o0-9)만 검사하고 음역 접기는 하지 않는다.
- 가져오기의 느슨한 모드(모르는 토큰 경고 후 건너뜀)는 없다. AI 응답 기준이라 모르는 토큰은 전부 오류다.
- `~±틱` 접미, `v<n>`은 AI 응답에서 금지이므로 오류로 처리한다.
- 문법이 말하지 않은 곳은 가장 엄격하게 해석하고, 그 지점을 warnings에 `AMBIG:` 접두로 남긴다.
"""
import re
from fractions import Fraction

LENGTHS = {1, 2, 4, 8, 16, 32, 64}
DRUM = {"bd", "sd", "rs", "cp", "ch", "ph", "oh", "lt", "mt", "ht", "cr", "rd", "rb", "tb", "cb", "sh"}

HEADER_RE = re.compile(r"^@track\s+(\S+)\s+ch(\d+)(?:\s+prog(\d+))?(?:\s+bank(\d+))?\s*$", re.I)
DIRECT_RE = re.compile(r"^@(tempo|time|key)\b(.*)$", re.I)

NOTE_RE = re.compile(r"([a-g])([+#\-]*)(\d+)?(\.*)", re.I)
REST_RE = re.compile(r"r(\d+)?(\.*)", re.I)
DRUMTOK_RE = re.compile(r"(bd|sd|rs|cp|ch|ph|oh|lt|mt|ht|cr|rd|rb|tb|cb|sh|n\d+)(\d+)?(\.*)", re.I)
# 숫자가 붙은 `n36`과 길이가 붙은 `ch8`을 구분하려면 n<번호>는 별도 처리
RAWDRUM_RE = re.compile(r"n(\d+)(?:-(\d+))?", re.I)


class Err(Exception):
    pass


def strip_comment(line):
    """`#`는 주석이자 올림표다(5.14가 둘 다 허용). 음이름 바로 뒤의 `#`는 올림표, 그 밖은 주석으로 본다."""
    out = []
    i = 0
    in_q = False
    while i < len(line):
        ch = line[i]
        if ch == '"':
            in_q = not in_q
        if ch == "#" and not in_q:
            prev = line[i - 1] if i > 0 else " "
            if prev.lower() in "abcdefg+#-" and i > 0 and (i < 2 or line[i - 2] in " \t[(|&" or prev in "+#-"):
                out.append(ch)  # 올림표
            else:
                break
        else:
            out.append(ch)
        i += 1
    return "".join(out)


def dur_of(num, dots, default):
    n = int(num) if num else default
    if n not in LENGTHS:
        raise Err(f"허용되지 않는 길이 {n}")
    d = Fraction(1, n)
    add = d
    for _ in range(len(dots)):
        add = add / 2
        d += add
    return d


def parse(text, bar_len=Fraction(1), strict_tracks=True):
    """반환: dict(ok, errors[], warnings[], tracks[{name,ch,dur,bars_by_len,bars_by_pipe,notes,lyrics}])"""
    errors, warnings, tracks = [], [], []
    t = text.strip("\n")
    cur = None
    for ln, raw in enumerate(t.split("\n"), 1):
        s = raw.strip()
        if s.startswith("```"):
            errors.append(f"L{ln}: 코드펜스 금지")
            continue
        body = strip_comment(raw).strip()
        if not body:
            continue
        if body.startswith("@"):
            m = HEADER_RE.match(body)
            if body.lower().startswith("@track"):
                if not m:
                    errors.append(f"L{ln}: @track 헤더 형식 오류: {body[:40]}")
                    cur = None
                    continue
                ch = int(m.group(2))
                if not 1 <= ch <= 16:
                    errors.append(f"L{ln}: 채널 범위 오류 {ch}")
                if m.group(3) and int(m.group(3)) > 127:
                    errors.append(f"L{ln}: prog 범위 오류")
                cur = dict(name=m.group(1), ch=ch, lines=[], first=ln)
                tracks.append(cur)
                continue
            if DIRECT_RE.match(body):
                continue
            errors.append(f"L{ln}: 모르는 지시어 {body[:30]}")
            continue
        if cur is None:
            errors.append(f"L{ln}: @track 밖의 텍스트(설명문 의심): {body[:40]}")
            continue
        cur["lines"].append((ln, body))
    for tr in tracks:
        try:
            info = parse_body(tr, bar_len, warnings)
            tr.update(info)
        except Err as e:
            errors.append(f"track {tr['name']}: {e}")
            tr.update(dur=None, bars_by_len=None, bars_by_pipe=None, notes=0)
        if not tr["lines"]:
            errors.append(f"track {tr['name']}: 본문 없음")
    if not tracks and not errors:
        errors.append("트랙 없음")
    return dict(ok=not errors, errors=errors, warnings=warnings, tracks=[{k: v for k, v in tr.items() if k != "lines"} for tr in tracks])


def parse_body(tr, bar_len, warnings):
    is_drum = tr["ch"] == 10
    src = []
    for ln, b in tr["lines"]:
        src.append((ln, b))
    # 토큰화: 줄 단위로 스캔. 괄호/마디선/타이/따옴표는 공백 없이 붙을 수 있다.
    toks = []
    for ln, b in src:
        i = 0
        while i < len(b):
            c = b[i]
            if c.isspace():
                i += 1
                continue
            if c in "|[]()&<>":
                toks.append((ln, c))
                i += 1
                continue
            if c == '"':
                j = b.find('"', i + 1)
                if j < 0:
                    raise Err(f"L{ln}: 닫히지 않은 따옴표")
                toks.append((ln, b[i : j + 1]))
                i = j + 1
                continue
            j = i
            while j < len(b) and not b[j].isspace() and b[j] not in '|[]()&<>"':
                j += 1
            # `n(`: n 다음이 '(' 인 경우
            w = b[i:j]
            if w.lower() == "n" and j < len(b) and b[j] == "(":
                toks.append((ln, "n("))
                i = j + 1
                continue
            toks.append((ln, w))
            i = j
    default = 4
    octv = None
    total = Fraction(0)
    pipes = 0
    nnotes = 0
    lyrics = 0
    last_elem_pitch = None
    pending_tie = False
    pipe_seen_content = False
    bars_by_pipe_ok = True
    seg = Fraction(0)
    seg_lens = []
    i = 0
    n = len(toks)

    def add(d):
        nonlocal total, seg
        total += d
        seg += d

    def parse_pitch_word(w, ln, in_chord):
        """w가 음/라벨이면 (길이문자열, 점, 키) 반환. 아니면 None."""
        if is_drum:
            m = DRUMTOK_RE.fullmatch(w)
            if m:
                lab = m.group(1).lower()
                if lab.startswith("n"):
                    # n36 처럼 번호만: 길이 숫자와 구분 불가 -> `n36`은 번호, 길이는 n36.8 같은 건 없음
                    m2 = re.fullmatch(r"n(\d+)(\.*)", w, re.I)
                    if m2:
                        if not 35 <= int(m2.group(1)) <= 81:
                            raise Err(f"L{ln}: n번호 범위 {w}")
                        return ("", m2.group(2), "n" + m2.group(1))
                    raise Err(f"L{ln}: 잘못된 드럼 토큰 {w}")
                return (m.group(2), m.group(3), lab)
            m = NOTE_RE.fullmatch(w)
            if m:
                raise Err(f"L{ln}: 드럼 트랙에 음이름 {w}")
            return None
        m = DRUMTOK_RE.fullmatch(w)
        m2 = NOTE_RE.fullmatch(w)
        if m2:
            return (m2.group(3), m2.group(4), m2.group(1).lower())
        if m and m.group(1).lower() in DRUM | {"n"}:
            raise Err(f"L{ln}: 멜로디 트랙에 드럼 라벨 {w}")
        return None

    def parse_unit(ln, w, in_chord=False):
        r = parse_pitch_word(w, ln, in_chord)
        return r

    while i < n:
        ln, w = toks[i]
        lw = w.lower()
        if w == "|":
            pipes += 1
            seg_lens.append(seg)
            seg = Fraction(0)
            i += 1
            continue
        if w == "&":
            i += 1
            continue
        if w == "<" or w == ">":
            i += 1
            continue
        if w.startswith('"'):
            if lyrics_attach_ok(toks, i):
                lyrics += 1
                i += 1
                continue
            raise Err(f"L{ln}: 가사가 음 뒤에 붙지 않음")
        if re.fullmatch(r"o\d+", lw):
            if int(lw[1:]) > 9:
                raise Err(f"L{ln}: 옥타브 범위 {w}")
            octv = int(lw[1:])
            i += 1
            continue
        m = re.fullmatch(r"l(\d+)(\.*)", lw)
        if m:
            d = int(m.group(1))
            if d not in LENGTHS:
                raise Err(f"L{ln}: 허용되지 않는 길이 {w}")
            default = d
            # 점 기본 길이는 `l8.` -> 길이 변수에 점 반영
            default_dots = m.group(2)
            default = (d, default_dots)
            i += 1
            continue
        if re.fullmatch(r"v\d+", lw):
            raise Err(f"L{ln}: v는 AI 응답에서 금지 {w}")
        if "~" in w:
            raise Err(f"L{ln}: ~틱 접미는 AI 응답에서 금지 {w}")
        if lw == "[":
            j = i + 1
            cnt = 0
            while j < n and toks[j][1] != "]":
                lnj, wj = toks[j]
                if wj in ("[", "|", "n(", ")") :
                    raise Err(f"L{lnj}: [ ] 안에 {wj}")
                r = parse_pitch_word(wj, lnj, True) if not wj.startswith('"') and wj not in "<>&" else "skip"
                if wj in ("<", ">", "&") or (isinstance(wj, str) and wj.lower().startswith("o") and re.fullmatch(r"o\d+", wj.lower())):
                    j += 1
                    continue
                if r is None:
                    raise Err(f"L{lnj}: [ ] 안의 알 수 없는 토큰 {wj}")
                if r != "skip":
                    if r[0] or r[1]:
                        raise Err(f"L{lnj}: [ ] 안의 음에 길이 {wj}")
                    cnt += 1
                j += 1
            if j >= n:
                raise Err(f"L{ln}: 닫히지 않은 [")
            if cnt == 0:
                raise Err(f"L{ln}: 빈 [ ]")
            # ] 뒤 길이: 다음 토큰이 숫자/점이어야 함
            k = j + 1
            num, dots = "", ""
            if k < n and re.fullmatch(r"\d+\.*|\.+", toks[k][1]):
                mm = re.fullmatch(r"(\d*)(\.*)", toks[k][1])
                num, dots = mm.group(1), mm.group(2)
                k += 1
            d = dur_with_default(num, dots, default)
            add(d)
            nnotes += cnt
            i = k
            continue
        if lw == "]":
            raise Err(f"L{ln}: 짝 없는 ]")
        if lw == "n(":
            j = i + 1
            elems = 0
            while j < n and toks[j][1] != ")":
                lnj, wj = toks[j]
                if wj == "[":
                    # 묶음 안 화음: ] 까지 건너뜀
                    while j < n and toks[j][1] != "]":
                        j += 1
                    elems += 1
                    j += 1
                    continue
                if wj in ("<", ">", "&") or re.fullmatch(r"o\d+", wj.lower()):
                    j += 1
                    continue
                if wj.startswith('"'):
                    j += 1
                    continue
                if wj.lower() == "r" or REST_RE.fullmatch(wj):
                    elems += 1
                    j += 1
                    continue
                r = parse_pitch_word(wj, lnj, False)
                if r is None:
                    raise Err(f"L{lnj}: n( ) 안의 알 수 없는 토큰 {wj}")
                elems += 1
                j += 1
            if j >= n:
                raise Err(f"L{ln}: 닫히지 않은 n(")
            if elems != 3:
                raise Err(f"L{ln}: n( ) 안은 3음이어야 함 (실제 {elems})")
            # 길이 해석: `)` 뒤 숫자가 있으면 AMBIG, 없으면 l 기본값을 각 음에 적용. 안의 음 길이도 허용(AMBIG)
            k = j + 1
            suffix = None
            if k < n and re.fullmatch(r"\d+\.*", toks[k][1]):
                suffix = toks[k][1]
                warnings.append(f"AMBIG: L{ln} n( ) 뒤 길이 `{suffix}`의 의미가 명세에 없음")
                k += 1
            sub = Fraction(0)
            jj = i + 1
            while jj < j:
                lnj, wj = toks[jj]
                if wj == "[":
                    kk = jj
                    while toks[kk][1] != "]":
                        kk += 1
                    jj = kk + 1
                    sub += dur_with_default("", "", default)
                    continue
                if wj in ("<", ">", "&") or re.fullmatch(r"o\d+", wj.lower()) or wj.startswith('"'):
                    jj += 1
                    continue
                mr = REST_RE.fullmatch(wj)
                if mr:
                    num, dots = mr.group(1) or "", mr.group(2)
                else:
                    num, dots, _ = parse_pitch_word(wj, lnj, False)
                if num:
                    warnings.append(f"AMBIG: L{lnj} n( ) 안 음에 길이 `{wj}`")
                if suffix and not num:
                    mm = re.fullmatch(r"(\d+)(\.*)", suffix)
                    num, dots = mm.group(1), mm.group(2)
                sub += dur_with_default(num, dots, default)
                jj += 1
            add(sub * Fraction(2, 3))
            nnotes += 3
            i = k
            continue
        if lw == ")":
            raise Err(f"L{ln}: 짝 없는 )")
        mr = REST_RE.fullmatch(w)
        if mr:
            add(dur_with_default(mr.group(1) or "", mr.group(2), default))
            i += 1
            continue
        r = parse_pitch_word(w, ln, False)
        if r is not None:
            num, dots, key = r
            if not is_drum and len(w) > 1 and re.search(r"#", w):
                warnings.append(f"AMBIG: L{ln} `#` 올림표 `{w}` (주석 문자와 충돌)")
            add(dur_with_default(num, dots, default))
            nnotes += 1
            i += 1
            continue
        # 공백 없이 붙인 음 (`cde`) 감지
        if re.fullmatch(r"(?:[a-g][+#\-]*\d*\.*)+", lw):
            raise Err(f"L{ln}: 공백 없는 음 연속 `{w}` (명세에 불명확)")
        raise Err(f"L{ln}: 알 수 없는 토큰 `{w}`")
    # 마지막 마디선 없이 끝났을 때 seg 처리
    if seg > 0:
        seg_lens.append(seg)
    bars_len = total / bar_len
    bars_pipe = None
    if pipes:
        bars_pipe = len([s for s in seg_lens if s > 0])
        for idx, s in enumerate(seg_lens):
            if s > 0 and s != bar_len:
                warnings.append(f"마디 {idx+1} 길이 {s} != {bar_len} (트랙 {tr['name']})")
    return dict(dur=str(total), bars_by_len=str(bars_len), bars_by_pipe=bars_pipe, notes=nnotes, lyrics=lyrics)


def lyrics_attach_ok(toks, i):
    if i == 0:
        return False
    prev = toks[i - 1][1].lower()
    if prev in ("]", ")"):
        return True
    return bool(NOTE_RE.fullmatch(prev) or DRUMTOK_RE.fullmatch(prev) or REST_RE.fullmatch(prev) or re.fullmatch(r"\d+\.*", prev))


def dur_with_default(num, dots, default):
    if isinstance(default, tuple):
        dn, ddots = default
    else:
        dn, ddots = default, ""
    if num:
        return dur_of(num, dots, dn)
    # 숫자가 없으면 현재 l(점 포함). 이 음에 점만 붙었으면(`c.`) 기본 길이에 점 추가
    return dur_of(str(dn), ddots + dots, dn)


if __name__ == "__main__":
    import sys, json
    txt = open(sys.argv[1]).read()
    print(json.dumps(parse(txt), ensure_ascii=False, indent=1))
