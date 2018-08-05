import os
import subprocess
import sys

from ced.handlers import RpcHandler
from ced.shells import Shell


class CoreConnection(object):
    def __init__(self, handler: RpcHandler, shell: Shell, argv=None):
        self.bin = os.getenv("CED_BIN_PATH", "ced")
        self.command = [self.bin]
        if argv is not None:
            self.command += argv
        self.handler = handler
        self.shell = shell

    def _print_prompt(self):
        if self.shell.prompt is not None:
            print(self.shell.prompt, end="")
            sys.stdout.flush()

    def start(self):
        proc = subprocess.Popen(
            self.command, stdin=subprocess.PIPE, stdout=subprocess.PIPE
        )
        self.handler.set_input(proc.stdin)
        command_list = self.shell.command_list()
        should_stop = False
        while not should_stop:
            line = proc.stdout.readline()
            if len(line) == 0:
                # no more data, connection has been closed
                break
            self.handler.handle(line.strip())
            if self.handler.is_awaiting():
                # the message was an update, we're still missing a response
                continue
            while not should_stop and not self.handler.is_awaiting():
                self._print_prompt()
                # consume commands that don't send data to the server
                command = next(command_list, None)
                if command is None:
                    # no more commands to execute
                    should_stop = True
                    break
                self.shell.execute(command)
        proc.stdin.close()
        print(f"connection to server closed")
