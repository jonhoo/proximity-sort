# proximity-sort

[![Crates.io](https://img.shields.io/crates/v/faktory.svg)](https://crates.io/crates/faktory)
[![Build Status](https://travis-ci.org/jonhoo/proximity-sort.svg?branch=master)](https://travis-ci.org/jonhoo/proximity-sort)

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
junegunn/fzf.vim#360 and junegunn/fzf.vim#492) without making
modifications to `fzf` itself (see junegunn/fzf#1380).

It can be used with `fzf` by running:

```console
$ fd -pFt f path/to/file | proximity-sort | fzf --tiebreak=index
```

And you can add it to your `.vimrc` with:

```vim
function! s:list_cmd()
  let base = fnamemodify(expand('%'), ':h:.:S')
  return base == '.' ? 'fd -t f' : printf('fd -t f -pF %s | proximity-sort %s', base, expand('%'))
endfunction

command! -bang -nargs=? -complete=dir Files
  \ call fzf#vim#files(<q-args>, {'source': s:list_cmd(),
  \                               'options': '--tiebreak=index'}, <bang>0)
```
