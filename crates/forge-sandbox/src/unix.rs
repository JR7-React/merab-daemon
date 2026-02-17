pub struct JobObject;

impl JobObject {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn assign_process(&self, _pid: u32) -> anyhow::Result<()> {
        Ok(())
    }
}
