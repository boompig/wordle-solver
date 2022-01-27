use clap::Parser;
use ordered_float::NotNan;
use std::{collections::HashMap, fs::File, io::Write};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
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

    // fn score(&self) -> (u32, u32) {
    //     todo!()
    //     // let n_greens = self.spots.iter().filter(|s| s.count_ones() == 1).count();
    //     // let n_zeros = self.spots.iter().map(|s| s.count_zeros()).sum();
    //     // // TODO yellows
    //     // (n_greens as _, n_zeros)
    // }

    fn reasonable_guess(&self, guess: Word) -> bool {
        let possible_letters = self.spots.iter().copied().reduce(|a, b| a | b).unwrap();
        self.spots.into_iter().zip(guess).any(|(spot, letter)| {
            // compute if this letter is *un*reasonable
            let bit = letter_bit(letter);
            let unreasonable = spot == bit || // guessing a known green doesn't help
            (spot & bit == 0 && self.must_have & bit == 0) || // guessing a known yellow doesn't help
            possible_letters & bit == 0; // known black
            !unreasonable
        })
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
    params: Params,
    n_hits: usize,
    hit_depths: [usize; 10],
    memo: HashMap<State, Node>,
}

#[derive(Clone)]
struct Node {
    max_guesses: usize,
    n_total_depth: usize,
    n_leaf_nodes: usize,
    guess: Word,
}

impl Node {
    fn avg_depth(&self) -> NotNan<f64> {
        NotNan::new(self.n_total_depth as f64 / self.n_leaf_nodes as f64).unwrap()
    }

    fn leaf(guess: Word) -> Self {
        Self {
            max_guesses: 1,
            n_total_depth: 1,
            n_leaf_nodes: 1,
            guess,
        }
    }
}

fn to_word(w: impl AsRef<[u8]>) -> Word {
    w.as_ref().try_into().unwrap()
}

impl Context {
    fn new(params: Params) -> Self {
        Self {
            params,
            n_hits: 0,
            hit_depths: Default::default(),
            memo: Default::default(),
        }
    }

    fn solve(
        &mut self,
        depth: usize,
        state: State,
        guesses: &[Word],
        answers: &[Word],
    ) -> Option<&Node> {
        assert!(!answers.is_empty());
        if depth >= 6 {
            return None;
        }
        if let &[only_answer] = answers {
            return Some(self.memo.entry(state).or_insert(Node::leaf(only_answer)));
        }

        // NLL workaround
        // https://stackoverflow.com/questions/38023871/returning-a-reference-from-a-hashmap-or-vec-causes-a-borrow-to-last-beyond-the-s
        if self.memo.contains_key(&state) {
            self.n_hits += 1;
            if depth < self.hit_depths.len() {
                self.hit_depths[depth] += 1;
            }
            return Some(&self.memo[&state]);
        }

        let mut top_guesses: Vec<Word>;
        let candidate_guesses = if depth == 0 && self.params.starting_word.is_some() {
            let word = self.params.starting_word.as_ref().unwrap();
            top_guesses = vec![to_word(word)];
            &top_guesses
        // } else if answers.len() <= self.params.brute_threshold {
        //     &answers
        } else {
            let mut groups = HashMap::<Word, usize>::new();
            let mut ranked_guesses: Vec<(_, Word)> = guesses
                .iter()
                // .filter(|&&guess| state.reasonable_guess(guess)) // TODO
                // .filter(|&&g| state.could_be_answer(g)) // TODO is this a good idea?
                .map(|&guess| {
                    groups.clear();

                    for &answer in answers {
                        let (_next_state, colors) = state.step_and_color(guess, answer);
                        *groups.entry(colors).or_default() += 1;
                    }

                    #[allow(unused)]
                    let max_bucket = groups.values().copied().max().unwrap();
                    let mut sum: usize = groups.values().copied().sum();
                    sum -= groups.get(&[b'g'; 5]).copied().unwrap_or_default();

                    let avg = NotNan::new(sum as f64 / groups.len() as f64).unwrap();
                    // ((max_bucket, -(groups.len() as i32)), guess)
                    // ((max_bucket, sum / groups.len()), guess)
                    (avg, guess)
                })
                .collect();
            ranked_guesses.sort_unstable();
            top_guesses = ranked_guesses
                .iter()
                .map(|(_score, guess)| *guess)
                .take(self.params.n_guesses)
                .collect();

            // if answers.len() < self.params.brute_threshold {
            //     top_guesses.extend(answers.iter().copied())
            // }

            top_guesses.as_slice()
        };

        assert!(candidate_guesses.len() <= 1);
        let node = candidate_guesses
            .iter()
            .filter_map(|&guess| {
                if depth == 0 {
                    print!("{}...\r", std::str::from_utf8(&guess).unwrap());
                    std::io::stdout().flush().unwrap();
                }

                let mut node = Node {
                    max_guesses: 0,
                    n_total_depth: 0,
                    n_leaf_nodes: 0,
                    guess,
                };

                for &answer in answers {
                    let next_state = state.step(guess, answer);
                    let n = if guess == answer {
                        self.memo.entry(next_state).or_insert(Node::leaf(guess))
                    } else {
                        let next_answers: Vec<Word> = answers
                            .iter()
                            .copied()
                            .filter(|&a| next_state.could_be_answer(a))
                            .collect();
                        self.solve(depth + 1, next_state, guesses, &next_answers)?
                    };
                    node.max_guesses = node.max_guesses.max(n.max_guesses + 1);
                    node.n_total_depth += n.n_total_depth + n.n_leaf_nodes;
                    node.n_leaf_nodes += n.n_leaf_nodes;
                }
                // node.n_total_depth += node.n_leaf_nodes;

                if depth <= 1 {
                    println!(
                        "{}, answers: {}, total: {}, avg: {}, max: {}",
                        std::str::from_utf8(&guess).unwrap(),
                        node.n_leaf_nodes,
                        node.n_total_depth,
                        node.avg_depth(),
                        node.max_guesses,
                    );
                }

                Some(node)
            })
            // .min_by_key(|node| (node.max_guesses, node.n_total_guesses))?;
            // .min_by_key(|node| node.n_total_guesses)?;
            // .min_by_key(|node| node.max_guesses)?;
            .min_by_key(|node| node.avg_depth())?;

        Some(self.memo.entry(state).or_insert(node))
    }

    fn get_path(&self, answer: Word) -> Vec<Word> {
        let mut state = State::default();
        let mut guesses = vec![];

        loop {
            let node = &self.memo[&state];
            guesses.push(node.guess);
            if node.guess == answer {
                // assert_eq!(state, state.step(node.guess, answer));
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

mod take2 {
    use super::*;

    pub type Colors = [u8; 5];

    pub struct Tree {
        pub guess: Word,
        total_guesses: usize,
        max_guesses: usize,
        n_total_depth: usize,
        n_leaf_nodes: usize,
        pub children: HashMap<Colors, Tree>,
    }

    impl Tree {
        fn leaf(guess: Word) -> Self {
            Self {
                guess,
                total_guesses: 1,
                max_guesses: 1,
                n_total_depth: 1,
                n_leaf_nodes: 1,
                children: Default::default(),
            }
        }

        fn avg_depth(&self) -> NotNan<f32> {
            NotNan::new(self.n_total_depth as f32 / self.n_leaf_nodes as f32).unwrap()
        }

        pub fn write(&self, w: &mut impl Write, mut line: Vec<u8>) -> std::io::Result<()> {
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
    #[test]
    fn test_colors() {
        let colors = score(to_word("silly"), to_word("hotel"));
        assert_eq!(colors, to_word("bbybb"));
        let colors = score(to_word("silly"), to_word("daily"));
        assert_eq!(colors, to_word("bybgg"));
    }

    fn filter(guess: Word, colors: Word, answers: &[Word]) -> Vec<Word> {
        // let could_be = [!0u32; 5];
        // let must_have = 0;

        // for i in 0..guess.len() {
        //     let bit = letter_bit(guess[i]);
        //     match colors[i] {
        //         b'g' => could_be[i] = bit,
        //         b'y' => {
        //             could_be[i] &= !bit;
        //             must_have |= bit;
        //         }
        //         b => {
        //             assert_eq!(b, b'b');
        //             if
        //         }
        //     }
        // }

        answers
            .iter()
            .copied()
            .filter(|&answer| {
                let mut answer = answer;
                let mut guess = guess;
                for i in 0..guess.len() {
                    if colors[i] == b'g' {
                        if guess[i] != answer[i] {
                            return false;
                        }
                        guess[i] = CROSSED_OUT;
                        answer[i] = CROSSED_OUT;
                    }
                }

                for i in 0..guess.len() {
                    if colors[i] == b'y' {
                        if let Some(j) = answer.iter().copied().position(|a| a == guess[i]) {
                            answer[j] = CROSSED_OUT;
                        } else {
                            return false;
                        }
                    } else if colors[i] == b'b' {
                        if answer.contains(&guess[i]) {
                            assert_ne!(guess[i], CROSSED_OUT);
                            // assert_ne!(answer[i], CROSSED_OUT);
                            return false;
                        }
                    } else {
                        assert_eq!(colors[i], b'g');
                    }
                }

                true
            })
            .collect()
    }

    pub fn solve(
        params: &Params,
        depth: usize,
        guesses: &[Word],
        answers: &[Word],
    ) -> Option<Tree> {
        assert!(!answers.is_empty());
        if depth >= 7 {
            return None;
        }

        if let &[only_answer] = answers {
            return Some(Tree::leaf(only_answer));
        }

        let top_guesses: Vec<Word> = if depth == 0 && params.starting_word.is_some() {
            vec![to_word(params.starting_word.as_ref().unwrap())]
        } else {
            let mut groups = HashMap::<Word, usize>::new();
            let mut ranked_guesses: Vec<(_, Word)> = guesses
                .iter()
                .map(|&guess| {
                    groups.clear();

                    for &answer in answers {
                        let colors = score(guess, answer);
                        *groups.entry(colors).or_default() += 1;
                    }

                    let mut sum: usize = groups.values().copied().sum();
                    sum -= groups.get(&[b'g'; 5]).copied().unwrap_or_default();

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
                    print!("{}...\r", std::str::from_utf8(&guess).unwrap());
                    std::io::stdout().flush().unwrap();
                }

                let mut tree = Tree {
                    total_guesses: answers.len(),
                    max_guesses: 0,
                    n_total_depth: 0,
                    n_leaf_nodes: 0,
                    guess,
                    children: Default::default(),
                };

                let mut groups = HashMap::<Word, Vec<Word>>::new();
                for &answer in answers {
                    let colors = score(guess, answer);
                    groups.entry(colors).or_default().push(answer);
                }

                for (score, grouped_answers) in groups {
                    let child = solve(params, depth + 1, guesses, &grouped_answers)?;
                    if score != [b'g'; 5] {
                        tree.total_guesses += child.total_guesses;
                    }
                    tree.max_guesses = tree.max_guesses.max(child.max_guesses + 1);
                    tree.n_total_depth += child.n_total_depth + child.n_leaf_nodes;
                    tree.n_leaf_nodes += child.n_leaf_nodes;
                    tree.children.insert(score, child);
                }

                // for &answer in answers {
                //     let child = if guess == answer {
                //         // println!("Found it {}", std::str::from_utf8(&guess).unwrap());
                //         Tree::leaf(guess)
                //     } else {
                //         let colors = score(guess, answer);
                //         let next_answers = filter(guess, colors, answers);
                //         solve(params, depth + 1, guesses, &next_answers)?
                //     };
                //     tree.max_guesses = tree.max_guesses.max(child.max_guesses + 1);
                //     tree.n_total_depth += child.n_total_depth + child.n_leaf_nodes;
                //     tree.n_leaf_nodes += child.n_leaf_nodes;
                //     tree.children.push(child)
                // }

                if depth == 0 {
                    println!(
                        "{depth} {}, total: {}",
                        std::str::from_utf8(&guess).unwrap(),
                        tree.total_guesses,
                    );
                    println!(
                        "{depth} {}, answers: {}, total: {}, avg: {}, max: {}",
                        std::str::from_utf8(&guess).unwrap(),
                        tree.n_leaf_nodes,
                        tree.n_total_depth,
                        // tree.avg_depth(),
                        tree.total_guesses as f32 / answers.len() as f32,
                        tree.max_guesses,
                    );
                }

                Some(tree)
            })
            // .min_by_key(|node| (node.max_guesses, node.n_total_guesses))?;
            // .min_by_key(|node| node.n_total_guesses)?;
            // .min_by_key(|node| node.max_guesses)?;
            .min_by_key(|tree| tree.total_guesses)?;
        // .min_by_key(|tree| tree.avg_depth())?;

        Some(tree)
    }
}

#[derive(Parser)]
pub struct Params {
    #[clap(short, long, default_value = "20")]
    n_guesses: usize,
    #[clap(short, long, default_value = "2")]
    brute_threshold: usize,
    #[clap(long)]
    answers_only: bool,
    #[clap(long)]
    starting_word: Option<String>,
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

    let tree = take2::solve(&params, 0, &guesses, &answers).unwrap();

    // let mut ctx = Context::new(params);

    // let node = ctx.solve(0, State::default(), &guesses, &answers).unwrap();
    // println!("{}", std::str::from_utf8(&node.guess).unwrap());
    // println!("{}", ctx.get_path_string(to_word("sugar")));

    // println!(
    //     "nodes: {}, hits: {}, hit depths: {:?}",
    //     ctx.memo.len(),
    //     ctx.n_hits,
    //     ctx.hit_depths
    // );

    // let mut total_guesses = 0;
    // let mut counts = [0; 10];

    // let mut output = String::new();
    // for &answer in &answers {
    //     output.push_str(&ctx.get_path_string(answer));
    //     output.push('\n');

    //     let path = ctx.get_path(answer);
    //     counts[path.len()] += 1;
    //     total_guesses += path.len();
    // }

    // println!("{:?}", counts);
    // println!(
    //     "total: {total_guesses}, avg: {}",
    //     total_guesses as f64 / answers.len() as f64
    // );

    let mut file = File::create("out.txt").unwrap();
    tree.write(&mut file, vec![]).unwrap();
}

#[test]
fn test_state() {
    let (_state, colors) = State::default().step_and_color(to_word("silly"), to_word("hotel"));
    assert_eq!(colors, to_word("bbybb"));
    let (_state, colors) = State::default().step_and_color(to_word("silly"), to_word("daily"));
    assert_eq!(colors, to_word("bybgg"));
}
