fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/icons/quickfolder.ico");
        winresource::WindowsResource::new()
            .set_icon("assets/icons/quickfolder.ico")
            .compile()
            .expect("failed to compile Windows application resources");
    }
}
