//! Shape Data (`Prop` section) field mapping (GitHub #30): a Prop
//! row's Type/Format cells choose the editor kind — Visio types 0-7.

use vsdx::model::Row;

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Str,
    /// Fixed (1) or variable (4) list: the ";"-separated `Format`
    /// choices.
    List(Vec<String>),
    Number,
    Bool,
    /// Duration (5), date (6), currency (7): free text with a Format
    /// hint for validation.
    Formatted(String),
}

/// The editor kind for one Prop row: `Type` cell (falling back to its
/// cached value / 0-string), `Format` for lists and formatted kinds.
pub fn field_kind(row: &Row) -> FieldKind {
    let cell_num = |n: &str| -> Option<f64> { row.cell(n).and_then(|c| c.numeric()) };
    let format = row
        .cell("Format")
        .and_then(|c| c.v.clone())
        .unwrap_or_default();
    match cell_num("Type").map(|t| t as i64) {
        Some(1) | Some(4) => FieldKind::List(
            format
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        ),
        Some(2) => FieldKind::Number,
        Some(3) => FieldKind::Bool,
        Some(5) | Some(6) | Some(7) => FieldKind::Formatted(format),
        _ => FieldKind::Str,
    }
}

/// The row's `SortKey` as text (Visio's ShapeSheet SortKey cell is a
/// string); None when absent or blank.
pub fn sort_key(row: &Row) -> Option<&str> {
    row.cell("SortKey")
        .and_then(|c| c.v.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Order Prop rows for display: string SortKey ascending, rows without
/// a SortKey after the keyed ones, original row order preserved when
/// keys are equal (the dialog's stable sort supplies that half).
pub fn cmp_sort_key(a: &Row, b: &Row) -> std::cmp::Ordering {
    match (sort_key(a), sort_key(b)) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

pub fn invisible(row: &Row) -> bool {
    row.cell("Invisible")
        .and_then(|c| c.numeric())
        .is_some_and(|v| v != 0.0)
}

/// The field label: `Label`, else the row name.
pub fn label(row: &Row, name: &str) -> String {
    row.cell("Label")
        .and_then(|c| c.v.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsdx::model::{Cell, Row};

    fn prop(cells: &[(&str, &str)]) -> Row {
        Row {
            cells: cells
                .iter()
                .map(|(n, v)| (n.to_string(), Cell::value(*v)))
                .collect(),
            ..Row::default()
        }
    }

    #[test]
    fn field_kinds_follow_type_and_format() {
        assert_eq!(field_kind(&prop(&[("Type", "0")])), FieldKind::Str);
        assert_eq!(
            field_kind(&prop(&[("Type", "1"), ("Format", "Sales; Eng ; Ops")])),
            FieldKind::List(vec!["Sales".into(), "Eng".into(), "Ops".into()])
        );
        // Variable lists (4) share the fixed-list editor.
        assert_eq!(
            field_kind(&prop(&[("Type", "4"), ("Format", "a;b")])),
            FieldKind::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(field_kind(&prop(&[("Type", "2")])), FieldKind::Number);
        assert_eq!(field_kind(&prop(&[("Type", "3")])), FieldKind::Bool);
        assert_eq!(
            field_kind(&prop(&[("Type", "6"), ("Format", "yyyy")])),
            FieldKind::Formatted("yyyy".into())
        );
        // Missing Type reads as a plain string.
        assert_eq!(field_kind(&prop(&[])), FieldKind::Str);
    }

    #[test]
    fn display_helpers() {
        let row = prop(&[("SortKey", "3"), ("Invisible", "1"), ("Label", "Dept")]);
        assert_eq!(sort_key(&row), Some("3"));
        assert!(invisible(&row));
        assert_eq!(label(&row, "Department"), "Dept");
        assert_eq!(label(&prop(&[]), "Department"), "Department");
    }

    #[test]
    fn sort_key_is_text_and_unkeyed_rows_sort_last() {
        let k10 = prop(&[("SortKey", "10")]);
        let k2 = prop(&[("SortKey", "2")]);
        let unkeyed = prop(&[]);
        // Text ordering: "10" sorts before "2" (a SortKey is a string
        // cell), and a blank key is no key at all.
        assert_eq!(sort_key(&prop(&[("SortKey", "  ")])), None);
        assert_eq!(cmp_sort_key(&k10, &k2), std::cmp::Ordering::Less);
        assert_eq!(cmp_sort_key(&k2, &k10), std::cmp::Ordering::Greater);
        assert_eq!(cmp_sort_key(&k2, &unkeyed), std::cmp::Ordering::Less);
        assert_eq!(cmp_sort_key(&unkeyed, &k2), std::cmp::Ordering::Greater);
        assert_eq!(cmp_sort_key(&unkeyed, &unkeyed), std::cmp::Ordering::Equal);
    }
}
