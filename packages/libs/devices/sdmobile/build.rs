// Compiles the reconstructed VSD2 schema. A vendored protoc keeps the build
// working on machines without a system protobuf installation.

fn main() {
    if std::env::var_os("PROTOC").is_none()
        && let Ok(path) = protoc_bin_vendored::protoc_bin_path()
    {
        // SAFETY: build scripts are single threaded and this runs before any
        // other thread is created.
        unsafe {
            std::env::set_var("PROTOC", path);
        }
    }

    println!("cargo:rerun-if-changed=proto/vsd2.proto");
    prost_build::Config::new()
        .compile_protos(&["proto/vsd2.proto"], &["proto"])
        .expect("compile proto/vsd2.proto");
}
