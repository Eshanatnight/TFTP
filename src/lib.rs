mod macros;
mod serialization;
mod interface;

use bin_layout::{Decoder, Encoder};
use macros::*;
use ErrorCod::*;
use Frame::*;

#[derive(Clone)]
struct Text(String);

enum ErrorCode {
    NotDefined,
    FileNotFound,
    AccessViolation,
    DiskFull,
    IllegalOperation,
    UnknownTransferID,
    FileAlreadyExists,
    NoSuchUser,
}

#[derive(Encoder, Decoder, Clone)]
struct Request {
    filename: Text,
    mode: Text,
}

enum Frame<'a> {
    Read(Request),
    Write(Request),
    Data { block: u16, bytes: &'a [u8] },
    Acknowledge(u16),
    ErrMsg { code: ErrorCode, msg: Text },
}

impl Request {
    pub fn new<I: Into<String>>(filename: I, mode: I) -> Self {
        Self {
            filename: Text(filename.into()),
            mode: Text(mode.into()),
        }
    }
}