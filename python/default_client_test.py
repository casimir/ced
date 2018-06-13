import os

from ced.core import CoreConnection
from ced.handlers import RpcHandler, State
from ced.shells import CommandShell, Shell

CWD = os.path.dirname(__file__)
if 'CED_BIN_PATH' not in os.environ:
    os.environ['CED_BIN_PATH'] = os.path.join(CWD, "../core/target/debug/ced")


def script(commands) -> State:
    handler = RpcHandler()
    shell = CommandShell(handler, commands=commands)
    conn = CoreConnection(handler, shell)
    conn.start()
    return handler.state


def test_connect():
    handler = RpcHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell)
    conn.start()
    state = handler.state

    assert ["*debug*", "*scratch*"] == list(state.buffer_list.keys())
    assert state.buffer_current == "*scratch*"


def test_connect_params():
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


def test_connect_open():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()
    state = script([f"edit \"{fpath}\""])

    assert ["*debug*", "*scratch*", "setup.cfg"] == list(state.buffer_list.keys())
    assert state.buffer_current == "setup.cfg"
    assert state.buffer_list[state.buffer_current]['content'] == fcontent


def test_connect_delete():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()
    state = script([f"edit \"{fpath}\"", "buffer-delete *scratch*"])

    assert ["*debug*", "setup.cfg"] == list(state.buffer_list.keys())
    assert state.buffer_current == "setup.cfg"
    assert state.buffer_list[state.buffer_current]['content'] == fcontent


def test_connect_delete_first():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()
    state = script([f"edit \"{fpath}\"", "buffer-delete *debug*"])

    assert ["*scratch*", "setup.cfg"] == list(state.buffer_list.keys())
    assert state.buffer_current == "setup.cfg"
    assert state.buffer_list[state.buffer_current]['content'] == fcontent


def test_connect_delete_last():
    fpath = os.path.join(CWD, "setup.cfg")
    state = script([f"edit \"{fpath}\"", "buffer-delete setup.cfg"])

    assert ["*debug*", "*scratch*"] == list(state.buffer_list.keys())
    assert state.buffer_current == "*scratch*"
    assert state.buffer_list[state.buffer_current]['content'] == ""
