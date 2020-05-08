#![warn(rust_2018_idioms)]

use clap::{App, Arg};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ffi::OsStr;
use std::io::prelude::*;
use std::io::{self, BufReader, BufWriter};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn main() {
    let matches =
        App::new("Proximity Sorter")
            .author("Jon Gjengset <jon@thesquareplanet.com>")
            .about("Sort inputs by proximity to the given path")
            .arg(
                Arg::with_name("PATH")
                    .help("Compute the proximity to this path.")
                    .required(true)
                    .index(1),
            )
            .arg(
                Arg::with_name("read0").short("0").long("read0").help(
                    "Read input delimited by ASCII NUL characters instead of newline characters",
                ),
            )
            .arg(Arg::with_name("print0").long("print0").help(
                "Print output delimited by ASCII NUL characters instead of newline characters",
            ))
            .get_matches();

    let stdin = io::stdin();
    let input = BufReader::new(stdin.lock());
    let insep = if matches.is_present("read0") {
        b'\0'
    } else {
        b'\n'
    };
    let input = input.split(insep).map(|line| match line {
        Ok(line) => line,
        Err(e) => {
            panic!("failed to read more paths: {}", e);
        }
    });

    let path = if let Some(path) = matches.value_of("PATH") {
        path
    } else {
        clap::Error::argument_not_found_auto("PATH").exit();
    };

    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let outsep = if matches.is_present("print0") {
        b'\0'
    } else {
        b'\n'
    };

    for mut line in reorder(input, path) {
        line.path.push(outsep);
        if let Err(e) = output.write_all(&line.path) {
            panic!("failed to write path: {}", e);
        }
    }
}

fn reorder<I>(input: I, context_path: &str) -> impl Iterator<Item = Line>
where
    I: IntoIterator<Item = Vec<u8>>,
{
    let path: Vec<_> = Path::new(context_path).components().collect();
    let mut lines = BinaryHeap::new();
    for line in input {
        let mut missed = false;
        let mut path = path.iter();
        let proximity = Path::new(OsStr::from_bytes(&line))
            .components()
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
        })
    }

    BinaryHeapIterator { heap: lines }
}

struct Line {
    score: isize,
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
            .then_with(|| other.path.cmp(&self.path))
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
                "bar/main.txt",
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
                "foobar/controller/admin.rb",
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
                "a/null.txt",
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
                vec![bts!("first.txt"), bts!("second.txt"), bts!("third.txt"),],
                "null.txt",
            )
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            vec![bts!("first.txt"), bts!("second.txt"), bts!("third.txt"),]
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
                "null.txt",
            )
            .map(Into::into)
            .collect::<Vec<Vec<u8>>>(),
            [
                bts!("a/1.txt"),
                bts!("a/2.txt"),
                bts!("b/1.txt"),
                bts!("b/2.txt"),
                bts!("a/x/1.txt"),
                bts!("a/x/2.txt"),
            ]
        );
    }
}
