//! Binary `agentgate`: dijalankan user di laptopnya untuk memasang aturan ke repo,
//! meng-index kode, dan menjalankan runner job.

use clap::Parser;

#[derive(Parser)]
#[command(name = "agentgate", version = agentgate_core::VERSION, about = "Agent Gate CLI")]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
