use crate::{
    DataType, DataTypeReference,
    DefOpts,
    EnumRepr, EnumType, EnumVariant,
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
        sanitise_key,
        sanitise_type_name,
    }
};

/// Convert a type which implements [`Type`](crate::Type) to a TypeScript string with an export.
///
/// Eg. `export const dfl_Foo = { demo: ""; };`
pub fn export<T: NamedType>(conf: &ExportConfiguration) -> Result<String, TsExportError> {
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
pub fn inline<T: Type>(conf: &ExportConfiguration) -> Result<String, TsExportError> {
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
    )?;

    let generics = match item {
        // Named struct
        NamedDataTypeItem::Object(ObjectType {
            generics, fields, ..
        }) => match fields.len() {
            0 => Some(generics),
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

    Ok(format!(
        "export const dfl_{name}{generics} = () => {{ ({inline_ts}) }}"
    ))
}

/// Convert a DataType to a TypeScript string
///
/// Eg. `{ demo: string; }`
pub fn dfldt(conf: &ExportConfiguration, typ: &DataType) -> Result<String, TsExportError> {
    // TODO: Duplicate type name detection?

    dfldt_inner(ExportContext { conf, path: vec![] }, typ)
}

/// Convert a DataType to a TypeScript string with forced expansion
///
/// Eg. `{ demo: string; }`
pub fn dfldt_inlined(conf: &ExportConfiguration, typ: &DataType) -> Result<String, TsExportError> {
    // TODO: Duplicate type name detection?

    dfldt_inner_inlined(ExportContext { conf, path: vec![] }, typ)
}

fn dfldt_inner_inlined(ctx: ExportContext, typ: &DataType) -> Result<String, TsExportError> {
    Ok(match &typ {
        DataType::Named(NamedDataType {
            name,
            item: NamedDataTypeItem::Tuple(TupleType { fields, .. }),
            ..
        }) => tuple_dfldt(ctx.with(PathItem::Type(name)), fields)?,
        DataType::Named(NamedDataType {
            name,
            item: NamedDataTypeItem::Object(item),
            ..
        }) => object_dfldt(ctx.with(PathItem::Type(name)), Some(name), item)?,
        DataType::Named(NamedDataType {
            name,
            item: NamedDataTypeItem::Enum(item),
            ..
        }) => enum_dfldt(ctx.with(PathItem::Type(name)), Some(name), item)?,
        DataType::Named(NamedDataType {
            item: NamedDataTypeItem::Custom(custom),
            ..
        }) => custom.to_string(),
        _ => dfldt_inner(ctx, typ)?
    })
}

fn dfldt_inner(ctx: ExportContext, typ: &DataType) -> Result<String, TsExportError> {
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
            "".into()
        }
        DataType::Record(def) => {
            let divider = match &def.0 {
                DataType::Enum(_) => " in",
                DataType::Named(dt) => match dt.item {
                    NamedDataTypeItem::Enum(_) => " in",
                    _ => ":",
                },
                _ => ":",
            };

            format!(
                // We use this isn't of `Record<K, V>` to avoid issues with circular references.
                "{{ [key{divider} {}]: {} }}",
                dfldt_inner(ctx.clone(), &def.0)?,
                dfldt_inner(ctx, &def.1)?
            )
        }
        // We use `T[]` instead of `Array<T>` to avoid issues with circular references.
        DataType::List(def) => {
            let dt = dfldt_inner(ctx, def)?;
            format!("[] as {dt}")
        }
        // TODO: why here we don't know if it's inlined?
        DataType::Named(NamedDataType {
            item: NamedDataTypeItem::Custom(custom),
            ..
        }) => custom.to_string(),

        DataType::Named(NamedDataType { name, .. }) => name.to_string(),
        DataType::Tuple(TupleType { fields, .. }) => tuple_dfldt(ctx, fields)?,
        DataType::Object(item) => object_dfldt(ctx, None, item)?,
        DataType::Enum(item) => enum_dfldt(ctx, None, item)?,
        DataType::Reference(DataTypeReference { name, generics, .. }) => match &generics[..] {
            [] => name.to_string(),
            generics => {
                let generics = generics
                    .iter()
                    .map(|v| dfldt_inner(ctx.with(PathItem::Type(name)), v))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");

                format!("{name}<{generics}>")
            }
        },
        DataType::Generic(GenericType(ident)) => ident.to_string(),
        DataType::Custom(custom) => custom.to_owned(),
    })
}

fn tuple_dfldt(ctx: ExportContext, fields: &[DataType]) -> Result<String, TsExportError> {
    match fields {
        [] => Ok("null".to_string()),
        [ty] => dfldt_inner(ctx, ty),
        tys => Ok(format!(
            "[{}]",
            tys.iter()
                .map(|v| dfldt_inner(ctx.clone(), v))
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
                    dfldt_inner(ctx.with(PathItem::Field(field.key)), &field.ty)
                        .map(|type_str| format!("({type_str})"))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut unflattened_fields = fields
                .iter()
                .filter(|f| !f.flatten)
                .filter_map(|f| object_field_to_dfl(ctx.with(PathItem::Field(f.key)), f))
                .collect::<Result<Vec<_>, _>>()?;

            if let Some(tag) = tag {
                unflattened_fields.push(format!(
                    "{tag}: \"{}\"",
                    name.ok_or_else(|| TsExportError::UnableToTagUnnamedType(ctx.export_path()))?
                ));
            }

            if !unflattened_fields.is_empty() {
                field_sections.push(format!("{{ {} }}", unflattened_fields.join("; ")));
            }

            Ok(field_sections.join(" & "))
        }
    }
}

fn enum_dfldt(
    ctx: ExportContext,
    _ty_name: Option<&'static str>,
    e: &EnumType,
) -> Result<String, TsExportError> {
    if e.variants_len() == 0 {
        return Ok("never".to_string());
    }

    Ok(match e {
        EnumType::Tagged { variants, repr, .. } => variants
            .iter()
            .map(|(variant_name, variant)| {
                let ctx = ctx.with(PathItem::Variant(variant_name));
                let sanitised_name = sanitise_key(variant_name, true);

                Ok(match (repr, variant) {
                    (EnumRepr::Internal { tag }, EnumVariant::Unit) => {
                        format!("{{ {tag}: {sanitised_name} }}")
                    }
                    (EnumRepr::Internal { tag }, EnumVariant::Unnamed(tuple)) => {
                        let typ = dfldt_inner(ctx, &DataType::Tuple(tuple.clone()))?;
                        format!("({{ {tag}: {sanitised_name} }} & {typ})")
                    }
                    (EnumRepr::Internal { tag }, EnumVariant::Named(obj)) => {
                        let mut fields = vec![format!("{tag}: {sanitised_name}")];

                        fields.extend(
                            obj.fields
                                .iter()
                                .filter_map(|v| object_field_to_dfl(ctx.with(PathItem::Field(v.key)), v))
                                .collect::<Result<Vec<_>, _>>()?,
                        );

                        format!("{{ {} }}", fields.join("; "))
                    }
                    (EnumRepr::External, EnumVariant::Unit) => {
                        sanitised_name.to_string()
                    }

                    (EnumRepr::External, v) => {
                        let ts_values = dfldt_inner(ctx.clone(), &v.data_type())?;
                        let sanitised_name = sanitise_key(variant_name, false);

                        format!("{{ {sanitised_name}: {ts_values} }}")
                    }
                    (EnumRepr::Adjacent { tag, .. }, EnumVariant::Unit) => {
                        format!("{{ {tag}: {sanitised_name} }}")
                    }
                    (EnumRepr::Adjacent { tag, content }, v) => {
                        let ts_values = dfldt_inner(ctx, &v.data_type())?;

                        format!("{{ {tag}: {sanitised_name}; {content}: {ts_values} }}")
                    }
                })
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(" | "),
        EnumType::Untagged { variants, .. } => variants
            .iter()
            .map(|variant| {
                Ok(match variant {
                    EnumVariant::Unit => "null".to_string(),
                    v => dfldt_inner(ctx.clone(), &v.data_type())?,
                })
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(" | "),
    })
}

/// convert an object field into a Typescript string
fn object_field_to_dfl(ctx: ExportContext, field: &ObjectField) -> Option<Result<String, TsExportError>> {
    match field.ty {
        DataType::Nullable(_) => None,
        _ if field.optional => {
            None
        },
        _ => Some(Ok(format!("{}: {}", sanitise_key(field.key, false), dfldt_inner(ctx, &field.ty).ok()?)))
    }
}
