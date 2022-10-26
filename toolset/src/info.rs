use std::{fs::File, io::BufReader};

use crate::{
    filter::{Filter, FilterArg},
    hash::HashNameTable,
    InputData,
};
use anyhow::{Context, Result};
use bdat::{
    io::{BdatFile, LittleEndian},
    types::Label,
};
use clap::Args;

#[derive(Args)]
pub struct InfoArgs {
    /// Only check these tables. If absent, returns data from all tables.
    #[arg(short, long)]
    tables: Vec<String>,
    /// Only print these columns. If absent, prints all columns.
    #[arg(short, long)]
    columns: Vec<String>,
    /// If this is set, saves a copy of the file's schema in the specified directory.
    #[arg(short, long)]
    out_schema: Option<String>,
}

pub fn get_info(input: InputData, args: InfoArgs) -> Result<()> {
    let hash_table = input.load_hashes()?;
    let table_filter: Filter = args.tables.into_iter().map(FilterArg).collect();
    let column_filter: Filter = args.columns.into_iter().map(FilterArg).collect();

    for file in input.list_files("bdat") {
        let path = file?;
        let file = BufReader::new(File::open(&path)?);
        let mut file =
            BdatFile::<_, LittleEndian>::read(file).context("Failed to read BDAT file")?;
        for mut table in file
            .get_tables()
            .with_context(|| format!("Could not parse BDAT tables ({})", path.to_string_lossy()))?
        {
            let name = match table.name {
                Some(ref mut n) => {
                    if !table_filter.contains(&n) {
                        continue;
                    }
                    n
                }
                None => continue,
            };
            println!("Table {}", format_unhashed_label(name, &hash_table));
            println!(
                "  Columns: {} / Rows: {}",
                table.column_count(),
                table.row_count()
            );

            if table.column_count() != 0 {
                println!("  Columns:");
                for mut col in table
                    .into_columns()
                    .filter(|c| column_filter.contains(&c.label))
                {
                    println!(
                        "    - [{}] {}: {:?}",
                        col.offset,
                        format_unhashed_label(&mut col.label, &hash_table),
                        col.ty
                    );
                }
            }
        }
    }

    Ok(())
}

fn format_unhashed_label(label: &mut Label, hash_table: &HashNameTable) -> String {
    let previous_hash = match label {
        Label::Hash(h) => Some(*h),
        _ => None,
    };

    hash_table.convert_label(label);
    match (label, previous_hash) {
        (l @ Label::Unhashed(_), Some(hash)) => format!("{l} (<{hash:08X}>)"),
        (l, _) => l.to_string(),
    }
}
