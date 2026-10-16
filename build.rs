use std::{env, fs, path::PathBuf};

fn main() {
    embed_resource::compile("assets/icons/key2gksrmf.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();

    // exe 파일 속성(자세히)·작업 관리자에 표시되는 버전 정보. Cargo.toml version에서 생성한다.
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers: Vec<u32> = version
        .split(['.', '-', '+'])
        .take(3)
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    let (major, minor, patch) = (numbers[0], numbers[1], numbers[2]);
    let rc = format!(
        r#"1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "GMTL"
            VALUE "FileDescription", "key2gksrmf"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "key2gksrmf"
            VALUE "OriginalFilename", "key2gksrmf.exe"
            VALUE "ProductName", "key2gksrmf"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    );
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("key2gksrmf_version.rc");
    fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_optional()
        .unwrap();

    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=assets");
}
