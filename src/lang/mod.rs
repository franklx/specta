/// [TypeScript](https://www.typescriptlang.org) language exporter.
#[cfg(feature = "typescript")]
#[cfg_attr(docsrs, doc(cfg(feature = "typescript")))]
pub mod ts;

macro_rules! primitive_def {
    ($($t:ident)+) => {
        $(PrimitiveType::$t)|+
    }
}

pub(crate) use primitive_def;
