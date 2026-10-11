# Algorithms & Academic Lenses

`ripwire` implements 11 academic and algorithmic lenses over codebase graphs and behavioral histories.

## 1. Personalized PageRank
- **Formulation**: Power iteration over transpose in-edge CSR matrix with dangling mass redistribution:
  $$p^{(k+1)} = \alpha P^T p^{(k)} + (1 - \alpha + \alpha m_{\text{dangling}}) v_{\text{teleport}}$$
- **Optimization**: 1024-block reduction sums with early convergence checking ($L_1 \text{ residual} < 10^{-6}$).

## 2. Kleinberg HITS (Hubs & Authorities)
- **Formulation**: Coupled iterative eigenvector convergence:
  $$a^{(k+1)} = \frac{W_{\text{in}} h^{(k)}}{\|W_{\text{in}} h^{(k)}\|_2}, \quad h^{(k+1)} = \frac{W_{\text{out}} a^{(k+1)}}{\|W_{\text{out}} a^{(k+1)}\|_2}$$
- **Interpretation**: Authorities highlight core utility APIs; hubs highlight orchestrators and test harnesses.

## 3. Louvain Community Modularity
- Partitions the call graph into architectural modules by maximizing Newman-Girvan modularity:
  $$\Delta Q = \frac{k_{i,\text{in}}}{2m} - \frac{\Sigma_{\text{tot}} \cdot k_i}{2m^2}$$
- Symmetrizes directed call edges and provides deterministic tie-breaking.

## 4. AST Clone Detector (Type-1, Type-2, Type-3 Gapped)
- Token-level normalization stripping comments, formatting, and identifier names.
- Type-1: Exact token matching via 64-bit rolling hash.
- Type-2: Parameter and identifier renaming normalization.
- Type-3: Near-miss duplicate detection using $k$-gram Jaccard prefiltering ($k=5$) and Longest Common Subsequence (LCS).
- Idiom Demotion: Demotes builder chains, threshold ladders, and switch tables to prevent false positives.

## 5. ARISE Intra-Procedural Slicing
- Traces forward and backward dataflow dependencies from a criterion variable $(s, v)$.
- Identifies reaching definitions, transitive uses, parameter bindings, and assignment propagation.

## 6. Git Churn Decay & Co-Change Mining
- Exponential half-life decay weighting commits over time:
  $$w(t) = 0.5^{\frac{\Delta t}{\tau}}$$
- Detects Fowler's Shotgun Surgery coupling patterns across file co-change history.

## 7. Maintenance Hotspots
- Combines structural complexity with historical change frequency:
  $$\text{Hotspot Score} = \text{Churn} \times \text{Cognitive Complexity}$$

## 8. McCabe Cyclomatic & Essential Complexity
- Counts independent control flow paths ($M = E - N + 2P$) and essential unstructured jump complexity.

## 9. Posnett Code Readability
- Information-theoretic readability metric combining entropy, Halstead volume, line length, and identifier vocabularies.
