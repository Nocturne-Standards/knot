#[dusk_forge::contract]
mod mock_atlas {
    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use dusk_core::abi::ContractId;

    pub struct MockAtlasState {
        services: BTreeMap<String, ContractId>,
    }

    impl MockAtlasState {
        pub const fn new() -> Self {
            Self {
                services: BTreeMap::new(),
            }
        }

        pub fn set_service(&mut self, name: String, id: ContractId) {
            self.services.insert(name, id);
        }

        pub fn resolve(&self, name: String) -> Option<ContractId> {
            self.services.get(&name).copied()
        }
    }
}
