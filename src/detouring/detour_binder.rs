use anyhow::Result;

use super::prelude::Module;

pub struct DetourBinder {
    pub bind: &'static (dyn Send + Sync + Fn(&mut Module) -> Result<()>),
    pub enable: &'static (dyn Send + Sync + Fn() -> Result<()>),
    pub disable: &'static (dyn Send + Sync + Fn() -> Result<()>),
}

impl DetourBinder {
    pub fn bind(&self, module: &mut Module) -> Result<()> {
        (self.bind)(module)
    }
    pub fn enable(&self) -> Result<()> {
        (self.enable)()
    }
    pub fn disable(&self) -> Result<()> {
        (self.disable)()
    }
}
