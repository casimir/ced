from .core import CoreConnection
from .handlers import RpcHandler
from .shells import InteractiveShell

handler = RpcHandler()
shell = InteractiveShell(handler)
conn = CoreConnection(handler, shell)
conn.start()
