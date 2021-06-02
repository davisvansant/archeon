pub struct Archeon {
    pub ignited: bool,
}

impl Archeon {
    pub async fn ignite() -> Archeon {
        let _ = Self::transfer().await;

        Archeon { ignited: true }
    }

    async fn transfer() {
        // unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn ignite() {
        let test_archeon = Archeon::ignite().await;
        assert_eq!(test_archeon.ignited, true);
    }
}
