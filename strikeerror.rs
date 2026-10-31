use std::ops::Range;

pub const FILE_ERR: u16 = 0;
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
pub const COMP_ERR: u16 = 14;
pub const ASM_SYNTAX: u16 = 15;
pub const COMP_INST: u16 = 16;
pub const IF_SYNTAX: u16 = 17;
pub const OVERFLOW_ERR: u16 = 18;
pub const PROC_DEF: u16 = 19;

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
            FILE_ERR => String::from("FileHandlingError"),
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
            COMP_ERR => String::from("CompilationError"),
            ASM_SYNTAX => String::from("AssemblyBlockSyntaxError"),
            COMP_INST => String::from("CompilerInstructionError"),
            IF_SYNTAX => String::from("IfStatementSyntaxError"),
            OVERFLOW_ERR => String::from("OverflowError"),
            PROC_DEF => String::from("ProcedureDefinitionError"),
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

fn source_excerpt(content: &str, location: Span) -> &str {
    let start = location.start.min(content.len());
    let end = location.end.min(content.len()).max(start);

    if start < end {
        return &content[start..end];
    }

    let mut line_start = content[..start]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    if line_start == start && start > 0 {
        line_start = content[..start - 1]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        return &content[line_start..start - 1];
    }
    let line_end = content[start..]
        .find('\n')
        .map_or(content.len(), |newline| start + newline);
    &content[line_start..line_end]
}

pub fn throw(error: Error) -> ! {
    if let Some(location) = error.location {
        eprintln!("{}", source_excerpt(&error.content, location));
    } else {
        eprintln!();
    }
    eprintln!(
        "{:<21}(code {}) ({})",
        error.name, error.code, error.fname
    );
    
    eprintln!("{}", error.body);
    let location = error.location.unwrap_or(Span::zero());
    eprintln!(
        "Line {}, Index {}",
        error.content[..location.start].matches('\n').count() + 1,
        location.start
    );
    std::process::exit(error.code as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_width_eof_excerpt_shows_its_source_line() {
        assert_eq!(source_excerpt("first\nsecond", Span::new(12, 12)), "second");
        assert_eq!(source_excerpt("first\n", Span::new(6, 6)), "first");
    }

}
