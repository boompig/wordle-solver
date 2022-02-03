# Wordle Solver

A Wordle solver written in Rust, [originally written by mwillsey](https://github.com/mwillsey/wordle-solver).

I optimized some aspects: specifically it now uses a pre-computed matrix of guess results rather than computing the guess result for every guess-answer pair at runtime. If the solver is compiled in release mode, it takes about a minute to run per root guess word, which is extremely fast compared to my other implementation in Python.

There are many more optimizations that could be done (like I did for my Python implementation) but this is fast enough.

### Setting the Number of Guesses

I added some of the generated decision trees in `out/decision-trees`. I specify how many guesses I set to get these results.  Note that some solutions require more guesses to find the optimal than others.

With this solver, you need to set the number of guesses to get ideal results:

First Word | # guesses | tree size obtained | optimal tree size
:----:|:----------:|:------------------:|:-------------------:
salet | 10               | 7920                        | 7920
reast | 10               | 7925                        | 7923
trace | 15 | 7927 | 7926 
crate | 15 | 7927 | 7926 
slate | 15 | 7928 | 7928 
crane | 15 | 7934 | 7930 
carle | 15 | 7938 | 7937 
slane | 15 | 7943 | 7943 
carte | 15 | 7950 | 7949 
torse | 15 | 7950 | 7950 
slant | 15 | 7952 | 7951 
trice | 15 | 7952 | 7952 
least | 15 | 7955 | 7954 

You can find the proven bounds for the best decision tree by alex1770 [here](https://github.com/alex1770/wordle/blob/main/normal.top105.proven).



