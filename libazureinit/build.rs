fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Pass in build-time environment variables, which could be used in
    // crates by `env!` macros.
    println!("cargo:rustc-env=PATH_HOSTNAMECTL=hostnamectl");
    println!("cargo:rustc-env=PATH_USERADD=useradd");
    // The list of supplementary groups to add a provisioned user to.
    println!("cargo:rustc-env=USERADD_GROUPS=adm,audio,cdrom,dialout,dip,floppy,lxd,netdev,plugdev,sudo,video");
    println!("cargo:rustc-env=PATH_PASSWD=passwd");
}
