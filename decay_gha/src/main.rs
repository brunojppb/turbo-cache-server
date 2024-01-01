use std::{
    fs::File,
    process::{self, Command},
};

fn main() {
    // @TODO: Use CLI args to get the server binary path
    let web_server = "./target/release/decay";

    let out_file = File::create("out.log").expect("out.log file could not be created");
    let err_file = File::create("err.log").expect("err.log file could not be created");
    match Command::new(web_server)
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
