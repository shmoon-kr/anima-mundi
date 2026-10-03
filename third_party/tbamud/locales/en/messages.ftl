# tbaMUD-derived (third_party/tbamud/NOTICE.md): the English messages of tbaMUD, as Fluent templates.
# The source of each is in docs/MECHANICS.md (section in brackets). Markup in lines is D20.
# Variables: $who (a being, already "someone" when not seen), $dir, $door, $text.

## connection
in-game-entered = Welcome to tbaMUD!  May your visit here be... Enlightening
in-game-reconnected = Reconnecting.
closed-quit = Goodbye, friend.. Come back soon!
login-name = By what name do you wish to be known?
login-password = Password:
login-invalid-name = Invalid name, please try another.
login-wrong-password = Wrong password.

## directions
dir-north = north
dir-east = east
dir-south = south
dir-west = west
dir-up = up
dir-down = down
exit-north = n
exit-east = e
exit-south = s
exit-west = w
exit-up = u
exit-down = d

## rooms [3.2]
room-dark = It is pitch black...
room-blind = You see nothing but infinite darkness...
exits-label = Exits
exits-none = None!
occupant-standing = { $who } is standing here.
occupant-sitting = { $who } is sitting here.
occupant-resting = { $who } is resting here.
occupant-sleeping = { $who } is sleeping here.
occupant-fighting = { $who } is here, fighting!
occupant-stunned = { $who } is lying here, stunned.
occupant-incapacitated = { $who } is lying here, incapacitated.
occupant-mortally-wounded = { $who } is lying here, mortally wounded.
occupant-dead = { $who } is lying here, dead.
flag-linkless = (linkless)

## movement [2.2, 2.4]
move-no-exit = Alas, you cannot go that way...
move-closed-door = The { $door } seems to be closed.
move-closed = It seems to be closed.
move-exhausted = You are too exhausted.
move-forbidden = A mysterious barrier forces you back! That area is off-limits.
zone-above-level = This zone is above your recommended level.
arrived = { $who } has arrived.
arrived-entered-game = { $who } has entered the game.
left = { $who } leaves { $dir }.
left-game = { $who } has left the game.
link-lost = { $who } has lost { $his } link.
link-reconnected = { $who } has reconnected.

## talking (act.comm.c:40-72)
say-out = You say, '{ $text }'
say-in = { $who } says, '{ $text }'

## refusals [4.2]
refused-dead = Lie still; you are DEAD!!! :-(
refused-incapacitated = You are in a pretty bad shape, unable to do anything!
refused-stunned = All you can do right now is think about the stars!
refused-sleeping = In your dreams, or what?
refused-resting = Nah... You feel too relaxed to do that..
refused-sitting = Maybe you should get on your feet first?
refused-fighting = No way!  You're fighting for your life!
refused-unknown-command = Huh!?!
refused-nothing-to-say = Yes, but WHAT do you want to say?
refused-quit-in-full = You have to type quit--no less, to quit!
refused-invalid-target = You do not see that here.

## time [1.2]
time-sunrise = The sun rises in the east.
time-day = The day has begun.
time-sunset = The sun slowly disappears in the west.
time-night = The night has begun.

## prompt (comm.c:1225-1235)
prompt = { $hp }H { $mp }M { $mv }V >
