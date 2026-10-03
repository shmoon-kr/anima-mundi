"""Movement regeneration on the live server, for checking limits.c move_gain (MECHANICS §10).

A test character walks back and forth to spend movement points, then stands, rests and sleeps,
one tick each, and prints its age and the points each tick gave.
Usage: python tools/compare/regen.py <new test character name>
"""
import asyncio
import re
import sys
import time

from tba import Mud

PROMPT = re.compile(r"(\d+)H (\d+)M (\d+)V")


async def points(m):
    out = await m.cmd("")
    found = PROMPT.findall(out)
    return tuple(map(int, found[-1])) if found else None


async def one_tick(m, label):
    """Polls the prompt until the movement points change: one tick's gain."""
    start = await points(m)
    t0 = time.time()
    while time.time() - t0 < 100:
        await asyncio.sleep(2)
        now = await points(m)
        if now and start and now[2] != start[2]:
            print(f"{label}: {start} -> {now} (+{now[2] - start[2]} moves, +{now[0] - start[0]} hit)", flush=True)
            return now
    print(f"{label}: no tick in 100 s", flush=True)


async def main(name):
    m = Mud()
    try:
        await m.login(name)
        await m.read(timeout=4)
        print([l for l in (await m.cmd("score")).splitlines() if "years old" in l or "hit," in l])
        await m.walk_to("tba:30:room:3001")
        for _ in range(35):
            await m.cmd("south")
            await m.cmd("north")
        print("after walking", await points(m), flush=True)
        await one_tick(m, "first (partial wait)")
        await one_tick(m, "standing")
        await m.cmd("rest")
        await one_tick(m, "resting")
        await m.cmd("sleep")
        await one_tick(m, "sleeping")
        await m.cmd("wake")
        await m.cmd("stand")
    finally:
        await m.close()


if __name__ == "__main__":
    asyncio.run(main(sys.argv[1]))
