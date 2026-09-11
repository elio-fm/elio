use super::goto_column_count;

#[test]
fn goto_columns_avoid_single_item_last_row() {
    assert_eq!(goto_column_count(5), 5);
    assert_eq!(goto_column_count(6), 3);
    assert_eq!(goto_column_count(7), 4);
    assert_eq!(goto_column_count(8), 4);
    assert_eq!(goto_column_count(10), 5);
    assert_eq!(goto_column_count(11), 4);
    assert_eq!(goto_column_count(13), 5);
    assert_eq!(goto_column_count(14), 5);
}
