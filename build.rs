fn main() {
    // Bundle the gettext `.po` translations under `lang/` so the UI's `@tr(...)`
    // strings can switch language at runtime via slint::select_bundled_translation.
    // Source language is Chinese (the msgids); `lang/<lc>/LC_MESSAGES/cloudshell.po`
    // provides other locales.  No per-component context, so msgids are the raw
    // Chinese strings.
    println!("cargo:rerun-if-changed=lang");
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent".into())
            .with_bundled_translations("lang")
            .with_default_translation_context(slint_build::DefaultTranslationContext::None),
    )
    .expect("Slint build failed");

    // Embed the application icon into the Windows executable so it shows up in
    // Explorer, the taskbar and shortcuts. No-op on non-Windows targets.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/cloudshell.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/cloudshell.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed Windows icon: {e}");
        }
    }
}
