import unittest

from ced.core import CoreConnection
from ced.handlers import InitAndQuitHandler
from ced.shells import Shell


class ConnectionTest(unittest.TestCase):

    def test_simple(self):
        handler = InitAndQuitHandler()
        shell = Shell(handler)
        conn = CoreConnection(handler, shell)
        conn.start()

        state = handler.state
        self.assertListEqual(["*debug*", "*scratch*"], list(state.buffer_list.keys()))
