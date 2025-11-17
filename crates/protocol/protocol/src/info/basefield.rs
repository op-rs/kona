/// For structs that has an embedded basefield.
pub(crate) trait HasBaseField<BaseField> {
    fn base(&self) -> BaseField;
}
