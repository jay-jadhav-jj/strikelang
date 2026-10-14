pub fn get_code_from_jcmd(path: &str) -> Result<String, String> {
    if !std::fs::exists(path).unwrap_or(false) {
        return Err(String::from(format!("Cannot locate {}", path)))
    }

    match std::fs::read(path) {
        Ok(bytes) => {
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }

        Err(e) => {
            Err(String::from(format!("Cannot read file: {}", e)))
        }
    }
}
