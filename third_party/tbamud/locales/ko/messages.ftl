# tbaMUD-derived (third_party/tbamud/NOTICE.md): tbaMUD's game messages in Korean, the same IDs as
# ../en/messages.ftl. Plain declarative style (해라체), like the room translations.
# Particles: { JOSA($who, "이/가") } picks by the word before it (D23). Directions are Korean words
# (dir-*); what to type stays English (D17), shown in parentheses where it can be typed (D18).

## connection
in-game-entered = tbaMUD에 온 것을 환영한다!  깨달음이 있는 여행이 되기를...
in-game-reconnected = 다시 접속했다.
closed-quit = 잘 가게, 친구.. 곧 다시 오게!
login-name = 어떤 이름으로 불리고 싶은가?
login-password = 비밀번호:
login-invalid-name = 쓸 수 없는 이름이다. 다른 이름을 골라라.
login-wrong-password = 비밀번호가 틀렸다.

## directions
dir-north = 북쪽
dir-east = 동쪽
dir-south = 남쪽
dir-west = 서쪽
dir-up = 위쪽
dir-down = 아래쪽
exit-north = 북
exit-east = 동
exit-south = 남
exit-west = 서
exit-up = 위
exit-down = 아래

## rooms [3.2]
room-dark = 칠흑같이 어둡다...
room-blind = 끝없는 어둠 말고는 아무것도 보이지 않는다...
exits-label = 출구
exits-none = 없음!
occupant-standing = { $who }{ JOSA($who, "이/가") } 여기 서 있다.
occupant-sitting = { $who }{ JOSA($who, "이/가") } 여기 앉아 있다.
occupant-resting = { $who }{ JOSA($who, "이/가") } 여기서 쉬고 있다.
occupant-sleeping = { $who }{ JOSA($who, "이/가") } 여기서 자고 있다.
occupant-fighting = { $who }{ JOSA($who, "이/가") } 여기서 싸우고 있다!
occupant-stunned = { $who }{ JOSA($who, "이/가") } 기절해 쓰러져 있다.
occupant-incapacitated = { $who }{ JOSA($who, "이/가") } 움직이지 못하고 쓰러져 있다.
occupant-mortally-wounded = { $who }{ JOSA($who, "이/가") } 죽어 가며 쓰러져 있다.
occupant-dead = { $who }{ JOSA($who, "이/가") } 죽어 쓰러져 있다.
flag-linkless = (연결 끊김)

## movement [2.2, 2.4]
move-no-exit = 아쉽게도 그쪽으로는 갈 수 없다...
move-closed-door = { $door }{ JOSA($door, "이/가") } 닫혀 있는 것 같다.
move-closed = 닫혀 있는 것 같다.
move-exhausted = 너무 지쳤다.
move-forbidden = 알 수 없는 장벽이 당신을 밀어낸다! 들어갈 수 없는 곳이다.
zone-above-level = 이 지역은 당신의 레벨에 비해 위험하다.
arrived = { $who }{ JOSA($who, "이/가") } 왔다.
arrived-entered-game = { $who }{ JOSA($who, "이/가") } 게임에 들어왔다.
left = { $who }{ JOSA($who, "이/가") } { $dir }{ JOSA($dir, "으로/로") } 떠났다.
left-game = { $who }{ JOSA($who, "이/가") } 게임을 떠났다.
link-lost = { $who }의 연결이 끊겼다.
link-reconnected = { $who }{ JOSA($who, "이/가") } 다시 접속했다.

## talking
say-out = 당신은 말한다, '{ $text }'
say-in = { $who }{ JOSA($who, "이/가") } 말한다, '{ $text }'

## refusals [4.2]
refused-dead = 가만히 있어라. 당신은 죽었다!!! :-(
refused-incapacitated = 상태가 너무 나빠 아무것도 할 수 없다!
refused-stunned = 지금 할 수 있는 건 별을 세는 것뿐이다!
refused-sleeping = 꿈에서나?
refused-resting = 에이... 너무 느긋해서 그럴 기분이 아니다..
refused-sitting = 먼저 일어서는 게 좋겠다.
refused-fighting = 안 된다!  목숨을 걸고 싸우는 중이다!
refused-unknown-command = 뭐라고?!
refused-nothing-to-say = 그래서, 무슨 말을 하고 싶은가?
refused-quit-in-full = 나가려면 quit 를 끝까지 쳐야 한다!
refused-invalid-target = 여기엔 그런 것이 없다.

## time [1.2]
time-sunrise = 해가 동쪽에서 떠오른다.
time-day = 낮이 시작되었다.
time-sunset = 해가 서쪽으로 천천히 사라진다.
time-night = 밤이 시작되었다.

## prompt
prompt = { $hp }H { $mp }M { $mv }V >

## positions [4.3]
pos-standing-sitting = 당신은 일어선다.
pos-room-standing-sitting = { $who }{ JOSA($who, "이/가") } 몸을 일으켜 선다.
pos-standing-resting = 당신은 쉬기를 그만두고 일어선다.
pos-room-standing-resting = { $who }{ JOSA($who, "이/가") } 쉬기를 그만두고 몸을 일으켜 선다.
pos-sitting-standing = 당신은 앉는다.
pos-room-sitting-standing = { $who }{ JOSA($who, "이/가") } 앉는다.
pos-sitting-resting = 당신은 쉬기를 그만두고 일어나 앉는다.
pos-room-sitting-resting = { $who }{ JOSA($who, "이/가") } 쉬기를 그만둔다.
pos-resting-standing = 당신은 앉아서 지친 몸을 쉰다.
pos-room-resting-standing = { $who }{ JOSA($who, "이/가") } 앉아서 쉰다.
pos-resting-sitting = 당신은 지친 몸을 쉰다.
pos-room-resting-sitting = { $who }{ JOSA($who, "이/가") } 쉰다.
pos-sleeping = 당신은 잠이 든다.
pos-room-sleeping = { $who }{ JOSA($who, "이/가") } 누워 잠이 든다.
pos-sitting-sleeping = 당신은 잠에서 깨어 일어나 앉는다.
pos-room-sitting-sleeping = { $who }{ JOSA($who, "이/가") } 잠에서 깬다.
pos-awakened-by = { $who }{ JOSA($who, "이/가") } 당신을 깨웠다.
refused-stand-already = 이미 서 있다.
refused-stand-asleep = 먼저 잠에서 깨야 한다!
refused-stand-fighting = 싸우는 게 서 있는 게 아니면 뭔가?
refused-sit-already = 이미 앉아 있다.
refused-sit-asleep = 먼저 잠에서 깨야 한다.
refused-sit-fighting = 싸우는 중에 앉겠다고? 제정신인가?
refused-rest-already = 이미 쉬고 있다.
refused-rest-asleep = 먼저 잠에서 깨야 한다.
refused-rest-fighting = 싸우는 중에 쉬겠다고?  제정신인가?
refused-sleep-already = 이미 깊이 잠들어 있다.
refused-sleep-fighting = 싸우는 중에 자겠다고?  제정신인가?
refused-wake-already = 이미 깨어 있다...
refused-wake-asleep = 당신부터 잠에서 깨는 게 좋겠다.
refused-wake-magic = 깨어날 수가 없다!
woke = { $who }{ JOSA($who, "을/를") } 깨웠다.
wake-failed-already-awake = { $who }{ JOSA($who, "은/는") } 이미 깨어 있다.
wake-failed-magic = { $who }{ JOSA($who, "을/를") } 깨울 수가 없다!
wake-failed-bad-shape = { $who }{ JOSA($who, "은/는") } 상태가 너무 나쁘다!
refused-not-here = 그런 이름을 가진 이는 여기 없다.

## perception [3.4]
glowing-eyes = 붉게 빛나는 한 쌍의 눈이 당신 쪽을 보고 있다.
flag-invisible = (투명)
flag-hidden = (숨음)

## conditions [6.1]
cond-hungry = 배가 고프다.
cond-thirsty = 목이 마르다.
cond-sober = 술이 깼다.

## lights [3.1]
light-flicker-self = 빛이 깜빡이며 희미해지기 시작한다.
light-flicker = { $who }의 빛이 깜빡이며 희미해지기 시작한다.
light-out-self = 빛이 지직거리다 꺼져 버렸다.
light-out = { $who }의 빛이 지직거리다 꺼져 버렸다.

## objects in a list [3.3]
obj-flag-invisible = (투명)
obj-flag-glow = ..은은한 빛을 두르고 있다!
obj-flag-hum = ..희미하게 웅웅거리는 소리를 낸다!

## objects in hand [13]
got = { $p }{ JOSA($p, "을/를") } 집었다.
got-from = { $c }에서 { $p }{ JOSA($p, "을/를") } 꺼냈다.
room-get = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 집는다.
room-get-from = { $who }{ JOSA($who, "이/가") } { $c }에서 { $p }{ JOSA($p, "을/를") } 꺼낸다.
coins-one = 동전이 1개 있었다.
coins = 동전이 { $n }개 있었다.
used-drop = { $p }{ JOSA($p, "을/를") } 내려놓았다.
room-drop = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 내려놓는다.
used-put = { $p }{ JOSA($p, "을/를") } { $c }에 넣었다.
room-put = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } { $c }에 넣는다.
gave = { $c }에게 { $p }{ JOSA($p, "을/를") } 주었다.
received = { $who }{ JOSA($who, "이/가") } 당신에게 { $p }{ JOSA($p, "을/를") } 준다.
room-give = { $who }{ JOSA($who, "이/가") } { $c }에게 { $p }{ JOSA($p, "을/를") } 준다.
used-remove = { $p }{ JOSA($p, "을/를") } 벗었다.
room-remove = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 벗는다.
used-wield = { $p }{ JOSA($p, "을/를") } 무기로 들었다.
room-wield = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 무기로 든다.
used-hold = { $p }{ JOSA($p, "을/를") } 손에 쥐었다.
room-hold = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 손에 쥔다.
used-light = { $p }에 불을 붙여 들었다.
room-light = { $who }{ JOSA($who, "이/가") } { $p }에 불을 붙여 든다.
used-wear-finger_right = { $p }{ JOSA($p, "을/를") } 오른손 약지에 끼웠다.
room-wear-finger_right = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 오른손 약지에 낀다.
used-wear-finger_left = { $p }{ JOSA($p, "을/를") } 왼손 약지에 끼웠다.
room-wear-finger_left = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 왼손 약지에 낀다.
used-wear-neck_1 = { $p }{ JOSA($p, "을/를") } 목에 걸었다.
room-wear-neck_1 = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 목에 건다.
used-wear-neck_2 = { $p }{ JOSA($p, "을/를") } 목에 걸었다.
room-wear-neck_2 = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 목에 건다.
used-wear-body = { $p }{ JOSA($p, "을/를") } 몸에 입었다.
room-wear-body = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 몸에 입는다.
used-wear-head = { $p }{ JOSA($p, "을/를") } 머리에 썼다.
room-wear-head = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 머리에 쓴다.
used-wear-legs = { $p }{ JOSA($p, "을/를") } 다리에 입었다.
room-wear-legs = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 다리에 입는다.
used-wear-feet = { $p }{ JOSA($p, "을/를") } 발에 신었다.
room-wear-feet = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 발에 신는다.
used-wear-hands = { $p }{ JOSA($p, "을/를") } 손에 꼈다.
room-wear-hands = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 손에 낀다.
used-wear-arms = { $p }{ JOSA($p, "을/를") } 팔에 둘렀다.
room-wear-arms = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 팔에 두른다.
used-wear-shield = { $p }{ JOSA($p, "을/를") } 방패로 쓰기 시작했다.
room-wear-shield = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 방패로 팔에 묶는다.
used-wear-about = { $p }{ JOSA($p, "을/를") } 몸에 둘렀다.
room-wear-about = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 몸에 두른다.
used-wear-waist = { $p }{ JOSA($p, "을/를") } 허리에 둘렀다.
room-wear-waist = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 허리에 두른다.
used-wear-wrist_right = { $p }{ JOSA($p, "을/를") } 오른손 손목에 찼다.
room-wear-wrist_right = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 오른손 손목에 찬다.
used-wear-wrist_left = { $p }{ JOSA($p, "을/를") } 왼손 손목에 찼다.
room-wear-wrist_left = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 왼손 손목에 찬다.
zapped = { $p }{ JOSA($p, "이/가") } 당신을 찌릿하게 쳐서 곧바로 놓아 버렸다.
room-zapped = { $p }{ JOSA($p, "이/가") } { $who }{ JOSA($who, "을/를") } 찌릿하게 쳐서, { $who }{ JOSA($who, "이/가") } 곧바로 놓아 버린다.
used-eat = { $p }{ JOSA($p, "을/를") } 먹었다.
room-eat = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 먹는다.
used-taste = { $p }{ JOSA($p, "을/를") } 조금 맛보았다.
room-taste = { $who }{ JOSA($who, "이/가") } { $p }{ JOSA($p, "을/를") } 조금 맛본다.
used-drink = { $liquid }{ JOSA($liquid, "을/를") } 마셨다.
room-drink = { $who }{ JOSA($who, "이/가") } { $p }에서 { $liquid }{ JOSA($liquid, "을/를") } 마신다.
used-sip = { $liquid } 맛이 난다.
room-sip = { $who }{ JOSA($who, "이/가") } { $p }에서 한 모금 마신다.
strange-eat = 이런, 맛이 좀 이상했다!
room-strange-eat = { $who }{ JOSA($who, "이/가") } 기침을 하며 이상한 소리를 낸다.
strange-drink = 이런, 맛이 좀 이상했다!
room-strange-drink = { $who }{ JOSA($who, "이/가") } 사레들려 이상한 소리를 낸다.
cond-full = 배가 부르다.
cond-quenched = 더는 목마르지 않다.
cond-drunk = 술기운이 오른다.
inventory = 가지고 있는 것:
equipment = 쓰고 있는 것:
list-nothing = {"  "}아무것도 없다.
equipment-nothing = {" "}아무것도 없다.
worn-something = 무언가.
slot-light = <빛으로 씀>
slot-finger_right = <손가락에 낌>
slot-finger_left = <손가락에 낌>
slot-neck_1 = <목에 걺>
slot-neck_2 = <목에 걺>
slot-body = <몸에 입음>
slot-head = <머리에 씀>
slot-legs = <다리에 입음>
slot-feet = <발에 신음>
slot-hands = <손에 낌>
slot-arms = <팔에 두름>
slot-shield = <방패로 씀>
slot-about = <몸에 두름>
slot-waist = <허리에 두름>
slot-wrist_right = <손목에 참>
slot-wrist_left = <손목에 참>
slot-wield = <무기로 듦>
slot-hold = <손에 쥠>
refused-not-yet = 그 명령은 아직 Mundi에 없다.

## object failures
fail-get-what = 무엇을 집을까?
fail-drop-what = 무엇을 내려놓을까?
fail-put-what = 무엇을 어디에 넣을까?
fail-give-what = 무엇을 누구에게 줄까?
fail-wear-what = 무엇을 입을까?
fail-wield-what = 무엇을 무기로 들까?
fail-hold-what = 무엇을 쥘까?
fail-remove-what = 무엇을 벗을까?
fail-eat-what = 무엇을 먹을까?
fail-taste-what = 무엇을 먹을까?
fail-drink-what = 무엇을 마실까?
fail-sip-what = 무엇을 마실까?
fail-get-all-of-what = 무엇을 모두 집을까?
fail-drop-all-of-what = 무엇을 모두 내려놓을까?
fail-give-all-of-what = 무엇을 모두?
fail-wear-all-of-what = 무엇을 모두 입을까?
fail-remove-all-of-what = 무엇을 모두 벗을까?
fail-not-here = 여기엔 { $w }{ JOSA($w, "이/가") } 보이지 않는다.
fail-get-not-here-in = { $c } 안에는 { $w }{ JOSA($w, "이/가") } 없는 것 같다.
fail-put-not-here = 여기엔 { $w }{ JOSA($w, "이/가") } 보이지 않는다.
fail-not-carried = { $w }{ JOSA($w, "을/를") } 가지고 있지 않은 것 같다.
fail-put-not-carried = { $w }{ JOSA($w, "을/를") } 가지고 있지 않다.
fail-not-using = { $w }{ JOSA($w, "을/를") } 쓰고 있지 않은 것 같다.
fail-get-none-of = 여기엔 { $word }{ JOSA($word, "이/가") } 하나도 보이지 않는다.
fail-get-none-of-in = { $p } 안에는 { $word }{ JOSA($word, "이/가") } 하나도 없는 것 같다.
fail-remove-none-of = { $word }{ JOSA($word, "을/를") } 하나도 쓰고 있지 않은 것 같다.
fail-none-of = { $word }{ JOSA($word, "을/를") } 하나도 가지고 있지 않은 것 같다.
fail-get-nothing = 여기엔 아무것도 없는 것 같다.
fail-drop-nothing = 아무것도 가지고 있지 않은 것 같다.
fail-put-nothing = 넣을 만한 것이 없는 것 같다.
fail-give-nothing = 아무것도 들고 있지 않은 것 같다.
fail-wear-nothing = 입을 만한 것이 없는 것 같다.
fail-remove-nothing = 아무것도 쓰고 있지 않다.
fail-get-cant-take-scenery = { $w }{ JOSA($w, "은/는") } 가져갈 수 없다.
fail-cant-take = { $p }: 가져갈 수 없는 것이다!
fail-too-many = { $p }: 그렇게 많이는 들 수 없다.
fail-remove-too-many = { $p }: 그렇게 많이는 들 수 없다!
fail-too-heavy = { $p }: 그렇게 무겁게는 들 수 없다.
fail-hold-no-more = { $p }: 더는 들 수 없다.
fail-not-container = { $p }{ JOSA($p, "은/는") } 그릇이 아니다.
fail-closed = { $p }{ JOSA($p, "은/는") } 닫혀 있다.
fail-put-closed = 먼저 여는 게 좋겠다!
fail-empty = { $p }{ JOSA($p, "은/는") } 비어 있는 것 같다.
fail-taste-empty = 이제 남은 게 없다.
fail-drink-empty = 비어 있다.
fail-sip-empty = 비어 있다.
fail-no-container = { $w }{ JOSA($w, "을/를") } 가지고 있지 않다.
fail-into-corpse = { $c }에는 아무것도 넣을 수 없다.
fail-wont-fit = { $p }{ JOSA($p, "은/는") } { $c }에 들어가지 않는다.
fail-into-itself = 그것을 그 자신 안에 접어 넣으려 했지만 실패했다.
fail-into-what = 어디에 넣을까?
fail-drop-cursed = { $p }{ JOSA($p, "을/를") } 내려놓을 수 없다. 저주받은 게 틀림없다!
fail-give-cursed = { $p }{ JOSA($p, "을/를") } 손에서 놓을 수가 없다!!  으윽!
fail-remove-cursed = { $p }{ JOSA($p, "을/를") } 벗을 수 없다. 저주받은 게 틀림없다!
fail-out-of-hand = { $p }{ JOSA($p, "을/를") } 손에서 뗄 수가 없다.
fail-no-person = 그런 이름을 가진 이는 여기 없다.
fail-to-who = 누구에게?
fail-give-self = 그게 무슨 소용인가?
fail-hands-full = { $c }{ JOSA($c, "은/는") } 손이 꽉 찬 것 같다.
fail-cant-carry = { $c }{ JOSA($c, "은/는") } 그렇게 무겁게는 들 수 없다.
fail-level = 그것을 쓰기에는 경험이 부족하다.
fail-cant-wear = { $p }{ JOSA($p, "은/는") } 입을 수 없다.
fail-cant-wear-there = { $p }{ JOSA($p, "은/는") } 거기에 입을 수 없다.
fail-bad-location = '{ $word }'?  그게 몸의 어디인가?
fail-cant-wield = 그것은 무기로 들 수 없다.
fail-too-heavy-to-wield = 쓰기에는 너무 무겁다.
fail-cant-hold = 그것은 쥘 수 없다.
fail-not-food = 그건 먹을 수 없다!
fail-too-full = 너무 배불러서 더 먹을 수 없다!
fail-cant-find = 찾을 수가 없다!
fail-cant-drink = 그것으로는 마실 수 없다!
fail-must-hold = 마시려면 손에 들고 있어야 한다.
fail-miss-mouth = 입에 제대로 갖다 댈 수가 없다.
room-miss-mouth = { $who }{ JOSA($who, "이/가") } 마시려다 입을 빗나간다!
fail-stomach-full = 배에 더 들어가지 않는다!
already-light = 이미 빛을 쓰고 있다.
already-finger_left = 이미 양손 약지에 무언가를 끼고 있다.
already-neck_2 = 목에 더는 걸 수 없다.
already-body = 이미 몸에 무언가를 입고 있다.
already-head = 이미 머리에 무언가를 쓰고 있다.
already-legs = 이미 다리에 무언가를 입고 있다.
already-feet = 이미 발에 무언가를 신고 있다.
already-hands = 이미 손에 무언가를 끼고 있다.
already-arms = 이미 팔에 무언가를 두르고 있다.
already-shield = 이미 방패를 쓰고 있다.
already-about = 이미 몸에 무언가를 두르고 있다.
already-waist = 이미 허리에 무언가를 두르고 있다.
already-wrist_left = 이미 양 손목에 무언가를 차고 있다.
already-wield = 이미 무기를 들고 있다.
already-hold = 이미 무언가를 쥐고 있다.

## liquids [6.3]
liquid-water = 물
liquid-beer = 맥주
liquid-wine = 포도주
liquid-ale = 에일
liquid-dark-ale = 흑맥주
liquid-whisky = 위스키
liquid-lemonade = 레모네이드
liquid-firebreather = 화주
liquid-local-speciality = 지역 특산주
liquid-slime-mold-juice = 슬라임 곰팡이 즙
liquid-milk = 우유
liquid-tea = 차
liquid-coffee = 커피
liquid-blood = 피
liquid-salt-water = 바닷물
liquid-clear-water = 맑은 물
