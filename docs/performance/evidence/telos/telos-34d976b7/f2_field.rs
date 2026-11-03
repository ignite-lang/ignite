    module_variants: BTreeSet<String>,
    /// Roots of the value of each block and `match` arm body walked so
    /// far (key: its address), taken while its scope was open.
    tail_roots: BTreeMap<usize, BTreeSet<String>>,
