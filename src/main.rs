#![warn(rust_2018_idioms)]

use clap::Parser;
use os_str_bytes::RawOsStr;
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
        let os_str = RawOsStr::assert_from_raw_bytes(&line);
        let os_str = os_str.to_os_str();
        let proximity = Path::new(&os_str)
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
