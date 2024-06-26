use anyhow::{Context, Result};
use bdat::compat::{CompatColumnRef, CompatTable};
use bdat::serde::SerializeCell;
use bdat::{Cell, Value};
use clap::Args;
use csv::WriterBuilder;
use std::io::Write;
use std::iter::Once;

use super::{BdatSerialize, ConvertArgs};

#[derive(Args)]
pub struct CsvOptions {
    #[arg(long)]
    csv_separator: Option<char>,
    /// When converting to CSV, expands legacy-BDAT lists into separate columns
    #[arg(long)]
    expand_lists: bool,
}

pub struct CsvConverter {
    separator_ch: char,
    expand_lists: bool,
    untyped: bool,
}

/// Utility to `flat_map` multiple iterator types
enum ColumnIter<E, T: Iterator<Item = E>, T2: Iterator<Item = E>> {
    Single(Once<E>),
    Flags(T),
    Array(T2),
}

impl CsvConverter {
    pub fn new(args: &ConvertArgs) -> Self {
        Self {
            separator_ch: args.csv_opts.csv_separator.unwrap_or(','),
            expand_lists: args.csv_opts.expand_lists,
            untyped: args.untyped,
        }
    }

    fn format_column<'b>(&'b self, column: CompatColumnRef<'b, 'b>) -> Vec<String> {
        let iter = {
            let label = column.label();
            if !column.flags().is_empty() {
                ColumnIter::Flags(
                    column
                        .flags()
                        .iter()
                        .map(move |flag| format!("{} [{}]", label, flag.label())),
                )
            } else if column.count() > 1 && self.expand_lists {
                ColumnIter::Array((0..column.count()).map(move |i| format!("{}[{i}]", label)))
            } else {
                ColumnIter::Single(std::iter::once(label.to_string()))
            }
        };
        let value_type = column.value_type() as u8;
        iter.map(move |s| {
            if !self.untyped {
                format!("{s} {{{}}}", value_type)
            } else {
                s
            }
        })
        .collect::<Vec<_>>()
    }

    fn format_cell<'b, 'a: 'b, 't: 'a>(
        &self,
        column: CompatColumnRef<'a, 't>,
        cell: Cell<'t>,
    ) -> ColumnIter<
        SerializeCell<'b, 't, CompatColumnRef<'a, 't>>,
        impl Iterator<Item = SerializeCell<'b, 't, CompatColumnRef<'a, 't>>>,
        impl Iterator<Item = SerializeCell<'b, 't, CompatColumnRef<'a, 't>>>,
    > {
        match cell {
            // Single values: serialize normally
            c @ Cell::Single(_) => {
                ColumnIter::Single(std::iter::once(SerializeCell::from_owned(column, c)))
            }
            // List values + expand lists: serialize into multiple columns
            Cell::List(values) if self.expand_lists => ColumnIter::Array(
                values
                    .into_iter()
                    .map(move |v| SerializeCell::from_owned(column, Cell::Single(v.clone()))),
            ),
            // List values: serialize as JSON
            Cell::List(values) => ColumnIter::Single(std::iter::once(SerializeCell::from_owned(
                column,
                Cell::Single(Value::String(
                    serde_json::to_string(&values).unwrap().into(),
                )),
            ))),
            // Flags: serialize into multiple columns
            Cell::Flags(flags) => ColumnIter::Flags(flags.into_iter().map(move |i| {
                SerializeCell::from_owned(column, Cell::Single(Value::UnsignedInt(i)))
            })),
        }
    }
}

impl BdatSerialize for CsvConverter {
    fn write_table(&self, table: CompatTable, writer: &mut dyn Write) -> Result<()> {
        let mut writer = WriterBuilder::new()
            .delimiter(self.separator_ch as u8)
            .from_writer(writer);

        let header = table
            .columns()
            .flat_map(|c| self.format_column(c))
            .collect::<Vec<_>>();

        writer.serialize(header).context("Failed to write header")?;

        for row in table.rows() {
            let serialized_row = row
                .cells()
                .zip(table.columns())
                .flat_map(|(cell, col)| self.format_cell(col, cell))
                .collect::<Vec<_>>();
            writer
                .serialize(serialized_row)
                .with_context(|| format!("Failed to write row {}", row.id()))?;
        }
        Ok(())
    }

    fn get_file_name(&self, table_name: &str) -> String {
        format!("{table_name}.csv")
    }
}

impl<E, T: Iterator<Item = E>, T2: Iterator<Item = E>> Iterator for ColumnIter<E, T, T2> {
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Single(i) => i.next(),
            Self::Flags(i) => i.next(),
            Self::Array(i) => i.next(),
        }
    }
}
