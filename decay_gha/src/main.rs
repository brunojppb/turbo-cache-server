extern crate clap;
mod commands;
use std::io::{self, Write};

use std::{
    fs::File,
    process::{self, Command},
};

use clap::Parser;
use commands::{Cli, Commands};

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
                    process::exit(0);
                }
                Err(err) => {
                    eprintln!("Could not start web server: {}", err);
                    process::exit(1);
                }
            }
        }
        Commands::Stop { pid } => {
            println!("Stopping process with pid: {}", pid);
            let output = Command::new("kill")
                .arg(&pid)
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
    }
}
