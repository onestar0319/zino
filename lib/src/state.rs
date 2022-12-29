use crate::ConnectionPool;
use std::{env, fs, path::Path, sync::LazyLock};
use toml::value::{Table, Value};

/// Application scoped state.
#[derive(Debug, Clone)]
pub struct State {
    /// Environment.
    env: String,
    /// Configuration.
    config: Table,
    /// Connection pools.
    pools: Vec<ConnectionPool>,
}

impl State {
    /// Creates a new instance.
    #[inline]
    pub fn new(env: String) -> Self {
        Self {
            env,
            config: Table::new(),
            pools: Vec::new(),
        }
    }

    /// Returns a reference to the shared `State`.
    #[inline]
    pub fn shared() -> &'static Self {
        LazyLock::force(&SHARED_STATE)
    }

    /// Loads the config file according to the specific env.
    pub fn load_config(&mut self) {
        let current_dir = env::current_dir().unwrap();
        let project_dir = Path::new(&current_dir);
        let path = if project_dir.join("./config").exists() {
            project_dir.join(format!("./config/config.{}.toml", self.env))
        } else {
            project_dir.join(format!("../config/config.{}.toml", self.env))
        };
        let config: Value = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("fail to read config file `{:#?}`", &path))
            .parse()
            .expect("fail to parse toml value");
        match config {
            Value::Table(table) => self.config = table,
            _ => eprintln!("toml config file should be a table"),
        }
    }

    /// Sets the connection pools.
    #[inline]
    pub(crate) fn set_pools(&mut self, pools: Vec<ConnectionPool>) {
        self.pools = pools;
    }

    /// Returns the env.
    #[inline]
    pub fn env(&self) -> &str {
        self.env.as_str()
    }

    /// Returns the config.
    #[inline]
    pub fn config(&self) -> &Table {
        &self.config
    }

    /// Returns a connection pool with the specific name.
    #[inline]
    pub(crate) fn get_pool(&self, name: &str) -> Option<&ConnectionPool> {
        self.pools.iter().find(|c| c.name() == name)
    }
}

impl Default for State {
    #[inline]
    fn default() -> Self {
        SHARED_STATE.clone()
    }
}

/// Shared server state.
pub(crate) static SHARED_STATE: LazyLock<State> = LazyLock::new(|| {
    let mut app_env = "dev".to_string();
    for arg in env::args() {
        if arg.starts_with("--env=") {
            app_env = arg.strip_prefix("--env=").unwrap().to_string();
        }
    }
    let mut state = State::new(app_env);
    state.load_config();

    // Database connection pools.
    let mut pools = Vec::new();
    let databases = state
        .config()
        .get("postgres")
        .expect("the `postgres` field is missing")
        .as_array()
        .expect("the `postgres` field should be an array of tables");
    for database in databases {
        if database.is_table() {
            let postgres = database
                .as_table()
                .expect("the `postgres` field should be a table");
            match ConnectionPool::connect_lazy(postgres) {
                Ok(pool) => pools.push(pool),
                Err(err) => eprintln!("{err}"),
            }
        }
    }
    if !pools.is_empty() {
        state.set_pools(pools);
    }
    state
});

/// Database namespace prefix.
pub(crate) static NAMESPACE_PREFIX: LazyLock<&'static str> = LazyLock::new(|| {
    State::shared()
        .config()
        .get("database")
        .expect("the `database` field is missing")
        .as_table()
        .expect("the `database` field should be a table")
        .get("namespace")
        .expect("the `database.namespace` field is missing")
        .as_str()
        .expect("the `database.namespace` field should be a str")
});
