# proximity-sort

[![Crates.io](https://img.shields.io/crates/v/proximity-sort.svg)](https://crates.io/crates/proximity-sort)
[![Codecov](https://codecov.io/github/jonhoo/proximity-sort/coverage.svg?branch=master)](https://codecov.io/gh/jonhoo/proximity-sort)
[![Dependency status](https://deps.rs/repo/github/jonhoo/proximity-sort/status.svg)](https://deps.rs/repo/github/jonhoo/proximity-sort)

This script provides a simple command-line utility that sorts its inputs
by their path proximity to a given path. For example, for a path
`foo/bar.txt`, the following input:

```
quox.txt
foo/bar.txt
foo/baz.txt
```

Would yield an output of:

```
foo/bar.txt
foo/baz.txt
quox.txt
```

The lines are sorted by the number of leading path components shared
between the input path and the provided path.

This program was primarily written to allow context-aware suggestions
for [`fzf`](https://github.com/junegunn/fzf) (requested in
[junegunn/fzf.vim#360](https://github.com/junegunn/fzf.vim/issues/360)
and
[junegunn/fzf.vim#492](https://github.com/junegunn/fzf.vim/issues/492))
without making modifications to `fzf` itself (see
[junegunn/fzf#1380](https://github.com/junegunn/fzf/pull/1380)).

## Installation

If you have [Rust installed](https://www.rust-lang.org/tools/install), you can
install this with:

```shell
cargo install proximity-sort
```

## Usage

It can be used with `fzf` by running:

```console
$ fd -t f | proximity-sort path/to/file | fzf --tiebreak=index
```

And you can add it to your `.vimrc` with:

```vim
function! s:list_cmd()
  let base = fnamemodify(expand('%'), ':h:.:S')
  return base == '.' ? 'fd -t f' : printf('fd -t f | proximity-sort %s', expand('%'))
endfunction

command! -bang -nargs=? -complete=dir Files
  \ call fzf#vim#files(<q-args>, {'source': s:list_cmd(),
  \                               'options': '--tiebreak=index'}, <bang>0)
```

Paths of the same proximity are sorted alphabetically:

```console
$ echo "banana\napple/pie\napple" | proximity-sort . -s
> apple
> banana
> apple/pie
```
