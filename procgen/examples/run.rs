use grid_2d::{Coord, Size};
use procgen::{Sewer, SewerCell, SewerSpec};
use rand::{Rng, SeedableRng};
use rand_isaac::Isaac64Rng;
use simon::*;

struct Args {
    size: Size,
    rng: Isaac64Rng,
}

impl Args {
    fn arg() -> impl Arg<Item = Self> {
        args_map! {
            let {
                rng_seed = opt::<u64>("r", "rng-seed", "rng seed", "INT")
                    .with_default_lazy(|| rand::thread_rng().gen());
                size = opt::<u32>("x", "width", "width", "INT").with_default(40)
                        .both(opt::<u32>("y", "height", "height", "INT").with_default(20))
                        .map(|(width, height)| Size::new(width, height));
            } in {{
                println!("RNG Seed: {}", rng_seed);
                let rng = Isaac64Rng::seed_from_u64(rng_seed);
                Self {
                    rng,
                    size,
                }
            }}
        }
    }
}

fn main() {
    let Args { size, mut rng } = Args::arg().with_help_default().parse_env_or_exit();
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
