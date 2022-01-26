use std::{collections::HashMap, fs::File, io::Write};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct State {
    must_have: u32,
    spots: [u32; 5],
}

impl Default for State {
    fn default() -> Self {
        Self {
            spots: [u32::MAX >> (32 - 26); 5],
            must_have: 0,
        }
    }
}

const CROSSED_OUT: u8 = 0;

#[inline(always)]
fn letter_bit(letter: u8) -> u32 {
    assert!(letter >= b'a', "{:?}", letter as char);
    let i = letter - b'a';
    assert!(i < 26);
    1 << i
}

impl State {
    fn step(&self, guess: Word, answer: Word) -> Self {
        self.step_and_color(guess, answer).0
    }

    fn score(&self) -> (u32, u32) {
        todo!()
        // let n_greens = self.spots.iter().filter(|s| s.count_ones() == 1).count();
        // let n_zeros = self.spots.iter().map(|s| s.count_zeros()).sum();
        // // TODO yellows
        // (n_greens as _, n_zeros)
    }

    fn could_be_answer(&self, guess: Word) -> bool {
        let mut has_letters = 0;
        for (spot, letter) in self.spots.into_iter().zip(guess) {
            has_letters |= letter_bit(letter);
            if spot & letter_bit(letter) == 0 {
                return false;
            }
        }

        self.must_have == self.must_have & has_letters
    }

    fn step_and_color(mut self, mut guess: Word, mut answer: Word) -> (Self, [u8; 5]) {
        let original_guess = guess;
        let original_spots = self.spots;
        let mut colors = [b'b'; 5];

        for i in 0..5 {
            if guess[i] == answer[i] {
                colors[i] = b'g';
                self.spots[i] = letter_bit(guess[i]);
                answer[i] = CROSSED_OUT;
                guess[i] = CROSSED_OUT;
            }
        }

        for i in 0..5 {
            if guess[i] != CROSSED_OUT {
                self.spots[i] &= !letter_bit(guess[i]);
                if let Some(j) = answer.iter().copied().position(|a| a == guess[i]) {
                    colors[i] = b'y';
                    assert_ne!(i, j, "{:?}", guess[i] as char);
                    answer[j] = CROSSED_OUT;
                    self.must_have |= letter_bit(guess[i]);
                } else {
                    // found a black, cross off this letter in all slots
                    if original_guess.iter().filter(|&&x| x == guess[i]).count() <= 1 {
                        self.spots
                            .iter_mut()
                            .for_each(|s| *s &= !letter_bit(guess[i]));
                    }
                }
            }
        }

        for spot in self.spots {
            assert_ne!(
                spot,
                0,
                "{} {:?}\n{:?}",
                std::str::from_utf8(&original_guess).unwrap(),
                self.spots,
                original_spots
            )
        }

        (self, colors)
    }
}

type Word = [u8; 5];

struct Context {
    memo: HashMap<State, Node>,
}

#[derive(Clone)]
struct Node {
    max_guesses: usize,
    n_total_guesses: usize,
    n_possible_answers: usize,
    guess: Word,
}

fn to_word(w: impl AsRef<[u8]>) -> Word {
    w.as_ref().try_into().unwrap()
}

impl Context {
    fn new() -> Self {
        // static ONLY_GUESSES: &[u8] = include_bytes!("../guesses.txt");
        // static ANSWERS: &[u8] = include_bytes!("../answers.txt");

        // let mut guesses: Vec<Word> = ONLY_GUESSES.split(|&b| b == b'\n').map(to_word).collect();
        // let answers: Vec<Word> = ANSWERS.split(|&b| b == b'\n').map(to_word).collect();

        // guesses.extend(answers.iter().copied());

        Self {
            memo: Default::default(),
        }
    }

    // fn solve(&mut self, depth: usize, state: State, guesses: &[Word], answers: &[Word]) -> &Node {
    //     // NLL workaround
    //     // https://stackoverflow.com/questions/38023871/returning-a-reference-from-a-hashmap-or-vec-causes-a-borrow-to-last-beyond-the-s
    //     if self.memo.contains_key(&state) {
    //         return &self.memo[&state];
    //     }

    //     if let &[only_answer] = answers {
    //         return self.memo.entry(state).or_insert(Node {
    //             max_guesses: 1,
    //             n_total_guesses: 1,
    //             n_possible_answers: 1,
    //             guess: only_answer,
    //         });
    //     }

    //     // let mut ranked_guesses: Vec<(usize, Word)> = guesses
    //     //     .iter()
    //     //     .map(|&guess| {
    //     //         let max_next_answers = answers
    //     //             .iter()
    //     //             .map(|&answer| {
    //     //                 let next_state = state.step(guess, answer);
    //     //                 answers
    //     //                     .iter()
    //     //                     .copied()
    //     //                     .filter(|&a| next_state.could_be_answer(a))
    //     //                     .count()
    //     //             })
    //     //             .max()
    //     //             .unwrap();
    //     //         (max_next_answers, guess)
    //     //     })
    //     //     .collect();
    //     // ranked_guesses.sort_unstable();
    //     // let top_guesses: Vec<Word> = ranked_guesses

    //     let node = guesses
    //         .iter()
    //         .map(|&guess| {
    //             let mut node = Node {
    //                 max_guesses: 0,
    //                 n_total_guesses: 0,
    //                 n_possible_answers: answers.len(),
    //                 guess,
    //             };

    //             for &answer in answers {
    //                 let next_state = state.step(guess, answer);
    //                 let next_answers: Vec<Word> = answers
    //                     .iter()
    //                     .copied()
    //                     .filter(|&a| next_state.could_be_answer(a))
    //                     .collect();
    //                 let n = self.solve(depth + 1, next_state, guesses, &next_answers);
    //                 node.max_guesses = node.max_guesses.max(n.max_guesses + 1);
    //                 node.n_total_guesses += n.n_total_guesses;
    //             }

    //             node
    //         })
    //         .min_by_key(|node| node.n_total_guesses)
    //         .unwrap();

    //     self.memo.entry(state).or_insert(node)
    // }

    fn solve(&mut self, depth: usize, state: State, guesses: &[Word], answers: &[Word]) -> &Node {
        // NLL workaround
        // https://stackoverflow.com/questions/38023871/returning-a-reference-from-a-hashmap-or-vec-causes-a-borrow-to-last-beyond-the-s
        if self.memo.contains_key(&state) {
            return &self.memo[&state];
        }

        assert!(!answers.is_empty());

        if let &[only_answer] = answers {
            return self.memo.entry(state).or_insert(Node {
                max_guesses: 1,
                n_total_guesses: 1,
                n_possible_answers: 1,
                guess: only_answer,
            });
        }

        if depth > 130 {
            for answer in answers {
                println!("{}", std::str::from_utf8(answer).unwrap())
            }
            panic!("Failed, {} answers", answers.len());
        }

        let mut letter_freq = HashMap::<u8, usize>::new();
        for answer in answers {
            for &letter in answer {
                *letter_freq.entry(letter).or_default() += 1;
            }
        }

        let top_guesses: Vec<Word>;
        let n_guesses = 20;
        let candidate_guesses = if answers.len() < n_guesses {
            &answers
        } else {
            let mut ranked_guesses: Vec<(_, Word)> = guesses
                .iter()
                // .filter(|&&g| state.could_be_answer(g)) // TODO is this a good idea?
                .enumerate()
                .map(|(i, &guess)| {
                    // let max_next_answers = answers
                    //     .iter()
                    //     .map(|&answer| {
                    //         let next_state = state.step(guess, answer);
                    //         answers
                    //             .iter()
                    //             .copied()
                    //             .filter(|&a| next_state.could_be_answer(a))
                    //             .count()
                    //     })
                    //     .max()
                    //     .unwrap_or(usize::MAX);
                    // (max_next_answers, guess)
                    let mut count = 0;
                    for (i, letter) in guess.iter().enumerate() {
                        if !guess[i + 1..].contains(letter) {
                            count += letter_freq.get(letter).copied().unwrap_or_default();
                        }
                    }
                    (-(count as i32), guess)
                })
                .collect();
            ranked_guesses.sort_unstable();
            top_guesses = ranked_guesses
                .iter()
                .map(|(_score, guess)| *guess)
                .take(n_guesses)
                .collect();
            top_guesses.as_slice()
        };

        let node = candidate_guesses
            .iter()
            .map(|&guess| {
                if depth <= 0 {
                    print!(
                        "depth: {depth}, guess: {}",
                        std::str::from_utf8(&guess).unwrap()
                    );
                }

                let mut node = Node {
                    max_guesses: 0,
                    n_total_guesses: 0,
                    n_possible_answers: answers.len(),
                    guess,
                };

                for &answer in answers {
                    let next_state = state.step(guess, answer);
                    let next_answers: Vec<Word> = answers
                        .iter()
                        .copied()
                        .filter(|&a| next_state.could_be_answer(a))
                        .collect();
                    // assert!(next_answers.len() < answers.len());
                    let n = self.solve(depth + 1, next_state, guesses, &next_answers);
                    node.max_guesses = node.max_guesses.max(n.max_guesses + 1);
                    node.n_total_guesses += n.n_total_guesses;
                }

                if depth <= 0 {
                    println!(
                        ", avg: {}, max: {}",
                        node.n_total_guesses as f32 / node.n_possible_answers as f32,
                        node.max_guesses,
                    );
                }

                node
            })
            .min_by_key(|node| node.max_guesses)
            .unwrap();

        self.memo.entry(state).or_insert(node)
    }

    fn get_path(&self, answer: Word) -> Vec<Word> {
        let mut state = State::default();
        let mut guesses = vec![];

        loop {
            let node = &self.memo[&state];
            guesses.push(node.guess);
            if node.guess == answer {
                return guesses;
            }
            state = state.step(node.guess, answer)
        }
    }

    fn get_path_string(&self, answer: Word) -> String {
        let path = self.get_path(answer);
        let strings = path
            .iter()
            .map(|g| std::str::from_utf8(g).unwrap())
            .collect::<Vec<_>>();
        strings.join(",")
    }
}

fn main() {
    static ONLY_GUESSES: &[u8] = include_bytes!("../guesses.txt");
    static ANSWERS: &[u8] = include_bytes!("../answers.txt");

    let mut guesses: Vec<Word> = ONLY_GUESSES.split(|&b| b == b'\n').map(to_word).collect();
    let answers: Vec<Word> = ANSWERS.split(|&b| b == b'\n').map(to_word).collect();

    guesses.extend(answers.iter().copied());
    guesses.sort_unstable();
    guesses.dedup();

    let guesses = answers.clone();

    let mut ctx = Context::new();
    let node = ctx.solve(0, State::default(), &guesses, &answers);
    println!("{}", std::str::from_utf8(&node.guess).unwrap());
    println!("{}", ctx.get_path_string(to_word("sugar")));

    let mut counts = [0; 50];

    let mut output = String::new();
    for answer in answers {
        output.push_str(&ctx.get_path_string(answer));
        output.push('\n');

        let path = ctx.get_path(answer);
        counts[path.len()] += 1;
    }

    println!("{:?}", counts);

    let mut file = File::create("out.txt").unwrap();
    file.write_all(output.as_bytes()).unwrap();
}

#[test]
fn test_state() {
    let (_state, colors) = State::default().step_and_color(to_word("silly"), to_word("hotel"));
    assert_eq!(colors, to_word("bbybb"));
    let (_state, colors) = State::default().step_and_color(to_word("silly"), to_word("daily"));
    assert_eq!(colors, to_word("bybgg"));
}
