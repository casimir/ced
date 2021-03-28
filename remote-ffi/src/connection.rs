use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crate::event::CedEvent;
use crate::{raw, set_last_error};
use futures_lite::*;
use remote::{ensure_session, Connection, ConnectionEvent, Session};

pub struct CedConnection {
    events: Receiver<ConnectionEvent>,
}

#[no_mangle]
pub unsafe extern "C" fn ced_connection_create(session: *const c_char) -> *mut CedConnection {
    let session = match CStr::from_ptr(session).to_str() {
        Ok(name) => Session::from_name(name),
        Err(_) => Session::from_pid(),
    };

    if let Err(err) = ensure_session(&session) {
        set_last_error(err);
        return ptr::null_mut();
    }

    let connection = Connection::new(session);
    let (events, request_loop) = future::block_on(connection.connect());
    std::thread::spawn(|| future::block_on(request_loop));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let sync_events = stream::block_on(events);
        for ev in sync_events {
            if let Err(err) = tx.send(ev) {
                dbg!(&err.0);
            }
        }
    });

    let handle = CedConnection { events: rx };
    raw!(handle)
}

#[no_mangle]
pub unsafe extern "C" fn ced_connection_next_event<'a>(p: *mut CedConnection) -> *mut CedEvent {
    let handle: &CedConnection = &*p;
    // TODO switch to this when it is possible to exit cleanly
    // match handle.events.try_recv() {
    //     Ok(ev) => {
    //         println!("<-- {:?}", ev);
    //         Box::into_raw(Box::new(Event::from(ev)))
    //     }
    //     Err(TryRecvError::Empty) => {
    //         println!("<-- <empty>");
    //         ptr::null_mut()
    //     }
    //     Err(TryRecvError::Disconnected) => {
    //         println!("<-- <disconnected>");
    //         ptr::null_mut()
    //     }
    // }
    if let Ok(ev) = handle.events.recv_timeout(Duration::from_secs(5)) {
        println!("<-- {:?}", ev);
        Box::into_raw(Box::new(CedEvent::from(ev)))
    } else {
        println!("<-- None");
        ptr::null_mut()
    }
}
