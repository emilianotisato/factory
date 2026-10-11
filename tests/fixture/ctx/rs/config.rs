use std::path::PathBuf;

/// Where the tool reads its settings.
pub struct Config {
    pub root: PathBuf,
    pub verbose: bool,
}

impl Config {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            verbose: false,
        }
    }

    pub fn loud(mut self) -> Self {
        self.verbose = true;
        self
    }
}

pub trait Load {
    fn load(&self) -> Config;
}

macro_rules! make_fn {
    ($name:ident) => {
        pub fn $name() -> u32 {
            7
        }
    };
}

make_fn!(made_by_macro);
