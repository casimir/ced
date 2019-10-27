import ctypes
import os
import pdb
import platform
import sys
from ctypes import (
    POINTER,
    Structure,
    Union,
    c_char_p,
    c_int,
    c_int64,
    c_uint32,
    cdll,
    create_string_buffer,
)
from enum import IntEnum
from typing import List

HEADER_PATH = os.path.dirname(__file__) + "/include/bindings.h"
DEBUG_DIR = os.path.normpath(
    os.path.abspath(os.path.dirname(__file__)) + "/../target/debug"
)
if platform.system() == "Windows":
    LIB_NAME = "ced_remote.dll"
elif platform.system() == "Darwin":
    LIB_NAME = "libced_remote.dylib"
else:
    LIB_NAME = "libced_remote.so"
LIB_PATH = os.path.join(os.getenv("LD_LIBRARY_PATH", DEBUG_DIR), LIB_NAME)

BIN_NAME = "ced"
if platform.system() == "Windows":
    BIN_NAME += ".exe"
os.environ["CED_BIN"] = os.path.join(DEBUG_DIR, BIN_NAME)


def extract_definitions():
    lines = []
    for line in open(HEADER_PATH):
        if line.startswith("#"):
            continue
        lines.append(line)
    return "\n".join(lines)


print("-" * 80)
print(f"lib: {LIB_PATH}")
print(f"bin: {os.environ['CED_BIN']}")
print("-" * 80)


class ConnectionHandle(Structure):
    pass


class Version(Structure):
    _fields_ = [
        ("major", c_char_p),
        ("minor", c_char_p),
        ("patch", c_char_p),
        ("pre", c_char_p),
    ]

    def __str__(self):
        major = self.major.decode()
        minor = self.minor.decode()
        patch = self.patch.decode()
        pre = self.pre.decode()

        if pre:
            return f"{major}.{minor}.{patch}-{pre}"
        else:
            return f"{major}.{minor}.{patch}"

    def __repr__(self):
        return f"<version: {self}>"


class TextItem(Structure):
    _fields_ = [("text", c_char_p), ("face", c_char_p)]


class TextIterator(Structure):
    pass


def decorated_string(it_p: POINTER(TextIterator)) -> List[List[str]]:
    items = []
    while True:
        item = lib.ced_text_next_item(it_p)
        if not item:
            break
        item = item[0]
        items.append((item.text.decode(), item.face.decode()))
        lib.ced_text_item_destroy(item)
    return items


class EventType(IntEnum):
    Echo = 0
    Info = 1
    Menu = 2
    Status = 3
    View = 4


class EventEcho(Structure):
    pass


class EventInfo(Structure):
    _fields_ = [("client", c_char_p), ("session", c_char_p)]


class EventMenu(Structure):
    _fields_ = [("command", c_char_p), ("search", c_char_p)]


class StatusIterator(Structure):
    pass


class StatusItem(Structure):
    _fields_ = [("index", c_int), ("text", POINTER(TextIterator))]

    # def __del__(self):
    #     lib.ced_status_item_destroy(ctypes.pointer(self))


class EventStatus(Structure):
    _fields_ = [("items", POINTER(StatusIterator))]

    def __iter__(self):
        return self

    def __next__(self):
        it = lib.ced_status_next_item(self.items)
        if not it:
            raise StopIteration()
        return it[0]


class ViewLensesIterator(Structure):
    pass


class EventViewItem(Structure):
    _fields_ = [
        ("buffer", c_char_p),
        ("start", c_uint32),
        ("end", c_uint32),
        ("lenses", POINTER(ViewLensesIterator)),
    ]

    def __str__(self):
        return f"{self.buffer.decode()}[{self.start}:{self.end}]"


class ViewIterator(Structure):
    pass


class EventView(Structure):
    _fields_ = [("items", POINTER(ViewIterator))]

    def __iter__(self):
        return self

    def __next__(self) -> EventViewItem:
        it = lib.ced_view_next_item(self.items)
        if not it:
            raise StopIteration()
        return it[0]


class _EventBody(Union):
    _fields_ = [
        ("ECHO", EventEcho),
        ("INFO", EventInfo),
        ("MENU", EventMenu),
        ("STATUS", EventStatus),
        ("VIEW", EventView),
    ]


class Event(Structure):
    _anonymous_ = ("_body",)
    _fields_ = [("tag", c_int), ("_body", _EventBody)]

    @property
    def type(self) -> EventType:
        return EventType(self.tag)

    # def __del__(self):
    #     lib.ced_event_destroy(ctypes.pointer(self))

    def __str__(self):
        s = f"<{self.type.name.lower()}: "
        if self.type == EventType.Info:
            client = self.INFO.client.decode()
            session = self.INFO.session.decode()
            s += f"client={client!r}, session={session!r}"
        elif self.type == EventType.Menu:
            command = self.MENU.command.decode()
            search = self.MENU.search.decode()
            s += f"command={command!r}, search={search!r}"
        elif self.type == EventType.Status:
            items = [(x.index, decorated_string(x.text)) for x in self.STATUS]
            s += f"items={items}"
            # for it in items:
            #     del it
        elif self.type == EventType.View:
            items = [str(x) for x in self.VIEW]
            s += f"items={items}"
            # for it in items:
            #     del it
        else:
            s += "?"
        s += ">"
        return s


lib = cdll.LoadLibrary(LIB_PATH)
lib.ced_version.restype = POINTER(Version)
lib.ced_version_destroy.argtypes = [POINTER(Version)]
lib.ced_connection_create.argtypes = [c_char_p]
lib.ced_connection_create.restype = POINTER(ConnectionHandle)
lib.ced_connection_destroy.argtypes = [POINTER(ConnectionHandle)]
lib.ced_connection_next_event.argtypes = [POINTER(ConnectionHandle)]
lib.ced_connection_next_event.restype = POINTER(Event)
lib.ced_event_destroy.argtypes = [POINTER(Event)]
lib.ced_status_next_item.restype = POINTER(StatusItem)
lib.ced_text_item_destroy.argtypes = [POINTER(TextItem)]
lib.ced_text_next_item.restype = POINTER(TextItem)
lib.ced_view_next_item.restype = POINTER(EventViewItem)


def _last_error():
    length = lib.ced_last_error_length() + 1
    if length == -1:
        return ""
    buffer = create_string_buffer(length)
    if lib.ced_last_error_message(buffer, length) > 0:
        return buffer.value.decode()
    else:
        return "<unknown>"


class ConnectionError(Exception):
    pass


class Connection:
    def __init__(self, session: str):
        self._inner = None
        self.session = session

    def __enter__(self):
        conn = lib.ced_connection_create(self.session.encode())
        if not conn:
            raise ConnectionError(_last_error())
        self._inner = conn
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        lib.ced_connection_destroy(self._inner)
        self._inner = None

    def __iter__(self):
        return self

    def __next__(self):
        ev = lib.ced_connection_next_event(self._inner)
        if not ev:
            raise StopIteration()
        return ev[0]


if __name__ == "__main__":
    # pdb.set_trace()
    version = lib.ced_version()
    print(f"version: {version[0]}")
    lib.ced_version_destroy(version)

    with Connection("ffi") as conn:
        # TODO look at async iterators
        for ev in conn:
            try:
                print(ev)
            except Exception as e:
                print(f"error: {repr(e)}: {_last_error()}")
            # finally:
            #     del ev
        print("OUT")
