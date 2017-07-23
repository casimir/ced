import shlex
import sys

from .handlers import RpcHandler


class Shell(object):
    is_interactive = False

    def __init__(self, handler: RpcHandler, commands=None):
        self.handler = handler
        self._commands = commands if commands is not None else []

    def command_list(self):
        return iter(self._commands)

    def execute(self, command):
        if self.handler is not None:
            self.handler.handle(command)


class CommandShell(Shell):
    is_interactive = True
    COMMANDS = {
        'buffer_delete': {
            'argc': 1
        },
        'buffer_list': {
            'argc': 0
        },
        'buffer_select': {
            'argc': 1
        },
        'edit': {
            'argc': 1
        },
        '_dump': {
            'argc': 0
        },
        '_print': {
            'argc': 1
        },
    }

    def __init__(self, *args, **kwargs):
        super(CommandShell, self).__init__(*args, **kwargs)
        self.current_buffer = None

    def execute(self, command):
        parts = list(shlex.shlex(command, punctuation_chars=True))
        if len(parts) == 0:
            return
        if parts[0] in self.COMMANDS:
            cmd_spec = self.COMMANDS[parts[0]]
            parts = parts[:cmd_spec['argc'] + 1]
            ex_fn = getattr(self, f"cmd_{parts[0]}", self.cmd_generic)
            ex_fn(*parts)
        else:
            print(f"shell error: {parts[0]}", file=sys.stderr)

    def cmd_generic(self, *args):
        if len(args) == 1:
            self.handler.call(args[0], None)
        else:
            self.handler.call(*args)

    def cmd__dump(self, *args):
        print(self.handler.state.__dict__)

    def cmd__print(self, *args):
        state = self.handler.state
        buffer_name = args[1] if len(args) > 1 else state.buffer_current
        buf = state.buffer_list.get(buffer_name)
        if buf:
            print(buf['content'], end='')


class InteractiveShell(CommandShell):

    def command_list(self):
        while True:
            line = sys.stdin.readline()
            if len(line) == 0:
                break
            yield line
