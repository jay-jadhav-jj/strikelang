use super::lexer::{Token, Value};

// Native JCMDLANG types
enum Type {
    Int,
    String,
    List
}

enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow
}

// Store locations with AST nodes
pub enum AST {
    VarSet {
        id: String,
        expr: Box<AST>
    },
    VarGet {
        id: String
    },
    BinOp {
        right: Box<AST>,
        op: Op,
        left: Box<AST>
    },
    UnaryOp {
        op: Op,
        expr: Box<AST>
    },
    Statement {
        stmt: Box<AST>
    },
    ForLoop {
        item_id: Option<String>,
        list: Box<AST>,
        body: Vec<Box<AST>>
    },
    Exit {
        code: Box<AST>
    },
    ASM {
        raw: String,
        replace_locs: Vec<usize>,
        replace_expr: Vec<Box<AST>>
    },
    MacroDef {
        name: String,
        params: Vec<(String, Type)>,
        body: Vec<Box<AST>>
    },
    MicroDef {
        name: String,
        params: Vec<(String, Type)>,
        body: Vec<Box<AST>>,
        rettype: Type
    },
    MacroCall {
        name: String,
        args: Vec<Box<AST>>
    },
    MicroCall {
        name: String,
        args: Vec<Box<AST>>
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    program: Vec<AST>
}
