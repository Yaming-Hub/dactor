fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/system.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");

    if std::env::var_os("PROTOC").is_none() {
        let protoc = protoc_bin_vendored::protoc_bin_path().map_err(|e| {
            std::io::Error::other(format!(
                "Failed to locate vendored protoc: {e}\n\
                 If PROTOC is explicitly set, ensure it points to a valid protoc binary."
            ))
        })?;
        std::env::set_var("PROTOC", protoc);
    }
    prost_build::compile_protos(&["proto/system.proto"], &["proto/"]).map_err(|e| {
        std::io::Error::other(format!(
            "Failed to compile proto/system.proto: {e}\n\
             If PROTOC is explicitly set, ensure it points to a valid protoc binary."
        ))
    })?;
    Ok(())
}
