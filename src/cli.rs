pub(crate) fn get_rustc_args() -> Vec<String> {
    vec![
        "".to_string(),
        "--edition=2021".to_string(),
        "test.rs".to_string(),
    ]
}
