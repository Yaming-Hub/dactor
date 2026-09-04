fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/system.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");

    let protoc_was_set = std::env::var_os("PROTOC").is_some();
    if !protoc_was_set {
        let protoc = protoc_bin_vendored::protoc_bin_path().map_err(|e| {
            std::io::Error::other(format!(
                "Failed to locate vendored protoc: {e}\nIf needed, set PROTOC to a valid protoc binary path."
            ))
        })?;
        std::env::set_var("PROTOC", protoc);
    }
    prost_build::compile_protos(&["proto/system.proto"], &["proto/"]).map_err(|e| {
        let hint = if protoc_was_set {
            "If you set PROTOC explicitly, ensure it points to a valid protoc binary."
        } else {
            "If you did not set PROTOC, this is likely a .proto syntax or import issue."
        };
        std::io::Error::other(format!(
            "Failed to compile proto/system.proto: {e}\n{hint}"
        ))
    })?;
    Ok(())
}
