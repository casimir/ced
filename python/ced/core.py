import asyncio
import os
import sys

from .handlers import RpcHandler
from .shells import Shell


class CoreConnection(object):

    @staticmethod
    def get_event_loop():
        if sys.platform == "win32":
            loop = asyncio.ProactorEventLoop()
            asyncio.set_event_loop(loop)
        else:
            loop = asyncio.get_event_loop()
        return loop

    def __init__(self, handler: RpcHandler, shell: Shell):
        self.bin = os.getenv("CED_BIN_PATH", "ced-core")
        self.proc: asyncio.Process = None
        self.handler = handler
        self.shell = shell

    async def _consume_stream(self, stream, fn):
        should_read = True
        while should_read:
            line = await stream.readline()
            if len(line) > 0:
                fn(line)
            else:
                should_read = False

    async def _exec_core(self, loop):
        self.proc = await asyncio.create_subprocess_exec(
            self.bin,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            loop=loop,
        )
        print(f"connection to server opened (pid: {self.proc.pid})")
        self.handler.set_input(self.proc.stdin)
        await asyncio.wait([
            self._consume_stream(self.proc.stdout, self.handler.handle),
            self._consume_stream(self.proc.stderr, lambda x: print(f"error: {x}", file=sys.stderr)),
        ])
        return await self.proc.wait()

    def _handle_stdin(self, *args):
        line = sys.stdin.readline()
        should_stop = False
        if len(line) > 0:
            stripped = line.strip()
            should_stop = self.shell.execute(stripped) is True
        else:
            should_stop = True
        if should_stop:
            self.proc.stdin.write_eof()

    def start(self):
        loop = self.get_event_loop()
        if self.shell.is_interactive:
            loop.add_reader(sys.stdin.fileno(), self._handle_stdin)
        ret_code = loop.run_until_complete(self._exec_core(loop))
        if self.shell.is_interactive:
            loop.remove_reader(sys.stdin.fileno())
        loop.close()
        print(f"connection to server closed with status {ret_code}")
