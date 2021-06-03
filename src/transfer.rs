pub(crate) struct Transfer {
    pub(crate) initialized: bool,
}

impl Transfer {
    pub(crate) async fn init() -> Transfer {
        Transfer { initialized: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn transfer() {
        let test_transfer = Transfer::init().await;
        assert_eq!(test_transfer.initialized, true);
    }
}
