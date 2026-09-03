fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut resources = winres::WindowsResource::new();
        resources.set_icon("assets/icon.ico");
        resources
            .compile()
            .expect("failed to compile the Goatpad Windows resources");
    }
}
