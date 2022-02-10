use super::prelude::Module;
use anyhow::Result;

pub struct HookLibrary {
    pub enable: fn(&mut Module) -> Result<()>,
    pub disable: fn() -> Result<()>,
}
