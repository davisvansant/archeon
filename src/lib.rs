mod transfer;

use crate::transfer::Transfer;

pub struct Archeon {
    pub ignited: bool,
}

impl Archeon {
    pub async fn ignite() -> Archeon {
        let _transfer = Transfer::init("http://some_test_authority/with/path/and/query").await;
        // transfer.launch().await;

        Archeon { ignited: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn ignite() {
        let test_archeon = Archeon::ignite().await;
        assert!(test_archeon.ignited);
    }
}
