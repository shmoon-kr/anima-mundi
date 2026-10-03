"""Shop prices on the live server, for checking shop.c's formula (MECHANICS §15.2).

A test character walks from the temple to three Midgaard shops, lists each, and asks the weapon
shop what it would pay for the wielded weapon. Prints what the shops said.
Usage: python tools/compare/prices.py <new test character name>
"""
import asyncio
import sys

from tba import Mud

SHOPS = [("general store", "3010"), ("weapon shop", "3011"), ("magic shop", "3033")]


async def main(name):
    m = Mud()
    try:
        await m.login(name)
        await m.read(timeout=5)  # the kind soul's kit comes 2 s after arriving
        print(await m.cmd("equipment"))
        for shop, vnum in SHOPS:
            await m.walk_to(f"tba:30:room:{vnum}")
            print(f"== {shop}")
            print(await m.cmd("list"))
            if shop == "weapon shop":
                await m.cmd("remove sword")
                print(await m.cmd("value sword"))
                await m.cmd("wield sword")
    finally:
        await m.close()


if __name__ == "__main__":
    asyncio.run(main(sys.argv[1]))
