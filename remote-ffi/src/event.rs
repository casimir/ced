use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::text::CedTextIterator;
use crate::{cstring, raw, IndexedIterator};
use remote::protocol::{
    notifications::{MenuParamsEntry, StatusParamsItem, ViewParamsItem, ViewParamsLens},
    Text,
};
use remote::ConnectionEvent;

#[repr(C)]
#[derive(Debug)]
pub enum CedEvent {
    Echo {
        message: *mut CedTextIterator,
    },
    Info {
        client: *const c_char,
        session: *const c_char,
    },
    Menu {
        command: *const c_char,
        title: *const c_char,
        search: *const c_char,
        items: *mut MenuIterator,
        selected: u32,
    },
    Status {
        items: *mut StatusIterator,
    },
    View {
        items: *mut ViewIterator,
    },
}

impl From<ConnectionEvent> for CedEvent {
    fn from(event: ConnectionEvent) -> CedEvent {
        match event {
            ConnectionEvent::Echo(text) => CedEvent::Echo {
                message: raw!(CedTextIterator::from(&text)),
            },
            ConnectionEvent::Hint(_hint) => todo!(),
            ConnectionEvent::Info(client, session) => CedEvent::Info {
                client: cstring!(client),
                session: cstring!(session),
            },
            ConnectionEvent::Menu(menu) => CedEvent::Menu {
                command: cstring!(menu.command),
                title: cstring!(menu.title),
                search: cstring!(menu.search),
                items: raw!(MenuIterator::from(&menu.entries)),
                selected: menu.selected as u32,
            },
            ConnectionEvent::Status(status) => CedEvent::Status {
                items: raw!(StatusIterator::from(&status)),
            },
            ConnectionEvent::View(view) => CedEvent::View {
                items: raw!(ViewIterator::from(&view)),
            },
            ConnectionEvent::ConnErr(_msg) => todo!(),
            ConnectionEvent::Noop => todo!(),
        }
    }
}

#[repr(C)]
pub struct MenuItem {
    value: *const c_char,
    text: *mut CedTextIterator,
    description: *const c_char,
}

impl From<MenuParamsEntry> for MenuItem {
    fn from(entry: MenuParamsEntry) -> MenuItem {
        MenuItem {
            value: cstring!(entry.value),
            text: raw!(CedTextIterator::from(&entry.text)),
            description: match entry.description {
                Some(desc) => cstring!(desc),
                None => ptr::null_mut(),
            },
        }
    }
}

pub type MenuIterator = IndexedIterator<MenuParamsEntry, MenuItem>;

#[repr(C)]
pub struct StatusItem {
    index: c_int,
    text: *mut CedTextIterator,
}

impl From<StatusParamsItem> for StatusItem {
    fn from(item: StatusParamsItem) -> StatusItem {
        StatusItem {
            index: item.index as i32,
            text: raw!(CedTextIterator::from(&item.text)),
        }
    }
}

pub type StatusIterator = IndexedIterator<StatusParamsItem, StatusItem>;

#[derive(Debug)]
#[repr(C)]
pub struct ViewItem {
    buffer: *const c_char,
    start: u32,
    end: u32,
    lenses: *mut ViewLensIterator,
}

impl From<ViewParamsItem> for ViewItem {
    fn from(item: ViewParamsItem) -> ViewItem {
        ViewItem {
            buffer: cstring!(item.buffer.as_str()),
            start: item.start as u32,
            end: item.end as u32,
            lenses: raw!(ViewLensIterator::from(&item.lenses)),
        }
    }
}

pub type ViewIterator = IndexedIterator<ViewParamsItem, ViewItem>;

#[derive(Debug)]
#[repr(C)]
pub struct ViewLens {
    lines: *mut ViewLensLineIterator,
    first_line_num: u32,
}

impl From<ViewParamsLens> for ViewLens {
    fn from(lens: ViewParamsLens) -> ViewLens {
        ViewLens {
            lines: raw!(ViewLensLineIterator::from(&lens.lines)),
            first_line_num: lens.first_line_num as u32,
        }
    }
}

pub type ViewLensIterator = IndexedIterator<ViewParamsLens, ViewLens>;

pub type ViewLensLineIterator = IndexedIterator<Text, CedTextIterator>;
