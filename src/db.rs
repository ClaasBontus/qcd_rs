use camino::{Utf8Path, Utf8PathBuf};
use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use rusqlite::Error::InvalidColumnType;
use std::path::PathBuf;

use crate::db::IdxAlias::{Alias, Idx};

pub const MAINTABLENAME: &str = "main";
pub const STACKTABLENAME: &str = "_stack";
const STACKEXPIRE_DAYS: i64 = 21;

#[derive(Debug, PartialEq)]
pub struct StdRow {
    pub id: Option<u64>,
    pub idx: u32,
    pub directory: Utf8PathBuf,
    pub alias: String,
}

#[derive(Debug, PartialEq)]
pub struct StackRow {
    pub id: Option<u64>,
    pub sessionid: String,
    pub directory: Utf8PathBuf,
}

#[derive(Debug, PartialEq)]
pub enum IdxAlias {
    Idx(u32),
    Alias(String),
}

impl IdxAlias {
    /// Create an Idx if entry can be parsed as u32 otherwise create an Alias.
    pub fn from(entry: &str) -> Self {
        match entry.parse::<u32>() {
            Ok(n) => Idx(n),
            Err(_) => Alias(entry.to_string()),
        }
    }

    /// Return a tuple with the associated column name
    /// and the value to search for.
    pub fn to_colname_query(&self) -> (String, String) {
        match self {
            Idx(idx) => ("idx".to_string(), idx.to_string()),
            Alias(alias) => ("alias".to_string(), alias.to_owned()),
        }
    }

    pub fn is_alias(&self) -> bool {
        matches!(self, Alias(_))
    }
}

/// Opens the database.
///
/// The database with the specified name is opened (or created).
/// If tables main and/or stack do not exist they are created.
pub fn open_db(db_name: &PathBuf) -> Result<Connection, String> {
    let conn_res = Connection::open(db_name);

    let conn = match conn_res {
        Ok(c) => c,
        Err(e) => {
            return Err(format!("Could not open database\n{e}"));
        }
    };
    if let Err(e) = conn.execute(
        &format!(
            "create table if not exists {} (
             id integer primary key,
             idx integer,
             directory text not null,
             alias text
         )",
            MAINTABLENAME
        ),
        (),
    ) {
        return Err(format!("Could not create main table\n{e}"));
    }
    if let Err(e) = conn.execute(
        &format!(
            "create table if not exists {} (
            id integer primary key,
            sessionid text not null,
            timestamp integer not null,
            directory text not null
        )",
            STACKTABLENAME
        ),
        (),
    ) {
        return Err(format!("Could not create stack table\n{e}"));
    }

    Ok(conn)
} // open_db

/// Add one row to tables like 'main'.
pub fn add_std_dir(conn: &Connection, table: &str, entry: &StdRow) -> Result<u32, String> {
    match contains_idx(conn, table, entry.idx) {
        Ok(b) => {
            if b {
                return Err("Idx already exists!".to_string());
            }
        }
        Err(e) => {
            return Err(format!("When checking if idx exists\n{e}"));
        }
    }
    if !entry.alias.is_empty() {
        match contains_alias(conn, table, &entry.alias) {
            Ok(b) => {
                if b {
                    return Err("Alias already exists!".to_string());
                }
            }
            Err(e) => {
                return Err(format!("When checking if alias exists\n{e}"));
            }
        }
    }

    let res = conn.execute(
        &format!(
            "INSERT INTO {} (idx, directory, alias) values (?1, ?2, ?3)",
            table
        ),
        rusqlite::params![entry.idx, entry.directory.as_str(), entry.alias],
    );
    if let Err(e) = res {
        return Err(format!("Could not add row to table\n{e}"));
    }

    Ok(entry.idx)
} // add_std_dir

/// Removes row with unique id (not idx!)
pub fn rm_std_dir(conn: &Connection, table: &str, id: u64) -> Result<(), String> {
    let stmt = conn.prepare(&format!("DELETE FROM {} WHERE id=?1", table));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare delete statement\n{e}"));
    }
    let mut stmt = stmt.unwrap();

    let res = stmt.execute([id]);
    if let Err(e) = res {
        return Err(format!("Could not delete row\n{e}"));
    }

    Ok(())
} // rm_std_dir

/// Returns the largest value found in column 'idx' for the specified table.
pub fn get_max_idx(conn: &Connection, table: &str) -> Result<u32, String> {
    let stmt = conn.prepare(&format!("SELECT max(idx) FROM {}", table));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare max idx query statement\n{e}"));
    }
    let mut stmt = stmt.unwrap();

    let res = stmt.query_row([], |row| row.get::<usize, u32>(0));
    if let Err(e) = res {
        if let InvalidColumnType(_, _, _) = e {
            return Ok(0u32);
        }
        return Err(format!("Could not query maximum idx value\n{e}"));
    }
    Ok(res.unwrap())
} // get_max_idx

/// Checks if idx can be found in table.
pub fn contains_idx(conn: &Connection, table: &str, idx: u32) -> Result<bool, String> {
    let stmt = conn.prepare(&format!(
        "SELECT EXISTS(SELECT 1 FROM {} WHERE idx=?1)",
        table
    ));
    if let Err(e) = stmt {
        return Err(format!(
            "Could not prepare idx existance check statement\n{e}"
        ));
    }
    let mut stmt = stmt.unwrap();

    let res = stmt.query_row([idx], |row| row.get::<usize, u32>(0));
    if let Err(e) = res {
        return Err(format!("Could not query idx existance state\n{e}"));
    }
    Ok(res.unwrap() != 0)
} // contains_idx

/// Checks if alias can be found in table.
pub fn contains_alias(conn: &Connection, table: &str, alias: &str) -> Result<bool, String> {
    let stmt = conn.prepare(&format!(
        "SELECT EXISTS(SELECT 1 FROM {} WHERE alias=?1)",
        table
    ));
    if let Err(e) = stmt {
        return Err(format!(
            "Could not prepare alias existance check statement\n{e}"
        ));
    }
    let mut stmt = stmt.unwrap();

    let res = stmt.query_row([alias], |row| row.get::<usize, u32>(0));
    if let Err(e) = res {
        return Err(format!("Could not query alias existance state\n{e}"));
    }
    Ok(res.unwrap() != 0)
} // contains_alias

/// Query all entries in tables like 'main'. Resulting Vec is sorted by idx.
pub fn get_std_rows(conn: &Connection, table: &str) -> Result<Vec<StdRow>, String> {
    let stmt = conn.prepare(&format!("SELECT * FROM {} ORDER BY idx", table));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare row query statement\n{e}"));
    }

    let mut stmt = stmt.unwrap();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<usize, u64>(0)?,
            row.get::<usize, u32>(1)?,
            row.get::<usize, String>(2)?,
            row.get::<usize, String>(3)?,
        ))
    });
    if let Err(e) = rows {
        return Err(format!("Could not query entries from table\n{e}"));
    }
    let rows = rows.unwrap();

    let mut entries = Vec::<StdRow>::new();
    for r in rows.flatten() {
        let entry = StdRow {
            id: Some(r.0),
            idx: r.1,
            directory: Utf8PathBuf::from(r.2),
            alias: r.3,
        };
        entries.push(entry);
    }
    Ok(entries)
} // get_std_rows

/// Search for an entry in specified column.
fn query_entry(
    conn: &Connection,
    table: &str,
    col_name: &str,
    query: &str,
) -> Result<StdRow, String> {
    let stmt = conn.prepare(&format!(
        "SELECT * FROM {} WHERE {}=?1 LIMIT 1",
        table, col_name
    ));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare find statement\n{e}"));
    }

    let mut stmt = stmt.unwrap();
    let rows = stmt.query_map([query], |row| {
        Ok((
            row.get::<usize, u64>(0)?,
            row.get::<usize, u32>(1)?,
            row.get::<usize, String>(2)?,
            row.get::<usize, String>(3)?,
        ))
    });
    if let Err(e) = rows {
        return Err(format!("Could not query entries for searching\n{e}"));
    }
    let rows = rows.unwrap();

    if let Some(r) = rows.flatten().next() {
        let entry = StdRow {
            id: Some(r.0),
            idx: r.1,
            directory: Utf8PathBuf::from(r.2),
            alias: r.3,
        };
        return Ok(entry);
    }
    Err("Entry not contained in table".to_string())
} // query_entry

/// Search for alias like "name*". Succeed only if query is unique.
fn query_alias_fuzzy(conn: &Connection, table: &str, alias: &str) -> Result<StdRow, String> {
    let stmt = conn.prepare(&format!("SELECT * FROM {} WHERE alias like ?1", table));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare find statement\n{e}"));
    }

    let mut stmt = stmt.unwrap();
    let rows = stmt.query_map([alias.to_owned() + "%"], |row| {
        Ok((
            row.get::<usize, u64>(0)?,
            row.get::<usize, u32>(1)?,
            row.get::<usize, String>(2)?,
            row.get::<usize, String>(3)?,
        ))
    });
    if let Err(e) = rows {
        return Err(format!("Could not query entries for searching\n{e}"));
    }
    let rows = rows.unwrap();

    let mut entry = StdRow {
        id: None,
        idx: 0,
        directory: Utf8PathBuf::from(""),
        alias: "".to_string(),
    };
    let mut count = 0;
    for r in rows.flatten() {
        entry = StdRow {
            id: Some(r.0),
            idx: r.1,
            directory: Utf8PathBuf::from(r.2),
            alias: r.3,
        };
        if entry.alias == alias {
            return Ok(entry);
        }
        count += 1;
    }
    if count == 1 {
        return Ok(entry);
    }
    if count > 1 {
        return Err("Ambiguous alias specification".to_string());
    }
    Err("Alias not found in table".to_string())
} // query_alias_fuzzy

/// Search for an entry where either the idx or the alias is specified
pub fn find_entry(conn: &Connection, table: &str, entry: &IdxAlias) -> Result<StdRow, String> {
    let (col_name, query) = entry.to_colname_query();
    if entry.is_alias() {
        query_alias_fuzzy(conn, table, &query)
    } else {
        query_entry(conn, table, &col_name, &query)
    }
} // find_entry

/// Search for a particular directory name
pub fn search_dir(conn: &Connection, table: &str, directory: &Utf8Path) -> Result<StdRow, String> {
    query_entry(conn, table, "directory", directory.as_str())
} // search_dir

/// Sets new idx or alias for row corresponding to idx
pub fn update_entry(
    conn: &Connection,
    table: &str,
    idx: u32,
    entry: &IdxAlias,
) -> Result<(), String> {
    let row = find_entry(conn, table, &Idx(idx))?;

    // Check if there is nothing to do and prevent duplicating values
    match entry {
        Idx(i) => {
            if i == &row.idx {
                return Ok(());
            }
            if contains_idx(conn, table, *i)? {
                return Err("Idx already contained in table".to_string());
            }
        }
        Alias(s) => {
            if s == &row.alias {
                return Ok(());
            }
            if contains_alias(conn, table, s)? {
                return Err("Alias already contained in table".to_string());
            }
        }
    }

    let (col_name, new_value) = entry.to_colname_query();
    let stmt = conn.prepare(&format!("UPDATE {} SET {}=?1 WHERE id=?2", table, col_name));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare update statement\n{e}"));
    }

    let mut stmt = stmt.unwrap();
    let res = stmt.execute(rusqlite::params![new_value, row.id]);
    if let Err(e) = res {
        return Err(format!("Could not update row\n{e}"));
    }

    Ok(())
} // update_entry

// Stack routines

fn get_timestamp(subtract: &Duration) -> i64 {
    let utc: DateTime<Utc> = Utc::now();
    (utc - *subtract).timestamp()
} // get_timestamp

/// Remove old entries from stack independent of sessionid
fn tidyup_stack(conn: &Connection) -> Result<(), String> {
    let best_after = get_timestamp(&Duration::days(STACKEXPIRE_DAYS));

    let stmt = conn.prepare(&format!(
        "DELETE FROM {} WHERE timestamp < ?1",
        STACKTABLENAME
    ));
    if let Err(e) = stmt {
        return Err(format!(
            "Could not prepare tidyup stack delete statement\n{e}"
        ));
    }
    let mut stmt = stmt.unwrap();

    let res = stmt.execute([best_after]);
    if let Err(e) = res {
        return Err(format!("Could not tidyup stack\n{e}"));
    }

    Ok(())
} // tidyup_stack

/// Query all entries on the stack. Resulting Vec is sorted by id.
pub fn get_stack_rows(conn: &Connection, sessionid: &str) -> Result<Vec<StackRow>, String> {
    let _ = tidyup_stack(conn);

    let stmt = conn.prepare(&format!(
        "SELECT * FROM {} WHERE sessionid=?1 ORDER BY id DESC",
        STACKTABLENAME
    ));
    if let Err(e) = stmt {
        return Err(format!("Stack: Could not prepare row query statement\n{e}"));
    }

    let mut stmt = stmt.unwrap();
    let rows = stmt.query_map([sessionid], |row| {
        Ok((
            row.get::<usize, u64>(0)?,
            row.get::<usize, String>(1)?,
            row.get::<usize, String>(3)?,
        ))
    });
    if let Err(e) = rows {
        return Err(format!("Could not query entries from stack\n{e}"));
    }
    let rows = rows.unwrap();

    let mut entries = Vec::<StackRow>::new();
    for r in rows.flatten() {
        let entry = StackRow {
            id: Some(r.0),
            sessionid: r.1,
            directory: Utf8PathBuf::from(r.2),
        };
        entries.push(entry);
    }
    Ok(entries)
} // get_stack_rows

/// Add one row to stack. Returns id of entry.
pub fn add_stack_dir(conn: &Connection, entry: &StackRow) -> Result<i64, String> {
    let _ = tidyup_stack(conn);

    let timestamp = get_timestamp(&Duration::seconds(0));
    let res = conn.execute(
        &format!(
            "INSERT INTO {} (sessionid, timestamp, directory) values (?1, ?2, ?3)",
            STACKTABLENAME
        ),
        rusqlite::params![entry.sessionid, timestamp, entry.directory.as_str()],
    );
    if let Err(e) = res {
        return Err(format!("Could not add row to table\n{e}"));
    }

    Ok(conn.last_insert_rowid())
} // add_stack_dir

/// Removes row from stack
fn rm_stack_dir(conn: &Connection, id: u64) -> Result<(), String> {
    let stmt = conn.prepare(&format!("DELETE FROM {} WHERE id=?1", STACKTABLENAME));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare stack delete statement\n{e}"));
    }
    let mut stmt = stmt.unwrap();

    let res = stmt.execute([id]);
    if let Err(e) = res {
        return Err(format!("Could not delete stack row\n{e}"));
    }

    Ok(())
} // rm_stack_dir

/// Returns top element on stack
pub fn stack_top(conn: &Connection, sessionid: &str) -> Result<StackRow, String> {
    let stmt = conn.prepare(&format!(
        "SELECT * FROM {} WHERE sessionid=?1 ORDER BY id DESC LIMIT 1",
        STACKTABLENAME
    ));
    if let Err(e) = stmt {
        return Err(format!("Could not prepare stack find statement\n{e}"));
    }

    let mut stmt = stmt.unwrap();
    let rows = stmt.query_map([sessionid], |row| {
        Ok((
            row.get::<usize, u64>(0)?,
            row.get::<usize, String>(1)?,
            row.get::<usize, String>(3)?,
        ))
    });
    if let Err(e) = rows {
        return Err(format!("Could not query stack entries for searching\n{e}"));
    }
    let rows = rows.unwrap();

    if let Some(r) = rows.flatten().next() {
        let entry = StackRow {
            id: Some(r.0),
            sessionid: r.1,
            directory: Utf8PathBuf::from(r.2),
        };
        return Ok(entry);
    }
    Err("Nothing on stack".to_string())
} // stack_top

/// Returns top of stack after removing that row from stack
pub fn stack_pop(conn: &Connection, sessionid: &str) -> Result<StackRow, String> {
    let _ = tidyup_stack(conn);

    let entry = stack_top(conn, sessionid)?;

    match rm_stack_dir(conn, entry.id.unwrap()) {
        Ok(()) => Ok(entry),
        Err(e) => Err(e),
    }
} // stack_pop

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::str::FromStr;

    const TESTDBNAME: &str = "test_qcd_database.sqlite";

    fn just_open_db() -> Connection {
        let _ = std::fs::remove_file(TESTDBNAME);
        let conn = open_db(&PathBuf::from(TESTDBNAME));
        let conn = conn.unwrap();
        conn
    }

    #[test]
    #[serial]
    fn max_idx() {
        let conn = just_open_db();
        let max_idx = get_max_idx(&conn, MAINTABLENAME).unwrap();
        assert_eq!(max_idx, 0);

        let entry = StdRow {
            id: None,
            idx: 42,
            directory: Utf8PathBuf::from_str("test").unwrap(),
            alias: "".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);
        let max_idx = get_max_idx(&conn, MAINTABLENAME).unwrap();
        assert_eq!(max_idx, 42);
        let in_table = contains_idx(&conn, MAINTABLENAME, 42);
        assert_eq!(in_table, Ok(true));
        let in_table = contains_idx(&conn, MAINTABLENAME, 41);
        assert_eq!(in_table, Ok(false));

        let entry = StdRow {
            id: None,
            idx: 52,
            directory: Utf8PathBuf::from_str("test2").unwrap(),
            alias: "".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);
        let max_idx = get_max_idx(&conn, MAINTABLENAME).unwrap();
        assert_eq!(max_idx, 52);

        let entry = StdRow {
            id: None,
            idx: 12,
            directory: Utf8PathBuf::from_str("test3").unwrap(),
            alias: "".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);
        let max_idx = get_max_idx(&conn, MAINTABLENAME).unwrap();
        assert_eq!(max_idx, 52);
    } // max_idx

    #[test]
    #[serial]
    fn add_rows_get_rows() {
        let conn = just_open_db();

        let entries = get_std_rows(&conn, MAINTABLENAME).unwrap();
        assert_eq!(entries.len(), 0);

        let entry = StdRow {
            id: None,
            idx: 44,
            directory: Utf8PathBuf::from_str("temp1").unwrap(),
            alias: "fst".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);
        let entries = get_std_rows(&conn, MAINTABLENAME).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            StdRow {
                id: Some(1),
                idx: 44,
                directory: Utf8PathBuf::from_str("temp1").unwrap(),
                alias: "fst".to_string()
            }
        );
        let in_table = contains_alias(&conn, MAINTABLENAME, "fst");
        assert_eq!(in_table, Ok(true));
        let in_table = contains_alias(&conn, MAINTABLENAME, "scd");
        assert_eq!(in_table, Ok(false));

        let entry = StdRow {
            id: None,
            idx: 24,
            directory: Utf8PathBuf::from_str("temp2").unwrap(),
            alias: "scd".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);
        let entry = StdRow {
            id: None,
            idx: 34,
            directory: Utf8PathBuf::from_str("temp3").unwrap(),
            alias: "five".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);

        let entries = get_std_rows(&conn, MAINTABLENAME).unwrap();
        assert_eq!(entries.len(), 3);

        // Sorted by idx:
        assert_eq!(
            entries[0],
            StdRow {
                id: Some(2),
                idx: 24,
                directory: Utf8PathBuf::from_str("temp2").unwrap(),
                alias: "scd".to_string()
            }
        );
        assert_eq!(
            entries[1],
            StdRow {
                id: Some(3),
                idx: 34,
                directory: Utf8PathBuf::from_str("temp3").unwrap(),
                alias: "five".to_string()
            }
        );
        assert_eq!(
            entries[2],
            StdRow {
                id: Some(1),
                idx: 44,
                directory: Utf8PathBuf::from_str("temp1").unwrap(),
                alias: "fst".to_string()
            }
        );

        let fnd = find_entry(&conn, MAINTABLENAME, &Idx(44)).unwrap();
        assert_eq!(
            fnd,
            StdRow {
                id: Some(1),
                idx: 44,
                directory: Utf8PathBuf::from_str("temp1").unwrap(),
                alias: "fst".to_string()
            }
        );
        let fnd = find_entry(&conn, MAINTABLENAME, &Alias("scd".to_string())).unwrap();
        assert_eq!(
            fnd,
            StdRow {
                id: Some(2),
                idx: 24,
                directory: Utf8PathBuf::from_str("temp2").unwrap(),
                alias: "scd".to_string()
            }
        );
        let fnd = find_entry(&conn, MAINTABLENAME, &Alias("s".to_string())).unwrap();
        assert_eq!(
            fnd,
            StdRow {
                id: Some(2),
                idx: 24,
                directory: Utf8PathBuf::from_str("temp2").unwrap(),
                alias: "scd".to_string()
            }
        );

        let fnd = find_entry(&conn, MAINTABLENAME, &Idx(144));
        assert_eq!(fnd, Err("Entry not contained in table".to_string()));
        let fnd = find_entry(&conn, MAINTABLENAME, &Alias("scdfst".to_string()));
        assert_eq!(fnd, Err("Alias not found in table".to_string()));
        let fnd = find_entry(&conn, MAINTABLENAME, &Alias("f".to_string()));
        assert_eq!(fnd, Err("Ambiguous alias specification".to_string()));
    } // add_rows_get_rows

    #[test]
    #[serial]
    fn remove_row() {
        let conn = just_open_db();

        let entry = StdRow {
            id: None,
            idx: 2,
            directory: Utf8PathBuf::from_str("qcd1").unwrap(),
            alias: "fst".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);

        let entry = StdRow {
            id: None,
            idx: 4,
            directory: Utf8PathBuf::from_str("qcd2").unwrap(),
            alias: "".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);

        let entry = StdRow {
            id: None,
            idx: 6,
            directory: Utf8PathBuf::from_str("qcd3").unwrap(),
            alias: "scd".to_string(),
        };
        let _ = add_std_dir(&conn, MAINTABLENAME, &entry);

        let entries = get_std_rows(&conn, MAINTABLENAME).unwrap();
        assert_eq!(entries.len(), 3);

        let _ = rm_std_dir(&conn, MAINTABLENAME, 2);
        let entries = get_std_rows(&conn, MAINTABLENAME).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].idx, 2);
        assert_eq!(entries[1].alias, "scd".to_string());
    } // remove_row

    // Test stack functions

    #[test]
    #[serial]
    fn stack_add_remove() {
        let sessionid = "194811104321123401118419";
        let conn = just_open_db();

        let entry = StackRow {
            id: None,
            sessionid: sessionid.to_string(),
            directory: Utf8PathBuf::from("/home/east"),
        };
        let _ = add_stack_dir(&conn, &entry);
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].directory, Utf8PathBuf::from("/home/east"));

        let entry = StackRow {
            id: None,
            sessionid: sessionid.to_string(),
            directory: Utf8PathBuf::from("/home/south"),
        };
        let _ = add_stack_dir(&conn, &entry);
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].directory, Utf8PathBuf::from("/home/south"));
        assert_eq!(rows[1].directory, Utf8PathBuf::from("/home/east"));

        let top = stack_top(&conn, sessionid).unwrap();
        assert_eq!(top.id.unwrap(), 2);
        let _ = rm_stack_dir(&conn, top.id.unwrap());
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].directory, Utf8PathBuf::from("/home/east"));

        let top = stack_top(&conn, sessionid).unwrap();
        assert_eq!(top.id.unwrap(), 1);
        let _ = rm_stack_dir(&conn, top.id.unwrap());
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 0);
    } // stack_add_remove

    #[test]
    #[serial]
    fn stack_tidyup() {
        let fake_timestamp = get_timestamp(&Duration::days(STACKEXPIRE_DAYS + 1));

        let sessionid = "198411104321123401114819";
        let conn = just_open_db();

        let entry = StackRow {
            id: None,
            sessionid: sessionid.to_string(),
            directory: Utf8PathBuf::from("/etc/west"),
        };
        let _ = add_stack_dir(&conn, &entry);
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].directory, Utf8PathBuf::from("/etc/west"));

        let entry = StackRow {
            id: None,
            sessionid: sessionid.to_string(),
            directory: Utf8PathBuf::from("/etc/north"),
        };
        let _ = add_stack_dir(&conn, &entry);
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].directory, Utf8PathBuf::from("/etc/north"));
        assert_eq!(rows[1].directory, Utf8PathBuf::from("/etc/west"));

        let mut stmt = conn
            .prepare(&format!(
                "UPDATE {} SET timestamp=?1 WHERE id=1",
                STACKTABLENAME
            ))
            .unwrap();
        let res = stmt.execute([fake_timestamp]);
        assert!(res.is_ok());
        let rows = get_stack_rows(&conn, &sessionid).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].directory, Utf8PathBuf::from("/etc/north"));
        assert_eq!(rows[0].id, Some(2));
    } // stack_tidyup
} // mod tests
