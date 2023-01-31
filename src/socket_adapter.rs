use std::{
    io::{Error, Read},
    io::{ErrorKind, Write},
    net::TcpStream,
};

use crate::io_sync;

pub(crate) struct SocketAdapter {
    pub(crate) internal: TcpStream,
    written: usize,
    to_write: usize,
    write: [u8; 4096],
    broken: Option<i32>,
}

impl SocketAdapter {
    pub fn new(tcp: TcpStream) -> SocketAdapter {
        Self {
            internal: tcp,
            written: 0,
            to_write: 0,
            write: [0u8; 4096],
            broken: None,
        }
    }

    pub fn write_later(&mut self, buf: &[u8]) -> Result<(), Error> {
        if let Some(ref x) = self.broken {
            return Err(Error::from_raw_os_error(*x));
        }
        let lidx = self.to_write + self.written + buf.len();
        if lidx > self.write.len() && lidx - self.to_write < self.write.len() {
            self.write
                .copy_within(self.written..self.written + self.to_write, 0);
            self.written = 0;
        }
        let Some(x) = self.write.get_mut(self.to_write + self.written..self.to_write + self.written + buf.len()) else {
            self.broken = Some(Error::from(ErrorKind::TimedOut).raw_os_error().unwrap());
            return Err(ErrorKind::TimedOut.into());
        };
        x.copy_from_slice(buf);
        self.to_write += buf.len();
        Ok(())
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_later(buf)?;
        if let Err(x) = self.update() {
            self.broken = Some(x.raw_os_error().unwrap());
            Err(x)
        } else {
            Ok(())
        }
    }

    pub fn update(&mut self) -> Result<(), Error> {
        if let Some(ref x) = self.broken {
            return Err(Error::from_raw_os_error(*x));
        }
        if self.to_write == 0 {
            return Ok(());
        }
        match self
            .internal
            .write(&self.write[self.written..self.written + self.to_write])
        {
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
                self.broken = Some(x.raw_os_error().unwrap());
                Err(x)
            }
        }
    }

    pub fn poll(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
        self.update()?;
        io_sync(self.internal.read(buf))
    }
}
