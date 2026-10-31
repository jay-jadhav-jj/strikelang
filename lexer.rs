use core::fmt;

use super::strikeerror::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Int(i64),
    Str(String),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Val(Value),
    ID(String),
    For,
    EndFor,
    Asm(String),
    Exit,
    Func,
    Proc,
    LParen,
    RParen,
    Comma,
    ReqStr,
    ReqMutStr,
    LCurly,
    RCurly,
    Eq, // TODO : filter, ! map
    Add,
    Sub,
    Mul,
    Div,
    Greater,
    Less,
    CompareEq,
    NotEq,
    GreatEq,
    LessEq,
    Not,
    And,
    Or,
    If,
    Else,
    Elif,
    EndIf,
    IsStatic,
    DynaStr,
    DynaAddr,
    EndProc,
    IntType,
    StrType,
    SCast, // Static zero-cost casting
    Use,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Val(val) => match val {
                Value::Float(flt) => write!(f, "Float({})", flt),
                Value::Int(int) => write!(f, "Int({})", int),
                Value::Str(strlit) => write!(f, "Str(\"{}\")", strlit),
                Value::Bool(b) => write!(f, "Bool({}", b),
            },
            Token::ID(id) => write!(f, "ID({})", id),
            Token::For => write!(f, "For"),
            Token::EndFor => write!(f, "EndFor"),
            Token::Asm(raw) => write!(f, "Asm({})", raw),
            Token::Exit => write!(f, "Exit"),
            Token::Func => write!(f, "Func"),
            Token::Proc => write!(f, "Proc"),
            Token::LParen => write!(f, "LParen"),
            Token::RParen => write!(f, "RParen"),
            Token::Comma => write!(f, "Comma"),
            Token::ReqStr => write!(f, "ReqStr"),
            Token::ReqMutStr => write!(f, "ReqMutStr"),
            Token::RCurly => write!(f, "RCurly"),
            Token::LCurly => write!(f, "LCurly"),
            Token::Eq => write!(f, "Equal"),
            Token::Add => write!(f, "Add"),
            Token::Sub => write!(f, "Sub"),
            Token::Mul => write!(f, "Mul"),
            Token::Div => write!(f, "Div"),
            Token::Greater => write!(f, "Greater"),
            Token::Less => write!(f, "Less"),
            Token::CompareEq => write!(f, "CompareEq"),
            Token::NotEq => write!(f, "NotEq"),
            Token::GreatEq => write!(f, "GreatEq"),
            Token::LessEq => write!(f, "LessEq"),
            Token::Not => write!(f, "Not"),
            Token::And => write!(f, "And"),
            Token::Or => write!(f, "Or"),
            Token::If => write!(f, "If"),
            Token::Else => write!(f, "Else"),
            Token::Elif => write!(f, "Elif"),
            Token::EndIf => write!(f, "EndIf"),
            Token::IsStatic => write!(f, "IsStatic"),
            Token::DynaStr => write!(f, "DynaStr"),
            Token::DynaAddr => write!(f, "DynaAddr"),
            Token::EndProc => write!(f, "EndProc"),
            Token::IntType => write!(f, "IntType"),
            Token::StrType => write!(f, "StrType"),
            Token::SCast => write!(f, "StaticCast"),
            Token::Use => write!(f, "Use"),
        }
    }
}

pub struct Lexer {
    idx: usize,
    tokens: Vec<(Token, Span)>,
    line: u32,
    file: String,
    chars: Vec<char>,
    byte_offsets: Vec<usize>,
}

impl Lexer {
    pub fn new(feed: String, file: String) -> Lexer {
        let chars: Vec<char> = feed.chars().collect();
        let mut byte_offsets: Vec<usize> = feed.char_indices().map(|(offset, _)| offset).collect();
        byte_offsets.push(feed.len());

        Lexer {
            idx: 0,
            tokens: Vec::new(),
            line: 1,
            file: file,
            chars,
            byte_offsets,
        }
    }

    fn error(&self, code : u16, start: usize, msg: &str) -> Error {
        let end = if self.idx < self.chars.len() {
            self.byte_offsets[self.idx + 1]
        } else {
            self.location()
        };
        Error::from_code(
            self.file.clone(),
            String::from(self.chars.iter().collect::<String>()),
            &code,
            Some(Span::new(start, end)),
            msg,
        )
    }

    fn nth_char(&self, idx: usize) -> Option<char> {
        if idx < self.chars.len() {
            Some(self.chars[idx])
        } else {
            None
        }
    }

    fn peek(&self) -> Option<char> {
        if self.idx < self.chars.len() - 1 {
            Some(self.nth_char(self.idx + 1).unwrap())
        } else {
            None
        }
    }

    fn peek_ahead(&self, count: u8) -> Option<char> {
        if self.idx < self.chars.len() - count as usize {
            Some(self.nth_char(self.idx + count as usize).unwrap())
        } else {
            None
        }
    }

    // just to organise code
    fn is_id_char(c: char) -> bool {
        c.is_alphabetic() || c == '_'
    }

    fn is_num_char(c: char) -> bool {
        c.is_ascii_digit() || c == '.'
    }

    // feed[idx] MUST be on alphabetic/_ or it will return blank string
    // returns a string of lexed id; idx will either be >= feed.len() or not id char
    fn lex_id(&mut self) -> String {
        let mut buf = String::new();

        while self.idx < self.chars.len()
            && (Lexer::is_id_char(self.nth_char(self.idx).unwrap()) || self.nth_char(self.idx).unwrap().is_ascii_digit())
        {
            buf.push(self.nth_char(self.idx).unwrap());
            self.idx += 1;
        }

        buf
    }

    // basically lex_id but numbers
    fn lex_num(&mut self) -> Result<String, String> {
        let mut buf = String::new();
        let mut haspoint = false;

        while self.idx < self.chars.len()
            && Lexer::is_num_char(self.nth_char(self.idx).unwrap())
        {
            if self.nth_char(self.idx).unwrap() == '.' {
                if haspoint {
                    return Err(String::from(
                        "Two decimal points in a single number literal",
                    ));
                }
                haspoint = true;
            }
            buf.push(self.nth_char(self.idx).unwrap());
            self.idx += 1;
        }

        Ok(buf)
    }

    // lex_num but cooler!
    fn lex_str(&mut self) -> Result<String, String> {
        self.idx += 1;
        let mut buf = String::new();

        while self.idx < self.chars.len() && self.nth_char(self.idx).unwrap() != '"' {
            buf.push(self.nth_char(self.idx).unwrap());
            self.idx += 1;
        }

        if self.idx >= self.chars.len()
            || self.nth_char(self.idx).unwrap() != '"'
        {
            return Err(String::from("Unclosed string literal (missing '\"'"));
        }

        self.idx += 1;
        Ok(buf)
    }

    fn location(&self) -> usize {
        self.byte_offsets[self.idx]
    }

    fn pushwloc(&mut self, tok: Token, start: usize) {
        let end = if self.location() == start {
            let width = match tok {
                Token::CompareEq | Token::NotEq | Token::GreatEq | Token::LessEq => 2,
                Token::And if self.nth_char(self.idx + 1) == Some('&') => 2,
                Token::Or if self.nth_char(self.idx + 1) == Some('|') => 2,
                _ => 1,
            };
            self.byte_offsets[(self.idx + width).min(self.chars.len())]
        } else {
            self.location()
        };
        self.tokens.push((tok, Span::new(start, end)));
    }

    fn lex_asm(&mut self) -> Result<Token, String> {
        self.idx += 1;
        if self.idx >= self.chars.len() || self.nth_char(self.idx).unwrap() != '{' {
            return Err(String::from("Expected asm block (@{ ... })"));
        }

        self.idx += 1;
        let mut buf = String::new();
        let mut block_closed = false;
        let mut brace_layers = 1u16;

        while self.idx < self.chars.len() {
            if self.nth_char(self.idx).unwrap() == '{' {
                brace_layers += 1;
            }
            if self.nth_char(self.idx).unwrap() == '}' {
                brace_layers -= 1;
                if brace_layers == 0 {
                    self.idx += 1;
                    block_closed = true;
                    break;
                }
            }

            buf += &self.nth_char(self.idx).unwrap().to_string();
            self.idx += 1;
        };

        if block_closed {
            return Ok(Token::Asm(buf));
        }

        Err(String::from("Asm block not closed (@{ ... })"))
    }

    pub fn lex(&mut self) -> Result<Vec<(Token, Span)>, Error> {
        self.idx = 0;
        loop {
            if self.idx >= self.chars.len() {
                break;
            }
            let cur = &self.nth_char(self.idx).unwrap();

            if *cur == '#' {
                while self.idx < self.chars.len() {
                    if self.nth_char(self.idx).unwrap() == '\n' {
                        self.idx += 1;
                        break;
                    }
                    self.idx += 1;
                }
            } else if *cur == '/' {
                if self.peek().unwrap_or('N') == '*' {
                        let start = self.location();
                        self.idx += 1;
                        let mut endtok = false;
                        while self.idx < self.chars.len() {
                            if self.nth_char(self.idx).unwrap() == '*' && self.peek().unwrap_or('N') == '/' {
                                self.idx += 2;
                                endtok = true;
                                break;
                            }
                            self.idx += 1;
                        }

                        if !endtok {
                            return Err(self.error(COMMENT_SYNTAX, start, "Multi line comment not ended (/* ... */)"));
                        }
                    continue;
                }
                self.pushwloc(Token::Div, self.location());
                self.idx += 1;
                continue;
            } else if *cur == '@' {
                let start = self.location();
                match self.lex_asm() {
                    Ok(tok) => {
                        self.pushwloc(tok, start);
                        continue;
                    }

                    Err(text) => {
                        return Err(self.error(ASM_SYNTAX, start, &text));
                    }
                }
            } else if cur.is_whitespace() {
                if *cur == '\n' {
                    self.line += 1;
                }
                self.idx += 1;
                continue;
            } else if Lexer::is_id_char(*cur) {
                let start = self.location();
                let id = self.lex_id();
                self.pushwloc(match id.as_str() {
                    "for" => Token::For,
                    "endfr" => Token::EndFor,
                    "exit" => Token::Exit,
                    "fn" => Token::Func,
                    "proc" => Token::Proc,
                    "reqstr" => Token::ReqStr,
                    "reqmutstr" => Token::ReqMutStr,
                    "true" => Token::Val(Value::Bool(true)),
                    "false" => Token::Val(Value::Bool(false)),
                    "if" => Token::If,
                    "else" => Token::Else,
                    "elif" => Token::Elif,
                    "endif" => Token::EndIf,
                    "isstatic" => Token::IsStatic,
                    "dynastr" => Token::DynaStr,
                    "dynaaddr" => Token::DynaAddr,
                    "endpc" => Token::EndProc,
                    "INT" => Token::IntType,
                    "STR" => Token::StrType,
                    "scast" => Token::SCast,
                    "use" => Token::Use,
                    _ => Token::ID(id),
                }, start);
                continue;
            } else if Lexer::is_num_char(*cur) {
                let start = self.location();
                let strlit = match self.lex_num() {
                    Ok(buf) => buf,

                    Err(text) => {
                        return Err(self.error(NUM_SYNTAX, start, &text));
                    }
                };

                if strlit.contains(".") {
                    self.pushwloc(Token::Val(Value::Float(strlit.parse::<f32>().unwrap())), start);
                } else {
                    self.pushwloc(Token::Val(Value::Int(strlit.parse::<i64>().unwrap())), start);
                }
                continue;
            } else if *cur == '"' {
                let start = self.location();
                let val = match self.lex_str() {
                    Ok(buf) => buf,

                    Err(text) => {
                        return Err(self.error(STRING_SYNTAX, start, &text));
                    }
                };

                self.pushwloc(Token::Val(Value::Str(val)), start);
                continue;
            } else {
                match *cur {
                    '(' => {
                        self.pushwloc(Token::LParen, self.location());
                    }

                    ')' => {
                        self.pushwloc(Token::RParen, self.location());
                    }

                    ',' => {
                        self.pushwloc(Token::Comma, self.location());
                    }

                    '{' => {
                        self.pushwloc(Token::LCurly, self.location());
                    }

                    '}' => {
                        self.pushwloc(Token::RCurly, self.location());
                    }

                    '=' => {
                        match self.peek() {
                            Some(c) => {
                                if c == '=' {
                                    self.pushwloc(Token::CompareEq, self.location());
                                    self.idx += 1;
                                } else {
                                    self.pushwloc(Token::Eq, self.location());
                                }
                            }

                            None => {
                                self.pushwloc(Token::Eq, self.location());
                            }
                        }
                    }

                    '+' => {
                        self.pushwloc(Token::Add, self.location());
                    }

                    '-' => {
                        self.pushwloc(Token::Sub, self.location());
                    }

                    '*' => {
                        self.pushwloc(Token::Mul, self.location());
                    }
                    
                    '>' => {
                        match self.peek() {
                            Some(c) => {
                                if c == '=' {
                                    self.pushwloc(Token::GreatEq, self.location());
                                    self.idx += 1;
                                } else {
                                    self.pushwloc(Token::Greater, self.location());
                                }
                            }

                            None => {
                                self.pushwloc(Token::Greater, self.location());
                            }
                        }
                    }

                    '<' => {
                        match self.peek() {
                            Some(c) => {
                                if c == '=' {
                                    self.pushwloc(Token::LessEq, self.location());
                                    self.idx += 1;
                                } else {
                                    self.pushwloc(Token::Less, self.location());
                                }
                            }

                            None => {
                                self.pushwloc(Token::Less, self.location());
                            }
                        }
                    }

                    '!' => {
                        match self.peek() {
                            Some(c) => {
                                if c == '=' {
                                    self.pushwloc(Token::NotEq, self.location());
                                    self.idx += 1;
                                } else {
                                    self.pushwloc(Token::Not, self.location());
                                }
                            }

                            None => {
                                self.pushwloc(Token::Not, self.location());
                            }
                        }
                    }

                    '&' => {
                        let start = self.location();
                        if let Some(c) = self.peek() {
                            if c == '&' {
                                self.pushwloc(Token::And, start);
                                self.idx += 1;
                            } else {
                                self.pushwloc(Token::And, start);
                            }
                        } else {
                            self.pushwloc(Token::And, start);
                        }
                    }

                    '|' => {
                        let start = self.location();
                        if let Some(c) = self.peek() {
                            if c == '|' {
                                self.pushwloc(Token::Or, start);
                                self.idx += 1;
                            } else {
                                self.pushwloc(Token::Or, start);
                            }
                        } else {
                            self.pushwloc(Token::Or, start);
                        }
                    }

                    _ => {
                        return Err(self.error(
                            UNEXPECTED_CHAR,
                            self.location(),
                            &format!("Unexpected character '{}' during lexing", cur),
                        ));
                    }
                }
                self.idx += 1;
            }
        }
        Ok(self.tokens.clone())
    }
}
