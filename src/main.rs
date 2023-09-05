mod actions;
mod db;

use crate::db::IdxAlias::{Alias, Idx};
use camino::Utf8PathBuf;
use chrono::Utc;
use clap::Parser;
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args = options::Arguments::try_parse();
    if let Err(e) = args {
        // We need to use correct exit code if help is requested
        e.print().expect("Error writing Error");
        process::exit(1);
    }
    let args = args.unwrap();

    const SESSID_KEY: &str = "QCD_RS_SESSIONID";
    const DBNAME: &str = ".qcd_rs.sqlite";
    const DBNAME_KEY: &str = "QCD_RS_DBNAME";
    const DBPATH_KEY: &str = "QCD_RS_DBPATH";

    let sessionid = match env::var(SESSID_KEY) {
        Ok(val) => val,
        Err(_) => "".to_string(),
    };

    let use_stack = sessionid.len() > 22;

    if args.methods.pid {
        let now = Utc::now();
        if !sessionid.is_empty() {
            println!("{}", sessionid);
        } else {
            println!("{}", now.format("%Y%m%d%H%M%S%f"));
        }
        process::exit(1);
    }

    let db_name = match env::var(DBNAME_KEY) {
        Ok(val) => val,
        Err(_) => DBNAME.to_string(),
    };
    let mut db_fullpath = match env::var(DBPATH_KEY) {
        Ok(val) => PathBuf::from(val),
        Err(_) => simple_home_dir::home_dir().unwrap(),
    };
    db_fullpath.push(db_name);

    // Actions

    let tablename = &db::MAINTABLENAME;

    // Conventional chdir
    if let Some(entry) = args.methods.entry {
        let push_dir = if !use_stack || args.no_push {
            None
        } else {
            Some(get_cwd())
        };
        actions::chdir(&db_fullpath, tablename, &entry, push_dir, &sessionid);
    }

    // Print contents of (main) table
    if args.methods.list_paths {
        actions::list_dirs(&db_fullpath, tablename);
    }

    // Add path to database
    if args.methods.add.is_some() || args.methods.add_current {
        let path = args.methods.add.unwrap_or_else(get_cwd);
        let idx = args.idx;
        let alias = args.alias;
        actions::add_row(&db_fullpath, tablename, idx, path, alias);
    }

    // Query a single directory
    if let Some(entry) = args.methods.echo {
        actions::print_row(&db_fullpath, tablename, &entry);
    }

    // Delete entry from database
    if let Some(entry) = args.methods.remove {
        actions::remove_row(&db_fullpath, tablename, &entry);
    }

    // Change alias or idx
    if args.methods.new_alias.is_some() || args.methods.new_idx.is_some() {
        let idx: u32;
        let entry: db::IdxAlias;
        if args.methods.new_idx.is_some() {
            let v = args.methods.new_idx.unwrap();
            idx = v[0];
            entry = Idx(v[1]);
        } else {
            let v = args.methods.new_alias.unwrap();
            idx = match v[0].parse::<u32>() {
                Ok(n) => n,
                Err(_) => {
                    println!("ERROR: Not an idx value");
                    process::exit(1);
                }
            };
            entry = Alias(v[1].clone());
        }
        actions::update_row(&db_fullpath, tablename, idx, &entry);
    }

    // Find idx of directory
    if let Some(dir) = args.methods.query_path {
        actions::find_directory(&db_fullpath, tablename, dir);
    }

    // Stack operations

    if !use_stack {
        eprintln!("Missing or wrong session-id!");
        process::exit(1);
    }

    // Print entries on stack
    if args.methods.list_stack {
        actions::stack_list_dirs(&db_fullpath, &sessionid);
    }

    // Add work dir to stack
    if args.methods.push {
        let cur_dir = get_cwd();
        let res = actions::stack_push(&db_fullpath, &sessionid, cur_dir);
        if let Err(e) = res {
            eprintln!("{e}");
        }
        process::exit(1);
    }

    // Change directory to top of stack, remove that entry
    if args.methods.pop {
        actions::stack_pop(&db_fullpath, &sessionid);
    }

    // Remove entry on top of stack
    if args.methods.drop {
        actions::stack_drop(&db_fullpath, &sessionid);
    }

    // Exchange top of stack with current work dir, chdir to former top of stack
    if args.methods.swap {
        let cur_dir = get_cwd();
        actions::stack_swap(&db_fullpath, &sessionid, cur_dir);
    }
} // main

/// Returns current work directory as Utf8PathBuf.
fn get_cwd() -> Utf8PathBuf {
    let cwd = env::current_dir().unwrap();
    match Utf8PathBuf::from_path_buf(cwd) {
        Ok(pth) => pth,
        Err(_) => {
            println!("Current work directory appears to be no UTF-8 path");
            process::exit(1);
        }
    }
} // get_cwd

mod options {
    use camino::Utf8PathBuf;
    use clap::{Args, ColorChoice, Parser};

    const POSTHELP: &str =
"Environment variables
=====================
  QCD_RS_DBNAME: Name of database. Default: '.qcd_rs.sqlite'
  QCD_RS_DBPATH: Path to database. Default: home-directory


Usage examples:
Change directory
================
  qcd ENTRY [-n]                    Chdir to path with idx or alias ENTRY (w/o -n: adds work dir to stack)
  qcd -o                            (pop)  Chdir to top of stack, remove that entry from stack
  
Add or remove an entry
======================
  qcd -a PATH [-i IDX] [-s ALIAS]   Add PATH to database
  qcd -p [-i IDX] [-s ALIAS]        Add current working directory to database
  qcd -r ENTRY                      Remove row with idx or alias ENTRY
  qcd -u                            (push) Add current working directory to (top of) stack
  
Queries
=======
  qcd -l                            List all indexes, aliases and paths
  qcd -q PATH                       Query index of PATH
  ls `qcd -e 4`                     List directory contents of path with idx 4

Alias matching
==============
Abbreviating an alias will match if the string equals the beginning of an alias in a unique
way. For instance, with aliases 'pets' and 'people' in the database 'qcd peo' will match the
second one while 'qcd pe' will match none.";

    /// Quickly change directories
    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about=None, after_help=POSTHELP, bin_name="qcd",
              color=ColorChoice::Always)]
    pub struct Arguments {
        #[command(flatten)]
        pub methods: Methods,

        /// Do not add current path to stack when changing directory
        #[arg(short = 'n', long = "no-push", requires = "chggrp")]
        pub no_push: bool,

        /// Specify idx value when adding path
        #[arg(short = 'i', long = "idx", requires = "addgrp")]
        pub idx: Option<u32>,

        /// Specify alias when adding path
        #[arg(short = 's', long = "alias", requires = "addgrp")]
        pub alias: Option<String>,
    } // struct Arguments

    #[derive(Args, Debug)]
    #[group(required = true, multiple = false)]
    pub struct Methods {
        /// Index or alias of path
        #[arg(group = "chggrp")]
        pub entry: Option<String>,

        /// List all path-names and id's
        #[arg(short = 'l', long = "list-paths")]
        pub list_paths: bool,

        /// Add PATH to database
        #[arg(short = 'a', long = "add", value_name = "PATH", group = "addgrp")]
        pub add: Option<Utf8PathBuf>,

        /// Add current work dir to database
        #[arg(short = 'p', long = "add-current", group = "addgrp")]
        pub add_current: bool,

        /// Remove path with index or alias equal to ENTRY
        #[arg(short = 'r', long = "remove", value_name = "ENTRY")]
        pub remove: Option<String>,

        /// Set alias for entry IDX
        #[arg(short='b', long="set-alias",  value_names=["IDX", "ALIAS"], num_args(2))]
        pub new_alias: Option<Vec<String>>,

        /// Change IDX
        #[arg(short='x', long="set-index", value_names=["OLDIDX", "NEWIDX"], num_args(2))]
        pub new_idx: Option<Vec<u32>>,

        /// List entries on stack (top to bottom)
        #[arg(short = 'c', long = "list-stack")]
        pub list_stack: bool,

        /// Add current work dir to stack
        #[arg(short = 'u', long = "push")]
        pub push: bool,

        /// Chdir to top of stack and remove path from stack
        #[arg(short = 'o', long = "pop")]
        pub pop: bool,

        /// Remove entry on top of stack
        #[arg(short = 'd', long = "drop")]
        pub drop: bool,

        /// Chdir to top of stack and exchange top of stack by current work dir
        #[arg(short = 'w', long = "swap")]
        pub swap: bool,

        /// Query index of PATH. Returns -1 if path not in table.
        #[arg(short = 'q', long = "query", value_name = "PATH")]
        pub query_path: Option<Utf8PathBuf>,

        /// Print path with index or alias equal to ENTRY
        #[arg(short = 'e', long = "echo", value_name = "ENTRY")]
        pub echo: Option<String>,

        #[arg(long = "pid", hide = true)]
        pub pid: bool,
    } // struct Methods
} // mod options
