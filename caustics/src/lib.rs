// Include generated code with composite registry
include!(concat!(env!("OUT_DIR"), "/caustics_client.rs"));

pub mod query_builders;
pub mod types;

pub use query_builders::*;
pub use types::*;

pub mod raw {
    use sea_orm::Value;

    pub fn ident(name: &str) -> String {
        let escaped = name.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    }

    pub fn in_list_params<T>(items: &[T]) -> (String, Vec<Value>)
    where
        T: Clone + Into<Value>,
    {
        if items.is_empty() {
            return (String::from("NULL"), Vec::new());
        }
        let mut placeholders = String::new();
        let mut params = Vec::with_capacity(items.len());
        for (i, it) in items.iter().cloned().enumerate() {
            if i > 0 { placeholders.push_str(", "); }
            placeholders.push('?');
            params.push(it.into());
        }
        (placeholders, params)
    }

    pub fn bind_param<T>(value: T) -> (String, Vec<Value>)
    where
        T: Into<Value>,
    {
        ("?".to_string(), vec![value.into()])
    }

    // Inline/newtype to mark identifier or raw SQL to be inlined, not bound
    pub struct Inline(pub String);

    pub enum RawArg {
        Bind(Value),
        Inline(String),
    }

    impl From<Inline> for RawArg {
        fn from(v: Inline) -> Self { RawArg::Inline(v.0) }
    }

    // Blanket conversions: any type that SeaORM can turn into a Value becomes a bound parameter
    impl<T> From<T> for RawArg where Value: From<T> {
        fn from(v: T) -> Self { RawArg::Bind(Value::from(v)) }
    }
    // Note: Option<T> is covered when `Value: From<Option<T>>` via the blanket impl above.

    pub fn finalize_sql_with_args(fmt: &str, args: Vec<RawArg>) -> (String, Vec<Value>) {
        // Walk fmt, replacing each {} with either inline text or ? + push param
        let mut sql = String::with_capacity(fmt.len() + args.len() * 2);
        let mut params: Vec<Value> = Vec::new();
        let mut arg_idx = 0usize;
        let bytes = fmt.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'}' {
                if arg_idx >= args.len() {
                    panic!("raw!: placeholders ({}) more than args ({})", count_braces(fmt), args.len());
                }
                match &args[arg_idx] {
                    RawArg::Bind(v) => { sql.push('?'); params.push(v.clone()); }
                    RawArg::Inline(s) => { sql.push_str(s); }
                }
                arg_idx += 1;
                i += 2;
            } else {
                sql.push(bytes[i] as char);
                i += 1;
            }
        }
        if arg_idx != args.len() {
            panic!("raw!: placeholders ({}) fewer than args ({})", count_braces(fmt), args.len());
        }
        (sql, params)
    }

    fn count_braces(fmt: &str) -> usize {
        let b = fmt.as_bytes();
        let mut i = 0usize; let mut n = 0usize;
        while i + 1 < b.len() { if b[i] == b'{' && b[i+1] == b'}' { n += 1; i += 2; } else { i += 1; } }
        n
    }
}

// Re-export DeferredLookup for use in macros
pub use query_builders::DeferredLookup;

// Re-export traits for use in generated code
pub use types::{EntityFetcher, EntityRegistry};
pub use types::ApplyNestedIncludes as ApplyNestedIncludes;

// Legacy Select! and select_typed! macros removed; use per-entity `entity::select!(...)` and builder `.select(...)`.

// Global typed selection macro that returns a SelectionSpec marker
// Global select_typed! macro no longer exposed

// ===== Raw SQL macros =====
#[macro_export]
macro_rules! __caustics_count_args {
    () => { 0usize };
    ($head:expr $(, $tail:expr)*) => { 1usize + $crate::__caustics_count_args!($($tail),*) };
}

#[macro_export]
macro_rules! raw {
    ($fmt:literal $(, $arg:expr )* $(,)?) => {{
        let mut __args: ::std::vec::Vec<$crate::raw::RawArg> = ::std::vec![];
        $( __args.push(($arg).into()); )*
        let (__sql, __params) = $crate::raw::finalize_sql_with_args($fmt, __args);
        $crate::Raw::new(__sql, __params)
    }};
}

#[macro_export]
macro_rules! ident {
    ($name:expr) => {{
        $crate::raw::Inline($crate::raw::ident($name))
    }};
}

#[macro_export]
macro_rules! in_params {
    ($slice:expr) => {{
        $crate::raw::in_list_params($slice)
    }};
}
