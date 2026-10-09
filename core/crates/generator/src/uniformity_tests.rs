//! Chi-square uniformity tests (SEC-C09).
//!
//! Method: fixed seeds make the draws deterministic, so these tests cannot be
//! flaky. For `df` degrees of freedom the chi-square statistic has mean `df`
//! and sd `sqrt(2*df)`; we require `stat < df + 6*sqrt(2*df)` (a ~1e-9 tail
//! for a truly uniform source), far above random fluctuation yet far below
//! the statistic a modulo-biased sampler produces at these draw counts.

use crate::password::{PasswordOptions, generate_password};
use crate::rng::{SeededRandom, shuffle, uniform_below};

const DRAWS: usize = 1_000_000;

fn threshold(df: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let df = df as f64;
    df + 6.0 * (2.0 * df).sqrt()
}

fn chi_square(counts: &[usize], total: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let expected = total as f64 / counts.len() as f64;
    counts
        .iter()
        .map(|&c| {
            #[allow(clippy::cast_precision_loss)]
            let d = c as f64 - expected;
            d * d / expected
        })
        .sum()
}

#[test]
fn single_char_selection_uniform_for_awkward_alphabet_sizes() {
    for (seed, n) in [(1u64, 62u32), (2, 71), (3, 94), (4, 7776)] {
        let mut rng = SeededRandom::new(seed);
        let mut counts = vec![0usize; n as usize];
        for _ in 0..DRAWS {
            counts[uniform_below(&mut rng, n).unwrap() as usize] += 1;
        }
        let stat = chi_square(&counts, DRAWS);
        assert!(stat < threshold(n as usize - 1), "n={n} chi2={stat}");
    }
}

#[test]
fn rejection_sampling_beats_modulo_on_biased_range() {
    // Sanity check that the test has power: a modulo sampler over a range
    // that does not divide 2^32 (n = 3 * 2^30 + 1 style bias) is detected.
    let n = 3_000_000_000u64;
    let mut rng = SeededRandom::new(9);
    let mut low = 0usize;
    for _ in 0..DRAWS {
        use crate::rng::RandomSource;
        let x = u64::from(rng.next_u32().unwrap()) % n;
        if x < 1_000_000_000 {
            low += 1;
        }
    }
    // Under modulo the lowest third is hit ~50% more than uniform's 33%.
    assert!(low > 400_000, "modulo bias should be visible: {low}");
    let mut rng = SeededRandom::new(9);
    let mut low = 0usize;
    for _ in 0..DRAWS {
        if u64::from(uniform_below(&mut rng, 3_000_000_000).unwrap()) < 1_000_000_000 {
            low += 1;
        }
    }
    assert!(
        (320_000..346_000).contains(&low),
        "unbiased share off: {low}"
    );
}

#[test]
fn shuffle_position_distribution_uniform() {
    // Track where element 0 of a 10-element array lands, and which element
    // lands in position 0: both must be uniform over 10 cells.
    let mut rng = SeededRandom::new(11);
    let mut landing = [0usize; 10];
    let mut first = [0usize; 10];
    for _ in 0..DRAWS {
        let mut a: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        shuffle(&mut rng, &mut a).unwrap();
        landing[a.iter().position(|&x| x == 0).unwrap()] += 1;
        first[a[0] as usize] += 1;
    }
    assert!(chi_square(&landing, DRAWS) < threshold(9));
    assert!(chi_square(&first, DRAWS) < threshold(9));
}

#[test]
fn required_class_position_uniform() {
    // Alphabet = 10 digits + the single symbol '!', require_each_class. The
    // guaranteed '!' is placed first and then shuffled, so over many
    // passwords '!' must occur equally often at every position.
    let o = PasswordOptions {
        length: 8,
        lower: false,
        upper: false,
        digits: true,
        symbols: true,
        symbol_set: "!".into(),
        require_each_class: true,
        ..PasswordOptions::default()
    };
    let mut rng = SeededRandom::new(13);
    let mut counts = [0usize; 8];
    for _ in 0..400_000 {
        for (i, c) in generate_password(&o, &mut rng).unwrap().chars().enumerate() {
            if c == '!' {
                counts[i] += 1;
            }
        }
    }
    let total: usize = counts.iter().sum();
    assert!(chi_square(&counts, total) < threshold(7));
}

#[test]
fn password_characters_uniform_without_class_constraint() {
    let o = PasswordOptions {
        length: 100,
        symbols: false,
        require_each_class: false,
        ..PasswordOptions::default()
    }; // alphabet = 62
    let mut rng = SeededRandom::new(17);
    let mut counts = [0usize; 128];
    let n_pw = DRAWS / 100;
    for _ in 0..n_pw {
        for b in generate_password(&o, &mut rng).unwrap().bytes() {
            counts[b as usize] += 1;
        }
    }
    let used: Vec<usize> = counts.iter().copied().filter(|&c| c > 0).collect();
    assert_eq!(used.len(), 62);
    assert!(chi_square(&used, n_pw * 100) < threshold(61));
}
