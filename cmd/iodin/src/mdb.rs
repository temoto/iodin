use crate::error::*;
use crate::pigpio;
use std::time::Duration;

pub const BLOCK_MAX_LENGTH: usize = 40;
pub const BAUD: u32 = 9600;
pub const DATA_BITS: u32 = 9;
pub const STOP_BITS: u32 = 2;
pub const RESPONSE_ACK: u8 = 0x00;
// pub const RESPONSE_RET: u8 = 0xaa;
pub const RESPONSE_NAK: u8 = 0xff;

// pigpio way to handle data_bits > 8
const WORD_SIZE: usize = 2;

const BUF_SIZE: usize = BLOCK_MAX_LENGTH * WORD_SIZE;
const TIMEOUT_SMALL_US: u32 = 10_000;

// time to send bits + 2 * inter-byte timeout (1ms)
const TIMEOUT_CHAR_US: u32 = ((1e6 as u32) * (DATA_BITS + 2/*start+stop*/) / BAUD) + 2000;
// time to receive whole response *not including* wait for first byte
const TIMEOUT_RECEIVE_US: u32 = BLOCK_MAX_LENGTH as u32 * TIMEOUT_CHAR_US;

#[inline]
fn mdb_wave_create(pin: u16, s: &[u8]) -> Result<pigpio::Wave> {
    const OFFSET: u32 = 0;
    let w = pigpio::Wave::new_serial(pin.into(), BAUD, DATA_BITS, STOP_BITS, OFFSET, s)?;
    Ok(w)
}

fn mdb_wave_send_wait(w: &pigpio::Wave, deadline: u32, wait_step: u32, err: &str) -> Result<()> {
    let start = pigpio::tick_since(0);
    let mut total: u32;
    w.send(pigpio::PI_WAVE_MODE_ONE_SHOT_SYNC)?;

    loop {
        unsafe { pigpio::gpioDelay(wait_step) };
        if !pigpio::wave_tx_busy()? {
            break;
        }
        total = pigpio::tick_since(start);
        if total > deadline {
            return Err(err.into());
        }
    }
    // check(unsafe { gpioWrite(self.tx_pin.into(), 0) })?;
    pigpio::check(unsafe { pigpio::gpioWaveTxStop() })?;
    // debug!("mdb_wave_send_wait time={}", total);
    Ok(())
}

pub struct GpioMdb {
    rx_pin: u16,
    tx_pin: u16,
    wave_ack: pigpio::Wave,
    wave_nak: pigpio::Wave,
    // wave_ret: Wave,
    wait_step: u32,
    buf: [u8; BUF_SIZE],
}

impl GpioMdb {
    pub fn new(rx_pin: u16, tx_pin: u16) -> Result<GpioMdb> {
        let wait_step: u32 = std::env::var("iodin_mdb_wait_step")
            .unwrap_or("101".to_string())
            .parse()
            .expect("env iodin_mdb_wait_step expect integer");

        debug!("GpioMdb::new rx={} tx={}", rx_pin, tx_pin);
        pigpio::check(unsafe { pigpio::gpioSetMode(rx_pin.into(), pigpio::PI_INPUT) })?;
        pigpio::check(unsafe { pigpio::gpioSetMode(tx_pin.into(), pigpio::PI_OUTPUT) })?;
        pigpio::check(unsafe { pigpio::gpioSerialReadOpen(rx_pin.into(), BAUD, DATA_BITS) })?;
        pigpio::check(unsafe { pigpio::gpioWaveTxStop() })?;

        let m = GpioMdb {
            rx_pin: rx_pin,
            tx_pin: tx_pin,
            wave_ack: mdb_wave_create(tx_pin, &[RESPONSE_ACK, 0x00])?,
            // wave_ret: mdb_wave_create(tx_pin, &[RESPONSE_RET, 0x00])?,
            wave_nak: mdb_wave_create(tx_pin, &[RESPONSE_NAK, 0x00])?,
            wait_step: wait_step,
            buf: unsafe { std::mem::uninitialized() },
        };
        Ok(m)
    }

    #[cold]
    pub fn close(&self) -> Result<()> {
        pigpio::check(unsafe { pigpio::gpioWaveTxStop() })?;
        pigpio::check(unsafe { pigpio::gpioSerialReadClose(self.rx_pin.into()) })?;
        Ok(())
    }

    pub fn bus_reset(&self, duration: Duration) -> Result<()> {
        if duration < Duration::from_millis(100) {
            // warn!("mdb bus_reset duration < 100ms as per MDB spec");
        }
        let duration_us: u32 = duration_as_micros32(duration)?; // FIXME use u32::try_from(duration.as_micro())?
        pigpio::check(unsafe { pigpio::gpioWrite(self.tx_pin.into(), 1) })?;
        unsafe { pigpio::gpioDelay(duration_us) };
        pigpio::check(unsafe { pigpio::gpioWrite(self.tx_pin.into(), 0) })?;
        Ok(())
    }

    pub fn tx(&mut self, request: &[u8], response: &mut Vec<u8>, timeout: Duration) -> Result<()> {
        const REQUEST_TIMEOUT: &str = "send request timeout";
        const NAK_TIMEOUT: &str = "send NAK timeout";
        const ACK_TIMEOUT: &str = "send ACK timeout";
        // const RET_TIMEOUT: &str = "send RET timeout";

        assert!(!request.is_empty());
        assert!(request.len() < BLOCK_MAX_LENGTH);
        assert!(response.capacity() >= BLOCK_MAX_LENGTH);

        debug!("mdb tx request={:02x?} timeout={:?}", request, timeout);
        self.buf[WORD_SIZE - 1] = 1; // first byte with 9bit set
        for (i, b) in request.iter().enumerate() {
            self.buf[i * WORD_SIZE] = *b;
        }
        let buf_req_len = request.len() * WORD_SIZE;
        self.buf[buf_req_len] = checksum(request);

        let wave = mdb_wave_create(self.tx_pin, &self.buf[..buf_req_len + WORD_SIZE])?;

        // Calculate deadlines after other CPU work, just before hardware IO.
        let timeout_us: u32 = duration_as_micros32(timeout)?; // FIXME use u32::try_from(timeout.as_micro())?
        let timeout_small_us: u32 = std::cmp::min(timeout_us, TIMEOUT_SMALL_US);
        let io_start_us = pigpio::tick_since(0);
        let receive_wait_deadline_us = io_start_us + timeout_us;
        let send_deadline_us: u32 = io_start_us + (TIMEOUT_CHAR_US * request.len() as u32);
        // TODO maybe yield to OS scheduler to reset process time slice?

        // critical section begin
        mdb_wave_send_wait(&wave, send_deadline_us, self.wait_step, REQUEST_TIMEOUT)?;

        let end_byte;
        let mut received_count = self.wait_receive(receive_wait_deadline_us, response)?;
        let receive_deadline = pigpio::tick_since(0) + TIMEOUT_RECEIVE_US;
        'receive: loop {
            for i in (0..received_count).step_by(2) {
                let (bvalue, bflag) = (self.buf[i], self.buf[i + 1]);
                if 1 == bflag {
                    end_byte = bvalue;
                    break 'receive;
                }
                response.push(bvalue);
            }
            received_count = self.wait_receive(receive_deadline, response)?;
        }
        if response.is_empty() {
            // received ACK/RET/NAK in end_byte
            match end_byte {
                RESPONSE_ACK => (),
                RESPONSE_NAK => return Err(ErrorKind::MdbNak.into()),
                _ => return Err(ErrorKind::MdbInvalidResponse(end_byte).into()),
            }
        } else {
            let deadline_us = pigpio::tick_since(0) + timeout_small_us;
            let computed_chk = checksum(response.as_slice());
            if end_byte != computed_chk {
                mdb_wave_send_wait(&self.wave_nak, deadline_us, self.wait_step, NAK_TIMEOUT)?;
                return Err(ErrorKind::MdbChecksum(computed_chk, end_byte, response.clone()).into());
            } else {
                mdb_wave_send_wait(&self.wave_ack, deadline_us, self.wait_step, ACK_TIMEOUT)?;
            }
        }
        // critical section end

        debug!(
            "mdb tx success request={:02x?} response={:02x?} io_time={}us",
            request,
            response.as_slice(),
            pigpio::tick_since(io_start_us),
        );
        Ok(())
    }

    #[inline(always)]
    fn wait_receive(&mut self, deadline: u32, debug_response: &[u8]) -> Result<usize> {
        loop {
            let n = pigpio::check(unsafe {
                pigpio::gpioSerialRead(
                    self.rx_pin.into(),
                    self.buf.as_mut_ptr() as *mut std::ffi::c_void,
                    self.buf.len(),
                )
            })? as usize;
            // debug!("mdb serial_read n={}/{} buf={:x?}", n, self.buf.len(), &self.buf[..n]);
            if n > 0 {
                return Ok(n);
            }
            if pigpio::tick_since(0) > deadline {
                debug!("mdb response(part)={:02x?}", debug_response);
                return Err("recv timeout".into());
            }
            unsafe { pigpio::gpioDelay(self.wait_step) };
        }
    }
}

impl Drop for GpioMdb {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[inline]
fn checksum(b: &[u8]) -> u8 {
    b.iter().fold(0, |sum, &x| sum.wrapping_add(x))
}

// FIXME use u32::try_from(Duration.as_micro())?
fn duration_as_micros32(d: Duration) -> Result<u32> {
    const OVERFLOW: &str = "duration_as_micros32 overflow";
    let x: u64 = d
        .as_secs()
        .checked_mul(1_000_000)
        .ok_or(OVERFLOW)?
        .checked_add(d.subsec_micros().into())
        .ok_or(OVERFLOW)?;
    if x >= u32::max_value().into() {
        return Err(OVERFLOW.into());
    }
    Ok(x as u32)
}
