use grid_2d::{Coord, Size};
use procgen::{Sewer, SewerCell, SewerSpec};
use rand::{Rng, SeedableRng};
use rand_isaac::Isaac64Rng;

struct Args {
    size: Size,
    rng: Isaac64Rng,
}

impl Args {
    fn parser() -> meap::LetMap<impl meap::Parser<Item = Self>> {
        meap::let_map! {
            let {
                rng_seed = opt_opt::<u64, _>("INT", 'r').name("rng-seed").desc("rng seed")
                    .with_general_default_lazy(|| rand::thread_rng().gen());
                width = opt_opt("INT", 'x').name("width").with_default(40);
                height = opt_opt("INT", 'y').name("height").with_default(20);
            } in {{
                println!("RNG Seed: {}", rng_seed);
                let rng = Isaac64Rng::seed_from_u64(rng_seed);
                let size = Size::new(width, height);
                Self {
                    rng,
                    size,
                }
            }}
        }
    }
}

fn main() {
    let Args { size, mut rng } = Args::parser().with_help_default().parse_env_or_exit();
    let spec = SewerSpec { size };
    let sewer = Sewer::generate(spec, &mut rng);
    println!("    abcdefghijklmnopqrstuvwxyz");
    for (i, row) in sewer.map.rows().enumerate() {
        print!("{:2}: ", i);
        for (j, cell) in row.into_iter().enumerate() {
            let coord = Coord::new(j as i32, i as i32);
            let ch = if coord == sewer.start {
                '@'
            } else if coord == sewer.goal {
                '>'
            } else {
                match cell {
                    SewerCell::Floor => '.',
                    SewerCell::Wall => '█',
                    SewerCell::Pool => '~',
                    SewerCell::Bridge => '=',
                    SewerCell::Door => '+',
                }
            };
            print!("{}", ch);
        }
        println!("");
    }
}
