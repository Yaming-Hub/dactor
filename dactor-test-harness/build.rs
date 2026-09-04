fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/test_node.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");

    let protoc_env = std::env::var_os("PROTOC");
    let protoc_was_set = protoc_env.is_some();
    let protoc = if let Some(protoc) = protoc_env {
        std::path::PathBuf::from(protoc)
    } else {
        protoc_bin_vendored::protoc_bin_path().map_err(|e| {
            std::io::Error::other(format!(
                "Failed to locate vendored protoc: {e}\nIf needed, set PROTOC to a valid protoc binary path."
            ))
        })?
    };

    let mut config = tonic_build::Config::new();
    config.protoc_executable(protoc);
    tonic_build::configure()
        .compile_protos_with_config(config, &["proto/test_node.proto"], &["proto/"])
        .map_err(|e| {
            let hint = if protoc_was_set {
                "If you set PROTOC explicitly, ensure it points to a valid protoc binary."
            } else {
                "If you did not set PROTOC, this is likely a .proto syntax or import issue."
            };
            std::io::Error::other(format!(
                "Failed to compile proto/test_node.proto: {e}\n{hint}"
            ))
        })?;
    Ok(())
}
