use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use clap::Parser;
use rayon::prelude::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};

#[derive(Parser)]
struct Args {
    word_file: PathBuf,
    #[clap(short)]
    dup_chars_per_word_allowed: bool,
}

fn main() {
    let args = Args::parse();

    let mut word_masks = BufReader::new(File::open(args.word_file).unwrap())
        .lines()
        .flatten()
        .filter_map(|line| {
            if line.len() != 5 {
                None
            } else {
                if !args.dup_chars_per_word_allowed {
                    let mut chars = line.chars().collect::<Vec<_>>();
                    chars.sort();
                    chars.dedup();
                    if chars.len() != 5 {
                        return None;
                    }
                }
                word_bitmask(&line).map(|mask| (mask, line.into_boxed_str()))
            }
        })
        .collect::<Vec<_>>();

    println!("collected all word masks");

    word_masks.sort();
    word_masks.dedup_by(|(a, _), (b, _)| a == b);

    println!("dedup'd all word masks");

    let mut res1 = word_masks
        .par_iter()
        .flat_map(|(m1, _w1)| {
            word_masks.par_iter().filter_map(move |(m2, _w2)| {
                (*m2 & m1 == 0).then(|| {
                    // println!("{} {}", _w1, _w2);
                    (*m1, *m2)
                })
            })
        })
        .collect::<Vec<_>>();
    res1.sort_by(|(a1, a2), (b1, b2)| (*a1 | *a2).cmp(&(*b1 | *b2)));
    res1.dedup_by(|(a1, a2), (b1, b2)| (*a1 | *a2) == (*b1 | *b2));
    println!("finished collecting all 2-word pairs ({})", res1.len());
    let amount = AtomicU32::new(0);
    let no_ana = AtomicU32::new(0);
    res1.iter()
        .enumerate()
        .par_bridge()
        .for_each(|(i, (m1, m2))| {
            for (m3, m4) in res1[i..].iter() {
                if (m3 | m4) & (m1 | m2) == 0 {
                    let w5s: Vec<Vec<&str>> = word_masks
                        .iter()
                        .filter(|(m5, _)| *m5 & (*m1 | *m2 | *m3 | *m4) == 0)
                        .fold(
                            HashMap::new(),
                            |mut hm: HashMap<u32, Vec<&str>>, (m5, w5)| {
                                hm.entry(*m5)
                                    .and_modify(|e| e.push(w5))
                                    .or_insert_with(|| vec![w5]);
                                hm
                            },
                        )
                        .into_values()
                        .collect();
                    if !w5s.is_empty() {
                        let words: Vec<_> = [m1, m2, m3, m4]
                            .iter()
                            .map(|mask| -> Vec<&str> {
                                word_masks
                                    .iter()
                                    .filter_map(|(m, w)| if m == *mask { Some(&**w) } else { None })
                                    .collect()
                            })
                            .collect();
                        for w5 in w5s {
                            let mut words = words.iter().collect::<Vec<_>>();
                            words.push(&w5);
                            no_ana.fetch_add(1, Ordering::AcqRel);
                            amount.fetch_add(w5.len() as _, Ordering::AcqRel);
                            println!("{:?}", words)
                        }
                    }
                }
            }
        });
    println!(
        "amount:\t{}\nno_anagrams:\t{}",
        amount.load(Ordering::Relaxed),
        no_ana.load(Ordering::Relaxed)
    )
}

fn word_bitmask(w: &str) -> Option<u32> {
    let mut bitmask = 0u32;
    for c in w.chars() {
        let i = 1 << char_to_index(c)?;
        bitmask |= i;
    }
    Some(bitmask)
}

fn char_to_index(c: char) -> Option<u32> {
    char::to_digit(c.to_ascii_lowercase(), 36)?.checked_sub(10)
}
