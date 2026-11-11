//! Compile les ressources Windows (icône, VERSIONINFO, manifeste) avec windres.
//!
//! La compilation n'échoue pas si windres est absent : un avertissement est
//! affiché et l'exécutable reste fonctionnel (sans métadonnées ni icône
//! embarquée). Le script build-installer.ps1 vérifie la présence de windres.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn find_windres() -> Option<PathBuf> {
    if let Some(path) = env::var_os("WINDRES") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let names = ["windres.exe", "windres", "x86_64-w64-mingw32-windres.exe"];
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path) {
            for name in names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    for dir in [
        r"C:\msys64\mingw64\bin",
        r"C:\mingw64\bin",
        r"C:\Program Files\mingw64\bin",
    ] {
        for name in names {
            let candidate = PathBuf::from(dir).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=resources/arxfund.rc");
    println!("cargo:rerun-if-changed=resources/arxfund.manifest");
    println!("cargo:rerun-if-changed=assets/arxfund.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "gnu" {
        println!(
            "cargo:warning=Ressources Windows non compilées (cible {target_os}/{target_env})."
        );
        return;
    }

    let Some(windres) = find_windres() else {
        println!(
            "cargo:warning=windres introuvable : ARXFUND.exe sera construit sans icône ni VERSIONINFO. Ajoutez C:\\msys64\\mingw64\\bin au PATH."
        );
        return;
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let obj = out_dir.join("arxfund_resources.o");
    let mut search_path = vec![windres.parent().expect("windres directory").to_path_buf()];
    if let Some(path) = env::var_os("PATH") {
        search_path.extend(env::split_paths(&path));
    }
    let status = Command::new(&windres)
        .env(
            "PATH",
            env::join_paths(search_path).expect("tool search path"),
        )
        .arg("-I")
        .arg("assets")
        .arg("-I")
        .arg("resources")
        .arg("--input")
        .arg("resources/arxfund.rc")
        .arg("--output")
        .arg(&obj)
        .status();

    match status {
        Ok(status) if status.success() && obj.is_file() => {
            println!("cargo:rustc-link-arg={}", obj.display());
        }
        Ok(status) => {
            println!(
                "cargo:warning=windres a échoué (code {:?}) : métadonnées Windows ignorées.",
                status.code()
            );
        }
        Err(error) => {
            println!("cargo:warning=Impossible d'exécuter windres : {error}");
        }
    }
}
