use std::{
    env::{self},
    fs::OpenOptions,
};

use std::io::prelude::*;

pub fn save_state(key: &str, value: &str) {
    let state = format!("{}={}", key, value);
    let gha_file_path = env::var("GITHUB_STATE")
        .unwrap_or_else(|_| panic!("Could not read GITHUB_STATE env variable"));
    println!("GHA state file: {}", gha_file_path);

    let mut gha_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(gha_file_path)
        .expect("GHA file should be available");

    if let Err(e) = writeln!(gha_file, "{}", &state) {
        panic!("Could not write to GHA state file: {}", e);
    }
}

pub fn get_state(key: &str) -> Result<String, env::VarError> {
    println!("Reading env 'STATE_{}'", key);
    env::var(format!("STATE_{}", key))
}
