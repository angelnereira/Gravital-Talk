//! Genera los stubs gRPC desde `proto/gravital/v1/*.proto` (feature `grpc`).

fn main() {
    println!("cargo:rerun-if-changed=../../proto/gravital/v1/server_control.proto");
    println!("cargo:rerun-if-changed=../../proto/gravital/v1/pairing.proto");

    #[cfg(feature = "grpc")]
    {
        let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
        std::env::set_var("PROTOC", protoc);

        tonic_build::configure()
            .build_client(true)
            .build_server(true)
            .compile_protos(
                &[
                    "../../proto/gravital/v1/server_control.proto",
                    "../../proto/gravital/v1/pairing.proto",
                ],
                &["../../proto"],
            )
            .expect("failed to compile gRPC protos");
    }
}
