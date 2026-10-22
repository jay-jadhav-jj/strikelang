mod filehandling;
mod strikeerror;
mod lexer;
mod parser;

use clap::{Parser, Subcommand};

use crate::{
    filehandling::get_code_from_jcmd, lexer::{Lexer, Token}, parser::AST, strikeerror::{Error, FILE_NOT_FOUND, Span, throw},
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

fn lex(code: &str, fname: &str) -> Result<Vec<(Token, Span)>, Error> {
    let mut lexer = Lexer::new(String::from(code), String::from(fname));
    lexer.lex()
}

fn parse(lex_res: Vec<(Token, Span)>, code: &str, fname: &str) -> Result<Vec<(AST, Span)>, Error> {
    let mut parser = parser::Parser::new(lex_res, String::from(fname), String::from(code));
    parser.parse()
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Compile { path, output } => match get_code_from_jcmd(&path) {
            Ok(code) => {
                match lex(&code, "stdin") {
                    Ok(v) => {
                        for (t, s) in &v {
                            println!("{:?} {}-{}", t, s.start, s.end);
                        }

                        match parse(v, &code, "stdin") {
                            Ok(ast) => {
                                for (node, s) in ast {
                                    println!("{:?} {}-{}", node, s.start, s.end);
                                }
                            }
                            
                            Err(e) => throw(e)
                        }
                    }

                    Err(e) => throw(e)
                }
            }

            Err(e) => {
                throw(Error::from_code(String::from("<invalid>"), String::new(), &FILE_NOT_FOUND, None, &e));
            }
        },
    }
}
