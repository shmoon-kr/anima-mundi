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
