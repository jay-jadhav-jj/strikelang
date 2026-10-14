mod filehandling;
mod jcmderror;

use clap::{Parser, Subcommand};

use crate::{filehandling::get_code_from_jcmd, jcmderror::{Error, FILE_NOT_FOUND, throw}};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "c")]
    Compile {
        path: String,
        output: String
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Compile { path, output } => {
            match get_code_from_jcmd(&path) {
                Ok(code) => {
                    println!("{}", code);
                }

                Err(e) => {
                    throw(Error::from_code(&FILE_NOT_FOUND, None, &e));
                }
            }
        }
    }
}
