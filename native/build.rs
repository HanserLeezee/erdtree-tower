use std::{fs,path::Path};
fn windows_version_resource()->String {
    println!("cargo:rerun-if-env-changed=TOWER_PRODUCT_VERSION");
    println!("cargo:rerun-if-env-changed=TOWER_RC");
    println!("cargo:rerun-if-changed=../ProductInfo.props");
    let props=fs::read_to_string("../ProductInfo.props").expect("ProductInfo.props missing");
    let fallback=props.lines().find(|line|line.contains("<TowerProductVersion Condition=\"'$(TowerProductVersion)' == ''\">"))
        .and_then(|line|line.split_once('>')).and_then(|(_,tail)|tail.split_once('<')).map(|(value,_)|value).expect("Product version missing");
    let version=std::env::var("TOWER_PRODUCT_VERSION").unwrap_or_else(|_|fallback.to_owned());
    let parts:Vec<_>=version.split('.').map(|n|n.parse::<u16>().expect("Version must contain numeric components <= 65535")).collect();
    assert_eq!(parts.len(),3,"Product version must be major.minor.patch");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref()!=Ok("windows") {return version;}
    assert_eq!(std::env::var("CARGO_CFG_TARGET_ENV").as_deref(),Ok("msvc"),"Windows release metadata requires the MSVC toolchain");
    let rc=std::env::var_os("TOWER_RC").map(std::path::PathBuf::from).unwrap_or_else(||{
        let kits=std::path::PathBuf::from(std::env::var_os("ProgramFiles(x86)").expect("Windows SDK path unavailable")).join("Windows Kits/10/bin");
        let mut candidates:Vec<_>=fs::read_dir(kits).expect("Install Windows SDK or set TOWER_RC").flatten().map(|entry|entry.path().join("x64/rc.exe")).filter(|path|path.is_file()).collect();
        candidates.sort();candidates.pop().expect("rc.exe unavailable; install Windows SDK or set TOWER_RC")
    });
    let out=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let resource=out.join("tower-version.rc");let compiled=out.join("tower-version.res");
    fs::write(&resource,format!(r#"1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0
FILEOS 0x40004L
FILETYPE 0x2L
FILESUBTYPE 0
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904B0"
  BEGIN
   VALUE "CompanyName", "TamikiP\0"
   VALUE "FileDescription", "Erdtree Tower game runtime\0"
   VALUE "FileVersion", "{version}.0\0"
   VALUE "InternalName", "erdtree_tower_preview\0"
   VALUE "OriginalFilename", "erdtree_tower_preview.dll\0"
   VALUE "ProductName", "Erdtree Tower\0"
   VALUE "ProductVersion", "{version}\0"
   VALUE "LegalCopyright", "Copyright (C) 2026 TamikiP. GPL-3.0-or-later.\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0409, 1200
 END
END
"#,major=parts[0],minor=parts[1],patch=parts[2])).unwrap();
    assert!(std::process::Command::new(rc).arg("/nologo").arg("/fo").arg(&compiled).arg(&resource).status().expect("Run rc.exe").success(),"Version resource compilation failed");
    println!("cargo:rustc-link-arg-cdylib={}",compiled.display());
    version
}
fn hash_tree(path:&Path,hash:&mut u64) {
    let mut entries:Vec<_>=fs::read_dir(path).unwrap().map(|e|e.unwrap().path()).collect();entries.sort();
    for entry in entries {
        if entry.is_dir(){hash_tree(&entry,hash);}else if matches!(entry.extension().and_then(|s|s.to_str()),Some("rs"|"cs"|"toml")) {
            println!("cargo:rerun-if-changed={}",entry.display());
            for byte in fs::read(entry).unwrap(){*hash=(*hash^u64::from(byte)).wrapping_mul(0x100000001b3);}
        }
    }
}
fn main() {
    let version=windows_version_resource();
    let mut hash=0xcbf29ce484222325;
    for byte in version.bytes(){hash=(hash^u64::from(byte)).wrapping_mul(0x100000001b3);}
    hash_tree(Path::new("src"),&mut hash);hash_tree(Path::new("../src"),&mut hash);
    hash_tree(Path::new("vendor/hudhook/src"),&mut hash);
    hash_tree(Path::new("vendor/fromsoftware-rs/crates"),&mut hash);
    for file in ["build.rs","Cargo.toml","Cargo.lock"] {
        println!("cargo:rerun-if-changed={file}");
        for byte in fs::read(file).unwrap(){hash=(hash^u64::from(byte)).wrapping_mul(0x100000001b3);}
    }
    println!("cargo:rustc-env=TOWER_BUILD_ID={hash:016x}");
}
