pub(crate) fn automatic_application_updates() -> bool {
    static CONFIG: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../traffic.edition.json"
        )))
        .unwrap_or_default()
    });
    CONFIG["automaticApplicationUpdates"].as_bool().unwrap_or(false)
}
