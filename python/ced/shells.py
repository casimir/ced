import shlex
import sys

from .handlers import RpcHandler


class Shell(object):
    is_interactive = False

    def __init__(self, handler: RpcHandler):
        self.handler = handler

    def execute(self, command) -> bool:
        if self.handler is not None:
            self.handler.handle(command)


class InteractiveShell(Shell):
    is_interactive = True
    COMMANDS = {
        'buffer_delete': {'argc': 1},
        'buffer_list': {'argc': 0},
        'buffer_select': {'argc': 1},
        'edit': {'argc': 1},
        '_dump': {'argc': 0},
        '_print': {'argc': 1},
        '_quit': {'argc': 0},
    }

    def __init__(self, *args, **kwargs):
        super(InteractiveShell, self).__init__(*args, **kwargs)
        self.current_buffer = None

    def execute(self, command):
        if not command:
            return
        parts = list(shlex.shlex(command, punctuation_chars=True))
        if parts[0] in self.COMMANDS:
            cmd_spec = self.COMMANDS[parts[0]]
            parts = parts[:cmd_spec['argc'] + 1]
            ex_fn = getattr(self, f"cmd_{parts[0]}", self.cmd_generic)
            ex_fn(*parts)
        else:
            print(f"shell error: {command}", file=sys.stderr)

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

    def cmd__quit(self, *args):
        return True
