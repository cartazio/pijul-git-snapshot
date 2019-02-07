use libc;
use std;
use std::io::{Read, Write};
use utf8parse;

fn size() -> (usize, usize) {
    unsafe {
        let mut size: libc::winsize = std::mem::zeroed();
        if libc::ioctl(1, libc::TIOCGWINSZ as libc::c_ulong, &mut size) == 0 {
            (size.ws_col as usize, size.ws_row as usize)
        } else {
            (0, 0)
        }
    }
}

pub struct Terminal {
    attr: libc::termios,
    posx: usize,
    posy: usize,
    posx0: usize,
    posy0: usize,
    cursor: usize,
    n_chars: usize,
    buf: String,
}

impl Terminal {
    pub fn new() -> Option<Terminal> {
        unsafe {
            if libc::isatty(0) != 0 {
                let mut attr = std::mem::zeroed();
                libc::tcgetattr(0, &mut attr);
                let attr_orig = attr.clone();

                attr.c_iflag &=
                    !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);
                attr.c_oflag &= !libc::OPOST;
                attr.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
                libc::tcsetattr(0, libc::TCSAFLUSH, &attr);
                Some(Terminal {
                    attr: attr_orig,
                    posx: 0,
                    posy: 0,
                    posx0: 0,
                    posy0: 0,
                    cursor: 0,
                    n_chars: 0,
                    buf: String::new(),
                })
            } else {
                None
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSAFLUSH, &self.attr);
        }
    }
}

fn next_char(s: &str, i: usize) -> usize {
    let s = s.as_bytes();
    if s[i] <= 0x7f {
        i + 1
    } else if s[i] >> 5 == 0b110 {
        i + 2
    } else if s[i] >> 4 == 0b1110 {
        i + 3
    } else {
        i + 4
    }
}

fn prev_char(s: &str, mut i: usize) -> usize {
    let s = s.as_bytes();
    i -= 1;
    while s[i] & 0b11000000 == 0b10000000 {
        i -= 1
    }
    i
}

impl Terminal {
    fn move_left(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        if self.cursor > 0 {
            self.cursor = prev_char(&self.buf, self.cursor);
            if self.posx > 1 {
                self.posx -= 1;
            } else {
                let (w, _) = size();
                self.posx = w;
                self.posy -= 1;
            }
            write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
        }
        o.flush()?;
        Ok(())
    }

    fn word_left(&mut self) -> Result<(), std::io::Error> {
        if self.cursor > 0 {
            let bytes = self.buf.as_bytes();
            let mut is_first = true;
            while self.cursor > 0 {
                self.cursor -= 1;
                if self.posx > 1 {
                    self.posx -= 1;
                } else {
                    let (w, _) = size();
                    self.posx = w;
                    self.posy -= 1;
                }
                if bytes[self.cursor] == b' ' {
                    if !is_first {
                        break;
                    }
                } else {
                    is_first = false
                }
            }
        }
        if self.buf.as_bytes()[self.cursor] == b' ' {
            self.move_right()?
        } else {
            let mut o = std::io::stdout();
            write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
            o.flush()?;
        }
        Ok(())
    }

    fn home(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        self.posx = self.posx0;
        self.posy = self.posy0;
        self.cursor = 0;
        write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
        o.flush()?;
        Ok(())
    }

    fn end(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        let remaining_chars = self.buf.split_at(self.cursor).1.chars().count();
        let (w, _) = size();
        self.cursor = self.buf.len();
        self.posy = self.posy + (self.posx + remaining_chars) / w;
        self.posx = 1 + ((self.posx - 1 + remaining_chars) % w);
        write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
        o.flush()?;
        Ok(())
    }

    fn move_right(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        if self.cursor < self.buf.len() {
            self.cursor = next_char(&self.buf, self.cursor);

            let (w, h) = size();
            if self.posx < w {
                self.posx += 1;
            } else {
                if self.posy >= h {
                    write!(o, "\x1b[1S").unwrap();
                }
                self.posx = 1;
                self.posy += 1;
            }
            write!(o, "\x1b[{};{}H", self.posy, self.posx).unwrap();
        }
        o.flush()?;
        Ok(())
    }

    fn word_right(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        if self.cursor < self.buf.len() {
            let bytes = self.buf.as_bytes();
            let (w, h) = size();
            let mut is_first = true;
            while self.cursor < self.buf.len() {
                self.cursor += 1;
                if self.posx < w {
                    self.posx += 1;
                } else {
                    if self.posy >= h {
                        write!(o, "\x1b[1S").unwrap();
                    }
                    self.posx = 1;
                    self.posy += 1;
                }
                if self.cursor >= self.buf.len() || bytes[self.cursor] == b' ' {
                    if !is_first {
                        break;
                    }
                } else {
                    is_first = false
                }
            }
        }
        if self.cursor < self.buf.len() && self.buf.as_bytes()[self.cursor] == b' ' {
            self.move_right()?
        } else {
            let mut o = std::io::stdout();
            write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
            o.flush()?;
        }
        Ok(())
    }

    fn backspace(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        if self.cursor >= 1 {
            self.cursor = prev_char(&self.buf, self.cursor);
            self.buf.remove(self.cursor);
            self.n_chars -= 1;
            if self.posx > 1 {
                self.posx -= 1;
                write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
            } else {
                let (w, _) = size();
                self.posx = w;
                if self.posy > 1 {
                    write!(o, "\x1b[{};{}H", self.posy - 1, w)?;
                    self.posy -= 1;
                } else {
                    // scroll down by one
                    write!(o, "\x1b[1T")?;
                    write!(o, "\x1b[{};{}H", 1, w)?;
                    self.posy0 = 1;
                    self.posy = 1;
                }
            }
            let (_, end) = self.buf.split_at(self.cursor);
            o.write(end.as_bytes())?;
            write!(o, "\x1b[0J")?;
            write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
            o.flush()?;
        }
        Ok(())
    }

    fn delete(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        if self.cursor < self.buf.len() {
            self.buf.remove(self.cursor);
            self.n_chars -= 1;
            let (_, end) = self.buf.split_at(self.cursor);
            o.write(end.as_bytes())?;
            write!(o, "\x1b[0J")?;
            write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
            o.flush()?;
        }
        Ok(())
    }

    fn erase_to_end(&mut self) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        if self.cursor < self.buf.len() {
            self.buf.truncate(self.cursor);
            write!(o, "\x1b[0J")?;
            write!(o, "\x1b[{};{}H", self.posy, self.posx)?;
            o.flush()?;
        }
        Ok(())
    }

    fn insert(&mut self, c: char) -> Result<(), std::io::Error> {
        let mut o = std::io::stdout();
        self.n_chars += 1;
        self.buf.insert(self.cursor, c);
        let (w, h) = size();

        let (_, end) = self.buf.split_at(self.cursor);
        o.write(end.as_bytes())?;

        // If the extra character goes to the next line.
        if self.posx + 1 > w {
            // We need to scroll up.
            if self.posy + 1 > h {
                write!(o, "\x1b[1S")?;
                self.posy -= 1;
                self.posy0 -= 1;
            }
            write!(o, "\x1b[{};{}H", self.posy + 1, 1)?;
            self.posy += 1;
            self.posx = 1;
        } else {
            write!(o, "\x1b[{};{}H", self.posy, self.posx + 1)?;
            self.posx += 1
        }
        self.cursor = next_char(&self.buf, self.cursor);
        o.flush()?;
        Ok(())
    }

    pub fn read_line(&mut self) -> Result<String, std::io::Error> {
        let mut i = std::io::stdin();
        let mut o = std::io::stdout();
        o.write(b"\x1b[6n").unwrap();
        o.flush().unwrap();
        let mut p = Parser {
            c: None,
            valid: true,
        };
        let mut pending = None;
        let mut utf8 = utf8parse::Parser::new();
        loop {
            let mut c = [0; 4];
            i.read_exact(&mut c[..1])?;
            if c[0] == 3 {
                // Ctrl+C
                return Ok(String::new());
            } else if c[0] == 26 {
                // Ctrl+Z
            } else if c[0] == 27 {
                i.read_exact(&mut c[..1])?;
                if c[0] == b'[' {
                    i.read_exact(&mut c[..1])?;
                    if c[0] == b'D' {
                        // self.move_left()?
                        o.write(b"\x1b[6n")?;
                        o.flush()?;
                        pending = Some(Pending::MoveLeft)
                    } else if c[0] == b'C' {
                        // self.move_right()?
                        o.write(b"\x1b[6n")?;
                        o.flush()?;
                        pending = Some(Pending::MoveRight)
                    } else {
                        let mut y = 0;
                        while c[0] >= b'0' && c[0] <= b'9' {
                            y = y * 10 + ((c[0] - b'0') as usize);
                            i.read_exact(&mut c[..1])?;
                        }
                        if c[0] == b';' {
                            i.read_exact(&mut c[..1])?;
                            let mut x = 0;
                            while c[0] >= b'0' && c[0] <= b'9' {
                                x = x * 10 + ((c[0] - b'0') as usize);
                                i.read_exact(&mut c[..1])?;
                            }
                            if c[0] == b'R' {
                                // The terminal is reporting its position.
                                self.posy = y;
                                self.posx = x;
                                if let Some(p) = pending.take() {
                                    self.do_pending(p)?
                                } else {
                                    self.posy0 = y;
                                    self.posx0 = x;
                                }
                            }
                        } else if c[0] == b'~' {
                            if y == 3 {
                                // self.delete()?
                                o.write(b"\x1b[6n")?;
                                o.flush()?;
                                pending = Some(Pending::Delete)
                            } else if y == 7 {
                                // home
                                // self.home()?
                                o.write(b"\x1b[6n")?;
                                o.flush()?;
                                pending = Some(Pending::Home)
                            } else if y == 8 {
                                // end
                                // self.end()?
                                o.write(b"\x1b[6n")?;
                                o.flush()?;
                                pending = Some(Pending::End)
                            }
                        }
                    }
                } else if c[0] == b'O' {
                    i.read_exact(&mut c[..1])?;
                    if c[0] == b'd' {
                        //crtl + <-
                        o.write(b"\x1b[6n")?;
                        o.flush()?;
                        pending = Some(Pending::WordLeft)
                    } else if c[0] == b'c' {
                        //crtl + ->
                        o.write(b"\x1b[6n")?;
                        o.flush()?;
                        pending = Some(Pending::WordRight)
                    }
                }
            } else if c[0] == 127 || c[0] == 8 {
                // backspace
                // self.backspace()?
                o.write(b"\x1b[6n")?;
                o.flush()?;
                pending = Some(Pending::Backspace)
            } else if c[0] == 10 || c[0] == 13 {
                println!("");
                write!(o, "\x1b[{};{}H", self.posy, 1)?;
                return Ok(std::mem::replace(&mut self.buf, String::new()));
            } else if c[0] == 11 {
                o.write(b"\x1b[6n")?;
                o.flush()?;
                pending = Some(Pending::EraseToEnd)
            } else {
                utf8.advance(&mut p, c[0]);
                if let Some(c) = p.c.take() {
                    // self.insert(c)?;
                    o.write(b"\x1b[6n")?;
                    o.flush()?;
                    pending = Some(Pending::Insert(c))
                }
            }
        }
    }

    fn do_pending(&mut self, p: Pending) -> Result<(), std::io::Error> {
        match p {
            Pending::Insert(c) => self.insert(c),
            Pending::Delete => self.delete(),
            Pending::Home => self.home(),
            Pending::End => self.end(),
            Pending::MoveRight => self.move_right(),
            Pending::MoveLeft => self.move_left(),
            Pending::Backspace => self.backspace(),
            Pending::EraseToEnd => self.erase_to_end(),
            Pending::WordLeft => self.word_left(),
            Pending::WordRight => self.word_right(),
        }
    }
}

enum Pending {
    Insert(char),
    Delete,
    Home,
    End,
    MoveRight,
    MoveLeft,
    Backspace,
    EraseToEnd,
    WordLeft,
    WordRight,
}

struct Parser {
    c: Option<char>,
    valid: bool,
}

impl utf8parse::Receiver for Parser {
    fn codepoint(&mut self, c: char) {
        self.c = Some(c);
        self.valid = true
    }
    fn invalid_sequence(&mut self) {
        self.c = None;
        self.valid = false;
    }
}
