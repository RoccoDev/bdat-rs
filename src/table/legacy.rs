use crate::{
    ColumnDef, ColumnMap, Label, LegacyCell, ModernCell, Row, RowIter, RowRef, RowRefMut,
    TableBuilder,
};

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyTable<'b> {
    pub(crate) name: Label,
    pub(crate) base_id: usize,
    pub(crate) columns: ColumnMap,
    pub(crate) rows: Vec<Row<'b>>,
}

impl<'b> LegacyTable<'b> {
    pub(crate) fn new(builder: TableBuilder<'b>) -> Self {
        Self {
            name: builder.name,
            columns: builder.columns,
            base_id: builder
                .rows
                .iter()
                .map(|r| r.id())
                .min()
                .unwrap_or_default(),
            rows: builder.rows,
        }
    }

    /// Returns the table's name.
    pub fn name(&self) -> &Label {
        &self.name
    }

    /// Updates the table's name.
    pub fn set_name(&mut self, name: Label) {
        self.name = name;
    }

    /// Gets the minimum row ID in the table.
    pub fn base_id(&self) -> usize {
        self.base_id
    }

    /// Gets a row by its ID.
    ///
    /// ## Panics
    /// If there is no row for the given ID.
    ///
    /// ## Example
    /// ```
    /// use bdat::{Label, ModernTable};
    ///
    /// fn foo(table: &ModernTable) -> u32 {
    ///     // This is a `ModernCell`, which is essentially a single value.
    ///     // As such, it can be used to avoid having to match on single-value cells
    ///     // that are included for legacy compatibility.
    ///     let cell = table.row(1).get(Label::Hash(0xDEADBEEF));
    ///     // Casting values is also supported:
    ///     cell.get_as::<u32>()
    /// }
    /// ```
    pub fn row(&self, id: usize) -> RowRef<'_, 'b, LegacyCell<'_, 'b>> {
        self.get_row(id).expect("no such row")
    }

    /// Gets a mutable view of a row by its ID
    ///
    /// Note: the ID is the row's numerical ID, which could be different
    /// from the index of the row in the table's row list. That is because
    /// BDAT tables can have arbitrary start IDs.
    ///
    /// ## Panics
    /// If there is no row for the given ID
    pub fn row_mut(&mut self, id: usize) -> RowRefMut<'_, 'b> {
        self.get_row_mut(id).expect("no such row")
    }

    /// Attempts to get a row by its ID.  
    /// If there is no row for the given ID, this returns [`None`].
    ///
    /// Note: the ID is the row's numerical ID, which could be different
    /// from the index of the row in the table's row list. That is because
    /// BDAT tables can have arbitrary start IDs.
    pub fn get_row(&self, id: usize) -> Option<RowRef<'_, 'b, LegacyCell<'_, 'b>>> {
        let index = id.checked_sub(self.base_id)?;
        self.rows
            .get(index)
            .map(|row| RowRef::new(row, &self.columns))
    }

    /// Attempts to get a mutable view of a row by its ID.  
    /// If there is no row for the given ID, this returns [`None`].
    ///
    /// Note: the ID is the row's numerical ID, which could be different
    /// from the index of the row in the table's row list. That is because
    /// BDAT tables can have arbitrary start IDs.
    pub fn get_row_mut(&mut self, id: usize) -> Option<RowRefMut<'_, 'b>> {
        let index = id.checked_sub(self.base_id)?;
        self.rows
            .get_mut(index)
            .map(|row| RowRefMut::new(row, &self.columns))
    }

    /// Gets an iterator that visits this table's rows
    pub fn rows(&self) -> impl Iterator<Item = RowRef<'_, 'b, LegacyCell<'_, 'b>>> {
        self.rows.iter().map(|row| RowRef::new(row, &self.columns))
    }

    /// Gets an iterator over mutable references to this table's
    /// rows.
    ///
    /// The iterator does not allow structural modifications to the table. To add, remove, or
    /// reorder rows, convert the table to a new builder first. (`TableBuilder::from(table)`)
    ///
    /// Additionally, if the iterator is used to replace rows, proper care must be taken to
    /// ensure the new rows have the same IDs, as to preserve the original table's row order.
    ///
    /// When the `hash-table` feature is enabled, the new rows must also retain their original
    /// hashed ID (for modern BDATs). Failure to do so will lead to improper behavior of
    /// [`get_row_by_hash`].
    ///
    /// [`get_row_by_hash`]: Table::get_row_by_hash
    pub fn rows_mut(&mut self) -> impl Iterator<Item = RowRefMut<'_, 'b>> {
        self.rows
            .iter_mut()
            .map(|row| RowRefMut::new(row, &self.columns))
    }

    /// Gets an owning iterator over this table's rows
    pub fn into_rows(self) -> impl Iterator<Item = Row<'b>> {
        self.rows.into_iter()
    }

    /// Gets an iterator that visits this table's column definitions
    pub fn columns(&self) -> impl Iterator<Item = &ColumnDef> {
        self.columns.as_slice().iter()
    }

    /// Gets an iterator over mutable references to this table's
    /// column definitions.
    pub fn columns_mut(&mut self) -> impl Iterator<Item = &mut ColumnDef> {
        self.columns.as_mut_slice().iter_mut()
    }

    /// Gets an owning iterator over this table's column definitions
    pub fn into_columns(self) -> impl Iterator<Item = ColumnDef> {
        self.columns.into_raw().into_iter()
    }

    /// Gets the number of rows in the table
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Gets the number of columns in the table
    pub fn column_count(&self) -> usize {
        self.columns.as_slice().len()
    }

    /// Returns an ergonomic iterator view over the table's rows and columns.
    pub fn iter(&self) -> RowIter<Self> {
        self.into_iter()
    }
}
