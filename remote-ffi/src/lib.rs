mod connection;
mod event;
mod ffi;
mod text;

use std::cell::RefCell;
use std::error::Error;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::slice;

#[macro_export]
macro_rules! raw {
    ($e:expr) => {
        Box::into_raw(Box::new($e))
    };
}

#[macro_export]
macro_rules! cstring {
    ($e:expr) => {
        std::ffi::CString::new($e)
            .expect("failed to encode string")
            .into_raw()
    };
}

pub struct IndexedIterator<T, U> {
    items: Vec<T>,
    next_idx: usize,
    phantom: PhantomData<*const U>,
}

impl<T: Clone, U> From<&Vec<T>> for IndexedIterator<T, U> {
    fn from(items: &Vec<T>) -> IndexedIterator<T, U> {
        IndexedIterator {
            items: items.clone(),
            next_idx: 0,
            phantom: PhantomData,
        }
    }
}

impl<T: Clone, U: From<T>> Iterator for IndexedIterator<T, U> {
    type Item = U;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.items.get(self.next_idx)?.to_owned();
        self.next_idx += 1;
        Some(U::from(item))
    }
}

#[repr(C)]
pub struct CedVersion {
    major: *const c_char,
    minor: *const c_char,
    patch: *const c_char,
    pre: *const c_char,
}

#[no_mangle]
pub unsafe extern "C" fn ced_version() -> *mut CedVersion {
    raw!(CedVersion {
        major: cstring!(env!("CARGO_PKG_VERSION_MAJOR")),
        minor: cstring!(env!("CARGO_PKG_VERSION_MINOR")),
        patch: cstring!(env!("CARGO_PKG_VERSION_PATCH")),
        pre: cstring!(option_env!("CARGO_PKG_VERSION_PRE").unwrap_or_default()),
    })
}

thread_local! {
    static LAST_ERROR: RefCell<Option<Box<dyn Error>>> = RefCell::new(None);
}

fn set_last_error<E: Into<Box<dyn Error>> + 'static>(err: E) {
    let boxed = err.into();
    LAST_ERROR.with(|last| {
        *last.borrow_mut() = Some(boxed);
    });
}

fn take_last_error() -> Option<Box<dyn Error>> {
    LAST_ERROR.with(|last| last.borrow_mut().take())
}

/// Length of the last error message. Useful for last_error_message.
/// If no error is pending the length will be -1.
#[no_mangle]
pub unsafe extern "C" fn ced_last_error_length() -> c_int {
    LAST_ERROR.with(|last| match last.borrow().as_ref() {
        Some(ref e) => e.to_string().len() as c_int,
        None => -1,
    })
}

/// Copy the last error message to the buffer, see last_error_length to get the length of the
/// message. It returns the length of the message if the copy succeed, -1 if the buffer was a
/// nullptr and -2 if the buffer was too small.
#[no_mangle]
pub unsafe extern "C" fn ced_last_error_message(buffer: *mut c_char, length: c_int) -> c_int {
    if buffer.is_null() {
        return -1;
    }
    let last_error = match take_last_error() {
        Some(err) => err,
        None => return 0,
    };

    let message = last_error.to_string();
    let buffer = slice::from_raw_parts_mut(buffer as *mut u8, length as usize);
    if message.len() >= buffer.len() {
        return -2;
    }
    ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr(), message.len());
    buffer[message.len()] = 0;

    message.len() as c_int
}
