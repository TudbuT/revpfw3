use std::{
    io::{self, ErrorKind, Read, Write},
    net::{Shutdown, TcpStream},
    ptr::NonNull,
    time::Duration,
};

use serial::SerialPort;

trait ReadWrite: Write + Read + 'static {}
impl<T> ReadWrite for T where T: Write + Read + 'static {}

pub struct Connection {
    readwrite: Box<dyn ReadWrite>,
    data: NonNull<()>,
    set_nonblocking_thunk: fn(NonNull<()>, bool) -> io::Result<()>,
    close_thunk: fn(NonNull<()>) -> io::Result<()>,
    is_nb: bool,
    is_serial: bool,
}

impl Write for Connection {
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.as_write().write_vectored(bufs)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.as_write().write_all(buf)
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        self.as_write().write_fmt(fmt)
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.as_write().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.as_write().flush()
    }
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.as_read().read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.as_read().read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.as_read().read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.as_read().read_to_string(buf)
    }

    fn read_exact(&mut self, mut buf: &mut [u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) if self.is_nb => {
                    return Err(io::Error::new(ErrorKind::WouldBlock, "would block"))
                }
                Ok(0) => (),
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ))
        } else {
            Ok(())
        }
    }
}

impl Connection {
    pub fn new_tcp(stream: TcpStream) -> Self {
        let mut stream = Box::new(stream);
        Connection {
            data: NonNull::from(stream.as_mut()).cast(),
            readwrite: stream,
            set_nonblocking_thunk: |data, nb| unsafe {
                data.cast::<TcpStream>().as_ref().set_nonblocking(nb)
            },
            close_thunk: |data| unsafe {
                data.cast::<TcpStream>().as_ref().shutdown(Shutdown::Both)
            },
            is_nb: false,
            is_serial: false,
        }
    }
    pub fn new_serial<T: SerialPort + 'static>(serial: T) -> Self {
        let mut serial = Box::new(serial);
        Connection {
            data: NonNull::from(serial.as_mut()).cast(),
            readwrite: serial,
            set_nonblocking_thunk: |data, nb| unsafe {
                data.cast::<T>()
                    .as_mut()
                    .set_timeout(Duration::from_millis(if nb { 0 } else { 600000 }))
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::ConnectionAborted, "serial port went down")
                    })
            },
            // no need to close this.
            close_thunk: |_data| Ok(()),
            is_nb: false,
            is_serial: true,
        }
    }
    fn as_read(&mut self) -> &mut (dyn Read) {
        &mut self.readwrite
    }
    fn as_write(&mut self) -> &mut (dyn Write) {
        &mut self.readwrite
    }
    pub fn is_nonblocking(&self) -> bool {
        self.is_nb
    }
    pub fn set_nonblocking(&mut self, nonblocking: bool) -> io::Result<()> {
        self.is_nb = nonblocking;
        (self.set_nonblocking_thunk)(self.data, nonblocking)
    }
    pub fn close(&self) -> io::Result<()> {
        (self.close_thunk)(self.data)
    }

    pub fn is_serial(&self) -> bool {
        self.is_serial
    }
}
