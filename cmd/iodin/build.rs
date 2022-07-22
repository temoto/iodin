use protobuf_codegen::Codegen;

fn main() {
    let inputs = &[
        "../../protobuf/iodin.proto",
    ];
    Codegen::new()
        .out_dir("src/proto")
        .inputs(inputs)
        .include("../../protobuf")
        .run_from_script();
}
