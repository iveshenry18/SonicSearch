BUG:
- [x] "Preparing" hangs
    - Adrianne & Daniel
    - Was a divide-by-zero (idiot!)

ENHANCEMENTS: 
- [ ] Improve search results (ensure preprocessing parity with Python version, try other model checkpoints)
    - +1s: Daniel
- [ ] Index other file types
    - [ ] mp3
        - +1s: Adrianne, Daniel
    - [ ] m4a
    - [ ] aiff
- [ ] Compile for other targets (Mac Universal Binary, Windows)
- [ ] Add index canceling
- [ ] Speed up "Initializing"
    - [ ] Store HNSW results
- [ ] Speed up "Preparing"
    - [ ] Match already-indexed files faster (using paths + header before hashing)
- [ ] Disable searching while Preparing
- [ ] Speed up "Indexing"
    - [ ] Investigate CoreML issues (max length of an inner dim reached, possibly due to allowed batch size)
- [ ] Improve directory selector
    - [ ] Clean up UI
    - [ ] Prevent redundancy
- [ ] Improve accessibility
