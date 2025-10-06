use caustics_build::generate_client_for_external_project;

fn main() {
    // Use the caustics-build library to generate client code
    // This demonstrates how examples can reuse caustics build functionality

    // Generate main client
    if let Err(e) = generate_client_for_external_project(&["src"], "caustics_client_school.rs") {
        eprintln!("Error generating main client: {}", e);
        std::process::exit(1);
    }
}
