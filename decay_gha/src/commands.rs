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
    /// Starts the given Decay server
    #[command(arg_required_else_help = true)]
    Start {
        /// Path to the Decay server binary
        binary_path: String,
        /// Port to bind the Decay server to during startup
        port: String,
    },
    /// Stop a previusly started Decay server using this CLI
    #[command(arg_required_else_help = true)]
    Stop {
        /// The pid of the Decay server
        pid: String,
    },
}
