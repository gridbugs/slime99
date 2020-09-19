use chargrid_ansi_terminal::{col_encode, Context};
use rand::Rng;
use slime99_app::{app, AutoPlay, EnvNull, Frontend, RngSeed};
use slime99_native::{meap, NativeCommon};

#[derive(Clone)]
enum ColEncodeChoice {
    TrueColour,
    Rgb,
    Greyscale,
    Ansi,
}

impl ColEncodeChoice {
    fn parser() -> impl meap::Parser<Item = Self> {
        use meap::Parser;
        use ColEncodeChoice::*;
        meap::choose_at_most_one!(
            flag("true-colour").some_if(TrueColour),
            flag("rgb").some_if(Rgb),
            flag("greyscale").some_if(Greyscale),
            flag("ansi").some_if(Ansi),
        )
        .with_default_general(TrueColour)
    }
}

struct Args {
    native_common: NativeCommon,
    col_encode_choice: ColEncodeChoice,
}

impl Args {
    fn parser() -> impl meap::Parser<Item = Self> {
        meap::let_map! {
            let {
                native_common = NativeCommon::parser();
                col_encode_choice = ColEncodeChoice::parser();
            } in {
                Self { native_common, col_encode_choice }
            }
        }
    }
}

fn main() {
    use meap::Parser;
    env_logger::init();
    let Args {
        native_common:
            NativeCommon {
                rng_seed,
                file_storage,
                controls,
                save_file,
                audio_player,
                game_config,
            },
        col_encode_choice,
    } = Args::parser().with_help_default().parse_env_or_exit();
    // We won't be able to print once the context is created. Choose the initial rng
    // seed before starting the game so it can be logged in case of error.
    let rng_seed_u64 = match rng_seed {
        RngSeed::U64(seed) => seed,
        RngSeed::Random => rand::thread_rng().gen(),
    };
    if let ColEncodeChoice::TrueColour = col_encode_choice {
        println!("Running in true-colour mode.\nIf colours look wrong, run with `--rgb` or try a different terminal emulator.");
    }
    println!("Initial RNG Seed: {}", rng_seed_u64);
    let context = Context::new().unwrap();
    let app = app(
        game_config,
        Frontend::AnsiTerminal,
        controls,
        file_storage,
        save_file,
        audio_player,
        RngSeed::U64(rng_seed_u64),
        Some(AutoPlay),
        None,
        Box::new(EnvNull),
    );
    use ColEncodeChoice as C;
    match col_encode_choice {
        C::TrueColour => context.run_app(app, col_encode::XtermTrueColour),
        C::Rgb => context.run_app(app, col_encode::FromTermInfoRgb),
        C::Greyscale => context.run_app(app, col_encode::FromTermInfoGreyscale),
        C::Ansi => context.run_app(app, col_encode::FromTermInfoAnsi16Colour),
    }
}
