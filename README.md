# qcd &ndash; quickly change directory


**qcd** is a Linux tool which helps improving your efficiency on the command line. With qcd
you change to another directory just by entering commands like `qcd 3` and step
back to where you came from with `qcd -o`. Or you toggle between two directories with
the swap functionality (`qcd -w`).

For this to work you store frequently visited directories in a database file (sqlite3).
Entries in this database get referred to via indices (value '3' in the example).
If you don't like remembering indices you can assign aliases to each entry. Indices and
aliases can be freely adjusted to your needs.

In addition `qcd` provides a stack concept. In the example (`qcd 3`) the
current working directory is put on (top of) the stack before changing the directory.
Entering `qcd -o` pops (removes) the path from top of the stack and changes the working
directory to that path.

Enter `qcd -h` to see the full help.

# Usage examples
## Change directory

    qcd ENTRY [-n]  Chdir to path with idx or alias ENTRY (w/o -n: adds work dir to stack)
    qcd -o          (pop)  Chdir to top of stack, remove that entry from stack

## Add or remove an entry

    qcd -a PATH [-i IDX] [-s ALIAS]   Add PATH to database
    qcd -p [-i IDX] [-s ALIAS]        Add current working directory to database
    qcd -r ENTRY                      Remove row with idx or alias ENTRY
    qcd -u                            (push) Add current working directory to (top of) stack

## Queries

    qcd -l          List all indexes, aliases and paths
    qcd -q PATH     Query index of PATH
    ls `qcd -e 4`   List directory contents of path with idx 4

## Alias matching
Your choices of alias names can have an influence on your efficiency. Abbreviating an alias
will match if the string equals the beginning of an alias in a unique way. For instance,
with aliases *pets* and *people* in the database `qcd pet` will match the first one while
`qcd pe` will match none. If there is a single alias starting e.g. with letter 'a'
`qcd a` will already do the job.


# Obtaining qcd
## Building qcd from source files
You need a Linux system with [Rust](https://www.rust-lang.org/tools/install) 
installed. Change to a convenient directory and enter

    git clone https://github.com/ClaasBontus/qcd_rs.git
    cd qcd_rs
    cargo build --release

After a successful build you should find the executable (named `qcd_rs`) in
subdirectory target/release/.


## Installation
It is **not sufficient** to copy the qcd-binary to a directory in your search path
([read this](https://stackoverflow.com/a/64617878/3876684]) if you want to know why).

1. Copy qcd_rs to a directory in your PATH (or use the full path specification to qcd_rs in the code below).
2. Add the following code to your `.bashrc` or `.zshrc` file:

```bash
qcdfunc()
{
  d=`qcd_rs "$@"`
  if (( $? ))
  then
    \builtin echo $d
  else
    \builtin cd $d
  fi
}

alias qcd=qcdfunc
export QCD_RS_SESSIONID=`qcd_rs --pid`
```

# Environment variables
- QCD_RS_DBPATH: Path to sqlite database (default: *home-directory*).
- QCD_RS_DBNAME: Name of sqlite database file (default: .qcd_rs.sqlite).
- QCD_RS_SESSIONID: Process ID. Needed for providing a separate stack for each opened shell.


# Remarks
- qcd prevents duplicate entries on top of stack.
- Old entries on stack (older than 21 days) eventually get removed.
- Support is restricted to [UTF-8 paths](https://github.com/camino-rs/camino).
