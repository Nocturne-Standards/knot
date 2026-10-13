use dusk_core::abi::ContractId as WardenId;
use dusk_core_git::abi::ContractId as KnotId;
use dusk_core_git::signatures::bls::{
    PublicKey as KnotKey, SecretKey as KnotSecret, Signature as KnotSig,
};
use knot_encoding::call_types::RegistryPendingChange;
use knot_encoding::events::{
    AccountChanged, AccountCreated, AuthorizedAccountSet, DataSet, PendingCancelled,
    PendingScheduled, ProposalApproved, ProposalCancelled, ProposalCreated, ProposalFinalized,
    ProposalQueued, ProposalTtlSet, Pruned, RegistrySet, TimelockSet, TombstoneSet,
};
use knot_event_decoder::{
    CONTRACT_PROPOSALS, CONTRACT_REGISTRY, CONTRACT_WARDEN, DecodeError, NAMED_EVENT_VERSION,
    decode,
};
use knot_warden_encoding::call_types::{Account, PendingAdmin};
use knot_warden_encoding::events::{
    AdminCancelled, AdminScheduled, AtlasCancelForwarded, AtlasSet, DelaySet, GuardianForwarded,
    SchedulerSet, ServiceCancelled, ServiceExecuted, ServiceScheduled, TimelockForwarded,
};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::Serialize;
use rkyv::ser::serializers::AllocSerializer;

fn archive<T>(value: &T) -> Vec<u8>
where
    T: Serialize<AllocSerializer<4096>>,
{
    rkyv::to_bytes::<_, 4096>(value)
        .expect("archive")
        .into_vec()
}

fn round_trip<T>(contract: &str, topic: &str, value: &T)
where
    T: Serialize<AllocSerializer<4096>> + serde::Serialize,
{
    let bytes = archive(value);
    let decoded = decode(contract, topic, NAMED_EVENT_VERSION, &bytes)
        .unwrap_or_else(|err| panic!("{contract}/{topic} failed: {err}"));
    assert_eq!(decoded, serde_json::to_value(value).expect("json"));
}

fn knot_id(byte: u8) -> KnotId {
    KnotId::from_bytes([byte; 32])
}

fn warden_id(byte: u8) -> WardenId {
    WardenId::from_bytes([byte; 32])
}

fn signer() -> (KnotKey, KnotSig) {
    let mut rng = StdRng::seed_from_u64(0x6e0d);
    let secret = KnotSecret::random(&mut rng);
    let public = KnotKey::from(&secret);
    let signature = secret.sign(b"knot-event-decoder");
    (public, signature)
}

#[test]
fn named_events_round_trip() {
    let (signer, signature) = signer();
    let members = vec![signer];
    let registry = knot_id(0x11);
    let target = knot_id(0x22);
    let digest = [0x33; 32];
    let warden = warden_id(0x44);
    let account = Account::Contract(warden);

    round_trip(CONTRACT_REGISTRY, "data_set", &DataSet { data: registry });
    round_trip(
        CONTRACT_REGISTRY,
        "account_created",
        &AccountCreated {
            id: 7,
            members: members.clone(),
            threshold: 1,
            timelock_blocks: 0,
            nonce: 0,
        },
    );
    round_trip(
        CONTRACT_REGISTRY,
        "pending_scheduled",
        &PendingScheduled {
            account_id: 7,
            execute_at: 20,
            change: RegistryPendingChange::SetTimelock(4),
        },
    );
    round_trip(
        CONTRACT_REGISTRY,
        "account_changed",
        &AccountChanged {
            account_id: 7,
            members: members.clone(),
            threshold: 1,
        },
    );
    round_trip(
        CONTRACT_REGISTRY,
        "timelock_set",
        &TimelockSet {
            account_id: 7,
            blocks: 4,
        },
    );
    round_trip(
        CONTRACT_REGISTRY,
        "pending_cancelled",
        &PendingCancelled {
            account_id: 7,
            execute_at: 20,
            change: RegistryPendingChange::ChangeAccount {
                new_members: members.clone(),
                new_threshold: 1,
            },
        },
    );

    round_trip(CONTRACT_PROPOSALS, "data_set", &DataSet { data: registry });
    round_trip(
        CONTRACT_PROPOSALS,
        "registry_set",
        &RegistrySet { registry, epoch: 2 },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "proposal_ttl_set",
        &ProposalTtlSet { blocks: 100 },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "tombstone_set",
        &TombstoneSet { tombstone: true },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "authorized_account_set",
        &AuthorizedAccountSet {
            account_id: 7,
            auth_generation: 3,
        },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "proposal_created",
        &ProposalCreated {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
            deadline: 50,
            epoch: 2,
            nonce: 1,
            auth_generation: 3,
            target,
            function_name: "set".to_owned(),
            call_args: vec![1, 2, 3],
        },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "proposal_approved",
        &ProposalApproved {
            proposal_id: 9,
            signed_digest: digest,
            signer,
            signature,
        },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "proposal_finalized",
        &ProposalFinalized {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
            target,
            function_name: "set".to_owned(),
            call_args: vec![1, 2, 3],
        },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "proposal_queued",
        &ProposalQueued {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
            execute_at: 30,
            target,
            function_name: "set".to_owned(),
            call_args: vec![1, 2, 3],
        },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "proposal_cancelled",
        &ProposalCancelled {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
        },
    );
    round_trip(
        CONTRACT_PROPOSALS,
        "pruned",
        &Pruned {
            proposal_ids: vec![9],
            digest_keys: vec![digest],
        },
    );

    round_trip(
        CONTRACT_WARDEN,
        "scheduler_set",
        &SchedulerSet {
            scheduler: account.clone(),
        },
    );
    round_trip(CONTRACT_WARDEN, "delay_set", &DelaySet { blocks: 5 });
    round_trip(
        CONTRACT_WARDEN,
        "service_scheduled",
        &ServiceScheduled {
            name: "book".to_owned(),
            id: warden,
            execute_at: 12,
        },
    );
    round_trip(
        CONTRACT_WARDEN,
        "service_executed",
        &ServiceExecuted {
            name: "book".to_owned(),
            id: warden,
        },
    );
    round_trip(
        CONTRACT_WARDEN,
        "service_cancelled",
        &ServiceCancelled {
            name: "book".to_owned(),
            id: warden,
        },
    );
    round_trip(
        CONTRACT_WARDEN,
        "admin_scheduled",
        &AdminScheduled {
            change: PendingAdmin::Delay(5),
            execute_at: 12,
        },
    );
    round_trip(
        CONTRACT_WARDEN,
        "admin_cancelled",
        &AdminCancelled {
            change: PendingAdmin::Scheduler(account.clone()),
        },
    );
    round_trip(CONTRACT_WARDEN, "atlas_set", &AtlasSet { atlas: warden });
    round_trip(
        CONTRACT_WARDEN,
        "guardian_forwarded",
        &GuardianForwarded { guardian: account },
    );
    round_trip(
        CONTRACT_WARDEN,
        "timelock_forwarded",
        &TimelockForwarded { blocks: 8 },
    );
    round_trip(
        CONTRACT_WARDEN,
        "atlas_cancel_forwarded",
        &AtlasCancelForwarded,
    );
}

#[test]
fn same_topic_is_two_contract_arms() {
    let value = DataSet {
        data: knot_id(0x11),
    };
    let bytes = archive(&value);
    round_trip(CONTRACT_REGISTRY, "data_set", &value);
    round_trip(CONTRACT_PROPOSALS, "data_set", &value);
    let err = decode(CONTRACT_WARDEN, "data_set", NAMED_EVENT_VERSION, &bytes).unwrap_err();
    assert_eq!(
        err,
        DecodeError::UnknownEvent {
            contract: CONTRACT_WARDEN.to_owned(),
            topic: "data_set".to_owned(),
        }
    );
}

#[test]
fn topic_alone_does_not_select_an_arm() {
    let bytes = archive(&TimelockSet {
        account_id: 1,
        blocks: 2,
    });
    let err = decode(CONTRACT_WARDEN, "timelock_set", NAMED_EVENT_VERSION, &bytes).unwrap_err();
    assert!(matches!(err, DecodeError::UnknownEvent { .. }));
    let err = decode(
        CONTRACT_REGISTRY,
        "timelock_forwarded",
        NAMED_EVENT_VERSION,
        &bytes,
    )
    .unwrap_err();
    assert!(matches!(err, DecodeError::UnknownEvent { .. }));
}

#[test]
fn other_versions_are_not_arms() {
    let bytes = archive(&DelaySet { blocks: 1 });
    let err = decode(CONTRACT_WARDEN, "delay_set", 0, &bytes).unwrap_err();
    assert_eq!(err, DecodeError::UnsupportedVersion(0));
}

#[test]
fn legacy_scalar_payload_is_rejected() {
    let legacy = rkyv::to_bytes::<u64, 64>(&7u64)
        .expect("archive")
        .into_vec();
    let err = decode(
        CONTRACT_REGISTRY,
        "account_created",
        NAMED_EVENT_VERSION,
        &legacy,
    )
    .unwrap_err();
    assert!(matches!(err, DecodeError::InvalidPayload(_)));
}

#[test]
fn legacy_contract_names_are_not_arms() {
    let bytes = archive(&AccountCreated {
        id: 1,
        members: vec![signer().0],
        threshold: 1,
        timelock_blocks: 0,
        nonce: 0,
    });
    for contract in ["multisig-registry", "multisig-proposals"] {
        let err = decode(contract, "account_created", NAMED_EVENT_VERSION, &bytes).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownEvent { .. }));
    }
}
