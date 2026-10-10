---
description: Use time and resources efficiently
off_lens: true
---

- Optimize user-visible latency, throughput, memory, I/O, network use, and compute cost where
  they matter.
- Examine algorithmic complexity, repeated work, allocation, contention, batching, caching,
  and hot paths when relevant.
- Measure or profile before introducing complexity, and state when evidence is unavailable. Flag
  caches, pools, and other performance machinery added without evidence that the path is hot.
- Add focused benchmarks or performance regression tests when they will remain reliable.
- Avoid micro-optimizations and never sacrifice correctness or security without an explicit
  tradeoff decision. Flag micro-optimizations that cost clarity without a measured benefit, and
  optimizations that return stale or wrong results.
