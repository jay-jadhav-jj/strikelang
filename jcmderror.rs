pub const FILE_NOT_FOUND: u16 = 0;
pub const UNEXPECTED_CHAR: u16 = 1;
pub const ID_SYNTAX: u16 = 2;
pub const NUM_SYNTAX: u16 = 3;
pub const STRING_SYNTAX: u16 = 4;
pub const COMMENT_SYNTAX: u16 = 5;

#[derive(Debug)]
pub struct Error {
    code: u16,
    location: String,
    name: String,
    body: String,
}

impl Error {
    pub fn from_code(error_code: &u16, error_loc: Option<String>, error_body: &str) -> Error {
        Error {
            code: *error_code,
            location: error_loc.unwrap_or(String::from("Unknown location")),
            name: Error::get_error_name(error_code),
            body: String::from(error_body),
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
            _ => String::from("UnknownError"),
        }
    }
}

pub fn throw(error: Error) -> ! {
    eprintln!(
        "{:<21}(code {}) at {}",
        error.name, error.code, error.location
    );
    eprintln!("{}", error.body);
    std::process::exit(error.code as i32);
}
