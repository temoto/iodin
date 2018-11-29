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
    }
}
use self::error::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::iodin::*;
    use std::os::unix::net::UnixDatagram;

    #[test]
    fn server_exec_zero_invalid_command() {
        let (_sock1, sock2) = UnixDatagram::pair().unwrap();
        let mut s = server::Server::new(sock2, true).unwrap();
        let req = Request::new();
        let mut resp = Response::new();
        let r = s.exec(&req, &mut resp);
        assert!(r.is_err());
        assert_eq!(r.err().unwrap().to_string(), "invalid command");
    }
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

        // Exit with a nonzero exit code
        // TODO: Decide how to allow code to set this to something other than 1
        ::std::process::exit(1);
    }
}

/// The actual main(), but with the ability to use ? for easy early return
fn run() -> Result<()> {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    let mut stdin = unsafe{File::from_raw_fd(0)};
    let mut stdout = unsafe{File::from_raw_fd(1)};
    server::Server::new(false)?.run(&mut stdin, &mut stdout)?;
    Ok(())
}
