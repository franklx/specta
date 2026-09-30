use crate::{
    DataType, DataTypeReference,
    DefOpts,
    GenericType,
    NamedDataType, NamedDataTypeItem, NamedType, ObjectField,
    ObjectType, PrimitiveType, TupleType,
    Type, TypeDefs,
    detect_duplicate_type_names,
    primitive_def,
    ts::{
        BigIntExportBehavior,
        ExportConfiguration, ExportContext,
        NamedLocation, PathItem,
        TsExportError,
        datatype_inner,
        sanitise_key,
        sanitise_type_name,
    }
};

/// Convert a type which implements [`Type`](crate::Type) to a TypeScript string with an export.
///
/// Eg. `export const dfl_Foo = { demo: ""; };`
fn __export<T: NamedType>(conf: &ExportConfiguration) -> Result<String, TsExportError> {
    let mut type_name = TypeDefs::default();
    let result = export_dfldt(
        conf,
        &T::definition_named_data_type(DefOpts {
            parent_inline: false,
            type_map: &mut type_name,
        })?,
    );

    if let Some((ty_name, l0, l1)) = detect_duplicate_type_names(&type_name).into_iter().next() {
        return Err(TsExportError::DuplicateTypeName(ty_name, l0, l1));
    }

    result
}

/// Convert a type which implements [`Type`](crate::Type) to a TypeScript string.
///
/// Eg. `{ demo: ""; };`
fn __inline<T: Type>(conf: &ExportConfiguration) -> Result<String, TsExportError> {
    let mut type_name = TypeDefs::default();
    let result = dfldt(
        conf,
        &T::inline(
            DefOpts {
                parent_inline: false,
                type_map: &mut type_name,
            },
            &[],
        )?,
    );

    if let Some((ty_name, l0, l1)) = detect_duplicate_type_names(&type_name).into_iter().next() {
        return Err(TsExportError::DuplicateTypeName(ty_name, l0, l1));
    }

    result
}

/// Convert a DataType to a TypeScript string
///
/// Eg. `export Name = { demo: string; }`
pub fn export_dfldt(
    conf: &ExportConfiguration,
    typ: &NamedDataType,
) -> Result<String, TsExportError> {
    // TODO: Duplicate type name detection?

    export_dfldt_inner(ExportContext { conf, path: vec![] }, typ)
}

fn export_dfldt_inner(
    ctx: ExportContext,
    NamedDataType {
        name,
        item,
        ..
    }: &NamedDataType,
) -> Result<String, TsExportError> {
    let ctx = ctx.with(PathItem::Type(name));
    let name = sanitise_type_name(ctx.clone(), NamedLocation::Type, name)?;

    let inline_ts = dfldt_inner(
        ctx.clone(),
        &match item {
            NamedDataTypeItem::Object(obj) => DataType::Object(obj.clone()),
            NamedDataTypeItem::Tuple(tuple) => DataType::Tuple(tuple.clone()),
            NamedDataTypeItem::Enum(enum_) => DataType::Enum(enum_.clone()),
            NamedDataTypeItem::Custom(custom) => DataType::Custom(custom.clone()),
        },
        false
    )?;

    let generics = match item {
        // Named struct
        NamedDataTypeItem::Object(ObjectType {
            generics, fields, ..
        }) => match fields.len() {
            0 => None,
            _ => (!generics.is_empty()).then_some(generics),
        },
        // Enum
        NamedDataTypeItem::Enum(e) => {
            let generics = e.generics();
            (!generics.is_empty()).then_some(generics)
        }
        // Struct with unnamed fields
        NamedDataTypeItem::Tuple(TupleType { generics, .. }) => {
            (!generics.is_empty()).then_some(generics)
        }
        // Custom definition
        NamedDataTypeItem::Custom(_) => None,
    };

    let generics = generics
        .map(|generics| format!("<{}>", generics.to_vec().join(", ")))
        .unwrap_or_default();

    Ok(format!("export const dfl_{name}{generics}: () => {name} = () => ({inline_ts})"))
}

/// Convert a DataType to a TypeScript string
///
/// Eg. `{ demo: string; }`
pub fn dfldt(conf: &ExportConfiguration, typ: &DataType) -> Result<String, TsExportError> {
    // TODO: Duplicate type name detection?

    dfldt_inner(ExportContext { conf, path: vec![] }, typ, false)
}

fn dfldt_inner(ctx: ExportContext, typ: &DataType, is_attr: bool) -> Result<String, TsExportError> {
    Ok(match &typ {
        DataType::Any => "null".into(),
        DataType::Primitive(p) => {
            let ctx = ctx.with(PathItem::Type(p.to_rust_str()));
            match p {
                primitive_def!(i8 i16 i32 u8 u16 u32 f32 f64) => "0".into(),
                primitive_def!(usize isize i64 u64 i128 u128) => match ctx.conf.bigint {
                    BigIntExportBehavior::String => r#""""#.into(),
                    BigIntExportBehavior::Number => "0".into(),
                    BigIntExportBehavior::BigInt => "0".into(),
                    BigIntExportBehavior::Fail => {
                        return Err(TsExportError::BigIntForbidden(ctx.export_path()))
                    }
                    BigIntExportBehavior::FailWithReason(reason) => {
                        return Err(TsExportError::Other(ctx.export_path(), reason.to_owned()))
                    }
                },
                primitive_def!(String char) => r#""""#.into(),
                primitive_def!(bool) => "false".into(),
            }
        }
        DataType::Literal(literal) => literal.to_ts(),
        DataType::Nullable(_) => {
            "{}".into()
        }
        DataType::Record(def) => {
            format!(
                "{{}} as Record<{},{}>",
                datatype_inner(ctx.clone(), &def.0)?,
                datatype_inner(ctx, &def.1)?
            )
        }
        // We use `T[]` instead of `Array<T>` to avoid issues with circular references.
        DataType::List(def) => {
            format!("[] as {}[]", datatype_inner(ctx, def)?)
        }
        DataType::Named(NamedDataType {
            item: NamedDataTypeItem::Custom(custom),
            ..
        }) => custom.to_string(),
        DataType::Named(NamedDataType { name, .. }) => if is_attr { format!("undefined as {name}") } else { name.to_string() },
        DataType::Tuple(TupleType { fields, .. }) => tuple_dfldt(ctx, fields)?,
        DataType::Object(item) => object_dfldt(ctx, None, item)?,
        DataType::Enum(_) => unimplemented!("Default for enums is not supported"),
        DataType::Reference(DataTypeReference { name, generics, .. }) => match &generics[..] {
            [] => if is_attr { format!("undefined as {name}") } else { name.to_string() },
            generics => {
                let generics = generics
                    .iter()
                    .map(|v| dfldt_inner(ctx.with(PathItem::Type(name)), v, false))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                if is_attr {
                    format!("undefined as {name}<{generics}>")
                }
                else {
                    format!("{name}<{generics}>")
                }
            }
        },
        DataType::Generic(GenericType(ident)) => ident.to_string(),
        DataType::Custom(custom) => custom.to_owned(),
    })
}

fn tuple_dfldt(ctx: ExportContext, fields: &[DataType]) -> Result<String, TsExportError> {
    match fields {
        [] => Ok("null".to_string()),
        [ty] => dfldt_inner(ctx, ty, true),
        tys => Ok(format!(
            "[{}]",
            tys.iter()
                .map(|v| dfldt_inner(ctx.clone(), v, true))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        )),
    }
}

fn object_dfldt(
    ctx: ExportContext,
    name: Option<&'static str>,
    ObjectType { fields, tag, .. }: &ObjectType,
) -> Result<String, TsExportError> {
    match &fields[..] {
        [] => Ok("null".to_string()),
        fields => {
            let mut field_sections = fields
                .iter()
                .filter(|f| f.flatten)
                .map(|field| {
                    dfldt_inner(ctx.with(PathItem::Field(field.key)), &field.ty, false)
                        .map(|type_str| format!("dfl_{type_str}()"))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut unflattened_fields = fields
                .iter()
                .filter(|f| !f.flatten)
                .filter_map(|f| object_field_to_dfl(ctx.with(PathItem::Field(f.key)), f))
                .collect::<Result<Vec<_>, _>>()?;

            if let Some(tag) = tag {
                unflattened_fields.push(format!(
                    r#"{tag}: "{}""#,
                    name.ok_or_else(|| TsExportError::UnableToTagUnnamedType(ctx.export_path()))?
                ));
            }

            if !unflattened_fields.is_empty() {
                field_sections.push(format!("{{ {} }}", unflattened_fields.join(", ")));
            }

            Ok(
                if field_sections.is_empty() {
                    "{}".to_owned()
                }
                else if field_sections.len() == 1 {
                    field_sections.join("")
                }
                else {
                    field_sections.iter().map(|s| format!("...{s}")).collect::<Vec<_>>().join(", ")
                }
            )
        }
    }
}

/// convert an object field into a Typescript string
fn object_field_to_dfl(ctx: ExportContext, field: &ObjectField) -> Option<Result<String, TsExportError>> {
    match field.ty {
        DataType::Nullable(_) =>
            None,
        _ if field.optional =>
            None,
        _ =>
            Some(Ok(format!("{}: {}", sanitise_key(field.key, false), dfldt_inner(ctx, &field.ty, true).ok()?)))
    }
}
