import shlex
import sys

from .handlers import RpcHandler


class Shell(object):
    prompt = None

    def __init__(self, handler: RpcHandler, commands=None):
        self.handler = handler
        self._commands = commands if commands is not None else []

    def command_list(self):
        return iter(self._commands)

    def execute(self, command):
        if self.handler is not None:
            self.handler.handle(command)


class CommandShell(Shell):

    def __init__(self, *args, **kwargs):
        super(CommandShell, self).__init__(*args, **kwargs)
        self.current_buffer = None

    def execute(self, command):
        parts = list(shlex.shlex(command, punctuation_chars=True))
        if len(parts) == 0:
            return
        ex_fn = getattr(self, f"cmd_{parts[0]}", self.cmd_generic)
        ex_fn(*parts)

    def cmd_generic(self, *args):
        if len(args) == 1:
            self.handler.call(args[0], None)
        else:
            self.handler.call(*args)

    def cmd__dump(self, *args):
        state = self.handler.state
        for name, buf in state.buffer_list.items():
            flag = ""
            if name == state.buffer_current:
                flag = " (current)"
            print(f"> {name}{flag}")
            print("----")
            print(buf['content'], end="")
            print("----")

    def cmd__print(self, *args):
        state = self.handler.state
        buffer_name = args[1] if len(args) > 1 else state.buffer_current
        buf = state.buffer_list.get(buffer_name)
        if buf:
            print(buf['content'], end='')


class InteractiveShell(CommandShell):
    prompt = "? "

    def command_list(self):
        while True:
            line = sys.stdin.readline()
            if len(line) == 0:
                break
            yield line
