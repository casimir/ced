import sys

from .core import CoreConnection
from .handlers import RpcHandler
from .shells import InteractiveShell

handler = RpcHandler()
shell = InteractiveShell(handler)
conn = CoreConnection(handler, shell, sys.argv[1:])
conn.start()
