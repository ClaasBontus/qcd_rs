use crate::db;

use crate::db::IdxAlias;
use camino::Utf8PathBuf;
use path_absolutize::*;
use std::cmp;
use std::path::PathBuf;
use std::process;

/// Unwraps 'what' if Ok, otherwise prints containing
/// error message and exits.
fn check_and_unwrap<T>(what: Result<T, String>) -> T {
    match what {
        Err(e) => {
            println!("ERROR: {e}");
            process::exit(1);
        }
        Ok(s) => s,
    }
} // check_and_unwrap

/// Tries to get a unique representation of a path.
fn clean_path(path: &Utf8PathBuf) -> Result<Utf8PathBuf, String> {
    let new_path = path.as_std_path().absolutize();
    match new_path {
        Ok(pth) => match Utf8PathBuf::from_path_buf(pth.to_path_buf()) {
            Ok(pth) => Ok(pth),
            Err(_) => Err("Only UTF-8 paths supported".to_string()),
        },
        Err(e) => Err(format!("Could not get absolute path\n{e}")),
    }
} // clean_path

/// Print directory associated with entry, push push_dir onto stack
pub fn chdir(
    db_name: &PathBuf,
    table: &str,
    entry: &str,
    push_dir: Option<Utf8PathBuf>,
    sessionid: &str,
) -> ! {
    let row = get_single_row(db_name, table, entry);

    if let Some(dir) = push_dir {
        let _ = stack_push(db_name, sessionid, dir);
    }

    println!("{}", row.directory);
    process::exit(0);
} // chdir

/// Prints all entries of the specified table sorted by idx.
pub fn list_dirs(db_name: &PathBuf, table: &str) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let entries = db::get_std_rows(&conn, table);
    let entries = check_and_unwrap(entries);

    let alias_len = entries
        .iter()
        .fold(0, |m, e| cmp::max(m, e.alias.chars().count()));
    for entry in entries {
        println!(
            "{0:>4} {1:<alias_len$} {2}",
            entry.idx, entry.alias, entry.directory
        );
    }
    process::exit(1);
} // list_dirs

/// Add one row to tables like 'main'
pub fn add_row(
    db_name: &PathBuf,
    table: &str,
    idx: Option<u32>,
    directory: Utf8PathBuf,
    alias: Option<String>,
) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let idx = match idx {
        Some(i) => i,
        None => {
            let max_idx = db::get_max_idx(&conn, table);
            let max_idx = check_and_unwrap(max_idx);
            max_idx + 1
        }
    };
    let alias = match alias {
        Some(s) => s,
        None => "".to_string(),
    };
    let clean_dir = clean_path(&directory);
    let clean_dir = check_and_unwrap(clean_dir);
    let entry = db::StdRow {
        id: None,
        idx,
        directory: clean_dir,
        alias,
    };
    let new_idx = db::add_std_dir(&conn, table, &entry);
    let new_idx = check_and_unwrap(new_idx);
    println!("Path added with index {new_idx}");
    process::exit(1);
} // add_row

/// Set new idx or alias for row corresponding to idx
pub fn update_row(db_name: &PathBuf, table: &str, idx: u32, entry: &IdxAlias) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let res = db::update_entry(&conn, table, idx, entry);
    check_and_unwrap(res);

    process::exit(1);
} // update_row

/// Searches for the row corresponding to entry
fn get_single_row(db_name: &PathBuf, table: &str, entry: &str) -> db::StdRow {
    let entry = db::IdxAlias::from(entry);

    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);
    let row = db::find_entry(&conn, table, &entry);
    check_and_unwrap(row)
} // get_single_row

/// Searches for directory name, prints idx value if found, prints -1 otherwise
pub fn find_directory(db_name: &PathBuf, table: &str, directory: Utf8PathBuf) -> ! {
    let clean_dir = clean_path(&directory);
    let clean_dir = check_and_unwrap(clean_dir);

    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);
    let row = db::search_dir(&conn, table, &clean_dir);
    match row {
        Ok(r) => {
            println!("{}", r.idx);
        }
        Err(_) => {
            println!("-1");
        }
    }
    process::exit(1);
} // find_directory

/// Removes one row from database corresponding to entry
pub fn remove_row(db_name: &PathBuf, table: &str, entry: &str) -> ! {
    let row = get_single_row(db_name, table, entry);

    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);
    let res = db::rm_std_dir(&conn, table, row.id.unwrap());
    check_and_unwrap(res);
    process::exit(1);
} // remove_row

/// Prints a single directory name corresponding to entry
pub fn print_row(db_name: &PathBuf, table: &str, entry: &str) -> ! {
    let row = get_single_row(db_name, table, entry);
    println!("{}", row.directory);
    process::exit(1);
} // print_row

// Stack routines

/// Print directories on stack top to bottom
pub fn stack_list_dirs(db_name: &PathBuf, sessionid: &str) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let entries = db::get_stack_rows(&conn, sessionid);
    let entries = check_and_unwrap(entries);

    for e in entries {
        println!("{}", e.directory);
    }
    process::exit(1);
} // stack_list_dirs

/// Add directory to top of stack but prevent duplication on top
pub fn stack_push(
    db_name: &PathBuf,
    sessionid: &str,
    directory: Utf8PathBuf,
) -> Result<(), String> {
    let clean_dir = clean_path(&directory)?;
    let conn = db::open_db(db_name)?;

    // Prevent duplicates on top of stack
    let top_entry = db::stack_top(&conn, sessionid);
    if let Ok(row) = top_entry {
        if clean_dir == row.directory {
            return Ok(());
        }
    }

    let entry = db::StackRow {
        id: None,
        sessionid: sessionid.to_owned(),
        directory: clean_dir,
    };

    db::add_stack_dir(&conn, &entry)?;
    Ok(())
} // stack_push

/// Print top of stack after removing corresponding row
pub fn stack_pop(db_name: &PathBuf, sessionid: &str) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let entry = db::stack_pop(&conn, sessionid);
    match entry {
        Ok(e) => {
            println!("{}", e.directory);
            process::exit(0);
        }
        Err(e) => {
            println!("{e}");
        }
    }
    process::exit(1);
} // stack_pop

/// Remove top entry on stack
pub fn stack_drop(db_name: &PathBuf, sessionid: &str) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let entry = db::stack_pop(&conn, sessionid);
    if let Err(e) = entry {
        println!("{e}");
    }
    process::exit(1);
} // stack_drop

/// Print top of stack after removing it. Push directory.
pub fn stack_swap(db_name: &PathBuf, sessionid: &str, directory: Utf8PathBuf) -> ! {
    let conn = db::open_db(db_name);
    let conn = check_and_unwrap(conn);

    let entry = db::stack_pop(&conn, sessionid);
    if let Err(e) = entry {
        println!("{e}");
        process::exit(1);
    }
    let entry = entry.unwrap();

    let res = stack_push(db_name, sessionid, directory);
    if let Err(e) = res {
        println!("{e}");
        process::exit(1);
    }

    println!("{}", entry.directory);
    process::exit(0);
} // stack_swap
