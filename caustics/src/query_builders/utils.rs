/// Extract entity name from a path string representation
pub(crate) fn extract_entity_name_from_path(path_str: &str) -> String {
    // The path is stored as a debug representation. Find all occurrences of ident: "..."
    let mut last_entity_name = "unknown".to_string();
    let mut pos = 0;

    while let Some(start) = path_str[pos..].find("ident: \"") {
        let full_start = pos + start + 8; // Skip "ident: \""
        if let Some(end) = path_str[full_start..].find('"') {
            let entity_name = path_str[full_start..full_start + end].to_string();
            last_entity_name = entity_name;
            pos = full_start + end + 1;
        } else {
            break;
        }
    }

    last_entity_name
}

