use std::{
    io::{Error, Read},
    io::{ErrorKind, Write},
    net::TcpStream,
};

use crate::io_sync;

#[derive(Clone, Copy)]
enum Broken {
    OsErr(i32),
    DirectErr(ErrorKind),
}

impl From<Broken> for Error {
    fn from(value: Broken) -> Self {
        match value {
            Broken::OsErr(x) => Error::from_raw_os_error(x),
            Broken::DirectErr(x) => Error::from(x),
        }
    }
}

pub(crate) struct SocketAdapter {
    pub(crate) internal: TcpStream,
    written: usize,
    to_write: usize,
    write: [u8; 65536],
    broken: Option<Broken>,
    wait_if_full: bool,
    is_nonblocking: bool,
}

impl SocketAdapter {
    pub fn new(tcp: TcpStream, wait_if_full: bool) -> SocketAdapter {
        Self {
            internal: tcp,
            written: 0,
            to_write: 0,
            write: [0u8; 65536],
            broken: None,
            wait_if_full,
            is_nonblocking: false,
        }
    }

    pub fn set_nonblocking(&mut self, nonblocking: bool) {
        if let Err(x) = self.internal.set_nonblocking(nonblocking) {
            self.broken = Some(Broken::OsErr(x.raw_os_error().unwrap()));
            return;
        }
        self.is_nonblocking = nonblocking;
    }

    pub fn write_later(&mut self, buf: &[u8]) -> Result<(), Error> {
        if let Some(ref x) = self.broken {
            return Err(Error::from(*x));
        }
        let lidx = self.to_write + self.written + buf.len();
        if lidx > self.write.len() && self.to_write + buf.len() < self.write.len() {
            self.write
                .copy_within(self.written..self.written + self.to_write, 0);
            self.written = 0;
        }
        let Some(x) = self.write.get_mut(self.to_write + self.written..self.to_write + self.written + buf.len()) else {
            if self.wait_if_full {
                self.internal.set_nonblocking(false)?;
                self.internal.write_all(&self.write[self.written..self.written + self.to_write])?;
                self.internal.set_nonblocking(self.is_nonblocking)?;
                self.written = 0;
                self.to_write = buf.len();
                self.write[..buf.len()].copy_from_slice(buf);
                return Ok(());
            }
            self.broken = Some(Broken::DirectErr(ErrorKind::TimedOut));
            return Err(ErrorKind::TimedOut.into());
        };
        x.copy_from_slice(buf);
        self.to_write += buf.len();
        Ok(())
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_later(buf)?;
        self.update()
    }

    pub fn update(&mut self) -> Result<(), Error> {
        if let Some(ref x) = self.broken {
            return Err(Error::from(*x));
        }
        if self.to_write == 0 {
            return Ok(());
        }
        match {
            self.internal.set_nonblocking(true)?;
            let r = self
                .internal
                .write(&self.write[self.written..self.written + self.to_write]);
            self.internal.set_nonblocking(self.is_nonblocking)?;
            r
        } {
            Ok(x) => {
                self.to_write -= x;
                self.written += x;
                if self.to_write == 0 {
                    self.written = 0;
                }
                Ok(())
            }
            Err(x) if x.kind() == ErrorKind::WouldBlock => {
                Ok(())
            }
            Err(x) => {
                self.broken = Some(Broken::OsErr(x.raw_os_error().unwrap()));
                Err(x)
            }
        }
    }

    pub fn poll(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
        self.update()?;
        io_sync(self.internal.read(buf))
    }
}
