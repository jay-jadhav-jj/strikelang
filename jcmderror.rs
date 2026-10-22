use std::ops::Range;

pub const FILE_NOT_FOUND: u16 = 0;
pub const UNEXPECTED_CHAR: u16 = 1;
pub const ID_SYNTAX: u16 = 2;
pub const NUM_SYNTAX: u16 = 3;
pub const STRING_SYNTAX: u16 = 4;
pub const COMMENT_SYNTAX: u16 = 5;
pub const PLACEHOLDER: u16 = 6;
pub const EXPECTED_VAL: u16 = 7;
pub const FLOATING_ID: u16 = 8;
pub const MACRO_CALL: u16 = 9;
pub const BRACKET: u16 = 10;
pub const NOT_IMPLEMENTED: u16 = 11;
pub const ID_ERR: u16 = 12;
pub const TYPE_ERR: u16 = 13;


#[derive(Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize
}

impl Span {
    pub fn new(s: usize, e: usize) -> Span {
        Span {
            start: s,
            end: e
        }
    }

    pub fn zero() -> Span {
        Span {
            start: 0,
            end: 0
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

pub struct Error {
    code: u16,
    fname: String,
    content: String,
    location: Option<Span>,
    name: String,
    body: String,
}

impl Error {
    pub fn from_code(filename: String, code_content: String, error_code: &u16, error_loc: Option<Span>, error_body: &str) -> Error {
        Error {
            code: *error_code,
            fname: filename,
            content: code_content,
            location: error_loc,
            name: Error::get_error_name(error_code),
            body: String::from(error_body),
        }
    }

    pub fn not_defined_err(filename: String, code_content: String, error_loc: Span, id: &str) -> Error {
        Error {
            code: ID_ERR,
            fname: filename,
            content: code_content,
            location: Some(error_loc),
            name: Error::get_error_name(&ID_ERR),
            body: format!("'{}' not defined", id),
        }
    }

    fn get_error_name(error_code: &u16) -> String {
        match *error_code {
            FILE_NOT_FOUND => String::from("FileNotFoundError"),
            UNEXPECTED_CHAR => String::from("UnexpectedCharError"),
            ID_SYNTAX => String::from("IdentifierSyntaxError"),
            NUM_SYNTAX => String::from("NumberSyntaxError"),
            STRING_SYNTAX => String::from("StringSyntaxError"),
            COMMENT_SYNTAX => String::from("CommentSyntaxError"),
            PLACEHOLDER => String::from("Placeholder(Debug)"),
            EXPECTED_VAL => String::from("ExpectedValueError"),
            FLOATING_ID => String::from("FloatingIdentifierError"),
            MACRO_CALL => String::from("MacroCallError"),
            BRACKET => String::from("BracketError"),
            NOT_IMPLEMENTED => String::from("NotImplementedError(Feature not implemented YET)"),
            ID_ERR => String::from("IdentifierError"),
            TYPE_ERR => String::from("TypeError"),
            _ => String::from("UnknownError"),
        }
    }

    pub fn placeholder() -> Error {
        Error {
            code: PLACEHOLDER,
            fname: String::new(),
            content: String::new(),
            location: None,
            name: Error::get_error_name(&PLACEHOLDER),
            body: String::from("This is not a real error; it is for development purposes only.")
        }
    }
}

pub fn throw(error: Error) -> ! {
    eprintln!(
        "{:<21}(code {}) ({})",
        error.name, error.code, error.fname
    );
    
    std::eprintln!("{}", &error.content[error.location.unwrap_or(Span::zero()).range()]);
    std::eprintln!("{}", error.body);
    std::eprintln!("Line {}, Index {}", &error.content[..error.location.unwrap_or(Span::zero()).start].matches('\n').count() + 2, error.location.unwrap_or(Span::zero()).start);

    std::process::exit(error.code as i32);
}
