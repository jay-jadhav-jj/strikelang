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
        len: Box<AST>,      // Static len of string
        // Reason: asm needs _len and compiler buffers need to know size at compile time
    },
    DynaAddr {
        expr: Box<AST>,
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
    scope: ParserScope,
    filename: String,
    raw: String,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>, fname: String, rawcode: String) -> Parser {
        Parser {
            tokens: tokens,
            program: Vec::new(),
            idx: 0,
            scope: ParserScope::new(),
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

    fn find_proc(&self, procname: &str) -> Option<AST> {
        for m in &self.scope.procs {
            if let AST::ProcDef { name, .. } = m {
                if name == procname {
                    return Some(m.clone());
                }
            }
        }

        None
    }

    fn find_func(&self, fnname: &str) -> Option<AST> {
        for m in &self.scope.funcs {
            if let AST::FuncDef {
                name,
                params,
                body,
                rettype,
            } = m
            {
                if name == fnname {
                    return Some(m.clone());
                }
            }
        }

        None
    }

    fn find_var(&self, varname: &str) -> Option<(String, Type)> {
        for var in &self.scope.vars {
            if var.0 == varname {
                return Some(var.clone());
            }
        }

        None
    }

    fn location(&self) -> Span {
        self.get_token_span()
            .unwrap_or(self.tokens[self.tokens.len() - 1].1)
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

            AST::UnaryOp { op, expr } => {
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

            AST::FuncCall { name, args } => {
                if let Some(AST::FuncDef { rettype, .. }) = self.find_func(name) {
                    return Some(rettype);
                } else {
                    return None;
                }
            }

            AST::VarGet { id } => {
                if let Some((n, t)) = self.find_var(&id) {
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

            AST::IsStatic { expr } => {
                return Some(Type::Bool);
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
                        name,
                        params,
                        body,
                        rettype,
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
                        || !matches!(self.tokens[self.idx].0, Token::RCurly)
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in reqstr compiler instruction (reqstr{STATIC STRING})",
                        ));
                    }

                    self.next_token();

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.get_type(&u) == Some(Type::Str) {
                                if self.idx >= self.tokens.len()
                                    || !matches!(self.tokens[self.idx].0, Token::LCurly)
                                {
                                    return Err(self.error(
                                        EXPECTED_VAL,
                                        start,
                                        Some(self.location().end),
                                        "Expected '}' in reqstr compiler instruction (reqstr{STATIC STRING})",
                                    ));
                                }

                                self.next_token();

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
                        || !matches!(self.tokens[self.idx].0, Token::RCurly)
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in reqmutstr compiler instruction (reqmutstr{STATIC INT})",
                        ));
                    }

                    self.next_token();

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.get_type(&u) == Some(Type::Int) {
                                if self.idx >= self.tokens.len()
                                    || !matches!(self.tokens[self.idx].0, Token::LCurly)
                                {
                                    return Err(self.error(
                                        EXPECTED_VAL,
                                        start,
                                        Some(self.location().end),
                                        "Expected '}' in reqmutstr compiler instruction (reqmutstr{STATIC INT})",
                                    ));
                                }

                                self.next_token();

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
                        || !matches!(self.tokens[self.idx].0, Token::RCurly)
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in isstatic compiler instruction (isstatic{VALUE})",
                        ));
                    }

                    self.next_token();

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.idx >= self.tokens.len()
                                || !matches!(self.tokens[self.idx].0, Token::LCurly)
                            {
                                return Err(self.error(
                                    EXPECTED_VAL,
                                    start,
                                    Some(self.location().end),
                                    "Expected '}' in isstatic compiler instruction (isstatic{VALUE})",
                                ));
                            }

                            self.next_token();

                            return Ok(AST::IsStatic { expr: Box::new(u) });
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::DynaStr => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.tokens[self.idx].0, Token::RCurly)
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '{' in dynastr compiler instruction (dynastr{ADDRESS, LEN})",
                        ));
                    }

                    self.next_token();

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(straddr) => {
                            // Now parse for a comma and then get another int
                            if let Some(Type::Str) = self.get_type(&straddr) {
                                if self.idx >= self.tokens.len() || self.next_token() != Some(Token::Comma) {
                                    return Err(self.error(
                                        EXPECTED_VAL,
                                        start,
                                        Some(self.location().end),
                                        "Expected ',' in dynastr compiler instruction (dynastr{ADDRESS, LEN})",
                                    ));
                                }
                                if let Ok(len) = self.parse_or() {
                                    if let Some(Type::Int) = self.get_type(&len) {
                                        if self.idx >= self.tokens.len()
                                            || !matches!(self.get_token().unwrap(), Token::LCurly)
                                        {
                                            return Err(self.error(
                                                EXPECTED_VAL,
                                                start,
                                                Some(self.location().end),
                                                "Expected '}' in dynastr compiler instruction (dynastr{ADDRESS, LEN})",
                                            ));
                                        }

                                        self.next_token();

                                        return Ok(AST::DynaStr {
                                            str_addr: Box::new(straddr),
                                            len: Box::new(len),
                                        });
                                    } else {
                                        return Err(self.error(
                                            TYPE_ERR,
                                            start,
                                            Some(self.location().end),
                                            "Expected integer value for dynastr compiler instruction (dynastr{ADDRESS, LEN})",
                                        ));
                                    }
                                } else {
                                    return Err(self.error(
                                        EXPECTED_VAL,
                                        start,
                                        Some(self.location().end),
                                        "Expected integer value for dynastr compiler instruction (dynastr{ADDRESS, LEN})",
                                    ));
                                }
                            } else {
                                return Err(self.error(
                                    TYPE_ERR,
                                    start,
                                    Some(self.location().end),
                                    "Expected string value for dynastr compiler instruction (dynastr{ADDRESS})",
                                ));
                            }
                        }

                        Err(e) => return Err(e),
                    };
                }

                Token::DynaAddr => {
                    if self.idx >= self.tokens.len()
                        || !matches!(self.tokens[self.idx].0, Token::RCurly)
                    {
                        return Err(self.error(
                            EXPECTED_VAL,
                            start,
                            Some(self.location().end),
                            "Expected '<' in dynaaddr compiler instruction (dynaaddr{DYNAMIC VALUE})",
                        ));
                    }

                    self.next_token();

                    let start = self.location().start;
                    match self.parse_or() {
                        Ok(u) => {
                            if self.idx >= self.tokens.len()
                                || !matches!(self.tokens[self.idx].0, Token::LCurly)
                            {
                                return Err(self.error(
                                    EXPECTED_VAL,
                                    start,
                                    Some(self.location().end),
                                    "Expected '}' in dynaaddr compiler instruction (dynaaddr{DYNAMIC VALUE})",
                                ));
                            }

                            self.next_token();

                            return Ok(AST::DynaAddr { expr: Box::new(u) });
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

    // So far: Proc calls, var = expr, ...
    fn parse_stmt(&mut self) -> Result<AST, Error> {
        let start = self.location().start;

        if let Some(tok) = self.get_token() {
            match tok {
                Token::ID(id) => {
                    self.next_token();
                    if let Some(AST::ProcDef { name, params, body }) = self.find_proc(&id) {
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

                        return Ok(AST::ProcCall {
                            name: name,
                            args: args,
                        });
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

                                self.scope.vars.push((id.clone(), expr_type));
                                return Ok(AST::VarSet {
                                    id: id,
                                    expr: Box::new(expr),
                                });
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
                                    return Ok(AST::Exit {
                                        code: Box::new(expr),
                                    });
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

                    return Ok(AST::ASM { body: raw });
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

                                body.push(Box::new(self.parse_stmt()?));
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

                                body.push(Box::new(self.parse_stmt()?));
                            }
                            elsebranch = Some(body);
                            continue;
                        }

                        mainbody.push(Box::new(self.parse_stmt()?));
                    }

                    if !ended {
                        return Err(self.error(
                            IF_SYNTAX,
                            start,
                            Some(self.location().end),
                            "If statement not ended (if ... endif)",
                        ));
                    }

                    return Ok(AST::If {
                        cond: condition,
                        body: mainbody,
                        branches: branches,
                        elsebranch: elsebranch,
                    });
                }

                _ => {
                    return Err(self.error(
                        EXPECTED_VAL,
                        start,
                        Some(self.location().end),
                        "Expected statement",
                    ));
                }
            };
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
        while self.idx < self.tokens.len() {
            let start = self.location().start;
            let stmt = match self.parse_stmt() {
                Ok(ok) => ok,
                Err(e) => {
                    return Err(e);
                }
            };

            self.program
                .push((stmt, Span::new(start, self.location().end)));
        }

        Ok(self.program.clone())
    }
}
