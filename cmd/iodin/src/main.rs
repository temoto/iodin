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
            StringUtf8(::std::string::FromUtf8Error);
        }
    }
}
use self::error::*;

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
    use std::os::unix::io::{FromRawFd, RawFd};
    use std::os::unix::net::UnixDatagram;
    use std::time::Duration;

    let sock_fd: RawFd = std::env::var("sock_fd")?.parse::<i32>()?;
    let socket: UnixDatagram = unsafe { UnixDatagram::from_raw_fd(sock_fd) };
    socket.set_write_timeout(Some(Duration::from_millis(15000)))?;
    server::Server::new(socket)?.run()
}
