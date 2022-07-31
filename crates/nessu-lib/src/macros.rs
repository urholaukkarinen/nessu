#[macro_export]
macro_rules! rand_vec {
    () => {
        vec![]
    };
    ($n:expr) => {
        (0..$n)
            .into_iter()
            .map(|_| rand::random())
            .collect::<Vec<_>>()
    };
}
