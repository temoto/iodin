use crate::error::*;
use crate::pigpio::*;
use std::time::Duration;

pub const BLOCK_MAX_LENGTH: usize = 40;
pub const BAUD: u32 = 9600;
pub const DATA_BITS: u32 = 9;
pub const STOP_BITS: u32 = 2;

// pigpio way to handle data_bits > 8
const WORD_SIZE: usize = 2;

fn mdb_wave_create(pin: u16, s: &[u8]) -> Result<Wave> {
    const OFFSET: u32 = 0;
    let w = Wave::new_serial(pin.into(), BAUD, DATA_BITS, STOP_BITS, OFFSET, s)?;
    Ok(w)
}

fn mdb_wave_send_wait(w: &Wave, deadline: u32, err: &str) -> Result<()> {
    let start = tick_since(0);
    let mut total: u32;
    w.send(PI_WAVE_MODE_ONE_SHOT_SYNC)?;

    loop {
        unsafe { gpioDelay(SEND_WAIT_STEP) };
        if !wave_tx_busy()? {
            break;
        }
        total = tick_since(start);
        if total > deadline {
            return Err(err.into());
        }
    }
    // check(unsafe { gpioWrite(self.tx_pin.into(), 0) })?;
    check(unsafe { gpioWaveTxStop() })?;
    // debug!("mdb wait time={}", total);
    Ok(())
}

pub struct GpioMdb {
    rx_pin: u16,
    tx_pin: u16,
    wave_ack: Wave,
    wave_nak: Wave,
    // wave_ret: Wave,
}

impl GpioMdb {
    pub fn new(rx_pin: u16, tx_pin: u16) -> Result<GpioMdb> {
        debug!("GpioMdb::new rx={} tx={}", rx_pin, tx_pin);
        check(unsafe { gpioSetMode(rx_pin.into(), PI_INPUT) })?;
        check(unsafe { gpioSetMode(tx_pin.into(), PI_OUTPUT) })?;
        check(unsafe { gpioSerialReadOpen(rx_pin.into(), BAUD, DATA_BITS) })?;
        check(unsafe { gpioWaveTxStop() })?;

        let m = GpioMdb {
            rx_pin: rx_pin,
            tx_pin: tx_pin,
            wave_ack: mdb_wave_create(tx_pin, &[0x00, 0x00])?,
            wave_nak: mdb_wave_create(tx_pin, &[0xaa, 0x00])?,
            // wave_ret: mdb_wave_create(tx_pin, &[0xff, 0x00])?,
        };
        Ok(m)
    }

    pub fn close(&self) -> Result<()> {
        check(unsafe { gpioWaveTxStop() })?;
        check(unsafe { gpioSerialReadClose(self.rx_pin.into()) })?;
        Ok(())
    }

    pub fn bus_reset(&self, duration: Duration) -> Result<()> {
        assert!(duration < Duration::from_secs(1));
        let duration_us: u32 = duration.subsec_micros();
        check(unsafe { gpioWrite(self.tx_pin.into(), 1) })?;
        unsafe { gpioDelay(duration_us) };
        check(unsafe { gpioWrite(self.tx_pin.into(), 0) })?;
        Ok(())
    }

    pub fn tx(&self, request: &[u8], response: &mut Vec<u8>, timeout: Duration) -> Result<()> {
        assert!(request.len() < BLOCK_MAX_LENGTH);
        assert!(timeout < Duration::from_secs(1));
        let start_us = tick_since(0);
        let timeout_us: u32 = timeout.subsec_micros();
        let mut buf = [0u8; BLOCK_MAX_LENGTH * WORD_SIZE];

        buf[WORD_SIZE - 1] = 1; // first byte with 9bit set
        for (i, b) in request.iter().enumerate() {
            buf[i * WORD_SIZE] = *b;
        }
        let buf_req_len = request.len() * WORD_SIZE;
        buf[buf_req_len] = checksum(request);

        {
            let request_buf = &buf[..buf_req_len + WORD_SIZE];
            debug!("mdb tx reqbuf={:x?}", request_buf);
            let wave = mdb_wave_create(self.tx_pin, request_buf)?;
            mdb_wave_send_wait(
                &wave,
                tick_since(start_us) + timeout_us,
                "send request timeout",
            )?;
        }

        let mut chk: u8 = 0;
        let mut read_chk: u8 = 0;
        let mut read_finished = false;
        let mut response_len = 0;
        while !read_finished {
            let n = check(unsafe {
                gpioSerialRead(
                    self.rx_pin.into(),
                    buf.as_mut_ptr() as *mut std::ffi::c_void,
                    buf.len(),
                )
            })? as usize;
            // debug!("mdb tx read n={} buf={:x?}", n, &buf[..n]);
            if n == 0 {
                if tick_since(start_us) > timeout_us {
                    return Err("recv timeout".into());
                }
                unsafe { gpioDelay(SEND_WAIT_STEP) };
                continue;
            }
            for i in (0..n).step_by(2) {
                let (b1, b2) = (buf[i], buf[i + 1]);
                if 1 == b2 {
                    read_chk = b1;
                    read_finished = true;
                    break;
                }
                chk = chk.wrapping_add(b1);
                response.push(b1);
                response_len += 1;
            }
        }

        if read_chk != chk {
            // TODO send NAK/RET
            mdb_wave_send_wait(
                &self.wave_nak,
                tick_since(start_us) + timeout_us,
                "send NAK timeout",
            )?;
            return Err(format!("invalid checksum recv={:02x} comp={:02x}", read_chk, chk).into());
        }

        if response_len > 0 {
            mdb_wave_send_wait(
                &self.wave_ack,
                tick_since(start_us) + timeout_us,
                "send NAK timeout",
            )?;
        }

        debug!(
            "mdb tx total time={} response={:02x?}\n\n",
            tick_since(start_us),
            response.as_slice(),
        );
        Ok(())
    }
}

impl Drop for GpioMdb {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn checksum(b: &[u8]) -> u8 {
    b.iter().fold(0, |sum, &x| sum.wrapping_add(x))
}
