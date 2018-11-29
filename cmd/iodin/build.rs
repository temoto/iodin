extern crate protoc_rust;

use protoc_rust::Customize;

fn main() {
    let input = "../../protobuf/iodin.proto";
    println!("rerun-if-changed={}", input);
    protoc_rust::run(protoc_rust::Args {
        out_dir: "src/proto",
        input: &[input],
        includes: &["../../protobuf"],
        customize: Customize {
            ..Default::default()
        },
    })
    .expect("protoc");
}
