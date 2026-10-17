mod filehandling;
mod jcmderror;
mod lexer;
mod parser;

use clap::{Parser, Subcommand};

use crate::{
    filehandling::get_code_from_jcmd,
    jcmderror::{throw, Error, FILE_NOT_FOUND},
    lexer::{Lexer, Token},
};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "c")]
    Compile { path: String, output: String },
}

fn lex(code: &str, fname: &str) -> Result<Vec<(Token, String)>, Error> {
    let mut lexer = Lexer::new(String::from(code), String::from(fname));
    lexer.lex()
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Compile { path, output } => match get_code_from_jcmd(&path) {
            Ok(code) => {
                let (tokens, token_locations): (Vec<Token>, Vec<String>) = match lex(&code, &path) {
                    Ok(res) => res.into_iter().unzip(),
                    Err(e) => {
                        throw(e);
                    }
                };

                for tok in tokens {
                    println!("{:?}", tok);
                }
            }

            Err(e) => {
                throw(Error::from_code(&FILE_NOT_FOUND, None, &e));
            }
        },
    }
}
