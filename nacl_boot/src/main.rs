#![feature(exit_status_error, iter_advance_by)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, io};

use anyhow::Context;

const RUN_ARGS: &[&str] = &["--no-reboot", "-s"];

const REGEX: &str = "\"executable\":\".+?\"";
const REGEX_HDR: &str = "\"executable\":\"";

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1); // skip executable name
    let cargo = env::var("CARGO")?;
    let output = Command::new(cargo)
        .arg("build")
        .arg("--release")
        .arg("--message-format=json")
        .current_dir(Path::new("./nacl").canonicalize()?)
        .output()?;
    output.status.exit_ok()?;
    let re = regex::Regex::new(REGEX)?;
    let output = String::from_utf8_lossy(&output.stdout);
    let mut match_ = re.find(&output).context("no executable generated")?.range();
    match_
        .advance_by(REGEX_HDR.len())
        .map_err(|_| anyhow::anyhow!("regex match"))?;
    match_
        .advance_back_by(1)
        .map_err(|_| anyhow::anyhow!("regex match"))?;
    let kernel_binary_path = Path::new(&output[match_]);
    eprintln!("kernel_binary: {kernel_binary_path:?}");

    let no_boot = if let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-run" => true,
            other => panic!("unexpected argument `{}`", other),
        }
    } else {
        false
    };

    let bios = create_disk_images(kernel_binary_path)?;
    Command::new("./limine/limine-install")
        .arg(&bios)
        .status()?
        .exit_ok()?;

    if no_boot {
        println!("Created disk image at `{}`", bios.display());
        return Ok(());
    }

    let mut run_cmd = Command::new("qemu-system-x86_64");
    run_cmd
        .arg("-drive")
        .arg(format!("format=raw,file={}", bios.display()))
        .arg("-serial")
        .arg("stdio")
        .arg("-smp")
        .arg("4")
        .arg("-m")
        .arg("1G");
    run_cmd.args(RUN_ARGS);
    run_cmd
        .status()
        .context("running qemu")?
        .exit_ok()
        .context("qemu exit status")?;

    Ok(())
}

pub fn create_disk_images(kernel_binary_path: &Path) -> anyhow::Result<PathBuf> {
    fs::create_dir_all("./iso_root/")?;
    for file in [
        "./limine/limine-eltorito-efi.bin",
        "./limine/limine-cd.bin",
        "./limine/limine.sys",
        "./limine.cfg",
    ]
    .iter()
    .map(Path::new)
    .chain([kernel_binary_path])
    .map(Path::canonicalize)
    {
        let file = file?;
        io::copy(
            &mut File::open(&file)?,
            &mut File::create(format!(
                "./iso_root/{}",
                file.file_name().unwrap().to_str().unwrap()
            ))?,
        )?;
    }
    let image_path = kernel_binary_path.with_extension("iso");
    Command::new("xorriso")
        .arg("-as")
        .arg("mkisofs")
        .arg("-b")
        .arg("limine-cd.bin")
        .arg("-no-emul-boot")
        .arg("-boot-load-size")
        .arg("4")
        .arg("-boot-info-table")
        .arg("--efi-boot")
        .arg("limine-eltorito-efi.bin")
        .arg("-efi-boot-part")
        .arg("--efi-boot-image")
        .arg("--protective-msdos-label")
        .arg("-o")
        .arg(&image_path)
        .arg("./iso_root")
        .status()?
        .exit_ok()?;
    Ok(image_path)
}
