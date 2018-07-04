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
        self.pending = None

    def _make_request(self, method, params):
        req_id = self._rpc_next_id
        self._rpc_next_id += 1
        return Request(id_=req_id, method=method, params=params)

    def is_awaiting(self):
        return self.pending is not None

    def set_input(self, input):
        self.input = input

    def call(self, method, params):
        if self.input is None:
            return
        message = self._make_request(method, params)
        print("<-", message)
        self.input.write(f"{message}\n".encode())
        self.input.flush()
        self.pending = message

    def handle(self, line):
        response = Response.parse(line)
        print("->", response)
        method = ""
        if response.is_notification():
            method = response.method.replace("-", "_")
        elif response.is_success():
            method = self.pending.method.replace("-", "_")
        else:
            method = "error"
        try:
            getattr(self, f"handle_{method}", lambda *args: None)(response)
        except Exception:
            traceback.print_exception(*sys.exc_info(), file=sys.stdout)
        if response.id == getattr(self.pending, "id", None):
            self.pending = None

    def handle_error(self, response: Response):
        data = response.error.get("data")
        print(f"-- error: {data}")

    def handle_init(self, response: Response):
        buffer_list = response.params["buffer_list"]
        for buf in buffer_list:
            self.state.buffer_list[buf["name"]] = buf
        self.state.buffer_current = response.params["buffer_current"]

    def handle_buffer_changed(self, response: Response):
        buf = response.params
        self.state.buffer_current = buf["name"]
        self.state.buffer_list[buf["name"]] = buf

    def handle_buffer_list(self, response: Response):
        for buf in response.result:
            self.state.buffer_list[buf["name"]] = buf

    def handle_buffer_select(self, response: Response):
        buf = response.result
        self.state.buffer_current = buf["name"]
        self.state.buffer_list[buf["name"]] = buf

    def handle_buffer_delete(self, response: Response):
        deleted = response.result["buffer_deleted"]
        if deleted in self.state.buffer_list:
            del self.state.buffer_list[deleted]

    def handle_edit(self, response: Response):
        self.handle_buffer_select(response)
