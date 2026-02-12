//! CLI argument parsing for ToastWindow.
//!
//! Modes: --save, --notify, --input, --notify-show, --cleanup
//! Flags: --debug/-d, --input-mode, --session <val>, --message <val>

#[derive(Debug, PartialEq)]
pub enum Mode {
    Save,
    Notify,
    Input,
    NotifyShow,
    Cleanup,
    None,
}

#[derive(Debug)]
pub struct Args {
    pub mode: Mode,
    pub debug: bool,
    pub input_mode: bool,
    pub session: String,
    pub message: String,
}

pub fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Args {
        mode: Mode::None,
        debug: false,
        input_mode: false,
        session: String::new(),
        message: String::new(),
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--save" => result.mode = Mode::Save,
            "--notify" => result.mode = Mode::Notify,
            "--input" => result.mode = Mode::Input,
            "--notify-show" => result.mode = Mode::NotifyShow,
            "--cleanup" => result.mode = Mode::Cleanup,
            "--debug" | "-d" => result.debug = true,
            "--input-mode" => result.input_mode = true,
            "--session" => {
                i += 1;
                if i < args.len() {
                    result.session = args[i].clone();
                }
            }
            "--message" => {
                i += 1;
                if i < args.len() {
                    result.message = args[i].clone();
                }
            }
            _ => {}
        }
        i += 1;
    }

    result
}
