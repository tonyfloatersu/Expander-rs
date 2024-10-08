use std::{
    sync::{Arc, Mutex},
    thread,
};

use clap::Parser;
use expander_rs::utils::{KECCAK_CIRCUIT, POSEIDON_CIRCUIT};
use expander_rs::{
    BN254ConfigSha2, Circuit, Config, GKRConfig, GKRScheme, M31ExtConfigSha2, Prover,
};

const M31_PACKSIZE: usize = 16;
const FR_PACKSIZE: usize = 1;

/// ...
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Field Identifier: fr, m31, m31ext3
    #[arg(short, long,default_value_t = String::from("m31ext3"))]
    field: String,

    // scheme: keccak, poseidon
    #[arg(short, long, default_value_t = String::from("keccak"))]
    scheme: String,

    /// number of repeat
    #[arg(short, long, default_value_t = 1)]
    repeats: usize,

    /// number of thread
    #[arg(short, long, default_value_t = 1)]
    threads: u64,
}

fn main() {
    let args = Args::parse();
    print_info(&args);

    match args.field.as_str() {
        "m31ext3" => match args.scheme.as_str() {
            "keccak" => run_benchmark::<M31ExtConfigSha2>(
                &args,
                Config::<M31ExtConfigSha2>::new(GKRScheme::Vanilla),
            ),
            "poseidon" => run_benchmark::<M31ExtConfigSha2>(
                &args,
                Config::<M31ExtConfigSha2>::new(GKRScheme::GkrSquare),
            ),
            _ => unreachable!(),
        },
        "fr" => match args.scheme.as_str() {
            "keccak" => run_benchmark::<BN254ConfigSha2>(
                &args,
                Config::<BN254ConfigSha2>::new(GKRScheme::Vanilla),
            ),
            "poseidon" => run_benchmark::<BN254ConfigSha2>(
                &args,
                Config::<BN254ConfigSha2>::new(GKRScheme::GkrSquare),
            ),
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };
}

fn run_benchmark<C: GKRConfig>(args: &Args, config: Config<C>) {
    let partial_proof_cnts = (0..args.threads)
        .map(|_| Arc::new(Mutex::new(0)))
        .collect::<Vec<_>>();
    let start_time = std::time::Instant::now();
    let pack_size = match args.field.as_str() {
        "m31ext3" => M31_PACKSIZE,
        "fr" => FR_PACKSIZE,
        _ => unreachable!(),
    };

    // load circuit
    let circuit_template = match args.scheme.as_str() {
        "keccak" => Circuit::<C>::load_circuit(KECCAK_CIRCUIT),
        "poseidon" => Circuit::<C>::load_circuit(POSEIDON_CIRCUIT),
        _ => unreachable!(),
    };

    let circuit_copy_size: usize = match args.scheme.as_str() {
        "keccak" => 2,
        "poseidon" => 120,
        _ => unreachable!(),
    };

    let circuits = (0..args.threads)
        .map(|_| {
            let mut c = circuit_template.clone();
            c.set_random_input_for_test();
            c.evaluate();
            c
        })
        .collect::<Vec<_>>();

    println!("Circuit loaded!");
    let _ = circuits
        .into_iter()
        .enumerate()
        .map(|(i, mut c)| {
            let partial_proof_cnt = partial_proof_cnts[i].clone();
            let local_config = config.clone();
            thread::spawn(move || {
                loop {
                    // bench func
                    let mut prover = Prover::new(&local_config);
                    prover.prepare_mem(&c);
                    prover.prove(&mut c);
                    // update cnt
                    let mut cnt = partial_proof_cnt.lock().unwrap();
                    let proof_cnt_this_round = circuit_copy_size * pack_size;
                    *cnt += proof_cnt_this_round;
                }
            })
        })
        .collect::<Vec<_>>();

    println!("We are now calculating average throughput, please wait for 1 minutes");
    for i in 0..args.repeats {
        thread::sleep(std::time::Duration::from_secs(60));
        let stop_time = std::time::Instant::now();
        let duration = stop_time.duration_since(start_time);
        let mut total_proof_cnt = 0;
        for cnt in &partial_proof_cnts {
            total_proof_cnt += *cnt.lock().unwrap();
        }
        let throughput = total_proof_cnt as f64 / duration.as_secs_f64();
        println!("{}-bench: throughput: {} hashes/s", i, throughput.round());
    }
}

fn print_info(args: &Args) {
    let prover = match args.scheme.as_str() {
        "keccak" => "GKR",
        "poseidon" => "GKR^2",
        _ => unreachable!(),
    };

    println!("===============================");
    println!(
        "benchmarking {} with {} over {}",
        args.scheme, prover, args.field
    );
    println!("field:          {}", args.field);
    println!("#threads:       {}", args.threads);
    println!("#bench repeats: {}", args.repeats);
    println!("hash scheme:    {}", args.scheme);
    println!("===============================")
}
