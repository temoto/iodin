fn main() {
    #[cfg(target_os = "linux")]
    main_linux()
}

#[cfg(target_os = "linux")]
fn main_linux() {
    use std::env;

    #[cfg(not(all(target_arch = "arm", target_os = "linux")))]
    {
        extern crate bindgen;
        use std::path::PathBuf;

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindgen::Builder::default()
            .header("wrapper.h")
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file(out_path.join("bindings-gen.rs"))
            .expect("Couldn't write bindings!");
    }

    extern crate cc;
    cc::Build::new()
        .flag("-pthread")
        .flag("-lrt")
        .pic(true)
        .opt_level(3)
        .warnings(false)
        .static_flag(true)
        .file("../pigpio/pigpio.c")
        .file("../pigpio/command.c")
        .include("../pigpio/")
        .compile("libpigpio");
}
