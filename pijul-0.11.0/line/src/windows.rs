use std;

pub struct Terminal {}

impl Terminal {
    pub fn new() -> Option<Terminal> {
        Some(Terminal {})
    }
}

impl Terminal {
    pub fn read_line(&mut self) -> Result<String, std::io::Error> {
        let mut s = String::new();
        std::io::stdin().read_line(&mut s)?;
        Ok(s)
    }
}
