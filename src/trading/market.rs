#[derive(Debug)]
pub struct Market {
    pub swap_pair: SwapPair,
}

#[derive(Debug)]
pub struct SwapPair(String, String);
