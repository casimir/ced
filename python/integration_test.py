import os

from ced.core import CoreConnection
from ced.handlers import RpcHandler, State
from ced.shells import CommandShell, Shell

CWD = os.path.dirname(__file__)
if "CED_BIN_PATH" not in os.environ:
    os.environ["CED_BIN_PATH"] = os.path.join(CWD, "../core/target/debug/ced")


def script(commands) -> State:
    handler = RpcHandler()
    shell = CommandShell(handler, commands=commands)
    conn = CoreConnection(handler, shell, argv=["--standalone"])
    conn.start()
    return handler.state


def test_connect():
    handler = RpcHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell, argv=["--standalone"])
    conn.start()
    state = handler.state

    assert "*scratch*" in state.buffer_list
    assert state.buffer_current == "*scratch*"


def test_connect_params():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()
    handler = RpcHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell, ["--standalone", fpath])
    conn.start()
    state = handler.state

    assert fpath in state.buffer_list
    assert state.buffer_current == fpath
    assert state.buffer_list[state.buffer_current]["content"] == fcontent


def test_connect_open():
    fpath = os.path.join(CWD, "setup.cfg")
    fcontent = open(fpath).read()
    state = script([f'edit "{fpath}"'])

    assert "*scratch*" in state.buffer_list
    assert fpath in state.buffer_list
    assert state.buffer_current == fpath
    assert state.buffer_list[state.buffer_current]["content"] == fcontent


def test_connect_delete():
    fpath = os.path.join(CWD, "setup.cfg")
    state = script([f'edit "{fpath}"', "buffer-delete *scratch*"])

    assert "*scratch*" not in state.buffer_list
    assert fpath in state.buffer_list
    assert state.buffer_current == fpath


def test_connect_delete_first():
    fpath = os.path.join(CWD, "setup.cfg")
    state = script([f'edit "{fpath}"', "buffer-delete *debug*"])

    assert "*debug*" not in state.buffer_list
    assert "*scratch*" in state.buffer_list
    assert fpath in state.buffer_list
    assert state.buffer_current == fpath


def test_connect_delete_last():
    fpath = os.path.join(CWD, "setup.cfg")
    state = script([f'edit "{fpath}"', f"buffer-delete {fpath}"])

    assert fpath not in state.buffer_list
    assert "*debug*" in state.buffer_list
    assert "*scratch*" in state.buffer_list
    assert state.buffer_current == "*scratch*"
