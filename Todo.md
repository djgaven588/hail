# Near Term
- [ ] Eval function (implement like import calls, but runtime programs)
- [ ] If let Some(t) support
- [ ] Add instruction and memory costs for certain native calls to prevent excessively expensive calls from being the same cost as an integer addition
- [ ] Make tracking an optional feature (as it can be annoying)
- [ ] Add benchmarking crate to compare performance with other languages
- [ ] Optimize the "arming" of program imports to just be once instead of every time it needs to execute (huge performance on the table)
- [ ] LoopNeedsExitCondition doesn't work as expected in all cases, needing extra branches. See Voxel Eras.
- [ ] Fix ``entity.velocity.y = JUMP_VELOCITY * jump_multiplier;`` not updating velocity on entity
- [ ] Make function call / definition trailing comma give better message
- [ ] Allow callee to be owned (builder patterns)
- [ ] Doc comment parsing / output
- [ ] Add ability to pass owned objects to method calls for builder patterns (instead of them having to clone and return the builder).

## Longer Term
- [ ] Document / deal with hardcoded tab spacing in location column(4)
- [ ] Continuations for self scope callbacks
- [ ] New Rust feature, cold_path (performance optimizations)
