use crate::error::*;
use crate::mdb;
use crate::proto::iodin::*;
use std::os::unix::net::UnixDatagram;
use std::time::Duration;

pub const MAX_MSG_SIZE: usize = 256;
pub const MDB_TIMEOUT: Duration = Duration::from_millis(300);

pub struct Server {
    mdb: Option<mdb::GpioMdb>,
    sock: UnixDatagram,
}

impl Server {
    pub fn new(s: UnixDatagram) -> Result<Self> {
        pigpio::init()?;
        Ok(Server { mdb: None, sock: s })
    }

    pub fn run(&mut self) -> Result<()> {
        use protobuf::Message;

        let mut buf: Vec<u8> = Vec::with_capacity(MAX_MSG_SIZE);
        loop {
            let msglen = self.sock.recv(&mut buf)?;
            let msg = &buf[..msglen];
            debug!("recv len={} buf={:02x?}", msglen, msg);

            let mut response = Response::new();
            match protobuf::parse_from_bytes::<Request>(msg) {
                Err(e) => {
                    error!("error protobuf parse: {}", e);
                    response.status = Response_Status::ERR_INPUT;
                    response.error = e.to_string();
                }
                Ok(request) => {
                    if let Err(e) = self.exec(&request, &mut response) {
                        error!("error: {}", e);
                        for e in e.iter().skip(1) {
                            error!("caused by: {}", e);
                        }
                    }
                }
            }
            buf.clear();
            match response.write_to_vec(&mut buf) {
                Ok(_) => {
                    if let Err(e) = self.sock.send(buf.as_slice()) {
                        error!("send response: {:?}", &e);
                    }
                }
                Err(e) => {
                    error!("response marshal: {:?}", &e);
                }
            };
        }
    }

    pub fn exec(&mut self, request: &Request, response: &mut Response) -> Result<()> {
        // debug!("exec {:x?}", request);
        match request.command {
            Request_Command::INVALID => {
                response.status = Response_Status::ERR_INPUT;
                response.error = "invalid command".to_string();
                return Err(response.error.clone().into());
            }
            Request_Command::MDB_OPEN => {
                self.mdb = None;
                if request.arg_bytes.len() != 2 {
                    response.status = Response_Status::ERR_INPUT;
                    response.error = "invalid arg_bytes".to_string();
                    return Err(response.error.clone().into());
                }
                let (rx, tx) = (request.arg_bytes[0], request.arg_bytes[1]);
                match mdb::GpioMdb::new(rx.into(), tx.into()) {
                    Ok(m) => {
                        self.mdb = Some(m);
                        response.status = Response_Status::OK;
                    }
                    Err(e) => {
                        response.status = Response_Status::ERR_HARDWARE;
                        response.error = e.to_string();
                        return Err(e);
                    }
                }
            }
            Request_Command::MDB_RESET => match &mut self.mdb {
                None => {
                    response.status = Response_Status::ERR_INPUT;
                    response.error = "must mdb_open".to_string();
                    return Err(response.error.clone().into());
                }
                Some(m) => {
                    if let Err(e) = m.bus_reset(Duration::from_millis(request.arg_uint.into())) {
                        response.status = Response_Status::ERR_HARDWARE;
                        response.error = e.to_string();
                        return Err(e);
                    }
                    response.status = Response_Status::OK;
                }
            },
            Request_Command::MDB_TX => match &mut self.mdb {
                None => {
                    response.status = Response_Status::ERR_INPUT;
                    response.error = "must mdb_open".to_string();
                    return Err(response.error.clone().into());
                }
                Some(m) => {
                    let mut mdb_response = Vec::with_capacity(mdb::BLOCK_MAX_LENGTH);
                    if let Err(e) = m.tx(&request.arg_bytes, &mut mdb_response, MDB_TIMEOUT) {
                        response.status = Response_Status::ERR_HARDWARE;
                        response.error = e.to_string();
                        return Err(e);
                    }
                    response.status = Response_Status::OK;
                }
            },
        };
        assert_ne!(response.status, Response_Status::INVALID);
        Ok(())
    }
}
