import os

from ced.core import CoreConnection
from ced.handlers import RpcHandler
from ced.shells import CommandShell, Shell

if 'CED_BIN_PATH' not in os.environ:
    CWD = os.path.dirname(__file__)
    os.environ['CED_BIN_PATH'] = os.path.join(CWD, "../core/target/debug/ced-core")


def test_connect_quit():
    handler = RpcHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell)
    conn.start()

    state = handler.state
    assert ["*debug*", "*scratch*"] == list(state.buffer_list.keys())
    assert state.buffer_current == "*scratch*"


def test_connect_params_quit():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()

    handler = RpcHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell, [fpath])
    conn.start()

    state = handler.state
    assert ["*debug*", "setup.cfg"] == list(state.buffer_list.keys())
    assert state.buffer_current == "setup.cfg"
    assert state.buffer_list[state.buffer_current]['content'] == fcontent


def test_connect_open_quit():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()

    handler = RpcHandler()
    shell = CommandShell(handler, commands=[f"edit {fpath}"])
    conn = CoreConnection(handler, shell)
    conn.start()

    state = handler.state
    assert ["*debug*", "*scratch*", "setup.cfg"] == list(state.buffer_list.keys())
    assert state.buffer_current == "setup.cfg"
    assert state.buffer_list[state.buffer_current]['content'] == fcontent
