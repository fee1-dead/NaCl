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
        .arg("--target")
        .arg("x86_64-unknown-none")
        .arg("--message-format=json")
        .env("RUSTFLAGS", "-C relocation-model=static")
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

    create_disk_images(kernel_binary_path)?;
    println!("bios created");
    /*Command::new("/usr/bin/env")
        .arg("sh")
        .arg("./limine/install-sh")
        .arg(&bios)
        .status()?
        .exit_ok()?;*/

    if no_boot {
        // println!("Created disk image at `{}`", bios.display());
        return Ok(());
    }

    let mut run_cmd = Command::new("qemu-system-x86_64-uefi");
    run_cmd
        .arg("-enable-kvm")
        .arg("-drive")
        .arg(format!("format=raw,file=fat:rw:iso_root"))
        .arg("-serial")
        .arg("stdio")
        // .arg("-smp")
        // .arg("4")
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

pub fn create_disk_images(kernel_binary_path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all("./iso_root/")?;
    for (file, dest) in [
        ("./limine/BOOTX64.EFI", "EFI/BOOT"),
        ("./limine/limine-uefi-cd.bin", ""),
        ("./limine/limine-bios-cd.bin", ""),
        ("./limine/limine-bios.sys", ""),
        ("./limine.conf", ""),
    ]
    .iter()
    .copied()
    .map(|(x, y)| (Path::new(x), y))
    .chain([(kernel_binary_path, "")])
    .map(|(x, y)| (x.canonicalize(), y))
    {
        let file = file?;
        let dest = if dest.is_empty() {
            PathBuf::from(format!(
                "./iso_root/{}",
                file.file_name().unwrap().to_str().unwrap()
            )).to_owned()
        } else {
            Path::new("./iso_root").join(dest).join(file.file_name().unwrap().to_str().unwrap())
        };
        fs::create_dir_all(dest.parent().unwrap())?;
        io::copy(
            &mut File::open(&file)?,
            &mut File::create(dest)?,
        )?;
    }
    /*let image_path = kernel_binary_path.with_extension("iso");
    Command::new("xorriso")
        .arg("-as")
        .arg("mkisofs")
        .arg("-b")
        .arg("limine-bios-cd.bin")
        .arg("-no-emul-boot")
        .arg("-boot-load-size")
        .arg("4")
        .arg("-boot-info-table")
        .arg("--efi-boot")
        .arg("limine-uefi-cd.bin")
        .arg("-efi-boot-part")
        .arg("--efi-boot-image")
        .arg("--protective-msdos-label")
        .arg("-o")
        .arg(&image_path)
        .arg("./iso_root")
        .status()?
        .exit_ok()?;
    Ok(image_path)*/
    Ok(())
}
