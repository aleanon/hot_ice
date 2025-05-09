use ui::Names;



fn main() {
    hot_ice::hot_application("target/debug/ui", Names::new, Names::update, Names::view)
        .theme(Names::theme)
        .run()
        .unwrap();
}