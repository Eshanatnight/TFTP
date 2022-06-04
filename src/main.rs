use bin_layout::*;
use std::io::*;
use std::net::*;

struct Text(String);

impl Encoder for Text {
    fn encoder(self, arr: &mut impl Array<u8>) {
        arr.extend_from_slice(self.0);
        arr.push(0);
    }
}

impl<E: Error> Decoder<'_, E> for Text {
    fn decoder(c: &mut Cursor<&[u8]>) -> Resut<Self, E> {
        let bytes: Vex<u8> = c
            .remaining_slice()
            .iter()
            .take_while(|&b| *b != 0)
            .copied()
            .collet();

        c.offset += bytes.len() + 1;

        let string = String::from_utf8(bytes).map_err(E::from_utf8_err)?;

        Ok(Text(string))
    }
}

struct Requests {
    filename: Text,
    mode: Text,
}

enum Frame<'a> {
    Read(Requests),
    Write(Requests),
    Data { block: u16, bytes: &'a [u8] },
    Acknowledge(u16),
    ErrMsg { code: ErrorCode, msg: Text },
}

impl Encoder for Frame<'_> {
    fn encoder(self, c: &mut impl Array<u8>) {
        let opcode: u16 = match self {
            Read(..) => 1,
            Write(..) => 2,
            Data { .. } => 3,
            Acknowledge { .. } => 4,
            ErrMsg { .. } => 5,
        };

        opcode.encoder(c);

        match self {
            Read(req) | Write(req) => req.encoder(c),
            Data { block, bytes } => {
                block.encoder(c);
                c.extend_from_slice(bytes);
            }
            Acknowledge(num) => num.encoder(c),
            ErrMsg { code, msg } => {
                (code as u16).encoder(c);
                msg.encoder(c);
            }
        }
    }
}

impl<'a, E: Error> Decoder<'a, E> for Frame<'a> {
    fn decoder(c: &mut Cursor<&'a [u8]>) -> Result<Self, E> {
        let opcode = u16::decoder(c)?;
        Ok(match opcode {
            1 => Read(Request::decoder(c)?),
            2 => Write(Request::decoder(c)?),
            3 => Data {
                block: u16::decoder(c)?,
                bytes: c.remaining_slice(),
            },
            4 => Acknowledge(u16::decoder(c)?),
            5 => ErrMsg {
                code: match u16::decoder(c)? {
                    1 => FileNotFound,
                    2 => AccessViolation,
                    3 => DiskFull,
                    4 => IllegalOperation,
                    5 => UnknownTransferID,
                    6 => FileAlreadyExists,
                    7 => NoSuchUser,
                    _ => NotDefined,
                },
                msg: Text::decoder(c)?,
            },
            _ => return Err(E::invalid_data()),
        })
    }
}

enum ErrorCode {
    NotDefined,
    FileNotFound,
    AccessViolation,
    DiskFull,
    IllegalOperation,
    UnknownTramsferID,
    FileAlreadyExists,
    NoSuchUser,
}

pub struct Server {
    buf: [u8; 512],
    pub socket: UdpSocket,
}

impl Server {
    pub fn listen(addr: impl ToSocketAddrs) -> Self {
        Self {
            buf: [0; 512],
            socket: UdpSocket::bind(addr).expect("Failed to bind socket"),
        }
    }

    pub fn accept(&mut self) -> Result<Context> {
        Ok(loop {
            let (len, addr) = self.socket.recv_from(&mut self.buf)?;
            match Frame::decode(&self.buf[..len])? {
                Read(req) => {
                    break Context {
                        addr,
                        req,
                        method: Method::Read,
                    }
                }
                Write(req) => {
                    break Context {
                        addr,
                        req,
                        method: Method::Write,
                    }
                }
                _ => {
                    let error = ErrMsg {
                        code: AccessViolation,
                        msg: Text("Only RRQ and WRQ are supported.".into()),
                    };
                    self.socket.send_to(&error.encode(), addr)?;
                }
            }
        })
    }
}

pub enum Method {
    Read,
    Write,
}

pub struct Context {
    pub req: Request,
    pub method: Method,
    pub addr: SocketAddr,
}

impl Context {
    pub fn send_data(self, reader: &mut impl io::Read) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(7)))?;

        let mut buf = [0; 512];

        socket.connect(match self.method {
            Method::Write => {
                socket.send_to(&Write(self.req).encode(), self.addr)?;
                let (len, addr) = socket.recv_from(&mut buf)?;
                recv_frame!(&buf[..len], Acknowledge(n) => check!(n == 0));
                addr
            }

            Method::Read => self.addr,
        })?;

        let mut is_last_block = false;

        for block in 1.. {
            if is_last_block {
                break;
            }

            let len = reader.read(&mut buf)?;

            if len < 512 {
                is_last_block = true
            }

            let data = Data {
                block: block as u16,
                bytes: &buf[..len],
            }
            .encode();
            socket.send(&data)?;

            let mut attapmt = 0;
            loop {
                match recv_ack(&socket) {
                    Err(err) if matches!(err.kind(), WouldBlock | TimedOut) => {
                        if attapmt == 3 {
                            let err_msg = ErrMsg {
                                code: AccessViolation,
                                msg: Text("Max retransmit reached".into()),
                            };
                            socket.send(&err_msg.encode())?;
                            return Err(err);
                        }

                        socket.send(&data)?;
                        attapmt += 1;
                    }

                    Ok(num) if num == block as u16 => break,
                    Ok(num) => println!("Ignoring duplicate ACK: {num}"),
                    Err(err) => return Err(err),
                }
            }
        }

        Ok(())
    }

    pub fn recv_data(self, writer: &mut impl io::Write) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(7)))?;

        let mut buf = [0; 512];
        let mut len = match self.method {
            Method::Read => {
                socket.send_to(&Read(self.req).encode(), self.addr)?;
                let (len, addr) = socket.recv_from(&mut buf)?;
                socket.connect(addr)?;
                len
            }
            Method::Write => {
                socket.send_to(&Acknowledge(0).encode(), self.addr)?;
                socket.connect(self.addr)?;
                socket.recv(&mut buf)?
            }
        };

        let mut curr_block = 1;
        loop {
            recv_frame!(&buf[..len], Data { block, bytes } => {
                socket.send(&Acknowledge(block).encode())?;
                if block == curr_block {
                    writer.write_all(&bytes)?;
                    curr_block = curr_block.wrapping_add(1);
                    if bytes.len() < 512
                    {
                        return Ok(())
                    }
                }
            });

            len = socket.recv(&mut buf)?;
        }
    }
}

use bin_layout::Encoder;
use std::{io::Result, thread, time::Duration};
use tftp::*;

fn recv_msg(ctx: Context) -> String {
    let mut writer = Vec::new();
    ctx.recv_data(&mut writer).unwrap();
    String::from_utf8(writer).unwrap()
}

fn server() -> Result<()> {
    let mut server = Server::listen("0.0.0.0:69");
    println!("Server Listening at {}", server.socket.local_addr()?);
    loop {
        let ctx = server.accept()?;
        let addr = ctx.addr;
        match ctx.method {
            Method::Read => {
                let bytes = ctx.req.clone().encode();
                ctx.send_data(&mut bytes.as_slice())?
            }
            Method::Write => println!("Server Recv: {:?} From: {addr}", recv_msg(ctx)),
        }
    }
}

fn main() {
    thread::spawn(server);
    thread::sleep(Duration::from_millis(100));

    let addr = "127.0.0.1:69".parse().unwrap();
    let req = Request::new("This message must echo from server", "!");

    let ctx = Context {
        addr,
        method: Method::Read,
        req,
    };
    println!("Client Recv: {}", recv_msg(ctx));

    let ctx = Context {
        addr,
        method: Method::Write,
        req: Request::new("", ""),
    };
    ctx.send_data(&mut b"Hello, Server!".as_ref()).unwrap();
}
