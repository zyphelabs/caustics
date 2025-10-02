// Include generated code with composite registry
include!(concat!(env!("OUT_DIR"), "/caustics_client.rs"));

pub mod query_builders;
pub mod types;

pub use query_builders::*;
pub use types::*;

pub mod hooks {
    use std::sync::{Arc, RwLock};

    #[derive(Clone, Debug)]
    pub struct QueryEvent {
        pub builder: &'static str,
        pub entity: &'static str,
        pub details: Option<String>,
    }

    #[derive(Clone, Debug)]
    pub struct QueryResultMeta {
        pub row_count: Option<usize>,
        pub error: Option<String>,
        pub elapsed_ms: Option<u128>,
    }

    pub trait QueryHook: Send + Sync {
        fn before(&self, _event: &QueryEvent) {}
        fn after(&self, _event: &QueryEvent, _meta: &QueryResultMeta) {}
    }

    static QUERY_HOOKS: RwLock<Vec<Arc<dyn QueryHook>>> = RwLock::new(Vec::new());
    thread_local! { static TX_HOOKS: std::cell::RefCell<Vec<Arc<dyn QueryHook>>> = std::cell::RefCell::new(Vec::new()); }
    thread_local! { static TX_CORR_ID: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) }; }
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    // Global hooks API
    pub fn set_query_hook(hook: Option<Arc<dyn QueryHook>>) {
        if let Ok(mut guard) = QUERY_HOOKS.write() {
            guard.clear();
            if let Some(h) = hook {
                guard.push(h);
            }
        }
    }
    pub fn set_query_hooks(hooks: Vec<Arc<dyn QueryHook>>) {
        if let Ok(mut guard) = QUERY_HOOKS.write() {
            *guard = hooks;
        }
    }
    pub fn add_query_hook(hook: Arc<dyn QueryHook>) {
        if let Ok(mut guard) = QUERY_HOOKS.write() {
            guard.push(hook);
        }
    }
    pub fn clear_query_hooks() {
        if let Ok(mut guard) = QUERY_HOOKS.write() {
            guard.clear();
        }
    }

    // Thread/transaction hooks API
    pub fn set_thread_hook(hook: Option<Arc<dyn QueryHook>>) {
        TX_HOOKS.with(|cell| {
            let mut v = cell.borrow_mut();
            v.clear();
            if let Some(h) = hook {
                v.push(h);
            }
        });
    }
    pub fn set_thread_hooks(hooks: Vec<Arc<dyn QueryHook>>) {
        TX_HOOKS.with(|cell| *cell.borrow_mut() = hooks);
    }
    pub fn add_thread_hook(hook: Arc<dyn QueryHook>) {
        TX_HOOKS.with(|cell| cell.borrow_mut().push(hook));
    }
    pub fn clear_thread_hooks() {
        TX_HOOKS.with(|cell| cell.borrow_mut().clear());
    }

    pub fn set_thread_correlation_id(id: Option<String>) {
        TX_CORR_ID.with(|cell| *cell.borrow_mut() = id);
    }

    pub fn set_new_correlation_id() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let id = format!("{:x}-{}", now, c);
        set_thread_correlation_id(Some(id.clone()));
        id
    }

    pub fn current_correlation_detail() -> Option<String> {
        let mut out: Option<String> = None;
        TX_CORR_ID.with(|cell| {
            if let Some(id) = cell.borrow().as_ref() {
                out = Some(format!("corr_id={}", id));
            }
        });
        out
    }

    pub fn compose_details(op: &str, entity: &str) -> Option<String> {
        match current_correlation_detail() {
            Some(c) => Some(format!("{} op={} entity={}", c, op, entity)),
            None => Some(format!("op={} entity={}", op, entity)),
        }
    }

    fn iter_hooks<F: Fn(&Arc<dyn QueryHook>)>(f: F) {
        // Transaction hooks first (FIFO), then global hooks (FIFO)
        TX_HOOKS.with(|cell| {
            for h in cell.borrow().iter() {
                f(h);
            }
        });
        if let Ok(guard) = QUERY_HOOKS.read() {
            for h in guard.iter() {
                f(h);
            }
        }
    }

    pub fn emit_before(event: &QueryEvent) {
        iter_hooks(|h| h.before(event));
    }

    pub fn emit_after(event: &QueryEvent, meta: &QueryResultMeta) {
        iter_hooks(|h| h.after(event, meta));
    }
}

pub mod raw {
    use sea_orm::DatabaseBackend;
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
            if i > 0 {
                placeholders.push_str(", ");
            }
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
        fn from(v: Inline) -> Self {
            RawArg::Inline(v.0)
        }
    }

    // Blanket conversions: any type that SeaORM can turn into a Value becomes a bound parameter
    impl<T> From<T> for RawArg
    where
        Value: From<T>,
    {
        fn from(v: T) -> Self {
            RawArg::Bind(Value::from(v))
        }
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
                    panic!(
                        "raw!: placeholders ({}) more than args ({})",
                        count_braces(fmt),
                        args.len()
                    );
                }
                match &args[arg_idx] {
                    RawArg::Bind(v) => {
                        sql.push('?');
                        params.push(v.clone());
                    }
                    RawArg::Inline(s) => {
                        sql.push_str(s);
                    }
                }
                arg_idx += 1;
                i += 2;
            } else {
                sql.push(bytes[i] as char);
                i += 1;
            }
        }
        if arg_idx != args.len() {
            panic!(
                "raw!: placeholders ({}) fewer than args ({})",
                count_braces(fmt),
                args.len()
            );
        }
        (sql, params)
    }

    fn count_braces(fmt: &str) -> usize {
        let b = fmt.as_bytes();
        let mut i = 0usize;
        let mut n = 0usize;
        while i + 1 < b.len() {
            if b[i] == b'{' && b[i + 1] == b'}' {
                n += 1;
                i += 2;
            } else {
                i += 1;
            }
        }
        n
    }

    // Backend-aware ANY/IN helper: on Postgres emit ANY(ARRAY[?,..]), otherwise IN (?,..)
    pub fn any_or_in_params<T>(backend: DatabaseBackend, items: &[T]) -> (String, Vec<Value>)
    where
        T: Clone + Into<Value>,
    {
        if items.is_empty() {
            return ("IN (NULL)".to_string(), Vec::new());
        }
        let mut params = Vec::with_capacity(items.len());
        for it in items.iter().cloned() {
            params.push(it.into());
        }
        match backend {
            DatabaseBackend::Postgres => {
                let mut inner = String::new();
                for i in 0..items.len() {
                    if i > 0 {
                        inner.push_str(", ");
                    }
                    inner.push('?');
                }
                (format!("ANY(ARRAY[{}])", inner), params)
            }
            _ => {
                let mut inner = String::new();
                for i in 0..items.len() {
                    if i > 0 {
                        inner.push_str(", ");
                    }
                    inner.push('?');
                }
                (format!("IN ({})", inner), params)
            }
        }
    }
}

// Re-export DeferredLookup for use in macros
pub use query_builders::DeferredLookup;

// Re-export traits for use in generated code
pub use types::ApplyNestedIncludes;
pub use types::{EntityFetcher, EntityRegistry};

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

#[macro_export]
macro_rules! any_params {
    ($backend:expr, $slice:expr) => {{
        $crate::raw::any_or_in_params($backend, $slice)
    }};
}
