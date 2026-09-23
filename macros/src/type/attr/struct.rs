use syn::Result;

use crate::utils::Attribute;

#[derive(Default)]
pub struct StructAttr {
    pub transparent: bool,
    pub custom: Option<String>,
}

impl_parse! {
    StructAttr(attr, out) {
        "transparent" => out.transparent = attr.parse_bool().unwrap_or(true),
        "custom" => out.custom = out.custom.take().or(Some(attr.parse_string()?)),
    }
}

impl StructAttr {
    pub fn from_attrs(attrs: &mut Vec<Attribute>) -> Result<Self> {
        let mut result = Self::default();
        Self::try_from_attrs("specta", attrs, &mut result)?;
        #[cfg(feature = "serde")]
        Self::try_from_attrs("serde", attrs, &mut result)?;
        Self::try_from_attrs("repr", attrs, &mut result)?; // To handle `#[repr(transparent)]`
        Ok(result)
    }
}
