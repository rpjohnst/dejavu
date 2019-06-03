use std::fmt::Write;
use gml::vm;

#[derive(Default)]
pub struct State;

#[gml::bind(Api)]
impl State {
    #[gml::function]
    pub fn show_debug_message(&mut self, arguments: &[vm::Value]) {
        let mut message = String::default();
        for argument in arguments {
            let _ = write!(&mut message, "{:?} ", argument);
        }
        println!("{}", message);
    }
}
