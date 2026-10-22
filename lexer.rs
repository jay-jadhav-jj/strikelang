 use core::fmt;

use super::strikeerror::*;

#[derive(Clone, Debug)]
pub enum Value {
    Float(f32),
    Int(i32),
    Str(String),
}

#[derive(Clone, Debug)]
pub enum Token {
    Val(Value),
    ID(String),
    ForOpen,
    ForClose,
    AsmOpen,
    AsmClose,
    Exit,
    Macro,
    LParen,
    RParen,
    Comma,
    ReqStr,
    ReqMutStr,
    LCurly,
    RCurly,
    Eq, // TODO doubleeq, noteq, : filter, ! map
    Add,
    Sub,
    Mul,
    Div
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Val(val) => match val {
                Value::Float(flt) => write!(f, "Float({})", flt),
                Value::Int(int) => write!(f, "Int({})", int),
                Value::Str(strlit) => write!(f, "Str(\"{}\")", strlit),
            },
            Token::ID(id) => write!(f, "ID({})", id),
            Token::ForOpen => write!(f, "ForOpen"),
            Token::ForClose => write!(f, "ForClose"),
            Token::AsmOpen => write!(f, "AsmOpen"),
            Token::AsmClose => write!(f, "AsmClose"),
            Token::Exit => write!(f, "Exit"),
            Token::Macro => write!(f, "Macro"),
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
        }
    }
}

pub struct Lexer {
    idx: usize,
    tokens: Vec<(Token, Span)>,
    line: u32,
    file: String,
    chars: Vec<char>
}

impl Lexer {
    pub fn new(feed: String, file: String) -> Lexer {
        Lexer {
            idx: 0,
            tokens: Vec::new(),
            line: 1,
            file: file,
            chars: feed.clone().chars().collect()
        }
    }

    fn error(&self, code : u16, start: usize, msg: &str) -> Error {
        Error::from_code(self.file.clone(), String::from(self.chars.iter().collect::<String>()), &code, Some(Span::new(start, self.location())), msg)
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
            && Lexer::is_id_char(self.nth_char(self.idx).unwrap())
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
            || buf == ""
            || self.nth_char(self.idx).unwrap() != '"'
        {
            return Err(String::from("Unclosed string literal (missing '\"'"));
        }

        self.idx += 1;
        Ok(buf)
    }

    fn location(&self) -> usize {
        self.idx
    }

    fn pushwloc(&mut self, tok: Token, start: usize) {
        self.tokens.push((tok, Span::new(start, self.location())));
    }
    
    pub fn lex(&mut self) -> Result<Vec<(Token, Span)>, Error> {
        self.idx = 0;
        loop {
            if self.idx >= self.chars.len() {
                break;
            }
            let cur = &self.nth_char(self.idx).unwrap();

            if *cur == '/' {
                if self.peek().unwrap_or('N') == '*' {
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
                            return Err(self.error(COMMENT_SYNTAX, self.location(), "Multi line comment not ended (/* ... */)"));
                        }
                    continue;
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
                    "for" => Token::ForOpen,
                    "endfor" => Token::ForClose,
                    "asm" => Token::AsmOpen,
                    "endasm" => Token::AsmClose,
                    "exit" => Token::Exit,
                    "macro" => Token::Macro,
                    "reqstr" => Token::ReqStr,
                    "reqmutstr" => Token::ReqMutStr,
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
                    self.pushwloc(Token::Val(Value::Float((strlit.parse::<f32>().unwrap()))), start);
                } else {
                    self.pushwloc(Token::Val(Value::Int(strlit.parse::<i32>().unwrap())), start);
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
                        self.pushwloc(Token::Eq, self.location());
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

                    '/' => {
                        self.pushwloc(Token::Div, self.location());
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
