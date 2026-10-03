"""A small telnet client for comparing Mundi with the live tbaMUD (PHASE-1-PLAN S5 comparison).

Test characters only (never the admin, never anima's party). Their passwords live in
run/compare-secret.toml (gitignored) and are never printed or logged.
"""
import asyncio
import re
import secrets
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SECRET = ROOT / "run" / "compare-secret.toml"
ANSI = re.compile(r"\x1b\[[0-9;]*m")
IAC = re.compile(rb"\xff[\xfb-\xfe].|\xff[\xf0-\xfa]")


def server():
    """Host and port: the same server anima uses (its example config names it)."""
    return "192.168.1.101", 4000


def password(name):
    """This character's password, made and kept in the secret file the first time."""
    data = tomllib.loads(SECRET.read_text()) if SECRET.exists() else {}
    if name not in data:
        data[name] = secrets.token_urlsafe(9)
        SECRET.write_text("".join(f'{k} = "{v}"\n' for k, v in data.items()))
        SECRET.chmod(0o600)
    return data[name]


class Mud:
    def __init__(self):
        self.r = self.w = None
        self.log = []

    async def open(self):
        host, port = server()
        self.r, self.w = await asyncio.open_connection(host, port)

    async def read(self, until=None, timeout=3.0):
        """Text until `until` (a regex) shows up, or quiet for `timeout`."""
        buf = ""
        while True:
            try:
                chunk = await asyncio.wait_for(self.r.read(4096), timeout)
            except asyncio.TimeoutError:
                break
            if not chunk:
                break
            text = IAC.sub(b"", chunk).decode("latin-1")
            buf += ANSI.sub("", text).replace("\r", "")
            if until and re.search(until, buf):
                break
        self.log.append(buf)
        return buf

    async def send(self, line, secret=False):
        self.w.write((line + "\r\n").encode("latin-1"))
        await self.w.drain()
        self.log.append("> ***" if secret else f"> {line}")

    async def cmd(self, line, until=r"> $|>\s*$", timeout=3.0):
        await self.send(line)
        return await self.read(until, timeout)

    async def login(self, name, sex="m", cls="w"):
        """Logs in, creating the character if the name is new. Returns the text up to the game."""
        await self.open()
        out = await self.read(r"name|known")
        await self.send(name)
        out = await self.read(r"right|Password|password")
        if re.search(r"right", out):
            await self.send("y")
            await self.read(r"assword")
            pw = password(name)
            await self.send(pw, secret=True)
            await self.read(r"etype|again")
            await self.send(pw, secret=True)
            await self.read(r"sex|Sex|\(M/F\)")
            await self.send(sex)
            await self.read(r"lass")
            await self.send(cls)
        else:
            await self.send(password(name), secret=True)
        out = await self.read(r"RETURN|return|choice|Make your", timeout=5)
        if re.search(r"RETURN|return", out):
            await self.send("")
            out = await self.read(r"choice|Make your", timeout=5)
        await self.send("1")
        return await self.read(timeout=6)

    async def close(self):
        if self.w:
            try:
                await self.send("quit")
                await self.read(timeout=1)
            except Exception:
                pass
            self.w.close()
