mod filehandling;
mod strikeerror;
mod lexer;
mod parser;
mod l64compiler;

use clap::{Parser, Subcommand};

use crate::{
    filehandling::{filename, get_code_from_jcmd, write}, lexer::{Lexer, Token}, parser::AST, strikeerror::{Error, FILE_ERR, Span, throw},
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

fn l64compile(parse_res: Vec<(AST, Span)>, code: &str, fname: &str) -> Result<String, Error> {
    let mut compiler = l64compiler::Compiler::new(String::from(fname), String::from(code), parse_res);
    compiler.compile()
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Compile { path, output } => match get_code_from_jcmd(&path) {
            Ok(code) => {
                let fname: &str = match filename(&path) {
                    Ok(f) => &f.clone(),
                    Err(e) => throw(Error::from_code(String::from("<invalid>"), String::new(), &FILE_ERR, None, &e))
                };

                match lex(&code, fname) {
                    Ok(v) => {
                        for (t, s) in &v {
                            println!("{:?} {}-{}", t, s.start, s.end);
                        };

                        match parse(v, &code, fname) {
                            Ok(ast) => {
                                for (node, s) in &ast {
                                    println!("{:?} {}-{}", node, s.start, s.end);
                                }

                                match l64compile(ast, &code, fname) {
                                    Ok(t) => {
                                        match write(&(output + ".asm"), &t) {
                                            Ok(_) => println!("Done"),

                                            Err(e) => throw(Error::from_code(String::from(fname), String::new(), &FILE_ERR, None, "Unable to create assembly file"))
                                        };
                                    }

                                    Err(e) => throw(e)
                                }
                            }
                            
                            Err(e) => throw(e)
                        };
                    }

                    Err(e) => throw(e)
                }
            }

            Err(e) => {
                throw(Error::from_code(String::from("<invalid>"), String::new(), &FILE_ERR, None, &e));
            }
        },
    }
}
