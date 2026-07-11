# Bog Kit

This repo contains some of the tooling we've been working on for building Bog style databases. We've compiled these tools and examples in one cargo workspace, so you can start building immediately. 

The best way to create your project is to run this terminal command in the root of this repo:
```bash
cargo new [project-name]
``` 


### Fold
Fold is our take on an incremental programming framework, it's the engine that powers Bog. It’s a rust crate with iterator like primitives for materializing a stream of ever changing data into views. Statically typed and very, very fast.

### Extremely Static Embedding (ESE)
ESE, our first take on a compiler oriented approach to static embedding. It’s a flattening of a tokenizer and map of embeddings into a perfect hash function. It’s also evidence that the approach is worth generalizing, and that there is much to be rethought about how embedding runtimes currently function.

### Approximate Nearest Nieghbors... yeah (ANNy)
This is a very fast crate for creating HNSWs