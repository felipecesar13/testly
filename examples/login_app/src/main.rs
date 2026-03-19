#![windows_subsystem = "windows"]

use native_windows_derive as nwd;
use native_windows_gui as nwg;

use nwd::NwgUi;
use nwg::NativeUi;

#[derive(Default, NwgUi)]
pub struct LoginApp {
    #[nwg_resource(family: "Segoe UI", size: 28, weight: 600)]
    hello_font: nwg::Font,

    #[nwg_control(size: (420, 260), position: (400, 250), title: "Login",
        flags: "WINDOW|VISIBLE")]
    #[nwg_events(OnWindowClose: [LoginApp::exit])]
    window: nwg::Window,

    // --- Tela de login ---

    #[nwg_control(text: "Nome:", size: (80, 25), position: (50, 40))]
    name_label: nwg::Label,

    #[nwg_control(text: "", size: (230, 25), position: (140, 38))]
    name_input: nwg::TextInput,

    #[nwg_control(text: "Senha:", size: (80, 25), position: (50, 85))]
    password_label: nwg::Label,

    #[nwg_control(text: "", size: (230, 25), position: (140, 83),
        password: Some('*'))]
    password_input: nwg::TextInput,

    #[nwg_control(text: "Avançar", size: (120, 35), position: (150, 135))]
    #[nwg_events(OnButtonClick: [LoginApp::advance])]
    advance_button: nwg::Button,

    // --- Tela Hello World ---

    #[nwg_control(text: "Hello World", size: (300, 60), position: (60, 80),
        font: Some(&data.hello_font))]
    hello_label: nwg::Label,
}

impl LoginApp {
    fn advance(&self) {
        self.name_label.set_visible(false);
        self.name_input.set_visible(false);
        self.password_label.set_visible(false);
        self.password_input.set_visible(false);
        self.advance_button.set_visible(false);

        self.hello_label.set_visible(true);
        self.window.set_text("Bem-vindo");
    }

    fn exit(&self) {
        nwg::stop_thread_dispatch();
    }
}

fn main() {
    nwg::init().expect("Failed to init Native Windows GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");

    let app = LoginApp::build_ui(Default::default()).expect("Failed to build UI");
    app.hello_label.set_visible(false);

    nwg::dispatch_thread_events();
}
