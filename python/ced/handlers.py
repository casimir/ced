import sys
import traceback

from .jsonrpc import Request, Response


class State(object):

    def __init__(self):
        self.buffer_list = {}
        self.buffer_current = None


class RpcHandler(object):
    _rpc_next_id = 1

    def __init__(self):
        self.state = State()
        self.input = None
        self.pending = {}

    def _make_request(self, method, params):
        req_id = self._rpc_next_id
        self._rpc_next_id += 1
        return Request(id_=req_id, method=method, params=params)

    def set_input(self, input):
        self.input = input

    def call(self, method, params):
        if self.input is None:
            return
        message = self._make_request(method, params)
        print("<-", message)
        self.input.write(f"{message}\n".encode())
        self.pending[message.id] = message

    def handle(self, line):
        response = Response.parse(line)
        print("->", response)
        method = ""
        if response.is_notification():
            method = response.method
        elif response.is_success() and response.id in self.pending:
            method = self.pending[response.id].method
        try:
            getattr(self, f"handle_{method}", lambda *args: None)(response)
        except Exception as e:
            traceback.print_exception(*sys.exc_info(), file=sys.stdout)

    def handle_update(self, response: Response):
        buffer_list = response.params['buffer_list']
        for buf in buffer_list:
            self.state.buffer_list[buf['name']] = buf
        self.state.buffer_current = response.params['buffer_current']

    def handle_buffer_list(self, response: Response):
        for buf in response.result:
            self.state.buffer_list[buf['name']] = buf

    def handle_buffer_select(self, response: Response):
        buf = response.result
        self.state.buffer_current = buf['name']
        self.state.buffer_list[buf['name']] = buf


class InitAndQuitHandler(RpcHandler):

    def handle_update(self, response):
        super(InitAndQuitHandler, self).handle_update(response)
        self.input.close()
