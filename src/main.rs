use clap::Parser;
use ordered_float::NotNan;
use rayon::prelude::*;

use std::{collections::HashMap, fs::File, io::Write};

const CROSSED_OUT: u8 = 0;
// const ALL_GREEN: u8 = 242;
const ALL_GREEN: Word = [b'g'; 5];

type Word = [u8; 5];
type Colors = [u8; 5];

fn to_word(w: impl AsRef<[u8]>) -> Word {
    w.as_ref().try_into().unwrap()
}

struct Tree {
    guess: Word,
    total_guesses: usize,
    max_guesses: usize,
    children: HashMap<Colors, Tree>,
}

impl Tree {
    fn leaf(guess: Word) -> Self {
        Self {
            guess,
            total_guesses: 1,
            max_guesses: 1,
            children: Default::default(),
        }
    }

    fn print(&self, n_answers: usize) {
        println!(
            "{}, total: {}, avg: {}, max: {}",
            std::str::from_utf8(&self.guess).unwrap(),
            self.total_guesses,
            self.total_guesses as f32 / n_answers as f32,
            self.max_guesses,
        )
    }

    fn write(&self, w: &mut impl Write, mut line: Vec<u8>) -> std::io::Result<()> {
        if self.children.is_empty() {
            line.extend(self.guess);
            line.push(b'\n');
            w.write_all(&line)
        } else {
            line.extend(self.guess);
            line.push(b',');
            for child in self.children.values() {
                child.write(w, line.clone())?;
            }
            Ok(())
        }
    }
}

fn score(mut guess: Word, mut answer: Word) -> Word {
    let mut colors = [b'b'; 5];

    for i in 0..5 {
        if guess[i] == answer[i] {
            colors[i] = b'g';
            answer[i] = CROSSED_OUT;
            guess[i] = CROSSED_OUT;
        }
    }

    for i in 0..5 {
        if guess[i] != CROSSED_OUT {
            if let Some(j) = answer.iter().copied().position(|a| a == guess[i]) {
                colors[i] = b'y';
                answer[j] = CROSSED_OUT;
                guess[i] = CROSSED_OUT;
            }
        }
    }

    colors
}

fn scoreToInt(w: &Word) -> u8 {
    let mut s = 0;
    for i in 0..5 {
        let v = match w[i] {
            b'b' => 0,
            b'y' => 1,
            b'g' => 2,
            // NOTE: this will never happen
            _ => 0,
        };
        s += v * u8::pow(3, i.try_into().unwrap());
    }
    return s;
}

/**
 * guesses -> indexes into guess_words
 * guess_words -> the full list of guess words
 */
fn solve(params: &Params, depth: usize, guesses: &[usize], answers: &[usize], guess_words: &[Word], answer_words: &[Word], matrix: &Vec<Vec<Word>>) -> Option<Tree> {
    assert!(!answers.is_empty());
    if depth >= 7 {
        return None;
    }

    if let &[only_answer] = answers {
        return Some(Tree::leaf(answer_words[only_answer]));
    }

    let top_guesses: Vec<usize> = if depth == 0 && params.starting_word.is_some() {
        let starting_word: Word = to_word(params.starting_word.as_ref().unwrap());
        let starting_word_i: usize = guess_words.iter().position(|&word| word == starting_word).unwrap();
        vec![starting_word_i,]
    } else {
        let mut groups = HashMap::<Word, usize>::new();
        let mut ranked_guesses: Vec<(_, usize)> = guesses
            .iter()
            .map(|&guess| {
                groups.clear();

                for &answer in answers {
                    // let colors = score(guess_words[guess], answer_words[answer]);
                    let colors = matrix[guess][answer];
                    *groups.entry(colors).or_default() += 1;
                }

                let mut sum: usize = groups.values().copied().sum();
                sum -= groups.get(&ALL_GREEN).copied().unwrap_or_default();

                let avg = NotNan::new(sum as f64 / groups.len() as f64).unwrap();
                (avg, guess)
            })
            .collect();
        ranked_guesses.sort_unstable();
        ranked_guesses
            .iter()
            .map(|(_score, guess)| *guess)
            .take(params.n_guesses)
            .collect()
    };

    assert!(!top_guesses.is_empty());

    let tree = top_guesses
        .iter()
        .filter_map(|&guess| {
            if depth == 0 {
                let guess_word = guess_words[guess];
                print!("{}...\n", std::str::from_utf8(&guess_word).unwrap());
                std::io::stdout().flush().unwrap();
            }

            let mut tree = Tree {
                total_guesses: answers.len(),
                max_guesses: 0,
                guess: guess_words[guess],
                children: Default::default(),
            };

            let mut groups = HashMap::<Word, Vec<usize>>::new();
            for &answer in answers {
                let colors = matrix[guess][answer];
                // let colors = score(guess_words[guess], answer_words[answer]);
                groups.entry(colors).or_default().push(answer);
            }

            let recurse = |(_score, grouped_answers): (&Colors, &Vec<usize>)| {
                solve(params, depth + 1, guesses, grouped_answers, guess_words, answer_words, matrix)
            };

            let children: Vec<Option<Tree>> = if depth <= 1 {
                groups.par_iter().map(recurse).collect()
            } else {
                groups.iter().map(recurse).collect()
            };

            for (&score, child) in groups.keys().zip(children) {
                let child = child?;
                if score != ALL_GREEN {
                    tree.total_guesses += child.total_guesses;
                }
                tree.max_guesses = tree.max_guesses.max(child.max_guesses + 1);
                tree.children.insert(score, child);
            }

            if depth == 0 {
                tree.print(answers.len())
            }

            Some(tree)
        })
        .min_by_key(|tree| tree.total_guesses)?;

    Some(tree)
}

#[derive(Parser)]
struct Params {
    #[clap(short, long, default_value = "20")]
    n_guesses: usize,
    #[clap(long)]
    answers_only: bool,
    #[clap(long)]
    starting_word: Option<String>,
}

fn computeMatrix(guesses: &[Word], answers: &[Word]) -> Vec<Vec<Word>> {
    let mut matrix: Vec<Vec<Word>> = Vec::new();

    print!("Precomputing {}x{} matrix of u8 elements (takes about 40s)...\n", guesses.len(), answers.len());
    std::io::stdout().flush().unwrap();

    guesses.iter().for_each(|guess| {
        let row = answers.iter().map(|answer| {
            let s = score(*guess, *answer);
            return s;
            // return scoreToInt(&s);
        }).collect();
        matrix.push(row);
    });

    print!("Done!\n");

    let g = matrix[4000][1000];
    print!("Entry for [4000, 1000] is {entry}!\n",
            entry=std::str::from_utf8(&g).unwrap()
            );
    return matrix;
}

fn main() {
    println!("Wordle solver!");

    static ONLY_GUESSES: &[u8] = include_bytes!("../guesses.txt");
    static ANSWERS: &[u8] = include_bytes!("../answers.txt");

    let params = Params::parse();

    let answers: Vec<Word> = ANSWERS.split(|&b| b == b'\n').map(to_word).collect();
    let mut guesses: Vec<Word> = answers.clone();

    if !params.answers_only {
        guesses.extend(ONLY_GUESSES.split(|&b| b == b'\n').map(to_word));
        guesses.sort_unstable();
        guesses.dedup();
    }

    // // pre-compute the matrix
    let matrix = computeMatrix(&guesses, &answers);
    let g = matrix[4000][1000];
    print!("Entry for [4000, 1000] is {entry}!\n",
            entry=std::str::from_utf8(&g).unwrap()
            );

    let mut guessIndexes: Vec<usize> = (1..guesses.len()).collect();
    let mut answerIndexes: Vec<usize> = (1..answers.len()).collect();

    let tree = solve(&params, 0, &guessIndexes, &answerIndexes, &guesses, &answers, &matrix).unwrap();
    println!("\nDone!");
    tree.print(answers.len());

    let mut file = File::create("out.txt").unwrap();
    tree.write(&mut file, vec![]).unwrap();
}

#[test]
fn test_colors() {
    let colors = score(to_word("silly"), to_word("hotel"));
    assert_eq!(colors, to_word("bbybb"));
    let colors = score(to_word("silly"), to_word("daily"));
    assert_eq!(colors, to_word("bybgg"));
}
