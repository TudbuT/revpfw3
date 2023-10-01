mod client;
mod packet;
mod server;
mod server_connection;
mod socket_adapter;

use std::io::{Error, ErrorKind};

pub use client::*;
pub(crate) use packet::*;
pub use server::*;
pub(crate) use server_connection::*;
pub(crate) use socket_adapter::*;

pub(crate) fn io_sync<T>(result: Result<T, Error>) -> Result<Option<T>, Error> {
    match result {
        Ok(x) => Ok(Some(x)),
        Err(x) if x.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(x) if x.kind() == ErrorKind::TimedOut => Ok(None),
        Err(x) => Err(x),
    }
}
