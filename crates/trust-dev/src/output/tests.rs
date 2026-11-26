//! What a `trust-dev` command prints on the standard output (specification 22, "trust-dev output";
//! checklist G323b): a reader that closes the pipe ends the output quietly, another write error is
//! still a panic, and every command prints through this module.

use std::io::{self, ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::write_to;

/// A writer that takes `room` bytes and then fails with `kind`, as a pipe whose reader has closed
/// it fails with `BrokenPipe`. It counts every write and flush it was asked for.
struct Closing {
    taken: Vec<u8>,
    room: usize,
    kind: ErrorKind,
    asked: usize,
}

impl Closing {
    fn new(room: usize, kind: ErrorKind) -> Self {
        Self {
            taken: Vec::new(),
            room,
            kind,
            asked: 0,
        }
    }
}

impl Write for Closing {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.asked += 1;
        if self.taken.len() >= self.room {
            return Err(io::Error::from(self.kind));
        }
        let take = bytes.len().min(self.room - self.taken.len());
        self.taken.extend_from_slice(&bytes[..take]);
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.asked += 1;
        if self.taken.len() >= self.room {
            return Err(io::Error::from(self.kind));
        }
        Ok(())
    }
}

#[test]
fn lines_are_printed_whole_with_their_line_end() {
    // Catches a line printed without its line end, or `out!`'s text given one.
    let closed = AtomicBool::new(false);
    let mut out = Closing::new(usize::MAX, ErrorKind::BrokenPipe);
    write_to(&mut out, &closed, format_args!("state={}", "running"), true);
    write_to(&mut out, &closed, format_args!("Name: "), false);
    assert_eq!(out.taken, b"state=running\nName: ");
    assert!(!closed.load(Ordering::Relaxed));
}

#[test]
fn a_reader_that_closes_the_pipe_ends_the_output_quietly() {
    // `trust-dev test --list | head -1`: the pipe takes the first line, and its reader
    // closes it. The next line is dropped without a panic, and no later line is even tried. Catches
    // the panic 'failed printing to stdout: Broken pipe (os error 32)' that every printing command
    // raised in trust-runtime (26 November 2026), and output written on into the closed pipe.
    let closed = AtomicBool::new(false);
    let mut pipe = Closing::new(6, ErrorKind::BrokenPipe);
    write_to(&mut pipe, &closed, format_args!("first"), true);
    write_to(&mut pipe, &closed, format_args!("second"), true);
    assert!(closed.load(Ordering::Relaxed));
    let asked = pipe.asked;
    write_to(&mut pipe, &closed, format_args!("third"), true);
    assert_eq!(pipe.taken, b"first\n");
    assert_eq!(
        pipe.asked, asked,
        "nothing is written after the reader closed the pipe"
    );
}

#[test]
#[should_panic(expected = "failed printing to stdout")]
fn another_write_error_is_still_a_panic() {
    // A full disk under `trust-dev ... > file` loses output that someone wants, so it must not
    // pass as quietly as a closed pipe. Catches every write error taken for the end of the output.
    let closed = AtomicBool::new(false);
    let mut full = Closing::new(0, ErrorKind::StorageFull);
    write_to(&mut full, &closed, format_args!("a line"), true);
}

#[test]
fn every_command_prints_through_this_module() {
    // Specification 22, "trust-dev output": the rule holds for every command. Catches a `println!` or
    // `print!` in trust-dev's sources, which would panic on a closed pipe again. Test files may print
    // as they like.
    let mut files = Vec::new();
    collect_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 30, "trust-dev's sources: {}", files.len());
    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read a source file");
        for (number, line) in text.lines().enumerate() {
            if prints_with_std(line) {
                found.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "print with outln! or out!:\n{}",
        found.join("\n")
    );
}

/// The `.rs` files under `folder`, test files left out.
fn collect_sources(folder: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(folder).expect("read a source folder") {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            collect_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !path.to_string_lossy().ends_with("tests.rs")
        {
            files.push(path);
        }
    }
}

/// Whether the code of `line`, before a comment, calls `println!` or `print!` (not `eprintln!` or
/// `eprint!`).
fn prints_with_std(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or_default();
    ["println!", "print!"].iter().any(|name| {
        code.match_indices(name).any(|(at, _)| {
            !code[..at]
                .chars()
                .next_back()
                .is_some_and(|before| before.is_alphanumeric() || before == '_')
        })
    })
}

#[test]
fn the_rule_finds_std_printing_and_nothing_else() {
    // The scan above: catches a scan that misses `println!` or `print!`, or flags `eprintln!`, this
    // module's macros or a comment that names `println!`.
    assert!(prints_with_std(r#"    println!("{payload}");"#));
    assert!(prints_with_std(r#"print!("Name: ");"#));
    assert!(prints_with_std(r#"Some(x) => std::println!("{x}"),"#));
    assert!(!prints_with_std(r#"    eprintln!("Error: {shown}");"#));
    assert!(!prints_with_std(r#"eprint!("> ");"#));
    assert!(!prints_with_std(r#"outln!("{payload}");"#));
    assert!(!prints_with_std(r#"out!("Name: ");"#));
    assert!(!prints_with_std("/// `println!` would panic instead."));
}
