fn main() {
    // Declare the nightly cfg condition
    println!("cargo:rustc-check-cfg=cfg(nightly)");
    
    // Check if we're on nightly Rust
    if let Ok(output) = std::process::Command::new("rustc").arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        if version.contains("nightly") {
            println!("cargo:rustc-cfg=nightly");
        }
    }
}
