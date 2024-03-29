use crate::error::*;
use crate::mdb;
use crate::proto::iodin::request::Command;
use crate::proto::iodin::response::Status;
use crate::proto::iodin::*;
use std::convert::TryInto;
use std::io;
use std::time::Duration;

pub const MDB_TIMEOUT: Duration = Duration::from_millis(300);

pub struct Server {
    mdb: Option<mdb::GpioMdb>,
    mock: bool,
    running: bool,
}

impl Server {
    pub fn new(mock: bool) -> Result<Self> {
        if !mock {
            pigpio::init(pigpio::PI_DISABLE_FIFO_IF | pigpio::PI_DISABLE_SOCK_IF)?;
        }
        Ok(Server {
            mdb: None,
            mock: mock,
            running: false,
        })
    }

    pub fn run(&mut self, mut r: &mut dyn io::Read, mut w: &mut dyn io::Write) -> Result<()> {
        use protobuf::Message;

        let mut is = protobuf::CodedInputStream::new(&mut r);
        let mut os = protobuf::CodedOutputStream::new(&mut w);
        self.running = true;
        while self.running {
            let mut request = Request::new();
            let mut response = Response::new();

            let msglen = is.read_fixed32()?;
            let old_limit = is.push_limit(msglen.into())?;
            match request.merge_from(&mut is) {
                Err(e) => {
                    error!("error protobuf parse: {}", e);
                    response.status = Status::ERR_INPUT.into();
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

            os.write_fixed32_no_tag(response.compute_size().try_into().unwrap())?;
            response.write_to(&mut os)?;
            os.flush()?;
        }
        Ok(())
    }

    pub fn exec(&mut self, request: &Request, response: &mut Response) -> Result<()> {
        // debug!("exec {:x?}", request);
        match request.command.enum_value_or_default() {
            Command::INVALID => {
                response.status = Status::ERR_INPUT.into();
                response.error = "invalid command".to_string();
                return Err(response.error.clone().into());
            }
            Command::STOP => {
                self.running = false;
                response.status = Status::OK.into();
                return Ok(());
            }
            Command::MDB_OPEN => {
                self.mdb = None;
                if request.arg_bytes.len() != 2 {
                    response.status = Status::ERR_INPUT.into();
                    response.error = "invalid arg_bytes".to_string();
                    return Err(response.error.clone().into());
                }
                let (rx, tx) = (request.arg_bytes[0], request.arg_bytes[1]);
                match mdb::GpioMdb::new(rx.into(), tx.into()) {
                    Ok(m) => {
                        self.mdb = Some(m);
                        response.status = Status::OK.into();
                    }
                    Err(e) => {
                        response.status = Status::ERR_HARDWARE.into();
                        response.error = e.to_string();
                        return Err(e);
                    }
                }
            }
            Command::MDB_RESET => match &mut self.mdb {
                None => {
                    response.status = Status::ERR_INPUT.into();
                    response.error = "must mdb_open".to_string();
                    return Err(response.error.clone().into());
                }
                Some(m) => {
                    if let Err(e) = m.bus_reset(Duration::from_millis(request.arg_uint.into())) {
                        response.status = Status::ERR_HARDWARE.into();
                        response.error = e.to_string();
                        return Err(e);
                    }
                    response.status = Status::OK.into();
                }
            },
            Command::MDB_TX => match &mut self.mdb {
                None => {
                    response.status = Status::ERR_INPUT.into();
                    response.error = "must mdb_open".to_string();
                    return Err(response.error.clone().into());
                }
                Some(m) => {
                    let mut mdb_response = Vec::with_capacity(mdb::BLOCK_MAX_LENGTH);
                    if self.mock {
                        mdb_response.extend_from_slice(&request.arg_bytes);
                    } else {
                        if let Err(e) = m.tx(&request.arg_bytes, &mut mdb_response, MDB_TIMEOUT) {
                            response.status = Status::ERR_HARDWARE.into();
                            response.error = e.to_string();
                            return Err(e);
                        }
                    }
                    response.status = Status::OK.into();
                    response.data_bytes.append(&mut mdb_response);
                }
            },
        };
        assert_ne!(response.status.enum_value_or_default(), Status::INVALID);
        Ok(())
    }
}
