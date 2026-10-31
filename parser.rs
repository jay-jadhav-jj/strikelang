use crate::filehandling::get_code_from_jcmd;
use crate::lexer::Lexer;
use crate::parser::AST::ProcDef;

use super::lexer::{Token, Value};
use super::strikeerror::*;
use std::fmt;

// Native JCMDLANG types; JUST TYPES not values
#[derive(Clone, PartialEq, Debug)]
pub enum Type {
    Int,
    Str,
    List(Vec<Type>),
    Bool,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "INT"),
            Type::Str => write!(f, "STR"),
            Type::List(types) => {
                write!(f, "LIST(")?;

                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{ty}")?;
                }

                write!(f, ")")
            }
            Type::Bool => write!(f, "BOOL"),
        }
    }
}

impl Type {
    pub fn val_is(&self, v: Value) -> bool {
        match self {
            Type::Int => {
                matches!(v, Value::Int(..))
            }

            Type::Str => {
                matches!(v, Value::Str(..))
            }

            Type::Bool => {
                matches!(v, Value::Bool(..))
            }
            _ => false,
        }
    }
}

// Store locations with AST nodes
#[derive(Clone, Debug, PartialEq)]
pub enum AST {
    VarSet {
        id: String,
        expr: Box<AST>,
    },
    VarGet {
        id: String,
    },
    BinOp {
        left: Box<AST>,
        op: Token,
        right: Box<AST>,
    },
    UnaryOp {
        op: Token,
        expr: Box<AST>,
    },
    ForLoop {
        item_id: Option<String>,
        list: Box<AST>,
        body: Vec<Box<AST>>,
    },
    Exit {
        code: Box<AST>,
    },
    ASM {
        body: String,
    },
    ProcDef {
        name: String,
        params: Vec<(String, Type)>,
        body: Vec<Box<AST>>,
    },
    FuncDef {
        name: String,
        params: Vec<(String, Type)>,
        body: Vec<Box<AST>>,
        rettype: Type,
    },
    ProcCall {
        name: String,
        args: Vec<Box<AST>>,
    },
    FuncCall {
        name: String,
        args: Vec<Box<AST>>,
    },
    Value {
        val: Value,
    },
    ReqStr {
        expr: Box<AST>,
    },
    ReqMutStr {
        expr: Box<AST>,
    },
    If {
        cond: Box<AST>,
        body: Vec<Box<AST>>,                      // main if body
        branches: Vec<(Box<AST>, Vec<Box<AST>>)>, // elifs
        elsebranch: Option<Vec<Box<AST>>>,
    },
    IsStatic {
        expr: Box<AST>, // Can take any value (e.g isstatic<1 + 2> OR isstatic<"Static string">)
    },
    DynaStr {
        str_addr: Box<AST>, // Developer must make sure addr + _len is where length is stored
        size: Box<AST>, // Capacity to know at compile time for buffers
    },
    DynaAddr {
        expr: Box<AST>,
    },
    SCast {
        expr: Box<AST>,
        t: Type,
    },
}

#[derive(Clone)]
struct ParserScope {
    procs: Vec<AST>,
    funcs: Vec<AST>,
    vars: Vec<(String, Type)>,
}

impl ParserScope {
    pub fn new() -> ParserScope {
        ParserScope {
            procs: Vec::new(),
            funcs: Vec::new(),
            vars: Vec::new(),
        }
    }
}

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    program: Vec<(AST, Span)>,
    idx: usize,
    scopes: Vec<ParserScope>,
    filename: String,
    raw: String,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>, fname: String, rawcode: String) -> Parser {
        Parser {
            tokens: tokens,
            program: Vec::new(),
            idx: 0,
            scopes: Vec::new(),
            filename: fname,
            raw: rawcode,
        }
    }

    fn error(&self, code: u16, start: usize, end: Option<usize>, msg: &str) -> Error {
        Error::from_code(
            self.filename.clone(),
            self.raw.clone(),
            &code,
            Some(Span::new(start, end.unwrap_or(self.location().end))),
            msg,
        )
    }

    fn not_defined_err(&self, start: usize, end: Option<usize>, id: String) -> Error {
        Error::not_defined_err(
            self.filename.clone(),
            self.raw.clone(),
            Span::new(start, end.unwrap_or(self.location().end)),
            &id,
        )
    }

    fn flatten_scopes(&self) -> ParserScope {
        let mut flattened = ParserScope::new();

        for scope in self.scopes.iter().rev() {
            for lvar in &scope.vars {
                let mut exists = false;
                for var in &flattened.vars {
                    if var.0 == lvar.0 {
                        exists = true;
                        break;
                    }
                }
                if !exists {
                    flattened.vars.push(lvar.clone());
                }
            }

            for lproc in &scope.procs {
                let mut exists = false;
                for proc in &flattened.procs {
                    if let AST::ProcDef { name, .. } = proc {
                        if let AST::ProcDef { name: lname, .. } = lproc {
                            if *name == *lname {
                                exists = true;
                                break;
                            }
                        }
                    }
                }
                if !exists {
                    flattened.procs.push(lproc.clone());
                }
            }

            for lfunc in &scope.funcs {
                let mut exists = false;
                for func in &flattened.funcs {
                    if let AST::FuncDef { name, .. } = func {
                        if let AST::FuncDef { name: lname, .. } = lfunc {
                            if *name == *lname {
                                exists = true;
                                break;
                            }
                        }
                    }
                }
                if !exists {
                    flattened.funcs.push(lfunc.clone());
                }
            }
        }

        flattened
    }

    fn find_proc(&self, procname: &str) -> Option<AST> {
        for m in self.flatten_scopes().procs {
            if let AST::ProcDef { name, .. } = &m {
                if name == procname {
                    return Some(m.clone());
                }
            }
        }

        None
    }

    fn find_func(&self, fnname: &str) -> Option<AST> {
        for m in self.flatten_scopes().funcs {
            if let AST::FuncDef {
                name,
                params: _,
                body: _,
                rettype: _,
            } = &m
            {
                if name == fnname {
                    return Some(m.clone());
                }
            }
        }

        None
    }

    fn find_var(&self, varname: &str) -> Option<(String, Type)> {
        for var in self.flatten_scopes().vars {
            if var.0 == varname {
                return Some(var.clone());
            }
        }

        None
    }

    fn location(&self) -> Span {
        self.get_token_span()
            .unwrap_or(Span::new(self.raw.len(), self.raw.len()))
    }

    fn get_token(&self) -> Option<Token> {
        if self.idx < self.tokens.len() {
            return Some(self.tokens[self.idx].0.clone());
        }

        None
    }

    fn get_token_span(&self) -> Option<Span> {
        if self.idx < self.tokens.len() {
            return Some(self.tokens[self.idx].1);
        }
        None
    }

    fn next_token(&mut self) -> Option<Token> {
        if self.idx < self.tokens.len() {
            let ret = Some(self.tokens[self.idx].0.clone());
            self.idx += 1;
            return ret;
        }

        None
    }

    fn get_type(&self, node: &AST) -> Option<Type> {
        match node {
            AST::BinOp { right, op, left } => {
                if (self.get_type(&*right).is_none() || self.get_type(&*left).is_none())
                    || self.get_type(&*right) != self.get_type(&*left)
                // Operands must be the same type
                {
                    return None;
                }

                if self.get_type(&*right) == self.get_type(&*left)
                    && self.get_type(&*left) == Some(Type::Int)
                {
                    return Some(Type::Int);
                }

                if self.get_type(&*right) == self.get_type(&*left)
                    && self.get_type(&*left) == Some(Type::Str)
                    && matches!(op, Token::Add)
                {
                    return Some(Type::Str);
                }

                return None;
            }

            AST::UnaryOp { op: _, expr } => {
                return self.get_type(&*expr);
            }

            AST::Value { val } => {
                return match val {
                    Value::Int(_) => Some(Type::Int),

                    Value::Str(_) => Some(Type::Str),

                    Value::Bool(_) => Some(Type::Bool),

                    Value::Float(_) => None,
                }
            }

            AST::FuncCall { name, args: _ } => {
                if let Some(AST::FuncDef { rettype, .. }) = self.find_func(name) {
                    return Some(rettype);
                } else {
                    return None;
                }
            }

            AST::VarGet { id } => {
                if let Some((_n, t)) = self.find_var(&id) {
                    return Some(t);
                } else {
                    return None;
                }
            }

            AST::ReqStr { .. }
            | AST::ReqMutStr { .. }
            | AST::DynaStr { .. }
            | AST::DynaAddr { .. } => {
                return Some(Type::Str);
            }

            AST::IsStatic { expr: _ } => {
                return Some(Type::Bool);
            }

            AST::SCast { expr, t } => {
                return Some(t.clone());
            }
            _ => None,
        }
    }

    fn parse_unit(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        if let Some(tok) = self.next_token() {
            match tok {
                // Unary sub op (e.g -x, -10, even ---3 is valid syntax who cares?)
                Token::Sub => {
                    return match self.parse_unit() {
                        Ok(unit) => {
                            return Ok(AST::UnaryOp {
                                op: Token::Sub,
                                expr: Box::new(unit),
                            })
                        }

                        Err(e) => Err(e),
                    }
                }

                // Same code as unary op, so !!!!!!!!true works
                Token::Not => {
                    return match self.parse_unit() {
                        Ok(unit) => {
                            return Ok(AST::UnaryOp {
                                op: Token::Not,
                                expr: Box::new(unit),
                            })
                        }

                        Err(e) => Err(e),
                    }
                }

                // Brackets 10 * (10 + 1) -> 10 * 11 Yay
                Token::LParen => {
                    return match self.parse_or() {
                        Ok(expr) => {
                            if let Some(Token::RParen) = self.next_token() {
                                return Ok(expr);
                            } else {
                                return Err(self.error(
                                    BRACKET,
                                    self.location().start,
                                    None,
                                    "I think you're missing a ')'",
                                ));
                            }
                        }

                        Err(e) => Err(e),
                    }
                }

                Token::Val(v) => {
                    if let Value::Float(_) = v {
                        return Err(self.error(
                            NOT_IMPLEMENTED,
                            self.location().start,
                            None,
                            "Floats no bueno yet",
                        ));
                    }

                    return Ok(AST::Value { val: v });
                }

                Token::ID(id) => {
                    if let Some(AST::FuncDef {
                        name: _,
                        params,
                        body: _,
                        rettype: _,
                    }) = self.find_func(&id)
                    {
                        let mut args: Vec<Box<AST>> = Vec::new();
                        let start = self.location().start;

                        if let Some(Token::LParen) = self.get_token() {
                            self.next_token();

                            for p in &params {
                                let start = self.location().start;
                                match self.parse_or() {
                                    Ok(arg) => {
                                        if let Some(t) = self.get_type(&arg) {
                                            if t == p.1 {
                                                args.push(Box::new(arg));
                                            } else {
                                                return Err(self.error(
                                                    TYPE_ERR,
                                                    start,
                                                    None,
                                                    &format!("Expected {}", p.1.to_string()),
                                                ));
                                            }
                                        } else {
                                            if let AST::VarGet { id } = arg {
                                                return Err(self.not_defined_err(
                                                    self.location().start,
                                                    None,
                                                    id,
                                                ));
                                            }
                                            return Err(self.error(
                                                TYPE_ERR,
                                                start,
                                                Some(self.location().end),
                                                "Unable to determine type of expression result",
                                            ));
                                        }
                                        if let Some(Token::Comma) = self.get_token() {
                                            self.next_token();
                                        } else {
                                            break;
                                        }
                                    }

                                    Err(e) => {
                                        return Err(e);
                                    }
                                };
                            }
                            if let Some(Token::RParen) = self.get_token() {
                                self.next_token();
                                return Ok(AST::FuncCall {
                                    name: id,
                                    args: args,
                                });
                            } else {
                                return Err(self.error(
                                    BRACKET,
                                    start,
                                    Some(self.location().end),
                                    "Expected closing ')' on function call",
                                ));
                            }
                        } else {
                            return Err(self.error(
                                ID_ERR,
                                start,
                                None,
                                "Expected function call ()",
                            ));
                        }
                    } else {
                        if self.find_var(&id).is_none() {
                            return Err(self.not_defined_err(start, Some(self.location().end), id));
                        }
                        return Ok(AST::VarGet { id: id });
                    }
                }

                Token::ReqStr => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.next_token(), Some(Token::LCurly))
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in reqstr compiler instruction (reqstr{STATIC STRING})",
                        ));
                    }

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.get_type(&u) == Some(Type::Str) {
                                if !matches!(self.next_token(), Some(Token::RCurly)) {
                                    return Err(self.error(
                                        EXPECTED_VAL,
                                        start,
                                        Some(self.location().end),
                                        "Expected '}' in reqstr compiler instruction (reqstr{STATIC STRING})",
                                    ));
                                }

                                return Ok(AST::ReqStr { expr: Box::new(u) });
                            }

                            return Err(self.error(
                                COMP_INST,
                                start,
                                Some(self.location().end),
                                "Expected string value",
                            ));
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::ReqMutStr => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.next_token(), Some(Token::LCurly))
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in reqmutstr compiler instruction (reqmutstr{STATIC INT})",
                        ));
                    };

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.get_type(&u) == Some(Type::Int) {
                                if !matches!(self.next_token(), Some(Token::RCurly)) {
                                    return Err(self.error(
                                        EXPECTED_VAL,
                                        start,
                                        Some(self.location().end),
                                        "Expected '}' in reqmutstr compiler instruction (reqmutstr{STATIC INT})",
                                    ));
                                }

                                return Ok(AST::ReqMutStr { expr: Box::new(u) });
                            }

                            return Err(self.error(
                                COMP_INST,
                                start,
                                Some(self.location().end),
                                "Expected integer value",
                            ));
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::IsStatic => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.next_token(), Some(Token::LCurly))
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in isstatic compiler instruction (isstatic{VALUE})",
                        ));
                    }

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.idx >= self.tokens.len()
                                || !matches!(self.next_token(), Some(Token::RCurly))
                            {
                                return Err(self.error(
                                    EXPECTED_VAL,
                                    start,
                                    Some(self.location().end),
                                    "Expected '}' in isstatic compiler instruction (isstatic{VALUE})",
                                ));
                            }

                            return Ok(AST::IsStatic { expr: Box::new(u) });
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::DynaStr => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.next_token(), Some(Token::LCurly))
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in dynastr compiler instruction (dynastr{ADDRESS, STATIC INT})",
                        ));
                    }

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(straddr) => {
                            if let Some(Type::Str) = self.get_type(&straddr) {
                                if !matches!(self.next_token(), Some(Token::Comma)) {
                                    return Err(self.error(EXPECTED_VAL, start, Some(self.location().end), "Expected comma in dynastr compiler instruction (dynastr{ADDRESS, STATIC INT})"));
                                }
                                let capacity = self.parse_or()?;

                                if let Some(Type::Int) = self.get_type(&capacity) {
                                    if !matches!(self.next_token(), Some(Token::RCurly)) {
                                        return Err(self.error(EXPECTED_VAL, start, Some(self.location().end), "Expected '}' in dynastr compiler instruction (dynastr{ADDRESS, STATIC INT})"));
                                    }

                                    return Ok(AST::DynaStr { str_addr: Box::new(straddr), size: Box::new(capacity) });
                                } else {
                                    return Err(self.error(TYPE_ERR, start, Some(self.location().end), "Expected integer value for dynastr compiler instruction (dynastr{ADDRESS, STATIC INT})"));
                                }
                            } else {
                                return Err(self.error(
                                    TYPE_ERR,
                                    start,
                                    Some(self.location().end),
                                    "Expected string value for dynastr compiler instruction (dynastr{ADDRESS, STATIC INT})",
                                ));
                            }
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::DynaAddr => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.next_token(), Some(Token::LCurly))
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in dynaaddr compiler instruction (dynaaddr{DYNAMIC VALUE})",
                        ));
                    }

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.idx >= self.tokens.len()
                                || !matches!(self.next_token(), Some(Token::RCurly))
                            {
                                return Err(self.error(
                                    EXPECTED_VAL,
                                    start,
                                    Some(self.location().end),
                                    "Expected '}' in dynaaddr compiler instruction (dynaaddr{DYNAMIC VALUE})",
                                ));
                            }

                            return Ok(AST::DynaAddr { expr: Box::new(u) });
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::SCast => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.next_token(), Some(Token::LCurly))
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in scast compiler instruction (scast{STATIC VALUE, TYPE})",
                        ));
                    }

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.next_token() != Some(Token::Comma) {
                                return Err(self.error(EXPECTED_VAL, start, Some(self.location().end), "Expected comma in scast compiler instruction (scast{STATIC VALUE, TYPE})"));
                            }

                            let t = match self.next_token() {
                                Some(ty) => match ty {
                                    Token::StrType => Type::Str,

                                    Token::IntType => Type::Int,

                                    _ => {
                                        return Err(self.error(EXPECTED_VAL, start, Some(self.location().end), "Expected type in scast compiler instruction (scast{STATIC VALUE, TYPE})"));
                                    }
                                },

                                None => {
                                    return Err(self.error(EXPECTED_VAL, start, Some(self.location().end), "Expected type in scast compiler instruction (scast{STATIC VALUE, TYPE})"));
                                }
                            };

                            if self.next_token() != Some(Token::RCurly) {
                                return Err(self.error(EXPECTED_VAL, start, Some(self.location().end), "Expected '}' in scast compiler instruction (scast{STATIC VALUE, TYPE})"));
                            }

                            return Ok(AST::SCast {
                                expr: Box::new(u),
                                t: t,
                            });
                        }

                        Err(e) => return Err(e),
                    };
                }

                _ => {
                    return Err(self.error(
                        EXPECTED_VAL,
                        start,
                        Some(self.location().end),
                        "Expected unit (value, identifier, function call, etc.)",
                    ))
                }
            };
        }

        return Err(self.error(
            EXPECTED_VAL,
            start,
            Some(self.location().end),
            "Expected unit (value, identifier, function call, etc.)",
        ));
    }

    fn valid_operand(node: &AST) -> bool {
        matches!(
            node,
            AST::FuncCall { .. }
                | AST::BinOp { .. }
                | AST::VarGet { .. }
                | AST::UnaryOp { .. }
                | AST::Value { .. }
                | AST::ReqStr { .. }
                | AST::ReqMutStr { .. }
                | AST::IsStatic { .. }
                | AST::DynaStr { .. }
                | AST::DynaAddr { .. }
                | AST::SCast { .. }
        )
    }

    fn parse_term(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        match self.parse_unit() {
            Ok(u) => {
                let mut left = u.clone();
                if Parser::valid_operand(&u) {
                    while matches!(
                        self.get_token().unwrap_or(Token::Exit),
                        Token::Mul | Token::Div
                    ) {
                        let op = self.next_token().unwrap();

                        match self.parse_unit() {
                            Ok(right) => {
                                left = AST::BinOp {
                                    left: Box::new(left),
                                    op: op,
                                    right: Box::new(right),
                                };
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    return Ok(left);
                }

                return Err(self.error(EXPECTED_VAL, start, None, "Expected a term"));
            }

            Err(e) => Err(e),
        }
    }

    fn parse_expr(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        match self.parse_term() {
            Ok(t) => {
                if Parser::valid_operand(&t) {
                    let mut left = t;

                    while matches!(
                        self.get_token().unwrap_or(Token::Exit),
                        Token::Add | Token::Sub
                    ) {
                        let op = self.next_token().unwrap();

                        match self.parse_term() {
                            Ok(right) => {
                                left = AST::BinOp {
                                    left: Box::new(left),
                                    op: op,
                                    right: Box::new(right),
                                };
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    return Ok(left);
                }

                return Err(self.error(EXPECTED_VAL, start, None, "Expected an expression"));
            }

            Err(e) => Err(e),
        }
    }

    fn parse_comp(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        match self.parse_expr() {
            Ok(e) => {
                if Parser::valid_operand(&e) {
                    let mut left = e;

                    while matches!(
                        self.get_token().unwrap_or(Token::Exit),
                        Token::Greater | Token::Less | Token::GreatEq | Token::LessEq
                    ) {
                        let op = self.next_token().unwrap();

                        match self.parse_expr() {
                            Ok(right) => {
                                left = AST::BinOp {
                                    left: Box::new(left),
                                    op: op,
                                    right: Box::new(right),
                                };
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    return Ok(left);
                }

                return Err(self.error(EXPECTED_VAL, start, None, "Expected an expression"));
            }

            Err(e) => Err(e),
        }
    }

    fn parse_equal(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        match self.parse_comp() {
            Ok(c) => {
                if Parser::valid_operand(&c) {
                    let mut left = c;

                    while matches!(
                        self.get_token().unwrap_or(Token::Exit),
                        Token::CompareEq | Token::NotEq
                    ) {
                        let op = self.next_token().unwrap();

                        match self.parse_comp() {
                            Ok(right) => {
                                left = AST::BinOp {
                                    left: Box::new(left),
                                    op: op,
                                    right: Box::new(right),
                                };
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    return Ok(left);
                }

                return Err(self.error(EXPECTED_VAL, start, None, "Expected an expression"));
            }

            Err(e) => Err(e),
        }
    }

    fn parse_and(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        match self.parse_equal() {
            Ok(e) => {
                if Parser::valid_operand(&e) {
                    let mut left = e;

                    while matches!(self.get_token().unwrap_or(Token::Exit), Token::And) {
                        self.next_token();

                        match self.parse_equal() {
                            Ok(right) => {
                                left = AST::BinOp {
                                    left: Box::new(left),
                                    op: Token::And,
                                    right: Box::new(right),
                                };
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    return Ok(left);
                }

                return Err(self.error(EXPECTED_VAL, start, None, "Expected an expression"));
            }

            Err(e) => Err(e),
        }
    }

    fn parse_or(&mut self) -> Result<AST, Error> {
        let start = self.location().start;
        match self.parse_and() {
            Ok(a) => {
                if Parser::valid_operand(&a) {
                    let mut left = a;

                    while matches!(self.get_token().unwrap_or(Token::Exit), Token::Or) {
                        self.next_token();

                        match self.parse_and() {
                            Ok(right) => {
                                left = AST::BinOp {
                                    left: Box::new(left),
                                    op: Token::Or,
                                    right: Box::new(right),
                                };
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    return Ok(left);
                }

                return Err(self.error(EXPECTED_VAL, start, None, "Expected an expression"));
            }

            Err(e) => Err(e),
        }
    }

    // So far: Proc calls, var = expr, proc defs, use statements, ...
    fn parse_stmt(&mut self) -> Result<Option<AST>, Error> {
        let start = self.location().start;

        if let Some(tok) = self.get_token() {
            match tok {
                Token::ID(id) => {
                    self.next_token();
                    if let Some(AST::ProcDef {
                        name,
                        params,
                        body: _,
                    }) = self.find_proc(&id)
                    {
                        let mut args: Vec<Box<AST>> = Vec::new();
                        for param in &params {
                            match self.parse_or() {
                                Ok(arg) => {
                                    if let Some(t) = self.get_type(&arg) {
                                        if t == param.1 {
                                            args.push(Box::new(arg));
                                        } else {
                                            return Err(self.error(
                                                TYPE_ERR,
                                                start,
                                                Some(self.location().end),
                                                &format!("Expected {}", param.1.to_string()),
                                            ));
                                        }
                                    } else {
                                        if let AST::VarGet { id } = arg {
                                            return Err(self.not_defined_err(
                                                start,
                                                Some(self.location().end),
                                                id,
                                            ));
                                        }
                                        return Err(self.error(
                                            TYPE_ERR,
                                            start,
                                            Some(self.location().end),
                                            "Unable to determine type of expression result",
                                        ));
                                    }

                                    // say "Procs have no commas"
                                }

                                Err(e) => {
                                    return Err(e);
                                }
                            }
                        }

                        return Ok(Some(AST::ProcCall {
                            name: name,
                            args: args,
                        }));
                    }
                    if !matches!(self.get_token().unwrap_or(Token::Exit), Token::Eq) {
                        let end = self.location().end;

                        return Err(self.error(FLOATING_ID, start, Some(end), &format!("Identifier '{}' not part of a statement or expression. Did you forget to define a procedure?", id)));
                    } else {
                        self.next_token();
                        match self.parse_or() {
                            Ok(expr) => {
                                let expr_type: Type;
                                if let Some(t) = self.get_type(&expr) {
                                    expr_type = t;
                                } else {
                                    if let AST::VarGet { id } = expr {
                                        return Err(self.not_defined_err(
                                            start,
                                            Some(self.location().end),
                                            id,
                                        ));
                                    }
                                    return Err(self.error(
                                        TYPE_ERR,
                                        start,
                                        Some(self.location().end),
                                        "Unable to determine type of expression result",
                                    ));
                                }

                                let scopes_len = self.scopes.len();
                                self.scopes[scopes_len - 1]
                                    .vars
                                    .push((id.clone(), expr_type));
                                return Ok(Some(AST::VarSet {
                                    id: id,
                                    expr: Box::new(expr),
                                }));
                            }

                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                }

                Token::Exit => {
                    self.next_token();

                    match self.parse_or() {
                        Ok(expr) => {
                            if let Some(t) = self.get_type(&expr) {
                                if t == Type::Int {
                                    return Ok(Some(AST::Exit {
                                        code: Box::new(expr),
                                    }));
                                }
                                return Err(self.error(
                                    TYPE_ERR,
                                    start,
                                    Some(self.location().end),
                                    "Exit requires a integer",
                                ));
                            } else {
                                if let AST::VarGet { id } = expr {
                                    return Err(self.not_defined_err(
                                        start,
                                        Some(self.location().end),
                                        id,
                                    ));
                                }
                                return Err(self.error(TYPE_ERR, start, Some(self.location().end), "Unable to determine type of expression result; exit requires an INT result"));
                            }
                        }

                        Err(e) => {
                            return Err(e);
                        }
                    };
                }

                Token::Asm(raw) => {
                    self.next_token();

                    return Ok(Some(AST::ASM { body: raw }));
                }

                Token::If => {
                    self.next_token();

                    let start = self.location().start;
                    let mut ended = false;
                    let condition = Box::new(self.parse_or()?);
                    let mut branches = Vec::new();
                    let mut elsebranch = None;
                    let mut mainbody = Vec::new();

                    while self.idx < self.tokens.len() {
                        if self.tokens[self.idx].0 == Token::EndIf {
                            self.next_token();
                            ended = true;
                            break;
                        }

                        if self.tokens[self.idx].0 == Token::Elif {
                            self.next_token();
                            let branchcondition = Box::new(self.parse_or()?);
                            let mut body = Vec::new();

                            while self.idx < self.tokens.len() {
                                if matches!(
                                    self.tokens[self.idx].0,
                                    Token::Elif | Token::Else | Token::EndIf
                                ) {
                                    break;
                                }

                                if let Some(stmt) = self.parse_stmt()? {
                                    body.push(Box::new(stmt));
                                }
                            }

                            branches.push((branchcondition, body));
                            continue;
                        }

                        if self.tokens[self.idx].0 == Token::Else {
                            self.next_token();
                            let mut body = Vec::new();

                            while self.idx < self.tokens.len() {
                                if self.tokens[self.idx].0 == Token::EndIf {
                                    break;
                                }

                                if let Some(stmt) = self.parse_stmt()? {
                                    body.push(Box::new(stmt));
                                }
                            }
                            elsebranch = Some(body);
                            continue;
                        }

                        if let Some(stmt) = self.parse_stmt()? {
                            mainbody.push(Box::new(stmt));
                        }
                    }

                    if !ended {
                        return Err(self.error(
                            IF_SYNTAX,
                            start,
                            Some(self.location().end),
                            "If statement not ended (if ... endif)",
                        ));
                    }

                    return Ok(Some(AST::If {
                        cond: condition,
                        body: mainbody,
                        branches: branches,
                        elsebranch: elsebranch,
                    }));
                }

                Token::Proc => {
                    self.next_token();

                    if let Some(Token::ID(name)) = self.next_token() {
                        if self.next_token() != Some(Token::LParen) {
                            return Err(self.error(PROC_DEF, start, Some(self.location().end), "Expected '(' in procedure definition (required even if you don't have arguments) (proc name(args) ... endpc)"));
                        }

                        if self.get_token() == Some(Token::RParen) {
                            self.next_token();
                            let mut statements = Vec::new();
                            let mut endedblock = false;

                            while self.idx < self.tokens.len() {
                                if self.get_token() == Some(Token::EndProc) {
                                    self.next_token();
                                    endedblock = true;
                                    break;
                                }
                                if let Some(stmt) = self.parse_stmt()? {
                                    statements.push(Box::new(stmt));
                                }
                            }

                            if !endedblock {
                                return Err(self.error(
                                    PROC_DEF,
                                    start,
                                    Some(self.location().end),
                                    "Procedure not ended with endpc (proc name(args) ... endpc)",
                                ));
                            }

                            let proc_def = AST::ProcDef {
                                name: name,
                                params: Vec::new(),
                                body: statements,
                            };

                            let scopes_len = self.scopes.len();
                            self.scopes[scopes_len - 1].procs.push(proc_def.clone());
                            return Ok(Some(proc_def));
                        }

                        let mut args = Vec::new();

                        while self.idx < self.tokens.len() {
                            if let Some(Token::ID(param_name)) = self.next_token() {
                                match self.next_token() {
                                    Some(t) => {
                                        match t {
                                            Token::StrType => {
                                                args.push((param_name, Type::Str));
                                            }

                                            Token::IntType => {
                                                args.push((param_name, Type::Int));
                                            }

                                            _ => {
                                                return Err(self.error(PROC_DEF, start, Some(self.location().end), "Expected type in procedure definition (e.g proc say (text STR) endpc)"));
                                            }
                                        };
                                    }

                                    None => {
                                        return Err(self.error(PROC_DEF, start, Some(self.location().end), "Expected type in procedure definition (e.g proc say (text STR) endpc)"));
                                    }
                                };
                                if self.get_token() == Some(Token::Comma) {
                                    self.next_token();
                                } else {
                                    break;
                                }
                            }
                        }
                        if self.next_token() != Some(Token::RParen) {
                            return Err(self.error(
                                PROC_DEF,
                                start,
                                Some(self.location().end),
                                "Expected ')' in procedure definition (proc name(args) ... endpc)",
                            ));
                        }

                        let mut statements = Vec::new();
                        let mut endedblock = false;
                        self.scopes.push(ParserScope::new());

                        for arg in &args {
                            let scopes_len = self.scopes.len();
                            self.scopes[scopes_len - 1].vars.push(arg.clone());
                        }

                        while self.idx < self.tokens.len() {
                            if self.get_token() == Some(Token::EndProc) {
                                self.next_token();
                                endedblock = true;
                                break;
                            }
                            if let Some(stmt) = self.parse_stmt()? {
                                statements.push(Box::new(stmt));
                            }
                        }

                        if !endedblock {
                            return Err(self.error(
                                PROC_DEF,
                                start,
                                Some(self.location().end),
                                "Procedure not ended with endpc (proc name(args) ... endpc)",
                            ));
                        }

                        let proc_def = AST::ProcDef {
                            name: name,
                            params: args,
                            body: statements,
                        };
                        self.scopes.remove(self.scopes.len() - 1);

                        let scopes_len = self.scopes.len();
                        self.scopes[scopes_len - 1].procs.push(proc_def.clone());
                        return Ok(Some(proc_def));
                    } else {
                        return Err(self.error(PROC_DEF, start, Some(self.location().end), "Expected identifier in procedure definition (proc NAME(args) ... endpc)"));
                    }
                }

                Token::Use => {
                    self.next_token();
                    if let Some(Token::Val(val)) = self.next_token() {
                        if let Value::Str(fpath) = val {

                            let code = match get_code_from_jcmd(&(fpath.clone() + ".jcmd")) {
                                Ok(c) => c,

                                Err(e) => {
                                    return Err(self.error(COMP_INST, start, Some(self.location().end), &e));
                                }
                            };

                            let mut lexer = Lexer::new(code.clone(), fpath.clone());
                            let mut parser = Parser::new(lexer.lex()?, fpath, code.clone());
                            let mut res = parser.parse()?;
                            self.program.append(&mut res);
                            for var in parser.flatten_scopes().vars {
                                if self.find_var(&var.0) == None {
                                    let scopes_len = self.scopes.len();
                                    self.scopes[scopes_len - 1].vars.push(var);
                                }
                            }

                            for proc in parser.flatten_scopes().procs {
                                if let AST::ProcDef { name, params, body } = &proc {
                                    if self.find_proc(&name) == None {
                                        let scopes_len = self.scopes.len();
                                        self.scopes[scopes_len - 1].procs.push(proc);
                                    }
                                }
                            }

                            for func in parser.flatten_scopes().funcs {
                                if let AST::FuncDef { name, params, body, rettype } = &func {
                                    if self.find_func(&name) == None {
                                        let scopes_len = self.scopes.len();
                                        self.scopes[scopes_len - 1].funcs.push(func);
                                    }
                                }
                            }


                            return Ok(None);
                        }
                    }
                    return Err(self.error(
                        EXPECTED_VAL,
                        start,
                        Some(self.location().end),
                        "Expected string value (use \"path\")",
                    ));
                }

                _ => {
                    return Err(self.error(
                        EXPECTED_VAL,
                        start,
                        Some(self.location().end),
                        "Expected statement",
                    ));
                }
            }
        } else {
            return Err(self.error(
                EXPECTED_VAL,
                start,
                Some(self.location().end),
                "Expected a value while parsing statement, received none",
            ));
        }
    }

    pub fn parse(&mut self) -> Result<Vec<(AST, Span)>, Error> {
        self.scopes.push(ParserScope::new());
        while self.idx < self.tokens.len() {
            let start = self.location().start;
            let stmt = match self.parse_stmt() {
                Ok(ok) => ok,
                Err(e) => {
                    return Err(e);
                }
            };

            if let Some(s) = stmt {
                let end = self.tokens[self.idx - 1].1.end;
                self.program
                    .push((s, Span::new(start, end)));
            }
        }

        Ok(self.program.clone())
    }
}
