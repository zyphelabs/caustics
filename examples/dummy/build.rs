use caustics_build::generate_caustics_client;

fn main() {
    if let Err(e) = generate_caustics_client(&["src"], "caustics_client_dummy.rs") {
        eprintln!("Error generating client: {}", e);
        std::process::exit(1);
    }
}
