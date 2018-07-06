import os

from ced.core import CoreConnection
from ced.handlers import RpcHandler, State
from ced.shells import CommandShell, Shell

if "CED_BIN_PATH" not in os.environ:
    os.environ["CED_BIN_PATH"] = os.path.join(
        os.path.dirname(__file__), "../core/target/debug/ced"
    )


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
    fname = "setup.cfg"
    handler = RpcHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell, ["--standalone", fname])
    conn.start()
    state = handler.state

    assert fname in state.buffer_list
    assert state.buffer_current == fname


def test_connect_open():
    fname = "setup.cfg"
    state = script([f"edit {fname}"])

    assert "*scratch*" in state.buffer_list
    assert fname in state.buffer_list
    assert state.buffer_current == fname


def test_connect_delete():
    fname = "setup.cfg"
    state = script([f"edit {fname}", "buffer-delete *scratch*"])

    assert "*scratch*" not in state.buffer_list
    assert fname in state.buffer_list
    assert state.buffer_current == fname


def test_connect_delete_first():
    fname = "setup.cfg"
    state = script([f"edit {fname}", "buffer-delete *debug*"])

    assert "*debug*" not in state.buffer_list
    assert "*scratch*" in state.buffer_list
    assert fname in state.buffer_list
    assert state.buffer_current == fname


def test_connect_delete_last():
    fname = "setup.cfg"
    state = script([f"edit {fname}", f"buffer-delete {fname}"])

    assert fname not in state.buffer_list
    assert "*debug*" in state.buffer_list
    assert "*scratch*" in state.buffer_list
    assert state.buffer_current == "*scratch*"
