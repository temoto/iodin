#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
// #![allow(clippy::unreadable_literal)]
#![allow(unknown_lints)]
#![allow(unreadable_literal)]
#![allow(const_static_lifetime)]

#[cfg(all(target_arch = "arm", target_os = "linux"))]
include!("./bindings-arm-linux.rs");
#[cfg(not(all(target_arch = "arm", target_os = "linux")))]
include!(concat!(env!("OUT_DIR"), "/bindings-gen.rs"));
