# SMF 테스트 데이터 출처

이 디렉터리의 파일은 모두 퍼블릭 도메인(또는 CC0)이며 SMF 읽기/쓰기 테스트용으로 수집했다(REQ IO-2).

각 파일의 라이선스는 출처 페이지에서 직접 확인했다. 파일 내용은 신뢰하지 않는 데이터로 취급하고, 헤더 검사만 별도 스크립트로 수행했다. 형식, ntrks, division, 메타 이벤트는 직접 파싱한 값이다.

| 파일 | 제목 / 작곡가 | 출처 페이지 | 직접 다운로드 URL | 페이지 표기 라이선스 | format | ntrks | division | 가사/텍스트 메타 | 크기 |
|---|---|---|---|---|---|---|---|---|---|
| chopin-mazurka-op33-4.mid | Mazurkas Op. 33 No. 4 / Chopin (제작 Michael Bednarek) | https://commons.wikimedia.org/wiki/File:Chopin,_Mazurkas,_Op._33,_No._4.mid | https://upload.wikimedia.org/wikipedia/commons/2/24/Chopin%2C_Mazurkas%2C_Op._33%2C_No._4.mid | CC0 | 0 | 1 | 480 | 없음 | 19311 B |
| bach-bwv269-chorale.mid | Aus meines Herzens Grunde BWV 269 / J. S. Bach (변환 Xavier) | https://commons.wikimedia.org/wiki/File:Johann_Sebastian_Bach-Aus_meines_Herzens_Grunde_(Chorale_1)-BWV_269.mid | https://upload.wikimedia.org/wikipedia/commons/1/1c/Johann_Sebastian_Bach-Aus_meines_Herzens_Grunde_%28Chorale_1%29-BWV_269.mid | Public domain | 0 | 1 | 43 (비정형 값) | 없음 | 7092 B |
| joplin-entertainer-pr.mid | The Entertainer / Scott Joplin | https://commons.wikimedia.org/wiki/File:The_Entertainer_-_Scott_Joplin.mid | https://upload.wikimedia.org/wikipedia/commons/9/9e/The_Entertainer_-_Scott_Joplin.mid | Public domain | 0 | 1 | 96 | 없음 | 18077 B |
| bach-sheep-safely-graze.mid | Schafe können sicher weiden BWV 208 / J. S. Bach (LilyPond) | https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=1794 | https://www.mutopiaproject.org/ftp/BachJS/BWV208/Sheep/Sheep.mid | Copyright: Public Domain | 1 | 6 | 384 | 있음 (0x01, 0x05) | 10217 B |
| mozart-k183-sym25-1.mid | Symphony No. 25 KV183 1악장 / W. A. Mozart (LilyPond) | https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=1380 | https://www.mutopiaproject.org/ftp/MozartWA/KV183/Symphony25_1/Symphony25_1.mid | Copyright: Public Domain | 1 | 8 | 384 | 있음 (0x01만) | 86979 B |
| aloha-oe.mid | Aloha Oe / Liliuokalani | https://commons.wikimedia.org/wiki/File:Aloha_oe_song_midi.mid | https://upload.wikimedia.org/wikipedia/commons/d/d9/Aloha_oe_song_midi.mid | Public domain | 1 | 7 | 256 | 있음 (0x01, 0x05) | 4934 B |
| terzetto-012.mid | Terzetto, opera Skupoy (Cheapskate) / M. Pashkevich (제작 O zanni) | https://commons.wikimedia.org/wiki/File:012_terzetto.MID | https://upload.wikimedia.org/wikipedia/commons/4/4a/012_terzetto.MID | Public domain | 1 | 5 | 1024 | 없음 | 47537 B |
| kentucky-home.mid | My Old Kentucky Home / Stephen Foster (제작 Hyacinth) | https://commons.wikimedia.org/wiki/File:10_My_Old_Kentucky_Home.mid | https://upload.wikimedia.org/wikipedia/commons/6/64/10_My_Old_Kentucky_Home.mid | Public domain | 1 | 2 | 960 | 없음 | 3729 B |
| alouette.mid | Alouette / 프랑스 민요 (제작 Ixnay) | https://commons.wikimedia.org/wiki/File:Alouette_(french_song).MID | https://upload.wikimedia.org/wikipedia/commons/a/a2/Alouette_%28french_song%29.MID | Public domain | 1 | 5 | 192 | 없음 | 3432 B |
| gm-128-sounds-demo.mid | Demonstration of all 128 sounds of the General MIDI spec / Dana Wheaton | https://commons.wikimedia.org/wiki/File:Demonstration_of_all_128_sounds_of_the_General_MIDI_spec.mid | https://upload.wikimedia.org/wikipedia/commons/6/61/Demonstration_of_all_128_sounds_of_the_General_MIDI_spec.mid | Public domain | 1 | 9 | 480 | 있음 (0x01만) | 31807 B |

## 비고

- format 0 파일이 3개, format 1 파일이 7개다. LilyPond 산출물은 Mutopia 2개이고 나머지는 Wikimedia Commons의 비-LilyPond 파일이다.
- bach-bwv269-chorale.mid의 division 43은 일반적인 PPQ가 아니다. 파서의 비정형 division 처리 확인용이다.
- Mutopia는 piece 페이지의 Copyright 항목과 CC 아이콘(licenses/publicdomain)으로 확인했다.
- Commons 파일은 파일 페이지의 라이선스 틀(pd 또는 cc0)로 확인했다.
