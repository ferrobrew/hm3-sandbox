fn main() -> anyhow::Result<()> {
    use std::path::Path;

    use anyhow::Context;
    use re_utilities::{launcher, launcher::spawn};

    let executable_path_builder = |p: &Path| p.join("Retail").join("HITMAN3.exe");
    let process_name = launcher::get_executable_name_from_builder(executable_path_builder)
        .context("failed to get executable filename")?;

    launcher::launch_and_inject(
        &process_name,
        || {
            if let Ok(process) = spawn::steam_process(1659040, executable_path_builder) {
                return Ok(process);
            }
            if let Ok(process) = spawn::egs_process("Eider", executable_path_builder) {
                return Ok(process);
            }

            Err(anyhow::anyhow!("Hitman 3 does not appear to be installed."))
        },
        "payload.dll",
        true,
    )
    .map(|_| ())
}
