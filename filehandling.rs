use crate::strikeerror::Error;

pub fn get_code_from_jcmd(path: &str) -> Result<String, String> {
    if !std::fs::exists(path).unwrap_or(false) {
        return Err(String::from(format!("Cannot locate {}", path)));
    }

    match std::fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).to_string()),

        Err(e) => Err(String::from(format!("Cannot read file: {}", e))),
    }
}

pub fn filename(path: &str) -> Result<String, String> {
    if !std::fs::exists(path).unwrap_or(false) {
        return Err(String::from(format!("Cannot locate {}", path)));
    }

    match std::path::Path::new(path).file_name() {
        Some(fname) => {
            Ok(String::from(fname.to_str().unwrap()))
        }

        None => Err(String::from(format!("Cannot create path instance from {}", path)))
    }
}

pub fn write(path: &str, content: &str) -> Result<(), String> {
    match std::fs::write(path, content) {
        Ok(_) => Ok(()),

        Err(e) => Err(e.to_string())
    }
}
