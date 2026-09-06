//! A simulation prepared off the lock answers what the instance's own answers

use {
    litesvm::LiteSVM,
    solana_address::Address,
    solana_address_lookup_table_interface::instruction::{
        create_lookup_table, extend_lookup_table,
    },
    solana_clock::Clock,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::{account_meta::AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{
        v0::Message as MessageV0, AddressLookupTableAccount, Message, VersionedMessage,
    },
    solana_signature::Signature,
    solana_signer::Signer,
    solana_stake_interface::instruction as stake_instruction,
    solana_system_interface::instruction::transfer,
    solana_transaction::{versioned::VersionedTransaction, Transaction},
    solana_transaction_error::TransactionError,
    std::time::Instant,
};

const LOGGING_PROGRAM: &[u8] =
    include_bytes!("../../node-litesvm/program_bytes/spl_example_logging.so");

const LOGGING_ID: Address = Address::from_str_const("Logging111111111111111111111111111111111111");

/// The shape the tapesvm daemon runs: signatures checked, blockhash ages its own business
fn daemon_svm() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new()
        .with_blockhash_check(false)
        .with_transaction_history(10_000);
    svm.add_program(LOGGING_ID, LOGGING_PROGRAM)
        .expect("logging program");
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000)
        .expect("airdrop");
    (svm, payer)
}

fn signed(svm: &LiteSVM, payer: &Keypair, ixs: &[Instruction]) -> VersionedTransaction {
    Transaction::new(
        &[payer],
        Message::new(ixs, Some(&payer.pubkey())),
        svm.latest_blockhash(),
    )
    .into()
}

/// The two paths on one state, which is the whole claim
fn agree(svm: &LiteSVM, case: &str, tx: VersionedTransaction) {
    let held = svm.simulate_transaction_no_verify(tx.clone());
    let off_lock = svm.prepare_simulation_no_verify(tx).run();
    assert_eq!(held, off_lock, "{case} answered differently off the lock");
}

#[test]
fn a_prepared_simulation_answers_what_the_held_one_does() {
    let (mut svm, payer) = daemon_svm();

    agree(
        &svm,
        "transfer",
        signed(
            &svm,
            &payer,
            &[transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000)],
        ),
    );

    agree(
        &svm,
        "transfer beyond the balance",
        signed(
            &svm,
            &payer,
            &[transfer(
                &payer.pubkey(),
                &Address::new_unique(),
                1_000_000_000_000_000,
            )],
        ),
    );

    agree(
        &svm,
        "a program that logs",
        signed(
            &svm,
            &payer,
            &[Instruction {
                program_id: LOGGING_ID,
                accounts: vec![AccountMeta::new(Address::new_unique(), false)],
                data: vec![5, 10, 11, 12, 13, 14],
            }],
        ),
    );

    agree(
        &svm,
        "a builtin that returns data",
        signed(&svm, &payer, &[stake_instruction::get_minimum_delegation()]),
    );

    agree(
        &svm,
        "a compute budget the transaction sets itself",
        signed(
            &svm,
            &payer,
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(7_531),
                transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000),
            ],
        ),
    );

    agree(
        &svm,
        "a program nobody deployed",
        signed(
            &svm,
            &payer,
            &[Instruction {
                program_id: Address::new_unique(),
                accounts: vec![],
                data: vec![1, 2, 3],
            }],
        ),
    );

    let mut corrupt = Transaction::new(
        &[&payer],
        Message::new(
            &[transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000)],
            Some(&payer.pubkey()),
        ),
        svm.latest_blockhash(),
    );
    corrupt.message.instructions[0].program_id_index = 200;
    agree(&svm, "a message that will not sanitize", corrupt.into());

    let landed = signed(
        &svm,
        &payer,
        &[transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000)],
    );
    svm.send_transaction(landed.clone()).expect("send");
    agree(&svm, "a transaction already processed", landed);

    let through_table = lookup_table_tx(&mut svm, &payer);
    agree(&svm, "a transaction through a lookup table", through_table);
}

/// A bad signature is refused off the lock exactly when the instance refuses it
#[test]
fn a_prepared_simulation_checks_signatures_the_same_way() {
    let (svm, payer) = daemon_svm();
    let mut tx = signed(
        &svm,
        &payer,
        &[transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000)],
    );
    tx.signatures[0] = Signature::default();

    let held = svm.simulate_transaction(tx.clone());
    let off_lock = svm.prepare_simulation(tx.clone()).run();
    assert_eq!(
        held, off_lock,
        "an unsigned transaction answered differently off the lock"
    );
    assert_eq!(
        held.expect_err("unsigned").err,
        TransactionError::SignatureFailure
    );

    svm.prepare_simulation_no_verify(tx)
        .run()
        .expect("no verify runs an unsigned transaction");
}

/// Simulation is not the only reader of a stale blockhash, so it answers for one too
#[test]
fn a_prepared_simulation_ages_a_blockhash_the_same_way() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000)
        .expect("airdrop");
    let tx = signed(
        &svm,
        &payer,
        &[transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000)],
    );
    svm.expire_blockhash();
    agree(&svm, "a blockhash nobody remembers", tx);
}

/// A transfer whose recipient only the lookup table lists
fn lookup_table_tx(svm: &mut LiteSVM, payer: &Keypair) -> VersionedTransaction {
    let recipient = Address::new_unique();
    let recent_slot = svm.get_sysvar::<Clock>().slot;
    let (create, table) = create_lookup_table(payer.pubkey(), payer.pubkey(), recent_slot);
    let extend = extend_lookup_table(table, payer.pubkey(), Some(payer.pubkey()), vec![recipient]);
    let blockhash = svm.latest_blockhash();
    svm.send_transaction(Transaction::new(
        &[payer],
        Message::new(&[create, extend], Some(&payer.pubkey())),
        blockhash,
    ))
    .expect("lookup table");
    svm.warp_to_slot(recent_slot + 1);

    let message = MessageV0::try_compile(
        &payer.pubkey(),
        &[transfer(&payer.pubkey(), &recipient, 2_000_000)],
        &[AddressLookupTableAccount {
            key: table,
            addresses: vec![recipient],
        }],
        svm.latest_blockhash(),
    )
    .expect("compile");
    VersionedTransaction::try_new(VersionedMessage::V0(message), &[payer]).expect("sign")
}

/// What a simulation costs the instance's lock, printed rather than asserted
#[test]
#[ignore = "a measurement, not an assertion"]
fn measure_prepare_against_run() {
    const ROUNDS: u32 = 2_000;

    let (svm, payer) = daemon_svm();
    let ixs: Vec<_> = (0..16)
        .map(|_| transfer(&payer.pubkey(), &Address::new_unique(), 2_000_000))
        .collect();
    let tx = signed(&svm, &payer, &ixs);

    let at = Instant::now();
    for _ in 0..ROUNDS {
        svm.simulate_transaction_no_verify(tx.clone())
            .expect("simulate");
    }
    let held = at.elapsed().as_nanos() as u64 / u64::from(ROUNDS);

    let mut prepare = 0u64;
    let mut run = 0u64;
    for _ in 0..ROUNDS {
        let at = Instant::now();
        let prepared = svm.prepare_simulation_no_verify(tx.clone());
        prepare += at.elapsed().as_nanos() as u64;
        let at = Instant::now();
        prepared.run().expect("simulate");
        run += at.elapsed().as_nanos() as u64;
    }

    println!(
        "held {held} ns; prepared: prepare {} ns, run {} ns",
        prepare / u64::from(ROUNDS),
        run / u64::from(ROUNDS),
    );
}
