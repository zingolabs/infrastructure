//! A crate-internal, minimal TOML-subset parser and renderer.
//!
//! This module is a drop-in replacement for the two entry points this
//! crate used from the `toml` crate — [`from_str`] and [`to_string`],
//! with the crate's `de::Error` / `ser::Error` split — implemented
//! in-repo so the manifest machinery carries no TOML dependency.
//!
//! # This is NOT a full TOML parser
//!
//! It deliberately implements only the minimal dialect the artifact
//! manifest needs. A document is accepted when it uses nothing but:
//!
//! - comments (`# …`), whole-line or trailing a value, and blank lines;
//! - bare keys (ASCII letters, digits, `_`, `-`);
//! - table headers (`[validator]`) naming a single bare key, each
//!   header appearing at most once;
//! - basic strings (`"…"`) confined to one line, with the standard
//!   TOML escapes (`\b \t \n \f \r \" \\ \uXXXX \UXXXXXXXX`);
//! - decimal integers (optional leading `-`, no underscores).
//!
//! Every other TOML construct is rejected with an error naming the
//! construct and its line — never silently misread. The rejected
//! constructs are:
//!
//! - literal strings (`'…'`) and multi-line strings (`"""…"""`,
//!   `'''…'''`);
//! - arrays, inline tables, and arrays of tables (`[[…]]`);
//! - dotted keys and quoted keys;
//! - floats, booleans, date-times, hex/octal/binary integers, and
//!   underscore-separated integers;
//! - duplicate keys and duplicate table headers (full TOML rejects
//!   these too).
//!
//! # Rendering
//!
//! [`to_string`] emits the canonical shape the `toml` crate (0.8.23)
//! emitted for the manifest schema: top-level scalar pairs first, then
//! each table preceded by one blank line, fields in struct declaration
//! order, no indentation, trailing newline. One deliberate divergence:
//! a string needing escapes renders as a basic string with backslash
//! escapes, where the `toml` crate switched to a literal string — this
//! module's renderer never emits what its own parser cannot read.
//! Equivalence with `toml` 0.8.23 is pinned by the golden fixtures in
//! this module's tests.

#![forbid(unsafe_code)]

use serde::de::DeserializeOwned;
use serde::ser::{Impossible, Serialize, SerializeStruct, Serializer};

/// Deserialization: the error type of [`from_str`].
pub(crate) mod de {
    /// A parse or schema error, mirroring `toml::de::Error`'s role.
    /// Syntax errors carry the offending line; schema errors (unknown
    /// field, wrong type) are line-less.
    #[derive(Debug)]
    pub struct Error {
        pub(super) line: Option<usize>,
        pub(super) message: String,
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.line {
                Some(line) => write!(
                    f,
                    "TOML-subset parse error at line {line}: {}",
                    self.message
                ),
                None => write!(f, "{}", self.message),
            }
        }
    }

    impl std::error::Error for Error {}
}

/// Serialization: the error type of [`to_string`].
pub(crate) mod ser {
    /// A rendering error, mirroring `toml::ser::Error`'s role.
    #[derive(Debug)]
    pub struct Error(pub(super) String);

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for Error {}

    impl serde::ser::Error for Error {
        fn custom<T: std::fmt::Display>(message: T) -> Self {
            Error(message.to_string())
        }
    }
}

/// Parse a document of the manifest dialect into any deserializable
/// type. Interface-compatible with `toml::from_str`.
pub(crate) fn from_str<T: DeserializeOwned>(text: &str) -> Result<T, de::Error> {
    let value = parse_document(text)?;
    serde_json::from_value(value).map_err(|schema_error| de::Error {
        line: None,
        message: schema_error.to_string(),
    })
}

/// Render a serializable value as a document of the manifest dialect.
/// Interface-compatible with `toml::to_string`.
pub(crate) fn to_string<T: ?Sized + Serialize>(value: &T) -> Result<String, ser::Error> {
    match value.serialize(NodeSerializer)? {
        Node::Table(pairs) => render_document(&pairs),
        Node::Scalar(_) => Err(ser::Error(
            "only a table-shaped (struct) top-level value can be rendered".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Parsing: dialect text → `serde_json::Value` tree. The schema mapping
// (field names, types, unknown-field rejection) is then serde_json's
// job, so the schema stays defined once, on the derives.
// ---------------------------------------------------------------------------

fn parse_document(text: &str) -> Result<serde_json::Value, de::Error> {
    let mut root = serde_json::Map::new();
    let mut current_table: Option<String> = None;

    for (index, raw_line) in text.lines().enumerate() {
        let line = index + 1;
        let error = |message: String| de::Error {
            line: Some(line),
            message,
        };
        let rest = raw_line.trim_start();
        if rest.is_empty() || rest.starts_with('#') {
            continue;
        }

        if let Some(header) = rest.strip_prefix('[') {
            if header.starts_with('[') {
                return Err(error(
                    "arrays of tables (`[[…]]`) are not supported by the manifest dialect"
                        .to_string(),
                ));
            }
            let (name, after_key) = take_bare_key(header.trim_start())
                .ok_or_else(|| error("expected a bare key as the table name".to_string()))?;
            let after_key = after_key.trim_start();
            if let Some(after_bracket) = after_key.strip_prefix(']') {
                require_only_trailing_comment(after_bracket).map_err(&error)?;
            } else if after_key.starts_with('.') {
                return Err(error(format!(
                    "dotted table name after `[{name}` — nested tables are not supported \
                     by the manifest dialect"
                )));
            } else {
                return Err(error(format!("expected `]` to close the `[{name}` header")));
            }
            if root.contains_key(&name) {
                return Err(error(format!("table `[{name}]` is defined more than once")));
            }
            root.insert(
                name.clone(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
            current_table = Some(name);
            continue;
        }

        let (key, after_key) = parse_key(rest).map_err(&error)?;
        let after_equals = after_key
            .trim_start()
            .strip_prefix('=')
            .ok_or_else(|| error(format!("expected `=` after key `{key}`")))?;
        let (value, after_value) = parse_value(after_equals.trim_start()).map_err(&error)?;
        require_only_trailing_comment(after_value).map_err(&error)?;

        let table = match &current_table {
            Some(name) => root
                .get_mut(name)
                .and_then(serde_json::Value::as_object_mut)
                .expect("the current table was inserted when its header was parsed"),
            None => &mut root,
        };
        if table.insert(key.clone(), value).is_some() {
            return Err(error(format!("key `{key}` is set more than once")));
        }
    }

    Ok(serde_json::Value::Object(root))
}

/// A bare key at the head of `rest`: the key and what follows it.
/// `None` when `rest` does not start with a bare-key character.
fn take_bare_key(rest: &str) -> Option<(String, &str)> {
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(rest.len());
    (end > 0).then(|| (rest[..end].to_string(), &rest[end..]))
}

/// The key position of a `key = value` line, rejecting the key forms
/// the dialect excludes.
fn parse_key(rest: &str) -> Result<(String, &str), String> {
    if rest.starts_with('"') || rest.starts_with('\'') {
        return Err("quoted keys are not supported by the manifest dialect; \
                    use a bare key (letters, digits, `_`, `-`)"
            .to_string());
    }
    let (key, after_key) =
        take_bare_key(rest).ok_or_else(|| format!("expected a bare key, found {rest:?}"))?;
    if after_key.trim_start().starts_with('.') {
        return Err(format!(
            "dotted key after `{key}` — dotted keys are not supported by the manifest dialect; \
             put the key under a `[table]` header instead"
        ));
    }
    Ok((key, after_key))
}

/// The value position of a `key = value` line: the parsed value and
/// what follows it. Rejects every value form outside the dialect with
/// an error naming the construct.
fn parse_value(rest: &str) -> Result<(serde_json::Value, &str), String> {
    match rest.chars().next() {
        Some('"') => {
            if rest.starts_with("\"\"\"") {
                return Err(
                    "multi-line strings (`\"\"\"…\"\"\"`) are not supported by the manifest \
                     dialect"
                        .to_string(),
                );
            }
            let (string, after) = parse_basic_string(&rest[1..])?;
            Ok((serde_json::Value::String(string), after))
        }
        Some('\'') => Err(
            "literal strings (`'…'`) are not supported by the manifest dialect; \
                           use a basic `\"…\"` string with escapes"
                .to_string(),
        ),
        Some('[') => Err("arrays are not supported by the manifest dialect".to_string()),
        Some('{') => Err("inline tables are not supported by the manifest dialect".to_string()),
        Some(_) => {
            let end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '#')
                .unwrap_or(rest.len());
            let token = &rest[..end];
            Ok((parse_scalar_token(token)?, &rest[end..]))
        }
        None => Err("expected a value after `=`".to_string()),
    }
}

/// Classify an unquoted value token: a decimal integer, or a precise
/// rejection naming which unsupported TOML form it looks like.
fn parse_scalar_token(token: &str) -> Result<serde_json::Value, String> {
    let unsupported = |construct: &str| {
        format!("{construct} are not supported by the manifest dialect (found {token:?})")
    };
    if token == "true" || token == "false" {
        return Err(unsupported("booleans"));
    }
    let digits = token.strip_prefix('-').unwrap_or(token);
    if digits.starts_with("0x") || digits.starts_with("0o") || digits.starts_with("0b") {
        return Err(unsupported("hex, octal, and binary integers"));
    }
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '_') {
        if digits.contains('_') {
            return Err(unsupported("underscore-separated integers"));
        }
        let value: i64 = token
            .parse()
            .map_err(|_| format!("integer {token:?} does not fit in 64 bits"))?;
        return Ok(serde_json::Value::from(value));
    }
    let numeric_head = digits.chars().next().is_some_and(|c| c.is_ascii_digit());
    if numeric_head && (digits.contains('-') || digits.contains(':')) {
        return Err(unsupported("date-times"));
    }
    if numeric_head || token.starts_with('+') || token == "inf" || token == "nan" {
        return Err(unsupported("floats and non-decimal number forms"));
    }
    Err(format!("unrecognized value {token:?}"))
}

/// The remainder of a basic string after its opening quote: the
/// unescaped content and what follows the closing quote.
fn parse_basic_string(rest: &str) -> Result<(String, &str), String> {
    let mut content = String::new();
    let mut chars = rest.char_indices();
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return Ok((content, &rest[index + 1..])),
            '\\' => content.push(parse_escape(&mut chars)?),
            c if c != '\t' && (c.is_control()) => {
                return Err(format!(
                    "control character {:?} in a basic string; escape it (e.g. `\\u{:04X}`)",
                    c, c as u32
                ));
            }
            c => content.push(c),
        }
    }
    Err("unterminated basic string (the dialect confines strings to one line)".to_string())
}

/// One escape sequence, with the cursor just past the backslash.
fn parse_escape(chars: &mut std::str::CharIndices<'_>) -> Result<char, String> {
    let (_, escape) = chars
        .next()
        .ok_or_else(|| "dangling `\\` at end of string".to_string())?;
    let hex_escape = |chars: &mut std::str::CharIndices<'_>, count: usize| {
        let hex: String = chars.take(count).map(|(_, c)| c).collect();
        if hex.len() < count {
            return Err(format!("`\\{escape}` expects {count} hex digits"));
        }
        let code = u32::from_str_radix(&hex, 16)
            .map_err(|_| format!("`\\{escape}{hex}` is not a hex escape"))?;
        char::from_u32(code).ok_or_else(|| format!("`\\{escape}{hex}` is not a Unicode scalar"))
    };
    match escape {
        'b' => Ok('\u{8}'),
        't' => Ok('\t'),
        'n' => Ok('\n'),
        'f' => Ok('\u{c}'),
        'r' => Ok('\r'),
        '"' => Ok('"'),
        '\\' => Ok('\\'),
        'u' => hex_escape(chars, 4),
        'U' => hex_escape(chars, 8),
        other => Err(format!("unknown escape `\\{other}` in a basic string")),
    }
}

/// After a value or table header, only whitespace or a comment may
/// remain on the line.
fn require_only_trailing_comment(rest: &str) -> Result<(), String> {
    let rest = rest.trim_start();
    if rest.is_empty() || rest.starts_with('#') {
        Ok(())
    } else {
        Err(format!("unexpected trailing content {rest:?}"))
    }
}

// ---------------------------------------------------------------------------
// Rendering: a serde `Serializer` that collects the value into a
// one-or-two-level node tree (fields in declaration order), then a
// writer that emits the `toml`-crate-compatible shape.
// ---------------------------------------------------------------------------

/// A rendered value: a ready-to-emit scalar, or a table of them.
enum Node {
    Scalar(String),
    Table(Vec<(String, Node)>),
}

/// Serializes scalars and structs into [`Node`]s; everything the
/// dialect cannot express is an error.
struct NodeSerializer;

impl NodeSerializer {
    fn scalar(text: String) -> Result<Node, ser::Error> {
        Ok(Node::Scalar(text))
    }

    fn unsupported(what: &str) -> ser::Error {
        ser::Error(format!("{what} cannot be rendered in the manifest dialect"))
    }
}

impl Serializer for NodeSerializer {
    type Ok = Node;
    type Error = ser::Error;
    type SerializeSeq = Impossible<Node, ser::Error>;
    type SerializeTuple = Impossible<Node, ser::Error>;
    type SerializeTupleStruct = Impossible<Node, ser::Error>;
    type SerializeTupleVariant = Impossible<Node, ser::Error>;
    type SerializeMap = Impossible<Node, ser::Error>;
    type SerializeStruct = TableCollector;
    type SerializeStructVariant = Impossible<Node, ser::Error>;

    fn serialize_str(self, v: &str) -> Result<Node, ser::Error> {
        Self::scalar(render_string(v))
    }

    fn serialize_char(self, v: char) -> Result<Node, ser::Error> {
        Self::scalar(render_string(&v.to_string()))
    }

    fn serialize_i8(self, v: i8) -> Result<Node, ser::Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<Node, ser::Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<Node, ser::Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i64(self, v: i64) -> Result<Node, ser::Error> {
        Self::scalar(v.to_string())
    }

    fn serialize_u8(self, v: u8) -> Result<Node, ser::Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<Node, ser::Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<Node, ser::Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u64(self, v: u64) -> Result<Node, ser::Error> {
        i64::try_from(v)
            .map_err(|_| ser::Error(format!("integer {v} does not fit in TOML's 64-bit range")))?;
        Self::scalar(v.to_string())
    }

    fn serialize_bool(self, _: bool) -> Result<Node, ser::Error> {
        Err(Self::unsupported("booleans"))
    }

    fn serialize_f32(self, _: f32) -> Result<Node, ser::Error> {
        Err(Self::unsupported("floats"))
    }

    fn serialize_f64(self, _: f64) -> Result<Node, ser::Error> {
        Err(Self::unsupported("floats"))
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Node, ser::Error> {
        Err(Self::unsupported("byte arrays"))
    }

    fn serialize_none(self) -> Result<Node, ser::Error> {
        Err(Self::unsupported(
            "`None` (mark the field `skip_serializing_if = \"Option::is_none\"`)",
        ))
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Node, ser::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Node, ser::Error> {
        Err(Self::unsupported("unit values"))
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Node, ser::Error> {
        Err(Self::unsupported("unit structs"))
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Node, ser::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Node, ser::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Node, ser::Error> {
        Err(Self::unsupported("enum variants with data"))
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, ser::Error> {
        Err(Self::unsupported("sequences"))
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, ser::Error> {
        Err(Self::unsupported("tuples"))
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, ser::Error> {
        Err(Self::unsupported("tuple structs"))
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, ser::Error> {
        Err(Self::unsupported("tuple variants"))
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, ser::Error> {
        Err(Self::unsupported("maps (use a struct)"))
    }

    fn serialize_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, ser::Error> {
        Ok(TableCollector {
            pairs: Vec::with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, ser::Error> {
        Err(Self::unsupported("struct variants"))
    }
}

/// Collects one struct's fields, in declaration order, into a
/// [`Node::Table`].
struct TableCollector {
    pairs: Vec<(String, Node)>,
}

impl SerializeStruct for TableCollector {
    type Ok = Node;
    type Error = ser::Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), ser::Error> {
        self.pairs
            .push((key.to_string(), value.serialize(NodeSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Node, ser::Error> {
        Ok(Node::Table(self.pairs))
    }
}

/// Emit the document: top-level scalars first (TOML requires them
/// before the first table header), then each table preceded by one
/// blank line — the shape `toml` 0.8.23 emitted.
fn render_document(pairs: &[(String, Node)]) -> Result<String, ser::Error> {
    let mut out = String::new();
    for (key, node) in pairs {
        if let Node::Scalar(text) = node {
            render_pair(&mut out, key, text)?;
        }
    }
    for (key, node) in pairs {
        let Node::Table(fields) = node else { continue };
        if !out.is_empty() {
            out.push('\n');
        }
        out.push('[');
        out.push_str(bare_key(key)?);
        out.push_str("]\n");
        for (field_key, field_node) in fields {
            match field_node {
                Node::Scalar(text) => render_pair(&mut out, field_key, text)?,
                Node::Table(_) => {
                    return Err(ser::Error(format!(
                        "table `{key}.{field_key}` nests below the first level, which the \
                         manifest dialect cannot express"
                    )));
                }
            }
        }
    }
    Ok(out)
}

fn render_pair(out: &mut String, key: &str, value: &str) -> Result<(), ser::Error> {
    out.push_str(bare_key(key)?);
    out.push_str(" = ");
    out.push_str(value);
    out.push('\n');
    Ok(())
}

/// Keys render bare; a key the dialect could not parse back is an
/// error rather than silently invalid output.
fn bare_key(key: &str) -> Result<&str, ser::Error> {
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    bare.then_some(key)
        .ok_or_else(|| ser::Error(format!("key {key:?} cannot be rendered as a bare TOML key")))
}

/// A basic string with the dialect's escapes. Where the `toml` crate
/// would switch to a literal string, this stays basic-with-escapes so
/// the output always re-parses under [`from_str`].
fn render_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    /// Manifest-shaped structs local to these tests: the module is
    /// generic over serde, so its tests do not reach into the real
    /// schema in `container::manifest`.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
    #[serde(deny_unknown_fields)]
    struct Artifact {
        #[serde(skip_serializing_if = "Option::is_none")]
        image: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        track: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        local: Option<std::path::PathBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        entrypoint: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pull: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        version: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        runtime: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validator: Option<Artifact>,
        #[serde(skip_serializing_if = "Option::is_none")]
        indexer: Option<Artifact>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wallet: Option<Artifact>,
    }

    /// Golden equivalence corpus. Each right-hand string is the exact
    /// output `toml = "0.8.23"` produced for the left-hand value,
    /// captured on 2026-07-11 immediately before the dependency was
    /// removed; a live A/B test at that commit asserted this module's
    /// renderer matched it byte for byte, and its parser read every
    /// fixture to the same value as the crate. These fixtures freeze
    /// that proof so it keeps running offline.
    fn golden_corpus() -> Vec<(Manifest, &'static str)> {
        vec![
            (
                Manifest {
                    version: 1,
                    runtime: None,
                    validator: None,
                    indexer: None,
                    wallet: None,
                },
                "version = 1\n",
            ),
            (
                Manifest {
                    version: 1,
                    runtime: Some("docker".into()),
                    validator: Some(Artifact {
                        image: Some("zfnd/zebra:v6.0.0@sha256:aaaa".into()),
                        track: Some("zfnd/zebra:latest".into()),
                        entrypoint: Some("zebrad".into()),
                        pull: Some("if-missing".into()),
                        ..Default::default()
                    }),
                    indexer: Some(Artifact {
                        local: Some("/builds/zainod".into()),
                        ..Default::default()
                    }),
                    wallet: Some(Artifact::default()),
                },
                "version = 1\n\
                 runtime = \"docker\"\n\
                 \n\
                 [validator]\n\
                 image = \"zfnd/zebra:v6.0.0@sha256:aaaa\"\n\
                 track = \"zfnd/zebra:latest\"\n\
                 entrypoint = \"zebrad\"\n\
                 pull = \"if-missing\"\n\
                 \n\
                 [indexer]\n\
                 local = \"/builds/zainod\"\n\
                 \n\
                 [wallet]\n",
            ),
            (
                Manifest {
                    version: 42,
                    runtime: Some("podman".into()),
                    validator: None,
                    indexer: None,
                    wallet: Some(Artifact {
                        pull: Some("never".into()),
                        ..Default::default()
                    }),
                },
                "version = 42\n\
                 runtime = \"podman\"\n\
                 \n\
                 [wallet]\n\
                 pull = \"never\"\n",
            ),
        ]
    }

    #[test]
    fn renders_the_toml_crates_exact_bytes() {
        for (manifest, golden) in golden_corpus() {
            assert_eq!(super::to_string(&manifest).unwrap(), golden, "{manifest:?}");
        }
    }

    #[test]
    fn parses_the_toml_crates_output_back_to_the_value() {
        for (manifest, golden) in golden_corpus() {
            let parsed: Manifest = super::from_str(golden).unwrap();
            assert_eq!(parsed, manifest, "{golden}");
        }
    }

    /// Dialect features the golden corpus does not exercise:
    /// indentation, trailing comments, `#` inside strings, and every
    /// escape form. The expected values were verified against
    /// `toml = "0.8.23"` in the same live A/B run as the goldens.
    #[test]
    fn parses_comments_indentation_and_escapes() {
        let manifest: Manifest = super::from_str(
            r#"
                version = 1 # trailing comment
                runtime = "a#not-comment"

                # whole-line comment
                [validator]
                entrypoint = "tab\there A A \U0001F980 quote\" back\\slash" # after string
            "#,
        )
        .unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.runtime.as_deref(), Some("a#not-comment"));
        assert_eq!(
            manifest.validator.unwrap().entrypoint.as_deref(),
            Some("tab\there A A \u{1F980} quote\" back\\slash"),
        );
    }

    #[test]
    fn parses_negative_integers() {
        #[derive(Deserialize)]
        struct Signed {
            offset: i64,
        }
        let signed: Signed = super::from_str("offset = -3\n").unwrap();
        assert_eq!(signed.offset, -3);
    }

    /// The one deliberate rendering divergence from the `toml` crate:
    /// a string needing escapes stays a basic string (the crate
    /// switched to a literal string, `'/builds/back\slash "quoted"'`,
    /// which this dialect cannot read back). Semantic equivalence is
    /// witnessed by the round trip.
    #[test]
    fn escape_needing_strings_render_basic_and_round_trip() {
        let manifest = Manifest {
            version: 1,
            runtime: Some("back\\slash \"quoted\" tab\t crab\u{1F980}".into()),
            validator: None,
            indexer: None,
            wallet: None,
        };
        let rendered = super::to_string(&manifest).unwrap();
        assert_eq!(
            rendered,
            "version = 1\nruntime = \"back\\\\slash \\\"quoted\\\" tab\\t crab\u{1F980}\"\n",
        );
        let round_tripped: Manifest = super::from_str(&rendered).unwrap();
        assert_eq!(round_tripped, manifest);
    }

    #[test]
    fn round_trips_the_whole_corpus() {
        for (manifest, _) in golden_corpus() {
            let round_tripped: Manifest =
                super::from_str(&super::to_string(&manifest).unwrap()).unwrap();
            assert_eq!(round_tripped, manifest);
        }
    }

    /// Every construct outside the dialect dies with an error naming
    /// the construct and its line.
    #[test]
    fn rejects_each_unsupported_construct_by_name_and_line() {
        let rejections = [
            ("version = 1\nlocal = 'literal'\n", "literal strings", 2),
            (
                "version = 1\ndesc = \"\"\"m\"\"\"\n",
                "multi-line strings",
                2,
            ),
            ("version = 1\nlist = [1, 2]\n", "arrays", 2),
            ("version = 1\ntbl = { a = 1 }\n", "inline tables", 2),
            ("version = 1\n[[wallet]]\n", "arrays of tables", 2),
            ("version = 1\nvalidator.image = \"z:1\"\n", "dotted keys", 2),
            ("version = 1\n\"quoted\" = 1\n", "quoted keys", 2),
            ("version = 1\nratio = 1.5\n", "floats", 2),
            ("version = 1\nflag = true\n", "booleans", 2),
            ("version = 1\nwhen = 1979-05-27\n", "date-times", 2),
            (
                "version = 1\nmask = 0xff\n",
                "hex, octal, and binary integers",
                2,
            ),
            (
                "version = 1\nbig = 1_000\n",
                "underscore-separated integers",
                2,
            ),
            ("version = 1\n[validator.env]\n", "nested tables", 2),
        ];
        for (text, construct, line) in rejections {
            let error = super::from_str::<Manifest>(text).unwrap_err().to_string();
            assert!(error.contains(construct), "{text:?} -> {error}");
            assert!(
                error.contains(&format!("line {line}")),
                "{text:?} -> {error}"
            );
        }
    }

    #[test]
    fn rejects_duplicates_and_malformed_lines() {
        let errors = [
            ("version = 1\nversion = 2\n", "more than once"),
            ("version = 1\n[wallet]\n[wallet]\n", "more than once"),
            ("version = 1\nrun = \"unterminated\n", "unterminated"),
            ("version = 1\nrun = \"bad \\q escape\"\n", "unknown escape"),
            ("version = 1\nrun = \"a\" junk\n", "trailing content"),
            ("version = 1\nnovalue =\n", "expected a value"),
            ("version = 1\nkeyonly\n", "expected `=`"),
        ];
        for (text, needle) in errors {
            let error = super::from_str::<Manifest>(text).unwrap_err().to_string();
            assert!(error.contains(needle), "{text:?} -> {error}");
        }
    }

    /// Schema-level failures surface serde's own prose (no line
    /// numbers — the document parsed; the shape was wrong).
    #[test]
    fn schema_errors_pass_through_serde() {
        let unknown = super::from_str::<Manifest>("version = 1\nbogus = 2\n")
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("unknown field `bogus`"), "{unknown}");
        let wrong_type = super::from_str::<Manifest>("version = \"1\"\n")
            .unwrap_err()
            .to_string();
        assert!(wrong_type.contains("invalid type"), "{wrong_type}");
    }

    /// Values the dialect cannot express fail to render rather than
    /// producing unparseable output.
    #[test]
    fn rendering_rejects_what_the_dialect_cannot_express() {
        #[derive(Serialize)]
        struct Flag {
            flag: bool,
        }
        let error = super::to_string(&Flag { flag: true })
            .unwrap_err()
            .to_string();
        assert!(error.contains("booleans"), "{error}");
    }
}
