pub const PI_DISABLE_FIFO_IF: u32 = 0;
pub const PI_DISABLE_SOCK_IF: u32 = 0;
pub const PI_INPUT: u32 = 0;
pub const PI_OUTPUT: u32 = 0;
pub const PI_WAVE_MODE_ONE_SHOT_SYNC: u32 = 0;

pub unsafe fn gpioCfgInterfaces(_: u32) -> i32 {
    0
}
pub unsafe fn gpioInitialise() -> i32 {
    0
}

pub unsafe fn gpioTick() -> u32 {
    0
}
pub unsafe fn gpioDelay(d: u32) -> u32 {
    d
}

pub unsafe fn gpioSerialReadOpen(_: u32, _: u32, _: u32) -> i32 {
    0
}
pub unsafe fn gpioSerialReadClose(_: u32) -> i32 {
    -1
}
pub unsafe fn gpioSerialRead(_: u32, _: *mut std::ffi::c_void, _: usize) -> i32 {
    -1
}

pub unsafe fn gpioWaveAddNew() -> i32 {
    0
}
pub unsafe fn gpioWaveAddSerial(
    _: u32,
    _: u32,
    _: u32,
    _: u32,
    _: u32,
    _: u32,
    _: *const ::std::os::raw::c_char,
) -> i32 {
    0
}
pub unsafe fn gpioWaveCreate() -> i32 {
    0
}
pub unsafe fn gpioWaveDelete(_: u32) -> i32 {
    -1
}
pub unsafe fn gpioWaveTxBusy() -> i32 {
    0
}
pub unsafe fn gpioWaveTxSend(_: u32, _: u32) -> i32 {
    -1
}
pub unsafe fn gpioWaveTxStop() -> i32 {
    0
}

pub unsafe fn gpioSetMode(_: u32, _: u32) -> i32 {
    0
}
pub unsafe fn gpioWrite(_: u32, _: u32) -> i32 {
    -1
}
