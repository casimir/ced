import os

from ced.core import CoreConnection
from ced.handlers import InitAndQuitHandler
from ced.shells import Shell


if 'CED_BIN_PATH' not in os.environ:
    cwd = os.path.dirname(__file__)
    os.environ['CED_BIN_PATH'] = os.path.join(cwd, "../core/target/debug/ced-core")


def test_simple():
    handler = InitAndQuitHandler()
    shell = Shell(handler)
    conn = CoreConnection(handler, shell)
    conn.start()

    state = handler.state
    assert ["*debug*", "*scratch*"] == list(state.buffer_list.keys())
