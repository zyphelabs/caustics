// Minimal dummy crate that tests caustics client generation


// Include the generated client
include!(concat!(env!("OUT_DIR"), "/caustics_client_dummy.rs"));

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic]
    fn test_create_caustics_client() {
        crate::CausticsClient::new(caustics::prelude::DatabaseConnection::default());
    }
}
