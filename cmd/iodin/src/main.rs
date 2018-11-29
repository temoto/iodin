// #![feature(test)]
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate pigpio;
extern crate protobuf;
extern crate stderrlog;

mod mdb;
mod proto;
mod server;

mod error {
    error_chain! {
        foreign_links {
            Env(::std::env::VarError);
            Fmt(::std::fmt::Error);
            IoError(::std::io::Error);
            NumParse(::std::num::ParseIntError);
            Protobuf(protobuf::ProtobufError);
            StringUtf8(::std::string::FromUtf8Error);
        }

        errors {
            MdbNak {
                description("MDB NAK")
                display("MDB received NAK, probably invalid command")
            }
            MdbInvalidResponse(b: u8) {
                description("MDB invalid response byte")
                display("MDB expected ACK/NAK, received unknown byte {:02x?}", b)
            }
            MdbChecksum(computed: u8, received: u8, response: Vec<u8>) {
                description("MDB invalid checksum")
                display("MDB invalid checksum recv={:02x} comp={:02x} response={:02x?}", received, computed, response.as_slice())
            }
        }
    }
}
use self::error::*;

#[cfg(test)]
mod tests {
    // extern crate test;
    use super::*;
    use crate::proto::iodin::*;
    use protobuf::Message;

    #[test]
    fn server_exec_zero_invalid_command() {
        let mut s = server::Server::new(true).unwrap();
        let req = Request::new();
        let mut resp = Response::new();
        let r = s.exec(&req, &mut resp);
        assert!(r.is_err());
        assert_eq!(r.err().unwrap().to_string(), "invalid command");
    }

    #[test]
    fn server_run_eof() {
        use crate::error::ErrorKind::Protobuf;

        let rv: Vec<u8> = Vec::new();
        let mut wv: Vec<u8> = Vec::new();
        let mut s = server::Server::new(true).unwrap();
        let r = s.run(&mut rv.as_slice(), &mut wv);
        assert!(r.is_err());
        let err = r.err().unwrap();
        assert!(match err {
            Error(Protobuf(protobuf::ProtobufError::IoError(e)), _) => {
                e.kind() == std::io::ErrorKind::UnexpectedEof
            }
            _ => false,
        });
    }

    #[test]
    fn server_run_stop_clean() {
        let mut request = Request::new();
        request.command = Request_Command::STOP;
        let mut rv: Vec<u8> = Vec::new();
        let mut wv: Vec<u8> = Vec::new();
        let mut os = protobuf::CodedOutputStream::vec(&mut rv);
        os.write_fixed32_no_tag(request.compute_size()).unwrap();
        request.write_to_with_cached_sizes(&mut os).unwrap();
        os.flush().unwrap();
        let mut s = server::Server::new(true).unwrap();
        let r = s.run(&mut rv.as_slice(), &mut wv);
        assert!(r.is_ok());
    }

    /*
        #[bench]
        fn bench_server_run_mdb_tx(b: &mut test::Bencher) {
            let mut request_open = Request::new();
            request_open.command = Request_Command::MDB_OPEN;
            request_open.arg_bytes = vec![15, 14];
            let mut request = Request::new();
            request.command = Request_Command::MDB_TX;
            request.arg_bytes = vec![0x01, 0x02, 0x03, 0x04];
            let mut request_stop = Request::new();
            request_stop.command = Request_Command::STOP;
            let mut rv: Vec<u8> = Vec::new();
            let mut wv: Vec<u8> = Vec::new();
            let mut os = protobuf::CodedOutputStream::vec(&mut rv);
            os.write_fixed32_no_tag(request_open.compute_size())
                .unwrap();
            request_open.write_to_with_cached_sizes(&mut os).unwrap();
            for _ in 0..1000 {
                os.write_fixed32_no_tag(request.compute_size()).unwrap();
                request.write_to_with_cached_sizes(&mut os).unwrap();
            }
            os.write_fixed32_no_tag(request_stop.compute_size())
                .unwrap();
            request_stop.write_to_with_cached_sizes(&mut os).unwrap();
            os.flush().unwrap();
            let mut s = server::Server::new(true).unwrap();
            let r = s.run(&mut rv.as_slice(), &mut wv);
            let mut response = Response::new();
            let mut is = protobuf::CodedInputStream::from_bytes(wv.as_slice());
            let len = is.read_fixed32().unwrap();
            let old_limit = is.push_limit(len.into()).unwrap();
            response.merge_from(&mut is).unwrap();
            is.pop_limit(old_limit);
            std::mem::drop(is);
            assert!(r.is_ok(), r.err().unwrap());
            test::black_box(r);
    
            b.iter(|| {
                wv.clear();
                let r = s.run(&mut rv.as_slice(), &mut wv);
                r
            });
        }
    */
}

fn main() {
    stderrlog::new()
        .quiet(false)
        .verbosity(3)
        .module(module_path!())
        .init()
        .unwrap();

    if let Err(ref e) = run() {
        // Write the top-level error message
        error!("error: {}", e);
        // Trace back through the chained errors
        for e in e.iter().skip(1) {
            error!("caused by: {}", e);
        }
        ::std::process::exit(1);
    }
}

/// The actual main(), but with the ability to use ? for easy early return
fn run() -> Result<()> {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    let mut stdin = unsafe { File::from_raw_fd(0) };
    let mut stdout = unsafe { File::from_raw_fd(1) };
    server::Server::new(false)?.run(&mut stdin, &mut stdout)?;
    Ok(())
}
