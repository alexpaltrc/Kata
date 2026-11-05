//! The words Kata shows, in the user's language.

use std::sync::OnceLock;

use kata_editor::{BLUR_LEVELS, STEP_SIZES};
use windows::Win32::Globalization::GetUserDefaultUILanguage;

pub struct Strings {
    /// The welcome, on first opening Kata: who it is, what it costs, and the
    /// invitation to a first capture.
    pub greeting: &'static str,
    pub free: &'static str,
    pub first_capture: &'static str,
    /// The welcome's Ready, with the first capture: how long it took to show and how
    /// much lighter it is; the setting that makes Kata the screenshot app, said for
    /// someone who doesn't know the keys or Windows' own app; the way on; and that
    /// nothing leaves the PC.
    ready: &'static str,
    lighter: [&'static str; 2],
    pub make_default: &'static str,
    pub make_default_detail: &'static str,
    pub go_on: &'static str,
    pub stays_here: &'static str,
    /// The welcome's Premium: that all so far is free, what Premium adds, and the
    /// two ways on.
    pub all_free: &'static str,
    pub forever: &'static str,
    pub premium_adds: &'static str,
    pub perks: [&'static str; 3],
    pub stay_free: &'static str,
    pub get_premium: &'static str,
    pub new_capture: &'static str,
    pub open_captures: &'static str,
    pub settings: &'static str,
    pub back: &'static str,
    pub exit: &'static str,
    /// Kata Dev's command to try the welcome again.
    pub replay_welcome: &'static str,
    pub blur: &'static str,
    pub focus: &'static str,
    pub steps: &'static str,
    pub shapes: &'static str,
    /// What each form of the shapes is called: rectangle, ellipse and arrow.
    pub forms: [&'static str; 3],
    pub blur_levels: [&'static str; BLUR_LEVELS],
    /// What assistive technologies call the setting that blurs around the focus, and
    /// its first step.
    pub blur_around: &'static str,
    pub no_blur: &'static str,
    /// What assistive technologies call the setting that sizes the steps.
    pub step_size: &'static str,
    pub step_sizes: [&'static str; STEP_SIZES.len()],
    /// What assistive technologies call the button that shows the capture's file.
    pub open_file_location: &'static str,
    /// The invitation to name the capture: after its date while it has no name, and
    /// in the empty box where the name is typed.
    pub add_name: &'static str,
    /// What clicking the file name does, for assistive technologies.
    pub rename: &'static str,
    pub shortcut: &'static str,
    pub open_editor: &'static str,
    pub tool_names: &'static str,
    pub sound: &'static str,
    pub folder: &'static str,
    pub change: &'static str,
    /// What assistive technologies call the button that changes the folder.
    pub change_folder: &'static str,
    pub screen_keys: &'static str,
    /// The info tooltip beside it: which keys it takes, and what turning it off does.
    pub screen_keys_help: &'static str,
    pub start_with_windows: &'static str,
    /// While waiting for a new shortcut.
    pub press_shortcut: &'static str,
    /// When a new shortcut needs a modifier.
    pub add_modifier: &'static str,
    /// When another app already uses a new shortcut.
    pub in_use: &'static str,
    /// The Shift key's name.
    pub shift: &'static str,
    /// The space bar's name, on the hint that it resets the zoom, and what it does.
    pub space: &'static str,
    pub reset_zoom: &'static str,
    /// What follows a number to make it a percentage.
    percent: &'static str,
    /// What parts a number's decimals from the rest.
    decimal: &'static str,
}

impl Strings {
    pub fn percent(&self, value: f32) -> String {
        format!("{}{}", (value * 100.0).round(), self.percent)
    }

    /// How long the first capture took to show, as its Ready says.
    pub fn ready(&self, seconds: f32) -> String {
        // Hundredths below a second, where they count.
        let digits = if seconds < 1.0 { 2 } else { 1 };
        let seconds = format!("{:.*}", digits, seconds.max(0.01));
        format!(
            "{}{}\u{a0}s",
            self.ready,
            seconds.replace('.', self.decimal)
        )
    }

    /// How much lighter than with Windows a capture's file is, `saved` being the
    /// share it saves.
    pub fn lighter(&self, saved: f32) -> String {
        let [before, after] = self.lighter;
        format!("{before}{}{after}", self.percent(saved))
    }
}

static SPANISH: Strings = Strings {
    greeting: "Hola, soy Kata",
    free: "Gratis y sin límites",
    first_capture: "Toma tu primera captura",
    ready: "Lista en ",
    lighter: ["Y pesa un ", " menos que con Windows"],
    make_default: "Haz de Kata tu app de capturas",
    make_default_detail: "Las teclas de captura de Windows, como Impr Pant, abrirán Kata",
    go_on: "Continuar",
    stays_here: "Todo se queda en tu equipo",
    all_free: "Todo esto es gratis",
    forever: "Para siempre y sin límites",
    premium_adds: "Premium añade",
    perks: [
        "Un atajo a tu medida",
        "Tus colores en formas y pasos",
        "Dibujo a mano alzada",
    ],
    stay_free: "Seguir gratis",
    get_premium: "Obtener Premium",
    new_capture: "Nuevo recorte",
    open_captures: "Abrir capturas",
    settings: "Ajustes",
    back: "Atrás",
    exit: "Salir",
    replay_welcome: "Repetir la bienvenida",
    blur: "Desenfocar",
    focus: "Foco",
    steps: "Pasos",
    shapes: "Formas",
    forms: ["Rectángulo", "Elipse", "Flecha"],
    blur_levels: ["Suave", "Medio", "Fuerte"],
    blur_around: "Desenfocar alrededor",
    no_blur: "Ninguno",
    step_size: "Tamaño de los pasos",
    step_sizes: ["Pequeño", "Mediano", "Grande", "Muy grande"],
    open_file_location: "Abrir la ubicación del archivo",
    add_name: "Ponle nombre",
    rename: "Cambiar el nombre",
    shortcut: "Atajo",
    open_editor: "Abrir el editor al capturar",
    tool_names: "Mostrar los nombres de las herramientas",
    sound: "Sonido",
    folder: "Carpeta de guardado",
    change: "Cambiar",
    change_folder: "Cambiar la carpeta de guardado",
    screen_keys: "Kata es tu app de capturas",
    screen_keys_help: "Impr Pant (sola o con Alt, Win, Ctrl o Mayús) y Win + Mayús + S abren un \
        recorte de Kata en vez de Recortes. Al apagarlo, esas teclas vuelven a funcionar como \
        siempre.",
    start_with_windows: "Iniciar con Windows",
    press_shortcut: "Pulsa el nuevo atajo",
    add_modifier: "Añade Ctrl o Alt",
    in_use: "Ya está en uso",
    shift: "Mayús",
    space: "Espacio",
    reset_zoom: "Restablecer el zoom",
    percent: "\u{a0}%",
    decimal: ",",
};

static ENGLISH: Strings = Strings {
    greeting: "Hi, I'm Kata",
    free: "Free, with no limits",
    first_capture: "Take your first capture",
    ready: "Ready in ",
    lighter: ["And it's ", " lighter than with Windows"],
    make_default: "Make Kata your screenshot app",
    make_default_detail: "Windows' screenshot keys, like Print Screen, will open Kata",
    go_on: "Continue",
    stays_here: "Everything stays on your PC",
    all_free: "All of this is free",
    forever: "Forever, with no limits",
    premium_adds: "Premium adds",
    perks: [
        "A shortcut of your own",
        "Your colors in shapes and steps",
        "Freehand drawing",
    ],
    stay_free: "Continue for free",
    get_premium: "Get Premium",
    new_capture: "New capture",
    open_captures: "Open captures",
    settings: "Settings",
    back: "Back",
    exit: "Exit",
    replay_welcome: "Replay the welcome",
    blur: "Blur",
    focus: "Focus",
    steps: "Steps",
    shapes: "Shapes",
    forms: ["Rectangle", "Ellipse", "Arrow"],
    blur_levels: ["Soft", "Medium", "Strong"],
    blur_around: "Blur around",
    no_blur: "None",
    step_size: "Step size",
    step_sizes: ["Small", "Medium", "Large", "Extra large"],
    open_file_location: "Open file location",
    add_name: "Add a name",
    rename: "Rename",
    shortcut: "Shortcut",
    open_editor: "Open the editor after capturing",
    tool_names: "Show tool names",
    sound: "Sound",
    folder: "Save folder",
    change: "Change",
    change_folder: "Change the save folder",
    screen_keys: "Kata is your screenshot app",
    screen_keys_help: "Print Screen (alone or with Alt, Win, Ctrl or Shift) and Win + Shift + S \
        start a capture in Kata instead of the Snipping Tool. Turned off, those keys work as \
        usual again.",
    start_with_windows: "Start with Windows",
    press_shortcut: "Press the new shortcut",
    add_modifier: "Add Ctrl or Alt",
    in_use: "Already in use",
    shift: "Shift",
    space: "Space",
    reset_zoom: "Reset zoom",
    percent: "%",
    decimal: ".",
};

pub fn strings() -> &'static Strings {
    static CURRENT: OnceLock<&Strings> = OnceLock::new();
    CURRENT.get_or_init(|| {
        const SPANISH_ID: u16 = 0x0a;
        // The low ten bits of a language id are the language, without the region.
        let language = unsafe { GetUserDefaultUILanguage() } & 0x3ff;
        if language == SPANISH_ID {
            &SPANISH
        } else {
            &ENGLISH
        }
    })
}
