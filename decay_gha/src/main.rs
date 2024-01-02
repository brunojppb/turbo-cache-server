extern crate clap;
mod cli_commands;
mod gha_commands;
use std::io::{self, Write};

use std::{
    fs::File,
    process::{self, Command},
};

use clap::Parser;
use cli_commands::{Cli, Commands};
use gha_commands::get_state;

use crate::gha_commands::save_state;

const PID_STATE_KEY: &str = "decay-pid";

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Start { binary_path, port } => {
            println!("Starting binary at {} on port {}", binary_path, port);
            let out_file = File::create("out.log").expect("out.log file could not be created");
            let err_file = File::create("err.log").expect("err.log file could not be created");
            match Command::new(binary_path)
                .env("PORT", port)
                .stderr(err_file)
                .stdout(out_file)
                .spawn()
            {
                Ok(child_process) => {
                    println!("Starting up server with pid {}", child_process.id());
                    save_state(PID_STATE_KEY, &child_process.id().to_string());
                    process::exit(0);
                }
                Err(err) => {
                    eprintln!("Could not start web server: {}", err);
                    process::exit(1);
                }
            }
        }
        Commands::StopWithPid { pid } => {
            stop_process(&pid);
        }
        Commands::Stop => {
            let pid = get_state(PID_STATE_KEY)
                .unwrap_or_else(|_| panic!("pid could not be read from GHA state"));
            stop_process(&pid);
        }
    }
}

fn stop_process(pid: &str) {
    println!("Stopping process with pid: {}", pid);
    let output = Command::new("kill")
        .arg(pid)
        .output()
        .unwrap_or_else(|_| panic!("Could not stop decay server with pid {}", &pid));
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    if output.status.success() {
        process::exit(0);
    } else {
        process::exit(1);
    }
}
