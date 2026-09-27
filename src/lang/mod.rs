pub mod ts;

macro_rules! primitive_def {
    ($($t:ident)+) => {
        $(PrimitiveType::$t)|+
    }
}

pub(crate) use primitive_def;
