use std::{
    io::{Error, Read},
    io::{ErrorKind, Write},
    time::SystemTime,
};

use crate::{io_sync, Connection};

#[derive(Clone, Copy)]
enum Broken {
    DirectErr(ErrorKind, &'static str),
}

impl From<Broken> for Error {
    fn from(value: Broken) -> Self {
        match value {
            Broken::DirectErr(x, s) => Error::new(x, s),
        }
    }
}

pub(crate) struct SocketAdapter {
    pub(crate) internal: Connection,
    written: usize,
    to_write: usize,
    write: [u8; 65536],
    broken: Option<Broken>,
    accumulated_delay: u128,
    ignore_until: Option<u128>,
}

impl SocketAdapter {
    pub fn new(connection: Connection) -> SocketAdapter {
        Self {
            internal: connection,
            written: 0,
            to_write: 0,
            write: [0u8; 65536],
            broken: None,
            accumulated_delay: 0,
            ignore_until: None,
        }
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
        let Some(x) = self
            .write
            .get_mut(self.to_write + self.written..self.to_write + self.written + buf.len())
        else {
            let sa = SystemTime::now();
            self.internal.set_nonblocking(false)?;
            self.internal
                .write_all(&self.write[self.written..self.written + self.to_write])?;
            self.written = 0;
            self.to_write = buf.len();
            self.write[..buf.len()].copy_from_slice(buf);
            self.accumulated_delay += sa.elapsed().unwrap().as_micros();
            return Ok(());
        };
        x.copy_from_slice(buf);
        self.to_write += buf.len();
        Ok(())
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_later(buf)?;
        self.update()
    }

    pub fn write_now(&mut self) -> Result<(), Error> {
        if let Some(ref x) = self.broken {
            return Err(Error::from(*x));
        }
        if self.to_write == 0 {
            return Ok(());
        }
        match {
            self.internal.set_nonblocking(false)?;
            let r = self
                .internal
                .write_all(&self.write[self.written..self.written + self.to_write]);
            r
        } {
            Ok(()) => {
                self.written = 0;
                self.to_write = 0;
                Ok(())
            }
            Err(x) => {
                self.broken = Some(Broken::DirectErr(x.kind(), "io error"));
                Err(x)
            }
        }
    }

    pub fn update(&mut self) -> Result<(), Error> {
        if Some(SystemTime::UNIX_EPOCH.elapsed().unwrap().as_micros()) < self.ignore_until {
            return Ok(());
        }
        if let Some(ref x) = self.broken {
            return Err(Error::from(*x));
        }
        if self.to_write == 0 {
            return Ok(());
        }
        match {
            self.internal.set_nonblocking(!self.internal.is_serial())?;
            let r = self
                .internal
                .write(&self.write[self.written..self.written + self.to_write]);
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
            Err(x) if x.kind() == ErrorKind::WouldBlock => Ok(()),
            Err(x) => {
                self.broken = Some(Broken::DirectErr(x.kind(), "io error"));
                Err(x)
            }
        }
    }

    pub fn read_now(&mut self, buf: &mut [u8]) -> Result<Option<()>, Error> {
        if Some(SystemTime::UNIX_EPOCH.elapsed().unwrap().as_micros()) < self.ignore_until {
            return Ok(None);
        }
        self.update()?;
        self.internal.set_nonblocking(false)?;
        io_sync(self.internal.read_exact(buf))
    }

    pub fn poll_exact(&mut self, buf: &mut [u8]) -> Result<Option<()>, Error> {
        if Some(SystemTime::UNIX_EPOCH.elapsed().unwrap().as_micros()) < self.ignore_until {
            return Ok(None);
        }
        self.update()?;
        self.internal.set_nonblocking(true)?;
        io_sync(self.internal.read_exact(buf))
    }

    pub fn poll(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
        if Some(SystemTime::UNIX_EPOCH.elapsed().unwrap().as_micros()) < self.ignore_until {
            return Ok(None);
        }
        self.update()?;
        self.internal.set_nonblocking(true)?;
        io_sync(self.internal.read(buf))
    }

    pub fn clear_delay(&mut self) -> u128 {
        (self.accumulated_delay, self.accumulated_delay = 0).0
    }

    pub fn punish(&mut self, time: u128) {
        if self.ignore_until.is_none() {
            self.ignore_until = Some(SystemTime::UNIX_EPOCH.elapsed().unwrap().as_micros());
        }
        self.ignore_until = self.ignore_until.map(|x| x + time);
    }
}
