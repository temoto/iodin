use crate::error::*;
use crate::mdb;
use crate::proto::iodin::*;
use std::time::Duration;
use std::io;

pub const MAX_MSG_SIZE: usize = 256;
pub const MDB_TIMEOUT: Duration = Duration::from_millis(300);

pub struct Server {
    mdb: Option<mdb::GpioMdb>,
}

impl Server {
    pub fn new(mock: bool) -> Result<Self> {
        if !mock {
            pigpio::init(pigpio::PI_DISABLE_FIFO_IF | pigpio::PI_DISABLE_SOCK_IF)?;
        }
        Ok(Server { mdb: None })
    }

    pub fn run(&mut self, mut r: &mut io::Read, mut w: &mut io::Write) -> Result<()> {
        use protobuf::Message;

        let mut is = protobuf::CodedInputStream::new(&mut r);
        let mut os = protobuf::CodedOutputStream::new(&mut w);
        loop {
            let mut request = Request::new();
            let mut response = Response::new();

            let msglen = is.read_fixed32()?;
            eprintln!("msglen={}", msglen);
            let old_limit = is.push_limit(msglen.into())?;
            match request.merge_from(&mut is) {
                Err(e) => {
                    error!("error protobuf parse: {}", e);
                    response.status = Response_Status::ERR_INPUT;
                    response.error = e.to_string();
                }
                Ok(()) => {
                    if let Err(e) = self.exec(&request, &mut response) {
                        error!("error: {}", e);
                        for e in e.iter().skip(1) {
                            error!("caused by: {}", e);
                        }
                    }
                }
            }
            is.pop_limit(old_limit);

            os.write_fixed32_no_tag(response.compute_size())?;
            response.write_to(&mut os)?;
            os.flush()?;
        }
    }

    pub fn exec(&mut self, request: &Request, response: &mut Response) -> Result<()> {
        debug!("exec {:x?}", request);
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
