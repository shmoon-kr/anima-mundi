# MECHANICS — 1단계 규칙 명세 (tbaMUD 에서)

Anima Mundi 가 tbaMUD 처럼 동작하기 위한 **사실·공식·표·순서·예외**. 출처는 `파일:줄`(`/home/shmoon/tbamud/src`, 이 워크스페이스의 tbaMUD 는 고쳐진 판이다).
구현은 이 문서만 보고 하고, 코드는 절 번호(§2.3 처럼)를 가리킨다(D9). tbaMUD 의 코드·주석·구조는 옮기지 않는다.

- 정수 계산은 C 의 `int`: 나눗셈은 0 쪽으로 버림(음수도). 이 문서의 "/" 는 그 뜻이다
- 메시지는 영어 원문을 인용하고, **누가 받는지**를 적는다: 행위자(CHAR), 대상(VICT), 방의 나머지(ROOM, 대상 제외면 NOTVICT), 자는 사람도(SLEEP).
  Mundi 에서 이 원문들은 영어 템플릿(`.ftl`)의 기준이 되고, 사건의 관점(§3.6)이 받는 사람을 정한다
- 레벨 31 이상(불멸자)만의 예외는 "(불멸자 예외)"로 적고 1단계 구현에서는 넣지 않아도 된다. 1단계 범위 밖의 것은 "(범위 밖)"
- 상태: **승인됨** (2026-10-03). 고칠 때는 해당 절을 먼저 고치고 커밋 메시지에 절 번호를 적는다. 남은 미확인 항목은 끝의 목록

---

## 1. 시간과 틱

### 1.1 펄스와 주기
- 1 펄스 = 0.1초(`OPT_USEC` 100000, `PASSES_PER_SEC` 10, structs.h:555-566)
- 매 펄스 하는 일과 주기 (comm.c:1048-1106, 이 순서로):

| 순서 | 일 | 주기 |
|---|---|---|
| 1 | 예약된 사건 처리 | 매 펄스 |
| 2 | 스크립트 트리거 검사 | 13초 (범위 밖) |
| 3 | 존 리셋 검사(§13) | 10초 |
| 4 | 몹 행동(§14) | 10초 |
| 5 | 전투 라운드(§7) | 2초 |
| 6 | **틱**: 시간·날씨 → 효과 만료(§11) → 회복·배고픔(§5, §6) | 75초 |
| 7 | 자동 저장 | 5분 (config.c:132, 137) |
| 8 | 지운 캐릭터 정리 | 매 펄스 |

- 한 번의 루프에서 명령은 연결마다 **최대 하나**만 처리하고, 명령을 처리하면 대기가 최소 1 펄스다(comm.c:931-976). 늦어진 펄스는 몰아서 처리하되 30초(300 펄스)까지만(comm.c:1008-1023)
- 프롬프트(comm.c:961, 979-996): 이번 루프에 출력이 있었던 연결과, **명령을 처리한 연결은 출력이 없어도** 프롬프트를 받는다(빈 줄도 명령이다). 그 밖에는 보내지 않는다 — 틱의 회복만으로는 프롬프트가 오지 않는다(배고픔·목마름 문구처럼 틱의 출력이 있을 때만). S6 에서 확인: tbaMUD 의 파티는 늘 배고파서 틱마다 그 문구와 프롬프트를 받았다
- 접속이 하나도 없으면 tbaMUD 는 루프를 멈춘다(comm.c:813-831). **Mundi 는 멈추지 않는다**(에이전트·관전·시간 가속 때문. 차이로 기록)

### 1.2 게임 시간
- 게임 1시간 = 75초 = 1틱 (utils.h:193). 하루 24시간, 한 달 35일, 한 해 17달 (utils.h:196-202) → 게임 하루 = 실제 30분
- 시간이 오를 때(weather.c:42-82): 시 +1 → 해 상태 바꾸기 → 23 을 넘으면 0 시와 날 +1 → 34 일을 넘으면 0 일과 달 +1 → 16 달을 넘으면 0 달과 해 +1
- 해 상태와 문구(weather.c:47-63). 문구는 **깨어 있고 `indoors` 가 아닌 방에 있는** 플레이어에게만(comm.c:2510-2529):

| 도달한 시 | 상태 | 문구 |
|---|---|---|
| 5 | 해 뜸 | "The sun rises in the east." |
| 6 | 낮 | "The day has begun." |
| 21 | 해 짐 | "The sun slowly disappears in the west." |
| 22 | 밤 | "The night has begun." |

- 부팅 때 상태(db.c:847-856): 0-4시 밤, 5 해 뜸, 6-20 낮, 21 해 짐, 22-23 밤
- 날씨(기압과 하늘, weather.c:89-188)와 `time`·`weather` 명령(act.informative.c:1074-1135)은 1단계에서 문구만 맞춘다(세부는 조사 기록 참고, 파티가 쓰지 않는다)

## 2. 이동

### 2.1 명령
- 방향 명령 north east south west up down: 최소 자세 **서 있음**(interpreter.c:67-80). 대각선은 설정으로 꺼져 있다(config.c:115)
- 명령 이름은 **표 순서대로 앞부분 일치**, 처음 맞는 것이 이긴다. 방향이 표 맨 앞이라 `n` 은 north(interpreter.c:520-524)

### 2.2 막히는 경우 (이 순서로 검사, act.movement.c:340-355, 129-264)
1. 싸우는 중: 명령 단계에서 "No way!  You're fighting for your life!" (§4.2)
2. 출구가 없거나 갈 곳이 없다 → "Alas, you cannot go that way..."
3. 닫힌 문 → "The <문의 첫 키워드> seems to be closed." (키워드가 없으면 "It seems to be closed.") (불멸자 예외)
4. 매혹(charm)되어 주인이 같은 방에 있다 → CHAR "The thought of leaving your master makes you weep.", ROOM "$n bursts into tears."
5. 출발지나 도착지가 배 필요 물(`water_noswim`)이고 배가 없다 → "You need a boat to go there."
   (배 = 물 위 걷기·비행 효과, 또는 소지품의 배 물건, 또는 입은 배 물건: act.movement.c:39-61)
6. 출발지나 도착지가 `flying` 이고 날 수 없다 → "You need to be flying to go there!" (act.movement.c:64-86)
7. 출발지나 도착지가 수중이고 숨을 못 쉰다(플레이어만) → "You need to be able to breathe water to go there!" (act.movement.c:89-111)
8. 도착 존의 최소 레벨이 내 레벨보다 높다 → "This zone is above your recommended level." **경고만, 이동은 한다**(act.movement.c:217-219)
9. 도착 존이 닫혔다 → "A mysterious barrier forces you back! That area is off-limits."
10. 도착지가 `tunnel` 이고 이미 플레이어가 `tunnel_size`(2, config.c:70) 명 → "There isn't enough room for you to go there!"
    (1 명이면 "There isn't enough room there for more than one person!"). 몹도 막힌다
11. 이동력이 걸음 비용보다 적다(플레이어만) → "You are too exhausted." (따라가는 중이면 "You are too exhausted to follow.")
- 범위 밖: 집(atrium), 신의 방, 트리거, 건축 걷기

### 2.3 걸음 비용
- 비용 = (떠나는 방 지형 비용 + 들어가는 방 지형 비용) / 2 (act.movement.c:252-253), 지형 비용 (constants.c:810-822):

| 실내 | 도시 | 들판 | 숲 | 언덕 | 산 | 헤엄 물 | 배 물 | 공중 | 수중 |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1 | 2 | 3 | 4 | 6 | 4 | 1 | 1 | 5 |

- 플레이어만 낸다. 몹은 막히지도 내지도 않는다

### 2.4 이동 순서와 문구 (act.movement.c:273-331)
1. 이동력을 뺀다
2. 몰래 걷기(sneak)가 아니면 ROOM "$n leaves <방향 전체 이름>." — 행위자가 보이는 사람에게만(§3.5)
3. 방을 옮긴다. 새 방의 사람 목록 **맨 앞**에 들어간다(handler.c:463-464): 방을 볼 때 최근 도착자가 먼저 나온다
4. 몰래 걷기가 아니면 새 방의 ROOM "$n has arrived."
5. 행위자에게 방 보기(§3.2, 간략 모드를 따른다)
6. 죽음의 방(`death`)이면: 죽음의 비명(§8.4)과 함께 캐릭터가 사라진다 — 시체가 남는지는 확인 필요(범위 안: 위험 출구는 anima 가 피한다)

### 2.5 따라가는 사람 (act.movement.c:357-372, utils.c:660-681)
- 리더가 옮기면 따라가는 사람들이 **가장 최근에 따라오기 시작한 사람부터** 따라 옮긴다(목록 맨 앞에 추가, utils.c:674-675)
- 조건: 아직 원래 방에 있고 **서 있다**. 앉거나 쉬거나 자거나 싸우는 사람은 남는다
- 따라가는 사람에게 CHAR "You follow $N." (원문은 뒤에 빈 줄이 하나 더 생긴다) 그리고 그 사람의 이동(§2.2-2.4, 그 사람의 따라오는 사람도 이어서)
- `follow <이름>`: CHAR "You now follow $N.", 리더에게(리더가 볼 수 있으면) "$n starts following you.", NOTVICT "$n starts to follow $N."
  거절: "Whom do you wish to follow?", "You are already following $M.", "Sorry, but following in loops is not allowed.", "You are already following yourself."

### 2.6 몹의 배회 (mobact.c:74, 98-108)
- 10초마다(§1.1), `sentinel` 이 아니고 서 있고 싸우지 않고 깨어 있는 몹. 0~18 을 굴려 6 미만이면 그 방향으로(6/19 확률)
- 출구가 있고 열려 있어야 하고, 도착지가 `no_mob`·`death` 가 아니고, `stay_zone` 이면 같은 존 안, 주인이 없어야 한다

### 2.7 문 (act.movement.c:383-677)
- `open close lock unlock`: 최소 자세 앉음, `pick`: 서 있음
- 대상 찾기: 소지품·방의 물건(그릇이어야 한다), 아니면 문. 방향을 주면 그 방향의 문, 안 주면 모든 출구의 문 키워드에서
  - 거절 문구: "Open what?"(명령별), "That's not a direction.", "I see no %s there.", "I really don't see how you can %s anything there.",
    "What is it you want to %s?", "There doesn't seem to be %s %s here."
- 검사 순서: 문이나 닫히는 그릇이 아니다 "You can't %s that!" → 이미 닫힘 "But it's already closed!" → 열려 있음 "But it's currently open!" →
  잠기지 않음 "Oh.. it wasn't locked, after all.." → 잠김 "It seems to be locked." → 열쇠 없음 "You don't seem to have the proper key."
- 열쇠: 소지품에 있거나 손에 들고 있으면 된다(act.movement.c:460-476)
- 성공: 행위자 열기·닫기 "Okay.", 잠그기·열기 "*Click*". ROOM "$n opens the <키워드>." 등. **반대쪽 문도 같이 바뀌고**, 열기·닫기는
  반대편 방에 "The <키워드|door> is opened from the other side." / "... is closed from the other side."
- 자동 문·자동 열쇠 설정(PRF_AUTODOOR, AUTOKEY)과 따기(pick) 세부는 조사 기록 참고 (pick: 1~101 굴림 ≤ 기술 + 민첩 보정)

## 3. 보기와 지각

### 3.1 방이 어두운가 (utils.c:911-931, 이 순서로)
1. 방의 빛이 0 보다 크다 → 밝다
2. 방이 `dark` → 어둡다
3. 지형이 실내·도시 → 밝다
4. 해 짐이나 밤 → 어둡다
5. 그 밖 → 밝다
- `indoors` 는 어둠과 상관없다
- 방의 빛 = 그 방 사람들이 광원 자리에 든, 켜진(남은 시간 ≠ 0) 광원 수 (handler.c:447-474, 606-640)
- 광원은 틱마다 남은 시간이 1 줄고, 1 이 되면 CHAR "Your light begins to flicker and fade." / ROOM "$n's light begins to flicker and fade.",
  0 이 되면 "Your light sputters out and dies." / "$n's light sputters out and dies." (handler.c:996-1020). 남은 시간 -1 은 꺼지지 않는다

### 3.2 방 보기 (act.informative.c:488-547)
1. 어둡고 어둠 속을 못 보면 → "It is pitch black..." 로 끝
2. 눈이 멀었으면 → "You see nothing but infinite darkness..." 로 끝
3. 방 이름 (노랑)
4. 설명 — 간략 모드가 아니거나, `look` 으로 직접 보거나, 죽음의 방일 때. 이동 때는 간략 모드를 따른다
5. 자동 출구 설정이면 출구 줄: 청록 "[ Exits: n e s ]" — 방향 순서 n e s w u d, 각 뒤에 공백, 없으면 "[ Exits: None!]".
   닫힌 출구는 빨간 "(n)" 처럼 괄호로(설정 `display_closed_doors` YES, config.c:307), 숨은 출구는 보이지 않는다
6. 물건 목록(§3.3), 7. 사람 목록(§3.4)

### 3.3 물건 목록 (act.informative.c:60-221)
- 같은 물건(짧은 이름과 이름이 같은 것)은 묶어 하나만, 둘 이상이면 앞에 "( 2) " 처럼 개수
- 방에서는 긴 설명(초록), 긴 설명이 "." 로 시작하는 물건은 보이지 않는다
- 꼬리표: " (invisible)", 축복+정렬 감지면 " ..It glows blue!", 마법+마법 감지면 " ..It glows yellow!", " ..It has a soft glowing aura!"(glow), " ..It emits a faint humming sound!"(hum)
- 목록이 비면 "  Nothing." (보여야 하는 목록일 때만: 소지품 등)

### 3.4 사람 목록 (act.informative.c:287-416)
- 방의 사람 목록 순서(최근 도착자 먼저), 자기는 빼고, 긴 설명이 "." 로 시작하는 몹은 빼고
- 보이면 한 줄(노랑). 안 보이지만 어둠 속에서 상대가 적외선 시야를 가졌으면 "You see a pair of glowing red eyes looking your way."
- 그룹 표시: "(leader) " 또는 "(group) " (같은 그룹이면 초록, 아니면 빨강)
- **몹이 기본 자세이고 긴 설명이 있으면**: 긴 설명 그대로 (보이지 않으면 앞에 "*", 정렬 감지면 "(Red Aura) "/"(Blue Aura) ")
- **그 밖**: 이름(몹은 짧은 이름 첫 글자 대문자, 플레이어는 이름+칭호) + 꼬리표(" (invisible)", " (hidden)", " (linkless)" ...) + 자세 문구:
  죽음 " is lying here, dead." / 빈사 " is lying here, mortally wounded." / 무력 " is lying here, incapacitated." / 기절 " is lying here, stunned." /
  잠 " is sleeping here." / 쉼 " is resting here." / 앉음 " is sitting here." / 섬 " is standing here." /
  싸움 " is here, fighting YOU!" · " is here, fighting <이름>!" · " is here, fighting someone who has already left!"
- 성역(sanctuary)이면 다음 줄 "...$e glows with a bright light!"

### 3.5 볼 수 있는가 (utils.h:676-832)
- 어둠 속 시야 = 적외선 시야 효과 (불멸자 예외)
- 빛 조건 = 눈이 멀지 않았고, (방이 밝거나 적외선 시야)
- 투명 조건 = (상대가 투명이 아니거나 내가 투명 감지) 그리고 (상대가 숨지 않았거나 내가 생명 감지)
- 사람이 보인다 = 나 자신이거나, 빛 조건과 투명 조건 둘 다
- 물건이 보인다 = 빛 조건, 투명 물건이 아니거나 투명 감지, 그리고 들고 있는 사람이 보인다
- 안 보이는 사람의 이름은 "someone" (utils.h:845)
- 숨기(hide)는 **아무 명령이나 치면 풀린다** (interpreter.c:487)

### 3.6 사건이 누구에게 가나 (act() 의 관점, utils.h:709-711, comm.c:2806-2814) — Mundi 렌더러의 관점 모델
- 행위자(CHAR), 대상(VICT), 방의 나머지(ROOM: 행위자 제외, NOTVICT: 대상도 제외)
- 방의 나머지에게 가지 않는 경우: 자는 사람(SLEEP 표시가 있는 사건은 예외), 글을 쓰는 중인 사람
- "보여야 받는" 사건(이동, 도착 등)은 행위자를 볼 수 없는 사람(어둠, 실명, 투명)에게 가지 않는다
- Mundi: 이것은 **지각 필터(sim, D13)**의 규칙이다. 걸러진 결과가 에이전트에게는 이벤트로, 사람에게는 문장으로 간다

### 3.7 look 과 그 친구들 (act.informative.c:443-874)
- `look`(최소 자세 쉼): 실명이면 "You can't see a damned thing, you're blind!", 어두우면 "It is pitch black..." 다음 사람 목록(빛나는 눈만)
- 인자 해석 순서: 없음 → 방 보기 / "in" 의 앞부분 → 안 보기 / 방향 이름의 앞부분 → 그 방향 / "at" → 대상 / "around" → 방의 추가 설명 목록 / 그 밖 → 대상
- 방향 보기: 출구의 설명 또는 "You see nothing special.", 닫힌 문이면 "The <kw> is closed.", 열린 문이면 "The <kw> is open.", 출구 없음 "Nothing special there..."
- 안 보기(그릇·물통·분수): "Look in what?", "There's nothing inside that!", 닫힘 "It is closed.", 물통이 비면 "It is empty.",
  아니면 "It's <양>full of a <색> liquid." (양 = 현재×3/용량을 0~3 으로: "less than half ", "about half ", "more than half ", "")
- 대상 보기: 찾는 순서 = 방의 사람 → 장비 → 소지품 → 방의 물건. 사람이면 설명(없으면 "You see nothing special about $m.") + 상태 한 줄 + 입은 것.
  상태(체력 %): 100 이상 "is in excellent condition." / 90 "has a few scratches." / 75 "has some small wounds and bruises." / 50 "has quite a few wounds." /
  30 "has some big nasty wounds and scratches." / 15 "looks pretty hurt." / 0 "is in awful condition." / 그 아래 "is bleeding awfully from big wounds."
  대상에게 VICT "$n looks at you.", NOTVICT "$n looks at $N.". 도적은 남의 소지품도 엿본다. 못 찾으면 "You do not see that here."
- `exits`(최소 자세 쉼): "Obvious exits:" 다음 줄마다 "%-5s - <방 이름>", 어두우면 "%-5s - Too dark to tell.", 닫혔으면 "%-5s - The <kw> is closed.", 없으면 " None."
- `examine`(최소 자세 앉음): 대상 보기 + 그릇이면 "When you look inside, you see:" 와 안 보기

## 4. 자세와 명령 허용

### 4.1 자세 (structs.h:172-180)
죽음 0, 빈사 1, 무력 2, 기절 3, 잠 4, 쉼 5, 앉음 6, 싸움 7, 섬 8. "깨어 있다" = 잠보다 높다.
체력에 따른 자세(fight.c:95-109): 체력 > 0 이고 기절보다 높으면 그대로 / 체력 > 0 이면 섬 / -11 이하 죽음 / -6 이하 빈사 / -3 이하 무력 / 그 밖 기절

### 4.2 명령 처리 (interpreter.c:481-592, 이 순서로)
1. 숨기 풀림 → 2. 빈 입력은 무시 → 3. 글자가 아닌 첫 글자는 한 글자 명령 → 4. 명령 찾기(표 순서 앞부분 일치, 일반 명령 먼저 그 다음 소셜) →
5. 없으면 "Huh!?!" (그리고 비슷한 명령 제안) → 6. **최소 자세보다 낮으면** 지금 자세의 문구:

| 지금 자세 | 문구 |
|---|---|
| 죽음 | "Lie still; you are DEAD!!! :-(" |
| 무력·빈사 | "You are in a pretty bad shape, unable to do anything!" |
| 기절 | "All you can do right now is think about the stars!" |
| 잠 | "In your dreams, or what?" |
| 쉼 | "Nah... You feel too relaxed to do that.." |
| 앉음 | "Maybe you should get on your feet first?" |
| 싸움 | "No way!  You're fighting for your life!" |

7. 특수 동작(길드마스터·상점 주인 등) 아니면 명령 실행
- 최소 자세: 쉼 = sit rest stand look read exits follow eat taste drink sip / 잠 = sleep wake / 앉음 = examine, 문 명령 / 섬 = 이동, pick, fill, pour

### 4.3 자세 명령 (act.movement.c:731-950)
| 명령 | 지금 | 행위자 | 방 | 새 자세 |
|---|---|---|---|---|
| stand | 앉음 | "You stand up." | "$n clambers to $s feet." | 섬 (싸우는 중이면 싸움) |
| stand | 쉼 | "You stop resting, and stand up." | "$n stops resting, and clambers on $s feet." | 섬 |
| stand | 섬 / 잠 / 싸움 | "You are already standing." / "You have to wake up first!" / "Do you not consider fighting as standing?" | | |
| sit | 섬 | "You sit down." | "$n sits down." | 앉음 |
| sit | 쉼 | "You stop resting, and sit up." | "$n stops resting." | 앉음 |
| sit | 앉음 / 잠 / 싸움 | "You're sitting already." / "You have to wake up first." / "Sit down while fighting? Are you MAD?" | | |
| rest | 섬 | "You sit down and rest your tired bones." | "$n sits down and rests." | 쉼 |
| rest | 앉음 | "You rest your tired bones." | "$n rests." | 쉼 |
| rest | 쉼 / 잠 / 싸움 | "You are already resting." / "You have to wake up first." / "Rest while fighting?  Are you MAD?" | | |
| sleep | 섬·앉음·쉼 | "You go to sleep." | "$n lies down and falls asleep." | 잠 |
| sleep | 잠 / 싸움 | "You are already sound asleep." / "Sleep while fighting?  Are you MAD?" | | |
| wake | 잠 | "You awaken, and sit up." | "$n awakens." | 앉음 |
| wake | 깨어 있음 | "You are already awake..." | | |

- `wake <대상>`: 내가 자면 "Maybe you should wake yourself up first.", 없으면 "No one by that name here.", 깨어 있으면 "$E is already awake.",
  수면 효과면 "You can't wake $M up!", 상태가 나쁘면 "$E's in pretty bad shape!". 성공: CHAR "You wake $M up.", VICT(자는 사람에게도) "You are awakened by $n.", 대상은 앉음
- 앉기·눕기 가구(furniture)는 범위 밖

## 5. 회복 (틱마다, limits.c:37-176, 385-420)

### 5.1 나이 곡선
나이 = 태어난 뒤 게임 년 수 + 17 (utils.c:543-551). 곡선 graf(p0..p6) (limits.c:37-52): 15 미만 p0, 15-29 p1+(나이-15)×(p2-p1)/15, 30-44 p2+(나이-30)×(p3-p2)/15,
45-59 p3+(나이-45)×(p4-p3)/15, 60-79 p4+(나이-60)×(p5-p4)/20, 80 이상 p6

### 5.2 회복량 (플레이어; 몹은 셋 다 레벨 값)
| | 기본 graf | 잠 | 쉼 | 앉음 | 직업 | 배고픔·목마름 0 |
|---|---|---|---|---|---|---|
| 체력 | (8,12,20,32,16,10,4) | +1/2 | +1/4 | +1/8 | 마법사·성직자 ÷2 | ÷4 |
| 마나 | (4,8,12,16,12,10,8) | ×2 | +1/2 | +1/4 | 마법사·성직자 ×2 | ÷4 |
| 이동력 | (16,20,24,20,16,12,10) | +1/2 | +1/4 | +1/8 | — | ÷4 |
- 계산 순서: 기본 → 자세 → 직업 → 배고픔·목마름. 독이면 (몹 포함) 마지막에 ÷4
- 서 있거나 싸울 때는 자세 보너스가 없다

### 5.3 틱의 순서 (point_update, limits.c:385-487)
캐릭터마다: 1) 배고픔 -1, 술 -1, 목마름 -1 (§6.1: 이번 틱에 0 이 되면 바로 줄어든 회복) → 2) 기절 이상이면 체력·마나·이동력 회복(최대까지),
독이면 독 피해 2(§11), 기절 이하면 자세 갱신 → 3) 무력이면 피해 1, 빈사면 피해 2 (회복 없음) → 4) 플레이어면 광원·물건 시간(§3.1), 자리 비움 시간 →
모든 캐릭터 다음 물건: 시체 시간이 다 되면 썩는다(§8.3)
- 자리 비움(범위 밖에 가까움): 8틱이면 "You have been idle, and are pulled into a void.", 48틱이면 강제 퇴장 (config.c:81-82). 에이전트는 계속 명령을 치므로 걸리지 않는다

## 6. 배고픔·목마름·먹고 마시기

### 6.1 상태 (limits.c:305-337, structs.h:493-495)
- 배부름과 갈증은 0~24 (24 가득, 0 굶주림), 술은 0 부터. 새 캐릭터는 배부름 24, 갈증 24, 술 0 (class.c:1471-1473). 불멸자는 -1(변하지 않음)
- 틱마다 -1, 0 이 되면 "You are hungry." / "You are thirsty." — **0 인 동안 틱마다 다시**. 술이 0 이 되면 "You are now sober."
- 0 의 효과는 회복 ÷4 뿐이다(§5.2). 굶어 죽지 않는다

### 6.2 먹기 (act.item.c:998-1069; `eat`, 최소 자세 쉼)
"Eat what?" / 소지품에서만 찾고 없으면 "You don't seem to have %s %s." / 음식이 아니면 "You can't eat THAT!" / 배부름 20 초과면 "You are too full to eat more!" →
CHAR "You eat $p.", ROOM "$n eats $p." → 배부름 += 음식 값0 → 20 을 넘으면 "You are full." → 독 음식(값3)이면 "Oops, that tasted rather strange!" /
ROOM "$n coughs and utters some strange sounds." 그리고 독(값0×2 시간) → 음식은 없어진다. `taste` 는 1 만큼, "You nibble a little bit of $p."

### 6.3 마시기 (act.item.c:855-996; `drink`, 최소 자세 쉼)
- 물통: 값0 용량, 값1 남은 양, 값2 액체, 값3 독. 용량이나 남은 양이 음수면 **무한**(분수 등)
- 순서: "Drink from what?"(인자 없음; 물 지형이면 그냥 마심) → 소지품 다음 방에서 찾기, 없으면 "You can't find it!" → 물통·분수가 아니면 "You can't drink from that!" →
  바닥의 물통은 "You have to be holding that to drink from it." (분수는 괜찮다) → 술 10 초과면 "You can't seem to get close enough to your mouth." →
  배부름 20 초과(갈증 > 0)면 "Your stomach can't contain anymore!" → 비었으면 **"It is empty."** →
  CHAR "You drink the <액체>.", ROOM "$n drinks <액체> from $p."
- 양: 액체의 술 값이 0 보다 크면 (25 - 갈증) / 그 술 값, 아니면 3~10 무작위. 유한 물통이면 남은 양을 넘지 않는다
- 상태 변화 = 액체표 값 × 양 / 4 (술, 배부름, 갈증 순). 그 뒤 "You feel drunk."(술 > 10), "You don't feel thirsty any more."(갈증 > 20), "You are full."(배부름 > 20)
- 남은 양 -= 양, 0 이 되면 빈 물통(액체·독 초기화)
- 액체표 (constants.c:551-568) 술/배부름/갈증: 물 0/1/10, 맥주 3/2/5, 포도주 5/2/5, 에일 2/2/5, 흑맥주 1/2/5, 위스키 6/1/4, 레모네이드 0/1/8, 화주 10/0/0,
  지역 특산주 3/3/3, 슬라임 곰팡이 즙 0/4/-8, 우유 0/3/6, 차 0/1/6, 커피 0/1/6, 피 0/2/-1, 바닷물 0/1/-2, 맑은 물 0/0/13

---

## 7. 전투

### 7.1 라운드 (fight.c:939-996, 2초마다)
- 싸우는 사람 목록을 앞에서부터. 싸움을 시작하면 목록 **맨 앞**에 들어가므로(fight.c:161-162) 가장 최근에 싸움을 시작한 사람이 먼저 친다
- 각자, 이 순서로:
  1. 상대가 없거나 다른 방에 있으면 싸움을 멈추고 넘어간다
  2. 몹만: 대기가 0 보다 크면 20 을 빼고 이번 라운드를 쉰다. 아니면 대기 0. 싸움 자세보다 낮으면 싸움 자세로, ROOM "$n scrambles to $s feet!" (보이는 사람에게), **이번 라운드에도 친다**
  3. 싸움 자세보다 낮으면(플레이어만 여기 온다) CHAR "You can't fight while sitting!!", 이번 라운드를 쉰다(자동 거들기도 없다). **플레이어는 저절로 일어나지 않는다**
  4. 자동 거들기(§7.3)
  5. 한 번 친다(§7.4). 라운드당 공격은 **하나**
  6. 특수 동작이 있는 몹이면 그것을 부른다
- 플레이어의 대기(명령 지연)는 명령 입력만 막고 펄스마다 1 줄어든다. 대기 중에도 라운드 공격은 한다. 몹의 대기는 싸우는 동안에만 준다

### 7.2 싸움의 시작 (fight.c:145-185, 659-678; act.offensive.c:68-125)
- 피해를 주면(피해 0 도 포함) 양쪽 모두, 기절보다 높고 싸우지 않던 사람은 싸움을 시작한다. 기억(memory) 몹은 플레이어 공격자를 기억한다
- 싸움을 시작하면 자세가 싸움, 수면 주문이 풀린다. 싸움이 끝나면 자세는 섬 → 체력으로 갱신(§4.1)
- **자는 사람도 맞으면 깬다**: 잠(4)은 기절(3)보다 높아서 첫 피해에 바로 싸움 자세가 된다(fight.c:664-665, 167). 그래서 "반드시 맞고 ×2"(§7.4)는 **첫 한 방**에만 해당한다.
  (PHASE-1-PLAN S1 표의 "맞아도 깨지 않는다"는 이 줄로 고친다. 앉거나 쉬던 사람도 같다: 일어서지는 않고 싸움 자세가 된다)
- 투명하거나 숨은 공격자는 나타난다: ROOM "$n slowly fades into existence."
- 플레이어끼리(PK)는 설정상 금지(config.c:52): "Player killing is not permitted." 몹이 끼면 언제나 허용
- `hit`/`kill <대상>`(최소 자세 싸움): "Hit who?", "That player is not here.", 자기 자신 "You hit yourself...OUCH!." / ROOM "$n hits $mself, and says OUCH!",
  이미 싸우는 중 "You're fighting the best you can!" (**대상을 바꿀 수 없다**)
- **선제**: 서서 새로 공격하면 민첩이 높은 쪽이 먼저 친다(같으면 반반). 내가 지면 **상대만** 한 번 친다. 어느 쪽이든 공격자는 대기 22 펄스(act.offensive.c:90-94)

### 7.3 거들기 (fight.c:968-988, act.offensive.c:24-66)
- 라운드마다, 싸우는 사람의 **그룹**(따라가기 말고) 구성원 중 몹이거나 자동 거들기를 켠 플레이어이고, 같은 방, 싸우지 않음, **서 있음**, 그를 볼 수 있는 사람이 거든다
- `assist <이름>`: "You're already fighting!  How can you assist someone else?", "Whom do you wish to assist?", "No one by that name here.",
  "You can't help yourself any more than this!", "But nobody is fighting $M!", "You can't see who is fighting $M!"
- 상대 = 그가 싸우는 상대, 없으면 방에서 그와 싸우는 첫 사람
- 성공: CHAR "You join the fight!", 거들어 받는 사람 "$N assists you!", NOTVICT "$n assists $N." 그리고 **바로 한 번 친다**(라운드 밖)

### 7.4 한 번 치기 (fight.c:822-936)
- 공격 종류: 든 무기면 무기의 공격(§7.7), 아니면 몹의 맨손 공격, 아니면 hit
- **THAC0** = 기본 − 힘의 명중 보정 − 명중(hitroll) − (지능−13)/1.5 − (지혜−13)/1.5 (나눗셈은 실수로 하고 0 쪽으로 버림).
  기본: 플레이어는 직업·레벨 표, 몹은 20. 몹 파일의 thac0 은 명중 = 20 − 그 값으로 들어가고 몹 지능·지혜는 11 이므로, 몹의 실제 THAC0 ≈ 파일 값 + 2 (추론, 확인 필요)
- 직업 THAC0 (class.c:1190-1358):
  - 마법사 20 − (L−1)/3 (34 레벨 9)
  - 성직자 L1-3 20, 이후 3레벨마다 −2 (4-6 18, … 28-30 2), 31+ 1
  - 도적 20 − (L−1)/2
  - 전사 L1 20 부터 레벨마다 −1, 단 **8 레벨은 14**(7 과 같다), 9 13, … 20 2, 21+ 1
- 힘 표 인덱스: 힘, 단 18 에 추가 힘이 있으면 1-50 → 26, 51-75 → 27, 76-90 → 28, 91-99 → 29, 100 → 30 (utils.h:661-667). 보정표 값은 §16
- **방어도(AC)** = 기본 AC + (깨어 있으면) 민첩 방어 보정 × 10, 아래로 −100 까지. 판정에는 AC / 10 을 쓴다. 기본 AC 는 플레이어 100, 몹은 파일 값 × 10.
  방어구는 값0 × 자리 배율(몸 3, 머리 2, 다리 2, 그 밖 1)만큼 AC 를 낮춘다(handler.c:526-555). 방어구 종류만 센다
- **명중**: 1~20 을 굴려 20 이면 맞고, 대상이 깨어 있지 않으면 맞고, 1 이면 빗나가고, 그 밖에는 THAC0 − 굴림 ≤ 대상 AC/10 이면 맞는다
- **피해**: 힘의 피해 보정 + 피해 보너스(damroll) + (무기면 무기 주사위, 아니면 몹은 몹 주사위, 플레이어는 0~2) → 대상 자세 배율 → 최소 1
  - 자세 배율(정수 계산 1 + (7 − 자세)/3): 앉음·쉼 ×1, 잠·기절·무력 ×2, 빈사 ×3, 싸움·섬 ×1
- 빗나가면 피해 0 으로 피해 처리(§8.1, 빗나감 문구)

### 7.5 공격적인 몹 (mobact.c:110-191, 10초마다 §14.2)
- 도우미(helper), 실명, 매혹된 몹은 공격하지 않는다. 방의 사람 중 처음으로 볼 수 있는 플레이어를 친다. 겁쟁이(wimpy) 몹은 깨어 있는 사람을 건너뛴다
- 조건: 공격적, 또는 악 공격(대상 정렬 ≤ −350), 중립 공격(−350 < 정렬 < 350), 선 공격(≥ 350). **레벨은 보지 않는다**
- 기억 몹: 기억한 플레이어가 방에 있으면 ROOM "'Hey!  You're the fiend that attacked me!!!', exclaims $n." 그리고 친다
- 도우미 몹: 방에서 플레이어와 싸우는 다른 몹이 있으면 ROOM "$n jumps to the aid of $N!" 그리고 그 플레이어를 친다
- 매혹 몹의 반항·주인의 억제(d20)는 범위 밖(매혹 주문이 1단계 직업 레벨에 없다)

### 7.6 피해 문구 (fight.c:435-536)
보내는 순서: 방(NOTVICT) → 공격자 → 대상(자는 사람도). #w 단수 동사, #W 복수:

| 피해 | 방 | 공격자 | 대상 |
|---|---|---|---|
| 0 | "$n tries to #w $N, but misses." | "You try to #w $N, but miss." | "$n tries to #w you, but misses." |
| 1-2 | "$n tickles $N as $e #W $M." | "You tickle $N as you #w $M." | "$n tickles you as $e #W you." |
| 3-4 | "$n barely #W $N." | "You barely #w $N." | "$n barely #W you." |
| 5-6 | "$n #W $N." | "You #w $N." | "$n #W you." |
| 7-10 | "$n #W $N hard." | "You #w $N hard." | "$n #W you hard." |
| 11-14 | "$n #W $N very hard." | "You #w $N very hard." | "$n #W you very hard." |
| 15-19 | "$n #W $N extremely hard." | "You #w $N extremely hard." | "$n #W you extremely hard." |
| 20-23 | "$n massacres $N to small fragments with $s #w." | "You massacre $N to small fragments with your #w." | "$n massacres you to small fragments with $s #w." |
| 24+ | "$n OBLITERATES $N with $s deadly #w!!" | "You OBLITERATE $N with your deadly #w!!" | "$n OBLITERATES you with $s deadly #w!!" |

- 빗나감과 죽이는 한 방은 공격 종류별 문구(§8.2)가 있으면 그것을 쓴다. 주문·기술·고통(399)은 언제나 그 문구다

### 7.7 공격 종류 (fight.c:34-51)
번호(무기 값3, 몹 맨손 공격) 0 hit, 1 sting, 2 whip, 3 slash, 4 bite, 5 bludgeon, 6 crush, 7 pound, 8 claw, 9 maul, 10 thrash, 11 pierce, 12 blast, 13 punch, 14 stab.
복수형은 s 또는 es (slashes, crushes, punches, thrashes)

## 8. 피해·죽음·시체

### 8.1 피해 처리 (fight.c:613-817, 이 순서로)
1. 대상이 이미 죽었으면 끝
2. PK 금지, 평화로운 방(`peaceful`) "This room just has such a peaceful, easy feeling...", 상점 주인·죽일 수 없는 몹 "This mob is protected." → 피해 없음
3. 싸움 시작(§7.2)
4. 성역이면 피해 2 이상일 때 /2
5. 피해를 **0~100** 으로 자른다(한 번에 100 이 최대, 기습 포함). 체력에서 뺀다
6. 공격자가 자신이 아니면 **경험치 += 대상 레벨 × 피해**(§9.1 상한)
7. 자세 갱신(§4.1)
8. 문구(§7.6, §8.2)
9. 자세 문구 (대상에게 / 방에게):
   - 빈사 "You are mortally wounded, and will die soon, if not aided." / "$n is mortally wounded, and will die soon, if not aided."
   - 무력 "You are incapacitated and will slowly die, if not aided." / "$n is incapacitated and will slowly die, if not aided."
   - 기절 "You're stunned, but will probably regain consciousness again." / "$n is stunned, but will probably regain consciousness again."
   - 죽음 "You are dead!  Sorry..." / "$n is dead!  R.I.P."
   - 그 밖: 피해 > 최대 체력/4 이면 "That really did HURT!", 체력 < 최대/4 이면 빨강 "You wish that your wounds would stop BLEEDING so much!"
     그리고 겁쟁이 몹이면 도주(§10.1). 플레이어의 도망 체력(wimpy)이 있고 0 < 체력 < 그 값이면 "You wimp out, and attempt to flee!" 그리고 도주
10. 연결이 끊긴 플레이어가 맞으면 도주, 그래도 싸우면 ROOM "$n is rescued by divine forces." 그리고 0 번 방으로 (범위 밖에 가깝다)
11. 기절 이하로 떨어진 대상은 싸움을 멈춘다(다른 사람은 계속 칠 수 있다)
12. 죽었으면: 경험치(§9.2, **죽기 전 경험치**로 계산) → 죽음(§8.3) → 죽인 사람의 자동 동작(§12.3)

### 8.2 문구 파일 (lib/misc/messages, fight.c:540-607)
- 공격 종류마다 여러 묶음, 한 묶음은 죽음·빗나감·맞음·신 각각 (공격자, 대상, 방) 세 줄. 무작위로 한 묶음
- 이 파일은 tbaMUD 의 텍스트(콘텐츠)이므로 변환기로 옮겼다: `third_party/tbamud/messages/combat.yaml` (번호, 이름, 변형마다 die·miss·hit·god × attacker·victim·room, 없는 줄은 비움). 변형의 순서는 파일 순서

### 8.3 죽음 (fight.c:267-329)
1. 경험치를 **절반** 잃는다(최대 500,000 까지, 0 아래로는 안 간다). 플레이어의 살인자·도둑 표시가 지워진다
2. 싸움 멈춤, **모든 효과 제거**
3. 죽음의 비명: ROOM "Your blood freezes as you hear $n's death cry.", 갈 수 있는(열린) 이웃 방마다 "Your blood freezes as you hear someone's death cry."
4. 그룹 구성원에게 "<이름> has died."
5. 시체(§8.4) 그리고 캐릭터가 사라진다
- **플레이어는**: 장비·소지품은 모두 시체에, 접속은 메뉴로 돌아간다(메뉴 원문 config.c:273-283). 메뉴에서 1 을 고르면
  체력·마나·이동력 중 0 이하인 것을 1 로, 자세 섬, **시작 방(3001)** 에서 다시 들어온다(db.c:3738-3748, interpreter.c:1270-1318).
  Mundi: 메뉴는 화면 문제, 규칙은 "같은 캐릭터가 3001 에서 0 이하 수치는 1 로 다시 시작, 장비는 시체에"

### 8.4 시체 (fight.c:187-257, limits.c:436-460)
- 이름 "corpse", 짧은 이름 "the corpse of <이름>", 긴 설명 "The corpse of <이름> is lying here." 그릇(용량 0, 값3 = 1 이 시체 표시), 들 수 있다
- 내용: 소지품 전부, 장비 전부, 금화(몹이거나 연결된 플레이어만; 금화 물건으로)
- 시간: 몹 5틱, 플레이어 10틱(config.c:77-78). 틱마다 1 씩, 0 이면 썩는다: 들고 있으면 "$p decays in your hands.", 아니면 방에 "A quivering horde of maggots consumes $p."
  내용물은 그 자리(방, 그릇, 들고 있는 사람의 방)에 쏟아진다
- 시체 안에 넣을 수 없다: "You can't put anything in $P."

### 8.5 쓰러진 사람
- 무력 틱마다 피해 1, 빈사 2 (§5.3). 기절은 회복한다
- 전사 기술 bandage(레벨 7)는 체력 < 0 인 대상을 0 으로 (파티가 쓰면 넣는다)

## 9. 경험치와 레벨

### 9.1 경험치 변화 (limits.c:223-268)
- 레벨 31 이상 플레이어는 변하지 않는다. 몹은 상한 없이
- 얻을 때: 한 번에 **최대 100,000** (config.c:73). 레벨 30 까지만 오르고(config.c:111), 넘친 경험치는 쌓인다
  - 오르면 "You rise a level!" (여러 레벨이면 "You rise %d levels!"), 그리고 레벨 오름(§9.4)
- 잃을 때: 한 번에 최대 500,000, 0 아래로 가지 않는다. **레벨은 내려가지 않는다**

### 9.2 죽였을 때 (fight.c:331-405)
- 혼자: 대상 경험치/3 (최대 100,000) + 그 값 × min(8, 대상 레벨 − 내 레벨) / 8 (음수면 0; 몹이 죽이면 8 대신 4), 최소 1.
  "You receive %d experience points." (1 이하면 "You receive one lousy experience point.")
- 그룹(죽인 사람이 그룹이면): 같은 방의 구성원 수 n(몹 포함), 몫 = max(1, (대상 경험치/3 + n − 1) / n), 각자 최대 100,000. **레벨 차 보너스 없음**.
  "You receive your share of experience -- %d points." (1 이하 "... -- one measly little point!")
- 정렬: 내 정렬 += (−대상 정렬 − 내 정렬) / 16
- 피해를 줄 때마다 얻는 경험치(§8.1-6)와 **따로** 받는다
- 몹도 때릴 때마다 경험치를 얻는다(레벨 × 피해, 몹은 상한 없이: limits.c:231-234). 그래서 오래 싸운 몹은 죽일 때 더 많이 준다(시험으로 확인, S5)

### 9.3 레벨표 (class.c:1673-1847)
레벨 1 = 1. 그 레벨에 필요한 누적 경험치:

| 레벨 | 마법사 | 성직자 | 도적 | 전사 |
|---|---|---|---|---|
| 2 | 2,500 | 1,500 | 1,250 | 2,000 |
| 3 | 5,000 | 3,000 | 2,500 | 4,000 |
| 4 | 10,000 | 6,000 | 5,000 | 8,000 |
| 5 | 20,000 | 13,000 | 10,000 | 16,000 |
| 6 | 40,000 | 27,500 | 20,000 | 32,000 |
| 7 | 60,000 | 55,000 | 40,000 | 64,000 |
| 8 | 90,000 | 110,000 | 70,000 | 125,000 |
| 9 | 135,000 | 225,000 | 110,000 | 250,000 |
| 10 | 250,000 | 450,000 | 160,000 | 500,000 |
| 11 | 375,000 | 675,000 | 220,000 | 750,000 |
| 12 | 750,000 | 900,000 | 440,000 | 1,000,000 |
| 13 | 1,125,000 | 1,125,000 | 660,000 | 1,250,000 |
| 14 | 1,500,000 | 1,350,000 | 880,000 | 1,500,000 |
| 15 | 1,875,000 | 1,575,000 | 1,100,000 | 1,850,000 |
| 16 | 2,250,000 | 1,800,000 | 1,500,000 | 2,200,000 |
| 17 | 2,625,000 | 2,100,000 | 2,000,000 | 2,550,000 |
| 18 | 3,000,000 | 2,400,000 | 2,500,000 | 2,900,000 |
| 19 | 3,375,000 | 2,700,000 | 3,000,000 | 3,250,000 |
| 20 | 3,750,000 | 3,000,000 | 3,500,000 | 3,600,000 |
| 21 | 4,000,000 | 3,250,000 | 3,650,000 | 3,900,000 |
| 22 | 4,300,000 | 3,500,000 | 3,800,000 | 4,200,000 |
| 23 | 4,600,000 | 3,800,000 | 4,100,000 | 4,500,000 |
| 24 | 4,900,000 | 4,100,000 | 4,400,000 | 4,800,000 |
| 25 | 5,200,000 | 4,400,000 | 4,700,000 | 5,150,000 |
| 26 | 5,500,000 | 4,800,000 | 5,100,000 | 5,500,000 |
| 27 | 5,950,000 | 5,200,000 | 5,500,000 | 5,950,000 |
| 28 | 6,400,000 | 5,600,000 | 5,900,000 | 6,400,000 |
| 29 | 6,850,000 | 6,000,000 | 6,300,000 | 6,850,000 |
| 30 | 7,400,000 | 6,400,000 | 6,650,000 | 7,400,000 |

### 9.4 레벨 오름 (class.c:1432-1546)
- 체력 += max(1, 체질 보정 + 직업 굴림): 마법사 3~8, 성직자 5~10, 도적 7~13, 전사 10~15
- 마나(새 레벨 2 부터, 마법사·성직자만) += 새 레벨 L 에서 L ~ 1.5L 무작위, 최대 10
- 이동력 += max(1, 굴림): 마법사·성직자 0~2, 도적·전사 1~3
- 연습 횟수 += 마법사·성직자 max(2, 지혜 보너스), 도적·전사 min(2, max(1, 지혜 보너스))
- 체질 보정 (constants.c:706+): 0:−4, 1:−3, 2-3:−2, 4-6:−1, 7-14:0, 15:1, 16-17:2, 18-19:3, 20:4. 지혜 보너스 (constants.c:767-794): 0-11:0, 12-13:2, 14-16:3, 17:4, 18:5, 19-22:6
- **새 캐릭터**: 레벨 1, 경험치 1, 최대 체력 10, 마나 100, 이동력 82, 도적은 sneak 10 hide 5 steal 15 backstab 10 pick 10 track 10, 그리고 레벨 오름 한 번, 모두 가득

## 10. 도주와 전투 기술

### 10.1 도주 (act.offensive.c:234-268; `flee`, 최소 자세 싸움)
- 싸움 자세보다 낮으면 "You are in pretty bad shape, unable to flee!"
- 최대 6 번, 무작위 방향(6 방향 중). 처음으로 갈 수 있고(열린 출구) 죽음의 방이 아닌 방향에서:
  ROOM "$n panics, and attempts to flee!" → 보통 이동(§2.2-2.4, 이동력·문 등 그대로) →
  - 성공: "You flee head over heels." 플레이어는 경험치를 **(상대 최대 체력 − 상대 체력) × 상대 레벨** 만큼 잃는다. 싸움 끝(상대도)
  - 이동 실패: ROOM "$n tries to flee, but can't!"
  - 어느 쪽이든 그 한 방향만 시도하고 끝
- 6 번 모두 못 찾으면 "PANIC!  You couldn't escape!"
- 도망 체력 `toggle wimpy <n>` (act.informative.c:2364-2391): 0 은 끄기 "Okay, you'll now tough out fights to the bitter end.",
  최대 체력/2 초과 "You can't set your wimp level above half your hit points.", 그 밖 "Okay, you'll wimp out if you drop below %d hit points."

### 10.2 기술 공통
- 1~101 을 굴려 익힘 % 보다 크면 실패(101 은 언제나 실패). 명령은 익힘 > 0 만 본다: 아니면 "You have no idea how." (명령마다 조금 다르다)
- 익힘 상한: 마법사·성직자 95, 도적 85, 전사 80 (class.c:127-133)
- 배우는 레벨 (class.c:1653-1664): 도적 sneak 1, pick 2, backstab 3, steal 4, hide 5, track 6 / 전사 kick 1, rescue 3, bandage 7, track 9, bash 12, whirlwind 16

### 10.3 kick (act.offensive.c:493-528; 최소 자세 싸움)
- 대상: 이름이 없으면 지금 상대, 없으면 "Kick who?", 자신 "Aren't we funny today..."
- 굴림 = (10 − 대상 AC/10) × 2 + (1~101). 굴림 > 익힘이면 피해 0(빗나감 문구), 아니면 **피해 = 레벨 / 2** (레벨 1 은 0 이라 빗나감 문구)
- 어느 쪽이든 대기 60 펄스(6초)

### 10.4 bash (act.offensive.c:270-330; 최소 자세 싸움)
- 무기를 들어야 한다 "You need to wield a weapon to make it a success.", 평화로운 방 거절, 대상은 kick 과 같이 ("Bash who?")
- 실패: 피해 0, **공격자가 앉는다**. 성공: 피해 1, 대상이 살아 있으면 대상 대기 20 펄스(몹은 한 라운드 쉰다)와 앉음
- 공격자 대기 40 펄스. 넘어뜨릴 수 없는 몹(nobash)은 언제나 실패

### 10.5 rescue (act.offensive.c:332-394; 최소 자세 싸움)
- "Whom do you want to rescue?", 자신 "What about fleeing instead?", 내 상대 "How can you rescue someone you are trying to kill?", 아무도 그를 공격하지 않음 "But nobody is fighting $M!"
- 실패 "You fail the rescue!" (대기 없음). 성공: CHAR "Banzai!  To the rescue...", VICT "You are rescued by $N, you are confused!", NOTVICT "$n heroically rescues $N!".
  공격자가 이제 나와 싸우고, 구해진 사람은 대기 40 펄스, 구한 사람은 대기 없음

### 10.6 backstab (act.offensive.c:127-178; 최소 자세 섬)
- "Backstab who?", 자신 "How can you sneak up on yourself?", 무기 없음 "You need to wield a weapon to make it a success.",
  찌르는 무기(공격 11) 아님 "Only piercing weapons can be used for backstabbing.", 싸우는 대상 "You can't backstab a fighting person -- they're too alert!"
- 눈치 빠른 몹(aware)이 깨어 있으면 들킨다(VICT "You notice $N lunging at you!", CHAR "$e notices you lunging at $m!", NOTVICT "$n notices $N lunging at $m!") 그리고 대상이 한 번 친다
- 깨어 있는 대상이고 굴림 > 익힘이면 빗나감. 아니면 보통 치기(§7.4, 명중 판정 있음; 자는 대상은 언제나 맞고 ×2)에 기습 배율:
  레벨 1-7 ×2, 8-13 ×3, 14-20 ×4, 21-28 ×5, 29-30 ×6. 그래도 100 이 최대(§8.1)
- 대기 40 펄스
- whirlwind(전사 16)는 범위 밖

## 11. 주문·마나·효과

### 11.1 cast (spell_parser.c:463-647)
- 형식 `cast '<주문>' [대상]`: "Cast what where?", 따옴표 없음 "Spell names must be enclosed in the Holy Magic Symbols: '"
- 주문 이름: 전체의 앞부분이거나 단어마다 앞부분("mag mis" = magic missile). 없음 "Cast what?!?", 레벨 부족 "You do not know that spell!", 익힘 0 "You are unfamiliar with that spell."
- 대상: 이름이 있으면 방의 사람 → 세계의 사람 → 소지품 → 장비 → 방의 물건 → 세계의 물건 중 주문이 허락하는 것. 없으면 싸우는 중 공격 주문은 상대, 공격 주문이 아니면 자신,
  아니면 "Upon who should the spell be cast?" (물건 주문이면 "what"). 공격 주문을 자신에게 "You shouldn't cast that on yourself -- could be bad for your health!", 못 찾음 "Cannot find the target of your spell!"
- **마나** = max(최대값 − 감소 × (레벨 − 배우는 레벨), 최소값). 모자라면 "You haven't the energy to cast that spell!" (실패 굴림 전에)
- **실패**: 0~101 > 익힘이면: 대기 20, 대상이 있으면 그 주문의 빗나감 문구, 없으면 "You lost your concentration!", 마나 /2 소모, 공격 주문이면 몹 대상이 공격해 온다
- 성공: 자세 검사(잠 "You dream about great magical powers.", 쉼 "You cannot concentrate while resting.", 앉음 "You can't do this sitting!",
  싸움 "Impossible!  You can't concentrate enough!") → "Okay." → 주문 소리(§11.2) → 효과 → 대기 20, 마나 전부 소모
- 마법 금지 방 "Your magic fizzles out and dies." / ROOM "$n's magic fizzles out and dies.", 평화로운 방의 공격 주문 "A flash of white light fills the room, dispelling your violent magic!"

### 11.2 주문 소리 (spell_parser.c:43-131)
- 같은 직업은 주문 이름을, 다른 사람은 음절표(§16)로 바꾼 말을 듣는다. 깨어 있는 사람만, 시전자와 대상 빼고:
  "$n closes $s eyes and utters the words, '%s'." (자신에게) / "$n stares at $N and utters the words, '%s'." (방의 다른 사람에게) /
  "$n stares at $p and utters the words, '%s'." (물건에게) / "$n utters the words, '%s'."
- 대상은 "$n stares at you and utters the words, '%s'." 시전자 자신은 "Okay." 만

### 11.3 1단계 주문 (spell_parser.c:744-877, class.c:1592-1635, magic.c)
| 주문 | 마나 최대/최소/감소 | 자세 | 배움 | 효과 |
|---|---|---|---|---|
| magic missile | 25/10/3 | 싸움 | 마법사 1 | 피해 1d8+1 (마법사 외 1d6+1), 레벨과 무관 |
| cure light | 30/10/2 | 싸움 | 성직자 1 | 체력 1d8 + 1 + 레벨/4 (최대까지), "You feel better." |
| armor | 30/15/3 | 싸움 | 성직자 1, 마법사 4 | AC −20, 24틱, 겹치면 시간만 더해진다. "You feel someone protecting you.", 끝 "You feel less protected." |
| create food | 30/5/4 | 섬 | 성직자 2 | 소지품에 waybread(물건 0번 존 vnum 10, 배부름 24), ROOM "$n creates $p." CHAR "You create $p." |
| create water | 30/5/4 | 섬 | 성직자 2 | 물통을 물로 가득 "$p is filled." (다른 액체가 있으면 조용히 슬라임 곰팡이 즙이 된다) |
| poison | 50/20/3 | 섬 | 성직자 8, 마법사 14 | 대상이 저항 못 하면 힘 −2, 독, 시전자 레벨 틱. "You feel very sick." / ROOM "$n gets violently ill!", 끝 "You feel less sick." |
| remove poison | 40/8/4 | 섬 | 성직자 10 | 독 제거 "A warm feeling runs through your body!" / ROOM "$n looks better." |
- 이미 걸린 효과(겹치지 않는 것) "Nothing seems to happen."
- 피해 주문(magic missile)은 대상이 저항(§11.1 의 저항 굴림, 주문 종류)하면 **피해가 절반**(magic.c:288-289). 저항 굴림: 직업 표(몹은 전사 표)[종류][레벨] + 몹 파일의 저항 + 저항 보정 apply, max(1, 값) < 0~99 면 저항 성공(magic.c:36-55)
- 음식·물의 독은 기존 독을 **바꾼다**(시간을 더하지 않는다, affect_join 의 add_dur 거짓). 주문 표·직업 레벨은 `third_party/tbamud/tables/spells.yaml`
- 더 높은 레벨의 주문은 파티가 그 레벨에 닿을 때 이 표에 더한다

### 11.4 효과와 독 (magic.c:58-79, limits.c:399-405)
- 효과는 틱마다 1 줄고, 0 인 효과는 **다음 틱**에 사라진다(그 주문의 마지막 효과일 때만 끝 문구). −1 은 영원
- 독: 틱마다 회복 뒤 피해 2(VICT "You feel burning poison in your blood, and suffer." / NOTVICT "$N looks really sick and shivers uncomfortably."), 회복 ÷4(§5.2)
- 독 음식·물(§6.2, §6.3), 뱀 몹의 독(spec_procs.c:343-355)도 같은 독이다. 물통 독은 양 × 3 틱, 음식은 양 × 2 틱

## 12. 그룹·따라가기·연습

### 12.1 그룹 (act.other.c:388-507, handler.c:1596-1690)
- 따라가기(§2.5)와 그룹은 **따로**다. 이동은 따라가기가, 경험치 나누기·거들기·금화 나누기는 그룹이 정한다
- `group new` (이미 그룹 "You are already in a group."), `group join <사람>` ("Join who?", "But you are already part of a group.", "That group isn't accepting members."),
  `group leave`, `group kick <사람>`(우두머리만), `group option open|anonymous`, `group` (목록 "Your group consists of:")
- 들어오면 그룹에 "%s joins the group.", 나가면(나가는 사람 포함) "%s has left the group.", 우두머리가 나가면 무작위 구성원이 "%s has assumed leadership of the group."
- `report`: 그룹에 "%s reports: %d/%dH, %d/%dM, %d/%dV"

### 12.2 split (act.other.c:509-573)
- 같은 방의 그룹 플레이어 수 n(나 포함), 몫 = 금액/n, 나머지는 내가 갖는다. 받는 사람 "%s splits %d coins; you receive %d.", 나 "You split %d coins among %d members -- %d coins each."
- 거절: "How many coins do you wish to split with your group?", "Sorry, you can't do that.", "You don't seem to have that much gold to split.", "With whom do you wish to share your gold?"

### 12.3 자동 설정 (act.other.c:735-744, fight.c:784-813)
- autoloot autogold autosplit autosac autoassist: "Autoloot enabled." / "Autoloot disabled." 꼴
- 죽였을 때 순서: (그룹 + 자동 나누기 + 금화 > 0) 시체의 금화 줍고 나누기, 아니면 자동 금화면 금화 줍기 → 자동 줍기면 시체의 모든 것 → 자동 바치기(몹 시체만)

### 12.4 연습 (spec_procs.c:49-188, act.other.c:288-301)
- 길드 밖 `practice`: 인자 없으면 목록, 있으면 "You can only practice skills in your guild."
- 길드 주인은 **직업을 가리지 않는다**: 연습 0 "You do not seem to be able to practice now.", 모르는 것 "You do not know of that spell." / "... skill.",
  이미 상한 "You are already learned in that area.", 성공 "You practice for a while...", 상한에 닿으면 "You are now learned in that area."
- 한 번에 오르는 양 = min(직업 최대, max(직업 최소, 지능 학습값)). 마법사·성직자 최대 100 최소 25, 도적·전사 최대 12 최소 0.
  지능 학습값 0..25: 3,5,7,8,9,10,11,12,13,15,17,19,22,25,30,35,40,45,50,53,55,56,57,58,59,60
- 목록: "You have %d practice session%s remaining." "You know of the following %ss:" 다음 이름순, 줄마다 "%-20s %s" 와 숙련 말:
  0 "(not learned)", ≤10 "(awful)", ≤20 "(bad)", ≤40 "(poor)", ≤55 "(average)", ≤70 "(fair)", ≤80 "(good)", ≤85 "(very good)", 그 위 "(superb)"

## 13. 물건과 장비

### 13.0 이름으로 찾기 (handler.c:89-111, 656-700, 1310-1333, 1584-1593; S5 에서 추가)
- 낱말이 키워드 하나의 **앞부분**이면 맞는다(`bre` = bread). 숫자로 시작하는 낱말은 키워드 전체와 같아야 한다
- `2.bread` 두 번째, `last.bread` 목록에서 가장 뒤의 것, `0.bread` 아무것도 아님. `all` 전부, `all.bread` 맞는 것 전부(`all.` 만이면 "... all of what?")
- 보이는 것만 찾는다(§3.5 의 물건 보기: 빛 조건 포함 — **어둠 속에서는 자기 소지품도 이름으로 집을 수 없다**, utils.h:790-832)
- 목록의 순서: 소지품·그릇 안은 새것이 **앞**(handler.c:488-489, 835-836), 방의 바닥은 새것이 **뒤**(이 tbaMUD 의 obj_to_room 은 끝에 붙인다, handler.c:772-793). 그래서 `last.corpse` 가 방금 생긴 시체다
- 명령 이름: tbaMUD 명령표(`third_party/tbamud/tables/commands.yaml`, interpreter.c cmd_info)의 순서로, 친 낱말로 **시작하는** 첫 명령, 레벨이 모자라는 명령은 건너뛴다(`go` 는 레벨 31 의 goto 를 지나 gold). 소셜은 그 다음(interpreter.c:520-530)
- Mundi 가 아직 하지 않는 tbaMUD 명령은 "Huh!?!" 가 아니라 "아직 없다"로 답한다(1단계가 끝나면 없어진다)

 (utils.h:670-672, constants.c:607-638)
- 무게 한도 = 힘 표의 carry_w (힘 0..25: 0,3,3,10,25,55,80,90,100,100,115,115,140,140,170,170,195,220,255,640,700,810,970,1130,1440,1750; 18/xx 인덱스 26-30: 280,305,330,380,480)
- 개수 한도 = 5 + 민첩/2 + 레벨/2
- 들 수 있는 무기 무게 = 힘 표의 wield_w

### 13.2 get / put / drop / give (act.item.c:53-761)
- get: 들 수 없음 "$p: you can't take that!", 개수 "$p: you can't carry that many items.", 무게 "$p: you can't carry that much weight."
  방에서 "You get $p." / ROOM "$n gets $p.", 그릇에서 "You get $p from $P." / "$n gets $p from $P.", 닫힌 그릇 "$p is closed.", 금화는 줍자마자 돈으로 "There were %d coins." ("There was 1 coin.")
  그 밖 "Get what?", "You don't see %s %s here.", "$p seems to be empty.", "$p is not a container."
- put: "$p is not a container.", 닫힘 "You'd better open it first!", 시체 거절(§8.4), 용량(값0 > 0 일 때 그릇 무게 + 물건 무게 > 값0) "$p won't fit in $P.", 성공 "You put $p in $P." / "$n puts $p in $P."
- drop: 저주(nodrop) "You can't drop $p, it must be CURSED!", 성공 "You drop $p." / "$n drops $p."
- give: 저주 "You can't let go of $p!!  Yeech!", 받는 쪽 개수 "$N seems to have $S hands full.", 무게 "$E can't carry that much weight.",
  성공 CHAR "You give $p to $N." VICT "$n gives you $p." NOTVICT "$n gives $p to $N."
- junk, donate 는 범위 밖

### 13.3 입기 (act.item.c:1218-1565, handler.c:557-603)
- 물건 레벨 > 내 레벨 "You are not experienced enough to use that."
- 자리와 필요한 착용 표시: 빛(들 수 있는 것), 손가락 둘, 목 둘, 몸, 머리, 다리, 발, 손, 팔, 방패, 두름(about), 허리, 손목 둘, 무기(wield), 쥠(hold). 둘인 자리는 첫째가 차면 둘째
- `wear <물건>` 자리 자동 선택: 손가락 목 몸 머리 다리 발 손 팔 방패 두름 허리 손목 순서에서 **마지막으로 맞는 것**. 무기·쥠·빛은 고르지 않는다. 틀린 자리 "You can't wear $p there."
- `wield`: "You can't wield that.", 무게 > 무기 한도 "It's too heavy for you to use." 성공 "You wield $p."
- `hold`/`grab`: 빛이면 빛 자리 "You light $p and hold it.", 아니면 쥘 수 있는 것 "You grab $p.", 아니면 "You can't hold that."
- 정렬·직업 금지(정렬 ±350): 입은 **뒤에** "You are zapped by $p and instantly let go of it." / ROOM "$n is zapped by $p and instantly lets go of it." 소지품으로 돌아간다
- 양손 무기 개념은 없다
- `remove`: 저주 "You can't remove $p, it must be CURSED!", 개수 "$p: you can't carry that many items!", 성공 "You stop using $p." / "$n stops using $p."
- 입은 것의 효과(apply)는 입는 동안 더해진다. 방어구는 §7.4

### 13.4 물건 시간 (limits.c:435-473, handler.c:985-1020)
- 빛: §3.1. 시체: §8.4
- 다른 물건의 시간은 줄기만 하고 저절로 사라지지 않는다(스크립트 없음). 플레이어가 가진 시간 있는 물건은 한 틱에 2~3 씩 준다(두 군데서 뺀다, 의도가 아닌 듯: **Mundi 는 1**, 차이로 기록)

## 14. 존 리셋과 몹 행동

### 14.1 존 리셋 (db.c:2776-3120)
- 1분마다 리셋 방식이 never 가 아닌 존의 나이 +1, 나이 ≥ 주기(분)이면 줄에 넣는다. 10초마다 줄에서 **하나만** 리셋: 방식 always, 또는 when_empty 이고 비었을 때
- 비었다 = 게임 중인 플레이어가 그 존에 없다(몹·끊긴 연결은 세지 않는다). 부팅 때 모든 존을 리셋
- 리셋 명령(Mundi 형식으로는 `resets.yaml`, D16):
  - 몹: 세계 전체의 그 몹 수 < 한도이면 방에 놓는다. 그 몹에게 장비(입히기)와 소지품
  - 물건: 세계 전체의 그 물건 수 < 한도이면 방에(또는 그릇 안에: 가장 최근에 만든 그 그릇)
  - 앞 명령에 달린 것은 앞 명령이 성공했을 때만. 몹이 안 나오면 그 몹의 장비도 없다
  - 방의 물건 지우기(remove): 그 방에 그 물건이 있으면 하나
  - 문: 그 방 쪽만 열림·닫힘·잠김으로(반대쪽은 그쪽 리셋이 한다)
- **있는 것은 건드리지 않는다**: 살아 있는 몹·놓인 물건을 다시 만들거나 치우지 않는다. 한도는 살아 있는 수로만 정해진다
  그래서 always 존은 아무도 죽지 않아도 리셋마다 몹이 **한도까지 늘어난다**(Midgaard 의 한도 5 인 몹은 1 에서 15분마다 하나씩). 시험으로 확인(S5)
- Mundi 의 순서: 지우기 → 놓기(파일 순서) → 문. tbaMUD 는 한 목록에서 섞어 하지만 지우기는 보통 그것을 다시 놓는 줄 바로 앞에 있다
- 리셋 뒤 나이 0

### 14.3 트리거로 된 행동 (S5 에서 발견)
- 이 tbaMUD 는 길드 경비·fido·janitor·cityguard·snake·thief·magic_user·puff 를 **DG 스크립트**(lib/world/trg, 몹에 붙은 트리거)로 한다(spec_assign.c:62-63). spec_assign.c 의 코드 배정은 길드 주인(guild) 등 몇 개뿐
- Mundi 1단계 (사용자 결정 a): 스크립트 계층 없이, 파티가 만나는 트리거를 **엔진의 일반 행동** + **콘텐츠 데이터**로 한다(원칙 6, D9).
  엔진(`crates/mundi-sim/src/triggers.rs`)은 행동만 안다. 어느 몹·방이 하는지, 무엇을 주는지, 무슨 말을 하는지, 수치는 `third_party/tbamud/tables/triggers.yaml`,
  그 문구의 한국어는 `locales/ko/triggers.yaml` (사건이 문구의 키를 함께 보내 렌더러가 고른다, D22 추가). 2단계 스크립트 계층의 첫 재료
  - `guild_guard`: 30.trg #3000-#3003 = class.c guild_info (마법사 3017 남, 성직자 3004 북, 도적 3027 동, 전사 3021 동; 경비가 깨어 있고 볼 때 그 직업만)
  - `zone_welcome` (로그인, #3017): `delay` 초 뒤 존 전체에 `welcome` 문구. 스크립트의 레벨 0 장비는 do_start 가 먼저 레벨 1 로 만들어 돌지 않는다(실서버 확인)
  - `outfit_newcomers` (인사, #3016): `below_level` 미만 플레이어가 들어오면 `delay` 초 뒤, 아무것도 안 입었으면 `kit` 전부를 입히고 `full` 문구, 아니면 빠진 첫 조각을 그 줄의 말과 함께 준다
  - 무작위 (13초마다, 플레이어가 있는 존의 몹, `chance`%): `guard` (#3009: 보이는 하나를 골라 매력 `below_charisma` 미만이면 침, 공격자보다 정렬이 높고 0 이상인 피해자 편에 끼어든다),
    `eat_corpses` (#3010: 첫 시체), `pick_up_litter` (#3011: 분수가 아니고 가격 `max_cost` 이하인 것 모두)
  - `reward_drops` (방의 drop, #3004): 버린 물건을 가져가고 `reward` (가격/per, min~max) 를 `below_level` 미만이면 경험치, 아니면 돈으로

### 14.2 몹 행동 (mobact.c:41-197, 10초마다, 이 순서로)
1. 특수 동작(상점·길드·뱀 등)이 있으면 하고, 했으면 이번은 끝
2. 싸우거나 깨어 있지 않으면 끝
3. 추적(hunt; 범위 밖)
4. 줍는 몹(scavenger): 1/11 확률로 방의 가장 비싼(가격 > 1) 들 수 있는 물건을 줍는다, ROOM "$n gets $p."
5. 배회(§2.6)
6. 공격(§7.5) → 7. 기억 → 8. 매혹 반항 → 9. 돕기

## 15. 상점 (shop.c)

### 15.1 언제 거래하나 (shop.c:107-166, 948-1026)
- 손님이 상점 방에 있고 주인이 깨어 있어야 한다. 명령: buy sell value list (identify 는 범위 밖)
- 영업시간(게임 시): 열기 전 "Come back later!", 첫 영업 끝과 둘째 영업 사이 "Sorry, we have closed, but come back later.", 다 끝남 "Sorry, come back tomorrow."
- 주인이 손님을 못 보면 "I don't trade with someone I can't see!", 정렬 거부 "Get out of here before I call the guards!", 직업 거부 "We don't serve your kind here!"
- 주인 공격: 상점이 싸움을 허락하지 않으면 "Get out of here before I call the guards!" 그리고 피해 없음(§8.1-2)

### 15.2 가격 (shop.c:456-473)
- 매력 보정 c = (주인 매력 − 손님 매력) / 70 (실수)
- 사는 값 = 물건 가격 × 판매 배율 × (1 + c), 0 쪽으로 버림
- 파는 값 = 물건 가격 × min(구입 배율 × (1 − c), 판매 배율 × (1 + c)), 0 쪽으로 버림 (되팔아 돈을 벌 수 없다)
- 변환된 `shops.yaml` 의 profit 이 이 두 배율이다
- 정밀도: 배율은 C `float` (shop.h:35-36) 이라 단정도로 계산한다. 사는 값은 전부 단정도 (shop.c:458-459), 파는 값은 두 배율을 배정도로 곱한 뒤 단정도로 저장하고 가격 × 배율을 단정도로 (shop.c:466-472). 배정도로 하면 값이 1 낮게 나오는 물건이 있다: 실서버에서 마법 상점의 gnarled staff (가격 700, 배율 1.15, c = 4/70) 가 851 인데 배정도는 850.9999 → 850 (2026-10-03, 테스트 캐릭터, 매력 7 로 상점 셋 16 품목과 되파는 값 1개가 모두 맞음)

### 15.3 list / buy / sell / value (shop.c:475-936)
- list: 머리 " ##   Available   Item                                               Cost" 와 대시 76개, 줄마다 " %2d)  %9s   %-48s %6d", 같은 물건은 묶는다.
  늘 파는 것(products)은 "Unlimited". 물통은 "<이름> of <액체>". 비면 "Currently, there is nothing for sale."
- buy `buy [n] <이름|#번호>`: "What do you want to buy??", 없음 → 상점 문구 no_such_item1, 돈 부족 → missing_cash2 (그리고 성질 temper 에 따라 소셜),
  개수·무게 "%s: You can't carry any more items." / "%s: You can't carry that much weight." 성공 ROOM "$n buys %s.", 주인 message_buy, 나 "You now have %s."
  늘 파는 것은 새로 만들고, 그 밖은 재고에서 뺀다
- sell: "What do you want to sell??", 없음 no_such_item2, 가격 < 1 "You've got to be kidding, that thing is worthless!", 사지 않는 종류 → do_not_buy,
  주인 돈 부족 → missing_cash1. 성공 ROOM "$n sells %s.", 주인 message_sell, 나 "The shopkeeper now has %s." 늘 파는 것의 복사본은 사라지고, 그 밖은 재고가 된다
- 사는 종류 + 키워드 식(| 또는, & 그리고, ^ 아님, 괄호. 물건 표시 이름이면 그 표시, 아니면 키워드). 변환된 `shops.yaml` 의 `buys`
- value: "I'll give you %d gold coins for that!"
- 상점 문구(%s 손님, %d 값)는 상점마다 콘텐츠에 있다(변환됨)

## 17. 캐릭터 만들기와 주사위 (S5 에서 추가)

### 17.1 주사위 (utils.c rand_number, dice)
- `rand(lo, hi)`: lo 와 hi 를 포함한 정수 하나, 고르게. `dice(n, s)`: rand(1, s) 를 n 번 더한 것(n 이나 s 가 0 이하면 0)
- Mundi 의 모든 굴림은 시뮬레이션의 RNG 하나(ChaCha, 시드)에서 나온다. tbaMUD 와 **같은 수열은 아니다**(분포가 같다): 비교는 분포로 한다(S5 비교)

### 17.2 새 캐릭터 (class.c:1365-1477, interpreter.c:1655)
- 직업을 고른다: magic_user, cleric, thief, warrior. 시작 물건은 **없다**
- 능력치: 4d6 에서 가장 낮은 하나를 뺀 합을 여섯 번 굴려 큰 것부터 줄 세우고, 직업 순서로 나눈다 (class.c:1374-1426):

| 직업 | 1등 | 2등 | 3등 | 4등 | 5등 | 6등 |
|---|---|---|---|---|---|---|
| 마법사 | 지능 | 지혜 | 민첩 | 힘 | 체질 | 매력 |
| 성직자 | 지혜 | 지능 | 힘 | 민첩 | 체질 | 매력 |
| 도적 | 민첩 | 힘 | 체질 | 지능 | 지혜 | 매력 |
| 전사 | 힘 | 민첩 | 체질 | 지혜 | 지능 | 매력 |

  추가 힘(18/xx)은 0, 단 전사의 힘이 18 이면 rand(0, 100)
- 그 다음은 §9.4 의 새 캐릭터(레벨 1, 경험치 1, 체력 10·마나 100·이동력 82, 도적 기술, 레벨 오름 한 번, 모두 가득) 그리고 §6.1 (배부름·갈증 24, 술 0)
- 나이 = 태어난 뒤 지난 **게임** 해 + 17 (utils.c:543-551; 게임 한 해 = 17달 × 35일 × 24시 × 75초). tbaMUD 는 실제 시각으로 재지만 Mundi 의 시간은 시뮬레이션 틱이다(§1.1 의 차이와 같은 까닭)

### 17.3 몹 (db.c:1680-1790, 2690-2720)
- 능력치는 몹 파일의 값, 없으면 11 (db.c:1687-1692). 체력은 파일의 주사위를 **만들 때 한 번** 굴린다: dice(n, s) + b, 최대 = 현재 (db.c:2704-2710). 최대 마나 10, 최대 이동력 50 (db.c:1716-1717)
- 금화·경험치·정렬·레벨·기본 자세·맨손 공격·AC·명중·피해 주사위는 파일 그대로(§7.4 의 몹 THAC0)
- 몹은 배고프지도 목마르지도 않는다(조건 -1)


순수 숫자표는 이 문서에 다 적지 않고, 원본에서 **값만** 옮겨 `third_party/tbamud/tables/` 의 `abilities.yaml`·`classes.yaml`·`world.yaml` 로 둔다. 엔진은 시작할 때 그 파일을 읽는다(D9, D21).
엔진 크레이트(AGPL)에는 tbaMUD 에서 온 표가 들어가지 않는다. 표를 바꿔 실험할 때도 파일만 바꾸면 된다.
대상: 힘(명중·피해·carry_w·wield_w, constants.c:607-638), 민첩(방어·기술 보정, constants.c:643-700), 체질(constants.c:706+), 지능 학습(constants.c:736-763), 지혜(constants.c:767-794),
직업 THAC0(class.c:1190-1358), 레벨표(§9.3), 저항(saving throws, class.c), 주문 음절(spell_parser.c:43-56), 액체(§6.3), 지형(§2.3)

## 18. score (act.informative.c:886-1025, S6 에서 추가)
플레이어만. 이 순서의 줄:
1. `You are N years old.` — 나이(§17.2). 게임 달·날이 0 이면(태어난 날) 같은 줄에 `  It's your birthday today.` (게임 시간: 시간 = 틱, 하루 24시간, 한 달 35일, 1년 17달, utils.c mud_time_passed)
2. `You have H(Hmax) hit, M(Mmax) mana and V(Vmax) movement points.`
3. `Your armor class is AC/10, and your alignment is A.` — AC 는 §7.4 compute_armor_class
4. `You have E exp, G gold coins, and Q questpoints.` — Mundi 에는 퀘스트가 없어 Q = 0
5. 레벨 31 미만이면 `You need N exp to reach your next level.` — N = level_exp(직업, 레벨+1) − 경험치
6. `You have earned Q quest points.` / `You have completed 0 quests, and you are not on a quest at the moment.`
7. `You have been playing for D day(s) and H hour(s).` — 게임 안에서 보낸 실제 시간(`lived`: Mundi 의 나이와 같은 시계). 하루 86400초 (utils.c real_time_passed)
8. `This ranks you as NAME TITLE (level L).` — 칭호는 class.c title_male/title_female (여자면 female), 레벨 0 이하·구현자는 함수 앞의 답, 직업 표에 없는 레벨은 그 직업의 default (전사 21-30: "the Warrior"). `tables/classes.yaml` titles
9. 자세: DEAD / mortally wounded / incapacitated / stunned / sleeping / resting / sitting / `fighting X` (보이는 이름, 없으면 thin air) / standing
10. 상태 (있는 것만, 이 순서): 취함(drunk > 10), 배고픔(full 0), 목마름(thirst 0), 실명(불멸 미만), 투명, 투명 감지, 성역, 독, 매혹, armor 주문, 적외선 시야, summonable(Mundi 에는 없음)
- 사건: `char.vitals_max {hp, mp, mv}` 와 `char.score` (위의 값 전부; 문장은 렌더러). anima PROTOCOL.md §3

## 미확인 (구현 전에 확인하거나 시험으로 정할 것)
- 죽음의 방에서 시체가 남는가 (§2.4)
- 몹 THAC0 의 +2 (§7.4), 한 몹이 한 번의 행동에서 공격과 기억 공격을 둘 다 하는가 (§14.2)
- 저항 표 값, 주문 문구 파일(§8.2)의 형식

---

## Mundi 가 일부러 다르게 하는 것
| 절 | tbaMUD | Mundi | 이유 |
|---|---|---|---|
| §1.1 | 접속이 없으면 시간이 멈춘다 | 멈추지 않는다 | 에이전트·관전·시간 가속(로드맵 3) |
| §3.6 | 문장을 바로 소켓에 쓴다 | 사건 → 지각 필터 → 이벤트/문장 | 원칙 1·2, D6, D13 |
| §7.3 | 거들기가 이름으로 대상을 찾는다(같은 이름의 다른 사람을 고를 수 있다) | ID 로 정확히 그 사람 | 이름 겹침은 규칙이 아니라 구현의 우연 |
| §13.4 | 플레이어가 가진 시간 있는 물건이 한 틱에 2~3 줄어든다(두 군데서 뺀다) | 한 틱에 1 | 의도가 아닌 이중 감소로 보인다 |
| §8.4 | 시체 무게 = 몸무게 + 소지품 | 소지품만 | 콘텐츠에 몸무게가 없다(1단계에서 시체 무게를 쓰는 규칙이 없다) |
| §8.3 | 죽은 플레이어는 메뉴로, 1 을 고르면 다시 들어온다 | 메뉴 없이 바로 시작 방에서 다시 들어온다 | 메뉴는 화면의 일(승인된 규칙) |
