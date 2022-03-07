use anyhow::Result;

use re_utilities::module::Module;

pub struct HookLibrary {
    pub enable: fn(&mut Module) -> Result<()>,
    pub disable: fn() -> Result<()>,
}
