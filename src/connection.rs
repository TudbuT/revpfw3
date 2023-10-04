use std::{
    io::{self, stdout, ErrorKind, Read, Write},
    net::{Shutdown, TcpStream},
    ptr::NonNull,
    time::{Duration, SystemTime},
};

use serial::SerialPort;

trait ReadWrite: Write + Read + 'static {}
impl<T> ReadWrite for T where T: Write + Read + 'static {}

enum PrintStatus {
    No,
    Yes {
        last_print: SystemTime,
        bytes: u128,
        last_bytes: u128,
    },
}

pub struct Connection {
    readwrite: Box<dyn ReadWrite>,
    data: NonNull<()>,
    set_nonblocking_thunk: fn(NonNull<()>, bool) -> io::Result<()>,
    close_thunk: fn(NonNull<()>) -> io::Result<()>,
    is_nb: bool,
    is_serial: bool,
    print: bool,
    print_status: PrintStatus,
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.as_write().write(buf);
        self.print_status_result(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.as_write().flush()
    }
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.as_read().read(buf);
        self.print_status_result(result)
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
    pub fn new_tcp(stream: TcpStream, print: bool) -> Self {
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(20)))
            .unwrap();
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
            print: true,
            print_status: if print {
                PrintStatus::Yes {
                    last_print: SystemTime::now(),
                    bytes: 0,
                    last_bytes: 0,
                }
            } else {
                PrintStatus::No
            },
        }
    }
    pub fn new_serial<T: SerialPort + 'static>(mut serial: T, print: bool) -> Self {
        serial.set_timeout(Duration::from_secs(20)).unwrap();
        let mut serial = Box::new(serial);
        Connection {
            data: NonNull::from(serial.as_mut()).cast(),
            readwrite: serial,
            set_nonblocking_thunk: |data, nb| unsafe {
                data.cast::<T>()
                    .as_mut()
                    .set_timeout(Duration::from_secs(if nb { 0 } else { 20 }))
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::ConnectionAborted, "serial port went down")
                    })
            },
            // no need to close this.
            close_thunk: |_data| Ok(()),
            is_nb: false,
            is_serial: true,
            print: true,
            print_status: if print {
                PrintStatus::Yes {
                    last_print: SystemTime::now(),
                    bytes: 0,
                    last_bytes: 0,
                }
            } else {
                PrintStatus::No
            },
        }
    }
    fn as_read(&mut self) -> &mut (dyn Read) {
        &mut self.readwrite
    }
    fn as_write(&mut self) -> &mut (dyn Write) {
        &mut self.readwrite
    }
    #[allow(dead_code)]
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

    pub fn set_print(&mut self, print: bool) {
        self.print = print;
    }

    fn print_status(&mut self, add: usize) {
        if let &mut PrintStatus::Yes {
            ref mut last_print,
            ref mut bytes,
            ref mut last_bytes,
        } = &mut self.print_status
        {
            *bytes += add as u128;
            if last_print.elapsed().unwrap().as_secs() > 0 {
                let diff = *bytes - *last_bytes;
                let bps = to_units(diff);
                let total = to_units(*bytes);
                if self.print {
                    print!(
                        "\r\x1b[KCurrent transfer speed: {bps}B/s, transferred {total}B so far."
                    );
                    stdout().flush().unwrap();
                }
                *last_bytes = *bytes;
                *last_print = SystemTime::now();
            }
        }
    }

    fn print_status_result(&mut self, result: io::Result<usize>) -> io::Result<usize> {
        if let Ok(b) = result {
            self.print_status(b)
        }
        result
    }
}

fn to_units(diff: u128) -> String {
    match diff {
        x @ 1_000_000_000_000.. => ((x / 1_000_000_000) as f64 / 1000.0).to_string() + "G",
        x @ 1_000_000_000.. => ((x / 1_000_000) as f64 / 1000.0).to_string() + "G",
        x @ 1_000_000.. => ((x / 1_000) as f64 / 1000.0).to_string() + "M",
        x @ 10_000.. => (x as f64 / 1000.0).to_string() + "K",
        x => x.to_string(),
    }
}
