use clap::{Parser, Subcommand};

/// Decay GHA CLI
#[derive(Debug, Parser)]
#[command(name = "decay_gha")]
#[command(about = "Decay Github Actions CLI for the Decay server", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Starts the given Decay server and
    /// store its pid on the Github state
    #[command(arg_required_else_help = true)]
    Start {
        /// Path to the Decay server binary
        binary_path: String,
        /// Port to bind the Decay server to during startup
        port: String,
    },
    /// Stop a process with the given ID
    #[command(arg_required_else_help = true)]
    StopWithPid {
        /// The pid of the Decay server
        pid: String,
    },
    /// Stop a previously running Decay server with
    /// its pid stored on the Github Actions runner state
    Stop,
}
