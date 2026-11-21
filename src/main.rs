#![warn(rust_2018_idioms)]

use clap::Parser;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io::prelude::*;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[clap(
    name = "proximity-sort",
    about = "Sort inputs by proximity to the given path",
    version
)]
struct Opt {
    /// Print output delimited by ASCII NUL characters instead of newline characters.
    #[clap(long)]
    print0: bool,

    /// Read input delimited by ASCII NUL characters instead of newline characters.
    #[clap(long, short = '0')]
    read0: bool,

    /// Compute the proximity to this path.
    #[clap(name = "PATH")]
    path: PathBuf,
}

fn main() {
    let args = Opt::parse();

    let stdin = io::stdin();
    let input = BufReader::new(stdin.lock());
    let insep = if args.read0 { b'\0' } else { b'\n' };
    let input = input.split(insep).map(|line| match line {
        Ok(line) => line,
        Err(e) => {
            panic!("failed to read more paths: {}", e);
        }
    });

    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let outsep = if args.print0 { b'\0' } else { b'\n' };
    for mut line in reorder(input, &args.path) {
        line.path.push(outsep);
        if let Err(e) = output.write_all(&line.path) {
            panic!("failed to write path: {}", e);
        }
    }
}

fn reorder<I>(input: I, context_path: &Path) -> impl Iterator<Item = Line>
where
    I: IntoIterator<Item = Vec<u8>>,
{
    let path: Vec<_> = context_path
        .components()
        .skip_while(|c| matches!(c, std::path::Component::CurDir))
        .collect();
    let mut lines = BinaryHeap::new();
    for (i, line) in input.into_iter().enumerate() {
        let mut missed = false;
        let mut path = path.iter();
        let proximity = line_as_path(&line)
            .components()
            .skip_while(|c| matches!(c, std::path::Component::CurDir))
            .map(|c| {
                // if we've already missed, each additional dir is one further away
                if missed {
                    return -1;
                }

                // we want to score positively if c matches the next segment from target path
                if let Some(p) = path.next() {
                    if p == &c {
                        // matching path segment!
                        return 1;
                    } else {
                        // non-matching path segment
                        missed = true;
                    }
                }

                -1
            })
            .sum();

        lines.push(Line {
            score: proximity,
            path: line,
            i,
        })
    }

    BinaryHeapIterator { heap: lines }
}

/// Interprets a raw input line as a path so that it can be split into components for scoring.
///
/// The result is only used to compute the proximity score; the original bytes are what get
/// printed, so any lossiness here affects ordering but never the output itself.
#[cfg(unix)]
fn line_as_path(line: &[u8]) -> Cow<'_, Path> {
    // On Unix, paths are arbitrary byte sequences, so every line is a valid path as-is.
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(Path::new(std::ffi::OsStr::from_bytes(line)))
}

/// Interprets a raw input line as a path so that it can be split into components for scoring.
///
/// The result is only used to compute the proximity score; the original bytes are what get
/// printed, so any lossiness here affects ordering but never the output itself.
#[cfg(not(unix))]
fn line_as_path(line: &[u8]) -> Cow<'_, Path> {
    // Elsewhere (notably Windows), `OsStr`'s internal encoding cannot be safely constructed from
    // arbitrary bytes: `OsStr::from_encoded_bytes_unchecked` requires the bytes to have come
    // from `OsStr::as_encoded_bytes` (or be valid UTF-8), which stdin does not guarantee. So we
    // decode as UTF-8 instead, replacing any invalid sequences rather than panicking on them.
    //
    // UTF-8 is the right assumption here given how proximity-sort is used: sitting between a
    // file lister and `fzf`. Windows file names are UTF-16 natively, so whatever produces our
    // input has already had to pick a byte encoding, and the common producers all pick UTF-8:
    // `fd` and `rg --files` (the usual `FZF_DEFAULT_COMMAND`s) write paths as lossy UTF-8 on
    // Windows, as does `fzf`'s own Go-based file walker. Our output also goes back to `fzf`,
    // which interprets it as UTF-8 too.
    //
    // The case this doesn't handle well is input in a legacy code page, such as `dir /b` in
    // `cmd` or Windows PowerShell 5.1 with its default `$OutputEncoding`. There, any path
    // component with non-ASCII characters gets U+FFFD replacements and so won't match the
    // context path, though earlier components still score normally and the line is still
    // printed byte-for-byte. Decoding such input properly would require guessing the code page
    // the producer used, which isn't knowable when stdin is a pipe, so we don't try.
    match String::from_utf8_lossy(line) {
        Cow::Borrowed(s) => Cow::Borrowed(Path::new(s)),
        Cow::Owned(s) => Cow::Owned(PathBuf::from(s)),
    }
}

#[derive(Debug)]
struct Line {
    score: isize,
    i: usize,
    path: Vec<u8>,
}

impl Into<Vec<u8>> for Line {
    fn into(self) -> Vec<u8> {
        self.path
    }
}

impl PartialEq for Line {
    fn eq(&self, other: &Line) -> bool {
        self.score == other.score
    }
}

impl Eq for Line {}

impl PartialOrd for Line {
    fn partial_cmp(&self, other: &Line) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Line {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .cmp(&other.score)
            .then_with(|| other.i.cmp(&self.i))
    }
}

struct BinaryHeapIterator<T> {
    heap: BinaryHeap<T>,
}

impl<T> Iterator for BinaryHeapIterator<T>
where
    T: Ord,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.heap.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! bts {
        ($bts:expr) => {
            Vec::from($bts.as_bytes())
        };
    }

    #[test]
    fn check_file_reorder() {
        assert_eq!(
            reorder(
                vec![
                    bts!("test.txt"),
                    bts!("bar/test.txt"),
                    bts!("bar/main.txt"),
                    bts!("misc/test.txt"),
                ],
                Path::new("bar/main.txt"),
            )
            .take(2)
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            vec![bts!("bar/main.txt"), bts!("bar/test.txt"),]
        );

        assert_eq!(
            reorder(
                vec![
                    bts!("baz/controller/admin.rb"),
                    bts!("foobar/controller/user.rb"),
                    bts!("baz/views/admin.rb"),
                    bts!("foobar/controller/admin.rb"),
                    bts!("foobar/views/admin.rb"),
                ],
                Path::new("foobar/controller/admin.rb"),
            )
            .take(3)
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            vec![
                bts!("foobar/controller/admin.rb"),
                bts!("foobar/controller/user.rb"),
                bts!("foobar/views/admin.rb"),
            ]
        );
    }

    #[test]
    fn check_root_is_closer() {
        assert_eq!(
            reorder(
                vec![bts!("a/foo.txt"), bts!("b/foo.txt"), bts!("foo.txt"),],
                Path::new("a/null.txt"),
            )
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            vec![bts!("a/foo.txt"), bts!("foo.txt"), bts!("b/foo.txt"),]
        );
    }

    #[test]
    fn check_stable() {
        assert_eq!(
            reorder(
                vec![bts!("c.txt"), bts!("b.txt"), bts!("a.txt"),],
                Path::new("null.txt"),
            )
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            vec![bts!("c.txt"), bts!("b.txt"), bts!("a.txt"),]
        );
    }

    #[test]
    fn skip_leading_dot() {
        assert_eq!(
            reorder(
                vec![
                    bts!("./first.txt"),
                    bts!("././second.txt"),
                    bts!("third.txt"),
                ],
                Path::new("null.txt"),
            )
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            vec![
                bts!("./first.txt"),
                bts!("././second.txt"),
                bts!("third.txt"),
            ]
        );
    }

    #[test]
    fn check_same_proximity_sorted() {
        assert_eq!(
            reorder(
                vec![
                    bts!("b/2.txt"),
                    bts!("b/1.txt"),
                    bts!("a/x/2.txt"),
                    bts!("a/x/1.txt"),
                    bts!("a/2.txt"),
                    bts!("a/1.txt"),
                ],
                Path::new("null.txt"),
            )
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            [
                bts!("b/2.txt"),
                bts!("b/1.txt"),
                bts!("a/2.txt"),
                bts!("a/1.txt"),
                bts!("a/x/2.txt"),
                bts!("a/x/1.txt"),
            ]
        );
    }
}
