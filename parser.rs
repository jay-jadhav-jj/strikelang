use super::lexer::{Token, Value};
use super::strikeerror::*;

// Native JCMDLANG types; JUST TYPES not values
#[derive(Clone, PartialEq, Debug)]
enum Type {
    Int,
    Str,
    List(Vec<Type>),
}

impl Type {
    pub fn to_string(&self) -> String {
        match self {
            Type::Int => String::from("INT"),
            Type::Str => String::from("STR"),
            Type::List(types) => {
                let mut ret = String::from("LIST(");

                for t in types {
                    ret += &t.to_string();
                }

                ret + ")"
            }
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

            _ => false,
        }
    }
}

// Store locations with AST nodes
#[derive(Clone, Debug)]
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
        body: Box<AST>,
    },
    MacroDef {
        name: String,
        params: Vec<(String, Type)>,
        body: Vec<Box<AST>>,
    },
    MicroDef {
        name: String,
        params: Vec<(String, Type)>,
        body: Vec<Box<AST>>,
        rettype: Type,
    },
    MacroCall {
        name: String,
        args: Vec<Box<AST>>,
    },
    MicroCall {
        name: String,
        args: Vec<Box<AST>>,
    },
    Value {
        val: Value,
    },
}


use std::fmt;

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
        }
    }
}

impl fmt::Display for AST {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AST::VarSet { id, expr } => {
                write!(f, "Set({id}, {expr})")
            }

            AST::VarGet { id } => {
                write!(f, "Get({id})")
            }

            AST::BinOp { left, op, right } => {
                write!(f, "BinOp({op:?}, {left}, {right})")
            }

            AST::UnaryOp { op, expr } => {
                write!(f, "UnaryOp({op:?}, {expr})")
            }

            AST::ForLoop { item_id, list, body } => {
                write!(f, "ForLoop(item={item_id:?}, list={list}, body=[")?;

                for (i, stmt) in body.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{stmt}")?;
                }

                write!(f, "])")
            }

            AST::Exit { code } => {
                write!(f, "Exit({code})")
            }

            AST::ASM { body } => {
                write!(f, "ASM({body})")
            }

            AST::MacroDef { name, params, body } => {
                write!(f, "MacroDef({name}, params=[")?;

                for (i, (param, ty)) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}: {ty}")?;
                }

                write!(f, "], body=[")?;

                for (i, stmt) in body.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{stmt}")?;
                }

                write!(f, "])")
            }

            AST::MicroDef {
                name,
                params,
                body,
                rettype,
            } => {
                write!(f, "MicroDef({name}, params=[")?;

                for (i, (param, ty)) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}: {ty}")?;
                }

                write!(f, "], returns={rettype}, body=[")?;

                for (i, stmt) in body.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{stmt}")?;
                }

                write!(f, "])")
            }

            AST::MacroCall { name, args } => {
                write!(f, "MacroCall({name}, args=[")?;

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }

                write!(f, "])")
            }

            AST::MicroCall { name, args } => {
                write!(f, "MicroCall({name}, args=[")?;

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }

                write!(f, "])")
            }

            AST::Value { val } => {
                match val {
                    Value::Int(v) => write!(f, "Int({v})"),
                    Value::Str(v) => write!(f, "Str({v:?})"),
                    Value::Float(v) => write!(f, "Float({v})"),
                }
            }
        }
    }
}



#[derive(Clone)]
struct ParserScope {
    macros: Vec<AST>,
    micros: Vec<AST>,
    vars: Vec<(String, Type)>,
}

impl ParserScope {
    pub fn new() -> ParserScope {
        ParserScope {
            macros: Vec::new(),
            micros: Vec::new(),
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
            raw: rawcode
        }
    }

    fn error(&self, code: u16, start: usize, end: Option<usize>, msg: &str) -> Error {
        Error::from_code(self.filename.clone(), self.raw.clone(), &code, Some(Span::new(start, end.unwrap_or(self.prev_location()))), msg)
    }

    fn not_defined_err(&self, start: usize, end: Option<usize>, id: String) -> Error {
        Error::not_defined_err(self.filename.clone(), self.raw.clone(), Span::new(start, end.unwrap_or(self.prev_location())), &id)
    }

    fn find_macro(&self, macname: &str) -> Option<AST> {
        for m in &self.scope.macros {
            if let AST::MacroDef { name, .. } = m {
                if name == macname {
                    return Some(m.clone());
                }
            }
        }

        None
    }

    fn find_micro(&self, micname: &str) -> Option<AST> {
        for m in &self.scope.micros {
            if let AST::MicroDef {
                name,
                params,
                body,
                rettype,
            } = m
            {
                if name == micname {
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

    fn location(&self) -> usize {
        if self.idx >= self.tokens.len() {
            return self.idx;
        }
        self.idx
    }

    fn prev_location(&self) -> usize {
        if self.idx - 1 >= self.tokens.len() {
            return self.idx;
        }
        self.idx
    }

    fn get_token(&mut self) -> Option<Token> {
        if self.idx < self.tokens.len() {
            return Some(self.tokens[self.idx].0.clone());
        }
        None
    }

    fn get_token_span(&mut self) -> Option<Span> {
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
                    || self.get_type(&*right) != self.get_type(&*left) // Operands must be the same type
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

                    Value::Float(_) => None,
                }
            }

            AST::MicroCall { name, args } => {
                if let Some(AST::MicroDef { rettype, .. }) = self.find_micro(name) {
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

            _ => None,
        }
    }

    fn parse_unit(&mut self) -> Result<AST, Error> {
        let start = self.location();
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

                // Brackets 10 * (10 + 1) -> 10 * 11 Yay
                Token::LParen => {
                    return match self.parse_expr() {
                        Ok(expr) => {
                            if let Some(Token::RParen) = self.next_token() {
                                return Ok(expr);
                            } else {
                                return Err(self.error(
                                    BRACKET,
                                    self.prev_location(),
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
                            self.location(),
                            None,
                            "Floats no bueno yet",
                        ));
                    }

                    return Ok(AST::Value { val: v });
                }

                Token::ID(id) => {
                    if let Some(AST::MicroDef {
                        name,
                        params,
                        body,
                        rettype,
                    }) = self.find_micro(&id)
                    {
                        let mut args: Vec<Box<AST>> = Vec::new();
                        let start = self.location();

                        if let Some(Token::LParen) = self.get_token() {
                            self.next_token();

                            for p in &params {
                                let start = self.location();
                                match self.parse_expr() {
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
                                                    self.location(),
                                                    None,
                                                    id,
                                                ));
                                            }
                                            return Err(self.error(
                                                TYPE_ERR,
                                                start,
                                                Some(self.location()),
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
                                return Ok(AST::MicroCall {
                                    name: id,
                                    args: args,
                                });
                            } else {
                                return Err(self.error(
                                    BRACKET,
                                    start,
                                    Some(self.location()),
                                    "Expected closing ')' on microcommand call",
                                ));
                            }
                        } else {
                            return Err(self.error(ID_ERR, start, None, "Expected microcommand call ()"));
                        }
                    } else {
                        if self.find_var(&id).is_none() {
                            return Err(self.not_defined_err(start, Some(self.location()), id));
                        }
                        return Ok(AST::VarGet { id: id });
                    }
                }

                _ => {
                    return Err(self.error(
                        EXPECTED_VAL,
                        start,
                        Some(self.location()),
                        "Expected unit (value, identifier, microcommand call, etc.)",
                    ))
                }
            };
        }
        return Err(self.error(
            EXPECTED_VAL,
            start,
            Some(self.location()),
            "Expected unit (value, identifier, microcommand call, etc.)",
        ));
    }

    fn parse_term(&mut self) -> Result<AST, Error> {
        let start = self.location();
        match self.parse_unit() {
            Ok(u) => {
                let mut left = u.clone();
                if matches!(
                    u,
                    AST::MicroCall { .. }
                        | AST::BinOp { .. }
                        | AST::VarGet { .. }
                        | AST::UnaryOp { .. }
                        | AST::Value { .. }
                ) {
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

                return Err(self.error(
                    EXPECTED_VAL,
                    start,
                    None,
                    "Expected a term",
                ));
            }

            Err(e) => Err(e),
        }
    }

    fn parse_expr(&mut self) -> Result<AST, Error> {
        let start = self.location();
        match self.parse_term() {
            Ok(t) => {
                if matches!(
                    t,
                    AST::MicroCall { .. }
                        | AST::BinOp { .. }
                        | AST::VarGet { .. }
                        | AST::UnaryOp { .. }
                        | AST::Value { .. }
                ) {
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

                return Err(self.error(
                    EXPECTED_VAL,
                    start,
                    None,
                    "Expected an expression",
                ));
            }

            Err(e) => Err(e),
        }
    }

    // So far: Macro calls, var = expr, ...
    fn parse_stmt(&mut self) -> Result<AST, Error> {
        let start = self.location();
        if let Some(tok) = self.get_token() {
            match tok {
                Token::ID(id) => {
                    self.next_token();
                    if let Some(AST::MacroDef { name, params, body }) = self.find_macro(&id) {
                        let mut args: Vec<Box<AST>> = Vec::new();
                        for param in &params {
                            match self.parse_expr() {
                                Ok(arg) => {
                                    if let Some(t) = self.get_type(&arg) {
                                        if t == param.1 {
                                            args.push(Box::new(arg));
                                        } else {
                                            return Err(self.error(
                                                TYPE_ERR,
                                                start,
                                                Some(self.location()),
                                                &format!("Expected {}", param.1.to_string()),
                                            ));
                                        }
                                    } else {
                                        if let AST::VarGet { id } = arg {
                                            return Err(self.not_defined_err(
                                                start,
                                                Some(self.location()),
                                                id,
                                            ));
                                        }
                                        return Err(self.error(
                                            TYPE_ERR,
                                            start,
                                            Some(self.location()),
                                            "Unable to determine type of expression result",
                                        ));
                                    }

                                    // say "Macros have no commas"
                                }

                                Err(e) => {
                                    return Err(e);
                                }
                            }
                        }

                        return Ok(AST::MacroCall {
                            name: name,
                            args: args,
                        });
                    }
                    if !matches!(self.get_token().unwrap_or(Token::Exit), Token::Eq) {
                        let end = match self.get_token_span() {
                            Some(span) => span.end,

                            None => { self.location() }
                        };
                        
                        return Err(self.error(FLOATING_ID, start, Some(end), &format!("Identifier '{}' not part of a statement or expression. Did you forget to define a macrocommand?", id)));
                    } else {
                        self.next_token();
                        match self.parse_expr() {
                            Ok(expr) => {
                                let expr_type: Type;
                                if let Some(t) = self.get_type(&expr) {
                                    expr_type = t;
                                } else {
                                    if let AST::VarGet { id } = expr {
                                        return Err(self.not_defined_err(
                                            start,
                                            Some(self.location()),
                                            id,
                                        ));
                                    }
                                    return Err(self.error(
                                        TYPE_ERR,
                                        start,
                                        Some(self.location()),
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

                    match self.parse_expr() {
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
                                    Some(self.location()),
                                    "Exit requires a integer",
                                ));
                            } else {
                                if let AST::VarGet { id } = expr {
                                    return Err(self.not_defined_err(start, Some(self.location()), id));
                                }
                                return Err(self.error(TYPE_ERR, start, Some(self.location()), "Unable to determine type of expression result; exit requires an INT result"));
                            }
                        }

                        Err(e) => {
                            return Err(e);
                        }
                    };
                }

                _ => {
                    return Err(self.error(
                        EXPECTED_VAL,
                        start,
                        Some(self.location()),
                        "Expected statement",
                    ));
                }
            }
        } else {
            return Err(self.error(
                EXPECTED_VAL,
                start,
                Some(self.location()),
                "Expected a value while parsing statement, received none",
            ));
        }
    }

    pub fn parse(&mut self) -> Result<Vec<(AST, Span)>, Error> {
        while self.idx < self.tokens.len() {
            let start = self.location();
            let stmt = match self.parse_stmt() {
                Ok(ok) => ok,
                Err(e) => {
                    return Err(e);
                },
            };

            self.program.push((stmt, Span::new(start, self.location())));
        }

        // Auto appends exit 0
        if self.program.len() > 0
            && !matches!(self.program[self.program.len() - 1].0, AST::Exit { .. })
        {
            self.program.push((AST::Exit {
                code: Box::new(AST::Value { val: Value::Int(0) }),
            }, Span::zero()))
        }

        Ok(self.program.clone())
    }
}
