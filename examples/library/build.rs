use caustics_build::generate_caustics_client;

fn main() {
    // Use the caustics-build library to generate client code
    // This demonstrates how examples can reuse caustics build functionality

    // Generate main client
    if let Err(e) = generate_caustics_client(&["src"], "caustics_client_library.rs") {
        eprintln!("Error generating main client: {}", e);
        std::process::exit(1);
    }
}
